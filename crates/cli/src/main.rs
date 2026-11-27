//! `phono`: try the phonocompressor engine from the command line.

mod by_word;
mod fit;
mod risks;

use clap::{Parser, Subcommand};
use phono_core::Engine;
use phono_core::chunks::{Script, Scripts};
use phono_core::phone::{fmt_phones, fmt_word_phones};
use phono_core::reading::Word;
use phono_core::score::Op;
use phono_core::search::{Candidate, SearchConfig, Searcher};
use serde::Serialize;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(about = "Phonetic compression: phrases → a few Unicode characters a TTS reads back")]
struct Cli {
    /// Data file from `mise run data`.
    #[arg(long, global = true, default_value = "data/generated/phono.bin")]
    data: PathBuf,
    /// ElevenLabs model to predict readings for. Calibration and fitting use only its clips.
    #[arg(long, global = true, default_value = phono_core::model::DEFAULT.model_ids[0])]
    model: String,
    /// Also search scripts the model's profile leaves off by default (on Flash v2.5:
    /// Ethiopic, Canadian syllabics, conjoining Hangul jamo, Runic, which exp0–exp4 found
    /// unreliable).
    #[arg(long, global = true)]
    all_scripts: bool,
    /// Search only these scripts (comma-separated names such as `latin,hangul,yi`). Naming
    /// one the model doesn't read by transliteration is an error that lists the usable ones.
    #[arg(
        long,
        global = true,
        value_delimiter = ',',
        conflicts_with = "all_scripts"
    )]
    scripts: Option<Vec<String>>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Target pronunciation of an English phrase.
    Phones { phrase: String },
    /// How the TTS reads a Unicode string (transliterate, then English).
    Read { text: String },
    /// Spelling options for ARPAbet phones (empty string = silent letters), cheapest first.
    Spell {
        phones: String,
        #[arg(short = 'n', long, default_value_t = 15)]
        top: usize,
    },
    /// How close `text` sounds to `phrase`, with the alignment.
    Score { text: String, phrase: String },
    /// Find short Unicode strings that sound like the phrase.
    Compress {
        phrase: String,
        #[arg(short = 'n', long, default_value_t = 10)]
        top: usize,
        /// Codepoint budget (spaces count).
        #[arg(short, long, default_value_t = 6)]
        max: usize,
        #[arg(long, default_value_t = SearchConfig::default().beam)]
        beam: usize,
        #[arg(long, default_value_t = SearchConfig::default().per_state)]
        per_state: usize,
        #[arg(long, default_value_t = SearchConfig::default().per_root)]
        per_root: usize,
        /// Debug: report where this known-good string ranks in generation.
        #[arg(long)]
        want: Option<String>,
        /// Print the results as a JSON array of `{text, latin, phones, score, codepoints}`.
        #[arg(long, conflicts_with = "want")]
        json: bool,
    },
    /// Compare predicted fidelity with what the TTS actually produced (phones recognized in
    /// harness clips, from `mise run //tools:export`).
    Calibrate {
        #[arg(default_value = "data/generated/clips.tsv")]
        clips: PathBuf,
        /// Show this many of the worst over-predictions.
        #[arg(long, default_value_t = 12)]
        worst: usize,
        /// Vowel-difference scale for recognized phones (recognizer vowel labels are noisy).
        #[arg(long, default_value_t = phono_core::score::Heard::default().vowel_scale)]
        heard_vowel_scale: f32,
        #[arg(long, default_value_t = phono_core::score::Heard::default().rhotic)]
        heard_rhotic: f32,
        /// Predict from the single best reading only (no n-best expectation).
        #[arg(long)]
        no_nbest: bool,
        /// Script risk multiplier [default: the model profile's].
        #[arg(long)]
        risk_scale: Option<f32>,
        /// Weight of spelling closeness to the phrase's real words (0 = sound only) [default:
        /// the model profile's].
        #[arg(long)]
        spelling: Option<f32>,
        /// Override the Phoible voicing weight (periodicGlottalSource).
        #[arg(long)]
        voicing: Option<f32>,
        /// Write per-clip predictions, outcomes and per-word features as TSV (for fitting).
        #[arg(long)]
        dump: Option<PathBuf>,
        /// Predict with v1–v5's hand-set weights instead of the fitted defaults (ignores
        /// --risk-scale and --spelling).
        #[arg(long)]
        hand: bool,
        /// Also summarize by script: each clip counts under every script in its own text, with
        /// the script-risk penalty it paid and the over-prediction with and without it.
        #[arg(long)]
        by_script: bool,
        /// Also summarize how each word of the reading was heard (recognized phones aligned
        /// per word), by word source and answer source, and list the worst-heard G2P words with
        /// any lower-ranked G2P reading that matches what was heard better.
        #[arg(long)]
        by_word: bool,
        /// Whisper text similarity from which an answer counts as heard well, for the
        /// close-enough table (each answer is the mean over its clips).
        #[arg(long, default_value_t = 0.8)]
        heard_well: f32,
        /// Also estimate each script's risk from these clips: least squares of the
        /// over-prediction without risk on codepoints per script, with a baseline per source
        /// (community, ours, probes). Prints current and fitted risks with 90% intervals.
        #[arg(long)]
        fit_risks: bool,
        #[command(flatten)]
        risk_fit: RiskFitArgs,
        #[command(flatten)]
        risks: RiskArgs,
    },
    /// Fit the scorer's weights to harness clips by within-phrase ranking, with cross-validation.
    Fit {
        #[arg(default_value = "data/generated/clips-all6.tsv")]
        clips: PathBuf,
        #[arg(long, default_value_t = 5)]
        folds: usize,
        /// What each cross-validation fold keeps together: phrases sharing a non-Latin chunk
        /// (default), or each phrase alone (v6's fit).
        #[arg(long, value_enum, default_value_t = fit::Grouping::Chunk)]
        group: fit::Grouping,
        #[arg(long, default_value_t = 4)]
        repeats: usize,
        /// What to fit against.
        #[arg(long, value_enum, default_value_t = fit::TruthKind::Mean)]
        truth: fit::TruthKind,
        /// Hold these parameters at their starting values (comma-separated names, or `all` to
        /// fit nothing, e.g. to measure `--refit-risks` alone).
        #[arg(long, value_delimiter = ',')]
        fix: Vec<String>,
        /// Start from, and regularize towards, the hand-set weights or the model profile's.
        #[arg(long, value_enum, default_value_t = fit::Prior::Hand)]
        prior: fit::Prior,
        /// L2 pull towards the hand-set weights (log space); cross-validation picks among these.
        #[arg(long, value_delimiter = ',', default_value = "0,0.003,0.01,0.03")]
        lambdas: Vec<f32>,
        /// Before each fit, set the script risks from its training answers as `calibrate
        /// --fit-risks` does, and hold `risk_scale`.
        #[arg(long)]
        refit_risks: bool,
        /// Make these scripts' risks free parameters of the ranking fit (comma-separated names,
        /// or `all`: every script but --risk-hold's). Scripts with fewer than --risk-min-clips
        /// clips stay put.
        #[arg(long, value_delimiter = ',', conflicts_with = "refit_risks")]
        free_risks: Vec<String>,
        #[command(flatten)]
        risk_fit: RiskFitArgs,
        #[command(flatten)]
        risks: RiskArgs,
    },
    /// Score the community library's entries and compress their phrases (each entry's
    /// `target`, else its `spoken` label). `mise run //tools:fetch-library` writes the library.
    Eval {
        #[arg(default_value = "sample/library.json")]
        messages: PathBuf,
        /// Also write our top answers in the same JSON shape, for a harness experiment
        /// (`cases_from`). It contains the sample phrases, so keep it out of git.
        #[arg(long)]
        export: Option<PathBuf>,
        /// Also summarize by script (each entry counts once under every script in its
        /// community text) and list the lowest-scoring community entries.
        #[arg(long)]
        by_script: bool,
        /// Score only the entries whose keys are listed in this file: one per line, or the
        /// last tab-separated field (so `tools/library-keys.tsv` selects the hand-picked 34).
        #[arg(long)]
        subset: Option<PathBuf>,
        /// Also summarize the entries listed in this file (same format as `--subset`), if it
        /// exists: by default the hand-picked 34 that exp1–exp10 used.
        #[arg(long, default_value = "tools/library-keys.tsv")]
        named_subset: PathBuf,
    },
}

/// How `calibrate --fit-risks` and `fit --refit-risks` estimate script risks.
#[derive(clap::Args)]
struct RiskFitArgs {
    /// Scripts whose risk stays as it is (the reference the others are measured against).
    #[arg(long, value_delimiter = ',', default_value = "latin,hangul,yi")]
    risk_hold: Vec<String>,
    /// Fit a script's risk only if at least this many clips use it.
    #[arg(long, default_value_t = risks::Opts::default().min_clips)]
    risk_min_clips: f32,
}

impl RiskFitArgs {
    fn opts(&self) -> Result<risks::Opts, String> {
        Ok(risks::Opts {
            hold: self
                .risk_hold
                .iter()
                .map(|n| Script::from_name(n))
                .collect::<Result<_, _>>()?,
            min_clips: self.risk_min_clips,
            ..risks::Opts::default()
        })
    }
}

/// Per-script risk overrides for prediction, to try values before they go in a profile.
#[derive(clap::Args)]
struct RiskArgs {
    /// Predict with these per-codepoint risks instead of the profile's (comma-separated
    /// `script=value`, e.g. `katakana=0.01,ucas=0.02`). The search's tiler still orders by the
    /// profile's.
    #[arg(long, value_delimiter = ',')]
    risks: Vec<String>,
}

impl RiskArgs {
    fn apply(&self, engine: &mut Engine) -> Result<(), String> {
        for spec in &self.risks {
            let (name, value) = spec
                .split_once('=')
                .ok_or(format!("--risks: expected script=value, got {spec:?}"))?;
            let value: f32 = value
                .trim()
                .parse()
                .map_err(|e| format!("--risks {spec:?}: {e}"))?;
            engine.risks[Script::from_name(name)?.index()] = value;
        }
        Ok(())
    }
}

/// One `phono compress --json` result. The wasm binding returns the same fields except
/// `codepoints`, and `//crates/wasm:parity` compares the two.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Guess<'a> {
    text: &'a str,
    latin: &'a str,
    phones: String,
    score: f32,
    /// `score` reaches the model profile's close-enough cut.
    close_enough: bool,
    codepoints: usize,
}

impl<'a> Guess<'a> {
    fn new(c: &'a Candidate, profile: &phono_core::model::ModelProfile) -> Self {
        Guess {
            text: &c.text,
            latin: &c.latin,
            phones: fmt_word_phones(c.words.iter().map(|w| w.phones.as_slice())),
            score: c.fidelity,
            close_enough: profile.is_close_enough(c.fidelity),
            codepoints: c.codepoints,
        }
    }
}

fn words_str(words: &[Word]) -> String {
    words
        .iter()
        .map(|w| format!("{}[{}]", w.text, fmt_phones(&w.phones)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    let t = Instant::now();
    let bytes = std::fs::read(&cli.data)
        .map_err(|e| format!("{}: {e} (run `mise run data`)", cli.data.display()))?;
    let profile = phono_core::model::profile(&cli.model)?;
    let mut engine = Engine::from_bytes(&bytes, profile)?;
    let scripts = match &cli.scripts {
        Some(names) => Scripts::from_names(names)?,
        None if cli.all_scripts => Scripts::All,
        None => Scripts::Default,
    };
    engine.configure(profile, scripts)?;
    eprintln!(
        "loaded {} in {:.2}s",
        cli.data.display(),
        t.elapsed().as_secs_f32()
    );

    match cli.command {
        Command::Phones { phrase } => {
            for w in engine.target_words(&phrase) {
                let kind = if w.lexical { "word" } else { "name" };
                println!("{:16} {kind}  {}", w.text, fmt_phones(&w.phones));
            }
        }
        Command::Spell { phones, top } => {
            let ph = phono_core::phone::parse_pron(&phones).ok_or("bad ARPAbet")?;
            for sp in engine.g2p.spellings(&ph).iter().take(top) {
                println!("{:6} {:.2}", sp.letters, sp.cost);
            }
        }
        Command::Read { text } => {
            let (latin, words) = engine.read(&text);
            println!("latin: {latin:?}");
            println!("reads: {}", words_str(&words));
        }
        Command::Score { text, phrase } => {
            let (latin, words) = engine.read(&text);
            let a = engine.score(&text, &phrase);
            println!("latin:    {latin:?}");
            println!("reads:    {}", words_str(&words));
            println!(
                "target:   {}",
                fmt_word_phones(engine.target(&phrase).iter().map(Vec::as_slice))
            );
            println!(
                "fidelity: {:.3} (cost {:.2} / {:.2})",
                a.fidelity, a.cost, a.max_cost
            );
            let predicted =
                engine.predict(&text, &engine.target_of(&phrase), &engine.profile.predict);
            println!(
                "predict:  {predicted:.3} (n-best expectation, script risk, spelling: what the search ranks by)"
            );
            let ops: Vec<String> = a
                .ops
                .iter()
                .map(|op| match op {
                    Op::Match(t, c, s) if t == c && *s == 0.0 => format!("{t}"),
                    Op::Match(t, c, s) => format!("{t}→{c}({s:.2})"),
                    Op::Delete(t, s) => format!("-{t}({s:.2})"),
                    Op::Insert(c, s) => format!("+{c}({s:.2})"),
                })
                .collect();
            println!("align:    {}", ops.join(" "));
        }
        Command::Compress {
            phrase,
            top,
            max,
            beam,
            per_state,
            per_root,
            want,
            json,
        } => {
            let searcher = Searcher::new(&engine);
            let cfg = SearchConfig {
                max_codepoints: max,
                top_n: top,
                beam,
                per_state,
                per_root,
                ..Default::default()
            };
            let t = Instant::now();
            let results = searcher.search(&phrase, &cfg);
            eprintln!("searched in {:.2}s", t.elapsed().as_secs_f32());
            if json {
                let guesses: Vec<Guess> = results
                    .iter()
                    .map(|c| Guess::new(c, engine.profile))
                    .collect();
                let out = serde_json::to_string(&guesses).map_err(|e| e.to_string())?;
                println!("{out}");
                return Ok(());
            }
            if let Some(w) = want {
                let (rank, total) = searcher.generation_rank(&phrase, &w, &cfg);
                let a = engine.score(&w, &phrase);
                match rank {
                    Some(r) => println!(
                        "want {w}: generated at rank {r} of {total} (re-rank budget {}), fidelity {:.3}",
                        cfg.rerank, a.fidelity
                    ),
                    None => println!(
                        "want {w}: not generated ({total} candidates), fidelity {:.3}",
                        a.fidelity
                    ),
                }
            }
            println!(
                "target: {}",
                fmt_word_phones(engine.target(&phrase).iter().map(Vec::as_slice))
            );
            for (i, c) in results.iter().enumerate() {
                println!(
                    "{:2}. {:.3}  {}cp  {:<8}  {:<28} {}",
                    i + 1,
                    c.fidelity,
                    c.codepoints,
                    c.text,
                    format!("{:?}", c.latin),
                    words_str(&c.words)
                );
            }
            if let Some(note) = no_good_result(&results, max, engine.profile) {
                println!("{note}");
            }
        }
        Command::Calibrate {
            clips,
            worst,
            heard_vowel_scale,
            heard_rhotic,
            no_nbest,
            risk_scale,
            spelling,
            voicing,
            dump,
            hand,
            by_script,
            by_word,
            heard_well,
            fit_risks,
            risk_fit,
            risks,
        } => {
            let heard = phono_core::score::Heard {
                vowel_scale: heard_vowel_scale,
                rhotic: heard_rhotic,
            };
            let defaults = engine.profile.predict;
            let mut predict = phono_core::Predict {
                nbest: !no_nbest,
                risk_scale: risk_scale.unwrap_or(defaults.risk_scale),
                spelling: spelling.unwrap_or(defaults.spelling),
            };
            let mut engine = engine;
            if hand {
                engine.weights = phono_core::score::Weights::HAND;
                predict = phono_core::Predict {
                    nbest: !no_nbest,
                    ..phono_core::Predict::HAND
                };
            }
            if let Some(v) = voicing {
                engine.set_feature_weight("periodicGlottalSource", v)?;
            }
            risks.apply(&mut engine)?;
            let report = Report {
                worst,
                dump,
                by_script,
                by_word,
                heard_well,
                fit_risks: if fit_risks {
                    Some(risk_fit.opts()?)
                } else {
                    None
                },
            };
            calibrate(&engine, &clips, &heard, &predict, &report)?
        }
        Command::Fit {
            clips,
            folds,
            group,
            repeats,
            truth,
            fix,
            lambdas,
            prior,
            refit_risks,
            free_risks,
            risk_fit,
            risks,
        } => {
            let mut engine = engine;
            risks.apply(&mut engine)?;
            let risk_opts = risk_fit.opts()?;
            let free_risks: Vec<Script> = if free_risks.iter().any(|n| n == "all") {
                Script::ALL
                    .into_iter()
                    .filter(|s| !risk_opts.hold.contains(s))
                    .collect()
            } else {
                free_risks
                    .iter()
                    .map(|n| Script::from_name(n))
                    .collect::<Result<_, _>>()?
            };
            let opts = fit::FitOpts {
                folds,
                group,
                repeats,
                lambdas,
                truth,
                fix,
                prior,
                risk_min_clips: risk_opts.min_clips,
                refit_risks: refit_risks.then_some(risk_opts),
                free_risks,
            };
            fit::run(
                &mut engine,
                &clips,
                &phono_core::score::Heard::default(),
                &opts,
            )?
        }
        Command::Eval {
            messages,
            export,
            by_script,
            subset,
            named_subset,
        } => eval(
            &engine,
            &messages,
            export.as_ref(),
            by_script,
            subset.as_ref(),
            &named_subset,
        )?,
    }
    Ok(())
}

/// What to tell the user when no guess reaches the profile's close-enough cut, or `None`.
fn no_good_result(
    results: &[Candidate],
    max: usize,
    profile: &phono_core::model::ModelProfile,
) -> Option<String> {
    if results.iter().any(|c| profile.is_close_enough(c.fidelity)) {
        return None;
    }
    Some(format!(
        "No string of {max} codepoints or fewer scores well (close enough on {} is {:.2} or \
         more). Try a higher budget or a shorter phrase.",
        profile.id(),
        profile.close_enough
    ))
}

/// Reads a reference messages file: `[{"<key>": {"unicode": .., "spoken": .., "target": ..}}, ..]`.
/// Returns `(key, unicode, phrase)` in file order, where the phrase is the entry's `target`
/// (set by `//tools:fetch-library`) or, when that is missing or empty, its `spoken` label.
fn load_messages(path: &PathBuf) -> Result<Vec<(String, String, String)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_messages(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn parse_messages(text: &str) -> Result<Vec<(String, String, String)>, String> {
    let items: Vec<serde_json::Map<String, serde_json::Value>> =
        serde_json::from_str(text).map_err(|e| e.to_string())?;
    let field = |m: &serde_json::Value, name: &str| {
        m.get(name)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let mut out = Vec::new();
    for item in &items {
        for (key, m) in item {
            let unicode = field(m, "unicode").ok_or(format!("{key}: no unicode field"))?;
            let phrase = field(m, "target")
                .or_else(|| field(m, "spoken"))
                .ok_or(format!("{key}: no spoken field"))?;
            out.push((key.clone(), unicode, phrase));
        }
    }
    Ok(out)
}

/// Keys from a subset file: one per line (the last tab-separated field), `#` comments skipped.
fn parse_keys(text: &str) -> std::collections::HashSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.rsplit('\t').next().map(|k| k.trim().to_string()))
        .collect()
}

/// The scripts a message is written in, in [`Script::ALL`] order. Whitespace and ASCII
/// punctuation don't count, so `latin` means Latin letters or digits.
fn scripts_in(text: &str) -> Vec<Script> {
    let mut out: Vec<Script> = text
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
        .map(Script::of)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Means of per-item values grouped by script, for `eval` and `calibrate --by-script`. Each
/// item counts once under every script in its text ([`scripts_in`]).
struct ByScript<const N: usize>(std::collections::BTreeMap<Script, (usize, [f32; N])>);

impl<const N: usize> ByScript<N> {
    fn new() -> Self {
        Self(Default::default())
    }

    fn add(&mut self, text: &str, values: [f32; N]) {
        for script in scripts_in(text) {
            let (n, sums) = self.0.entry(script).or_insert((0, [0.0; N]));
            *n += 1;
            for (s, v) in sums.iter_mut().zip(values) {
                *s += v;
            }
        }
    }

    /// (script, items, mean of each value), in [`Script::ALL`] order.
    fn rows(&self) -> Vec<(Script, usize, [f32; N])> {
        self.0
            .iter()
            .map(|(&s, &(n, sums))| (s, n, sums.map(|x| x / n as f32)))
            .collect()
    }
}

fn eval(
    engine: &Engine,
    path: &PathBuf,
    export: Option<&PathBuf>,
    by_script: bool,
    subset: Option<&PathBuf>,
    named_subset: &PathBuf,
) -> Result<(), String> {
    let mut messages = load_messages(path)?;
    if let Some(keys_path) = subset {
        let text = std::fs::read_to_string(keys_path)
            .map_err(|e| format!("{}: {e}", keys_path.display()))?;
        let keys = parse_keys(&text);
        messages.retain(|(id, _, _)| keys.contains(id));
        eprintln!(
            "subset {}: {} of {} keys found",
            keys_path.display(),
            messages.len(),
            keys.len()
        );
    }
    if messages.is_empty() {
        return Err("no messages to score".into());
    }
    let searcher = Searcher::new(engine);
    let cfg = SearchConfig {
        top_n: 1,
        ..Default::default()
    };
    let mut results: Vec<(&str, f32, f32)> = Vec::new(); // id, community, ours
    let mut stats = ByScript::<3>::new(); // community, ours, ours ≥ community
    let mut scored: Vec<(f32, &str, &str)> = Vec::new();
    let mut exported = Vec::new();
    // Library keys (`u` + 8 hex) are wider than the hand-picked ones.
    let w = messages
        .iter()
        .map(|(id, _, _)| id.len())
        .max()
        .unwrap_or(0)
        .max(6);
    println!(
        "{:<w$} {:<28} {:>6} {:<10} {:>6} {:<10}",
        "id", "phrase", "theirs", "", "ours", ""
    );
    for (id, unicode, phrase) in &messages {
        let target = engine.target_of(phrase);
        let their = engine.predict(unicode, &target, &engine.profile.predict);
        let best = searcher.search(phrase, &cfg);
        let (our_text, our) = best
            .first()
            .map(|c| (c.text.clone(), c.fidelity))
            .unwrap_or_default();
        results.push((id, their, our));
        stats.add(unicode, [their, our, f32::from(u8::from(our >= their))]);
        scored.push((their, id, unicode));
        println!(
            "{id:<w$} {:<28} {their:>6.3} {unicode:<10} {our:>6.3} {our_text:<10}",
            truncate(phrase, 28)
        );
        exported.push(format!(
            "  {{\"{id}\": {{\"unicode\": {}, \"spoken\": {}, \"notes\": \"phono top-1, predicted {our:.3} (community {their:.3})\"}}}}",
            json_quote(&our_text),
            json_quote(phrase)
        ));
    }
    if let Some(out) = export {
        std::fs::write(out, format!("[\n{}\n]\n", exported.join(",\n")))
            .map_err(|e| format!("{}: {e}", out.display()))?;
        eprintln!("wrote {}", out.display());
    }
    if by_script {
        println!(
            "\n{:<20} {:>4} {:>9} {:>6} {:>6}",
            "script", "n", "community", "ours", "ours≥"
        );
        for (script, n, [t, o, w]) in stats.rows() {
            let w = (w * n as f32).round();
            println!("{:<20} {n:>4} {t:>9.3} {o:>6.3} {w:>6}", script.name());
        }
        scored.sort_by(|a, b| a.0.total_cmp(&b.0));
        println!("\nlowest community scores:");
        for (their, id, unicode) in scored.iter().take(15) {
            let names: Vec<_> = scripts_in(unicode).into_iter().map(Script::name).collect();
            println!("  {id:<w$} {their:>6.3}  {}", names.join(","));
        }
    }
    let all = Summary::of(&results);
    println!(
        "\nmean fidelity: community {:.3}, ours {:.3}; ours ≥ community on {}/{}",
        all.community, all.ours, all.wins, all.n
    );
    // A missing file just means no named subset, e.g. outside this repo's layout.
    if let Ok(text) = std::fs::read_to_string(named_subset) {
        let keys = parse_keys(&text);
        let picked: Vec<_> = results
            .iter()
            .copied()
            .filter(|(id, _, _)| keys.contains(*id))
            .collect();
        if !picked.is_empty() && picked.len() < results.len() {
            let sub = Summary::of(&picked);
            println!(
                "  {} ({} of {} keys): community {:.3}, ours {:.3}; ours ≥ community on {}/{}",
                named_subset.display(),
                sub.n,
                keys.len(),
                sub.community,
                sub.ours,
                sub.wins,
                sub.n
            );
        }
    }
    Ok(())
}

/// Mean community and ours fidelity over `eval` results, and how often ours ≥ community.
struct Summary {
    n: usize,
    community: f32,
    ours: f32,
    wins: usize,
}

impl Summary {
    fn of(results: &[(&str, f32, f32)]) -> Summary {
        let n = results.len();
        let mean =
            |f: fn(&(&str, f32, f32)) -> f32| results.iter().map(f).sum::<f32>() / n.max(1) as f32;
        Summary {
            n,
            community: mean(|r| r.1),
            ours: mean(|r| r.2),
            wins: results.iter().filter(|r| r.2 >= r.1).count(),
        }
    }
}

fn json_quote(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "…"
    }
}

struct Clip {
    exp: String,
    case: String,
    text: String,
    target: String,
    whisper: String,
    phones: String,
}

/// Harness clips recorded with one of `profile`'s models; clips from other models are skipped.
fn load_clips(
    path: &PathBuf,
    profile: &phono_core::model::ModelProfile,
) -> Result<Vec<Clip>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("{}: {e} (run `mise run //tools:export`)", path.display()))?;
    let mut out = Vec::new();
    let mut other = 0;
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 8 {
            continue;
        }
        if !profile.model_ids.contains(&f[3]) {
            other += 1;
            continue;
        }
        out.push(Clip {
            exp: f[0].into(),
            case: f[1].into(),
            text: f[4].into(),
            target: f[5].into(),
            whisper: f[6].into(),
            phones: f[7].into(),
        });
    }
    if other > 0 {
        eprintln!(
            "skipped {other} clips from models other than {}",
            profile.model_ids.join("/")
        );
    }
    if out.is_empty() {
        return Err(format!(
            "{}: no clips from {}",
            path.display(),
            profile.model_ids.join("/")
        ));
    }
    Ok(out)
}

fn pearson(x: &[f32], y: &[f32]) -> f32 {
    let n = x.len() as f32;
    let (mx, my) = (x.iter().sum::<f32>() / n, y.iter().sum::<f32>() / n);
    let cov: f32 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let vx: f32 = x.iter().map(|a| (a - mx).powi(2)).sum();
    let vy: f32 = y.iter().map(|b| (b - my).powi(2)).sum();
    cov / (vx * vy).sqrt()
}

fn ranks(v: &[f32]) -> Vec<f32> {
    let mut idx: Vec<usize> = (0..v.len()).collect();
    idx.sort_by(|&a, &b| v[a].total_cmp(&v[b]));
    let mut r = vec![0.0; v.len()];
    for (pos, &i) in idx.iter().enumerate() {
        r[i] = pos as f32;
    }
    r
}

/// Letter-level similarity (1 − normalized edit distance), ignoring case, spaces, punctuation.
fn text_sim(a: &str, b: &str) -> f32 {
    let norm = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect::<Vec<char>>()
    };
    let (a, b) = (norm(a), norm(b));
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(x != y))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        prev = cur;
    }
    1.0 - prev[b.len()] as f32 / a.len().max(b.len()) as f32
}

/// What the TTS actually produced for a clip: (phone-level fidelity of the recognized phones,
/// Whisper text similarity). Truth keeps its original definition (phones of the reader's words,
/// hand-set weights in `scorer`) across versions. `phono calibrate` and `phono fit` use their mean.
fn clip_truth(
    engine: &Engine,
    scorer: &phono_core::score::Scorer,
    c: &Clip,
    heard: &phono_core::score::Heard,
) -> (f32, f32) {
    let target = phono_core::score::flatten_target(&engine.target(&c.target));
    let heard_phones: Vec<usize> = c
        .phones
        .split_whitespace()
        .flat_map(|t| engine.features.parse_ipa_token(t))
        .collect();
    (
        scorer.fidelity_segments(&engine.features, &target, &heard_phones, heard),
        text_sim(&c.whisper, &c.target),
    )
}

/// Truth difference below which two answers count as a tie.
const RANK_MARGIN: f32 = 0.05;

/// The decision the search makes: of two answers for the same phrase, which is better? Clips
/// are averaged per (phrase, text) over seeds; pairs closer than [`RANK_MARGIN`] are skipped.
/// Returns (pairs ordered right, pairs).
fn within_phrase_ranking(rows: &[(&Clip, f32)], truth: &[f32]) -> (usize, usize) {
    let mut by_text: std::collections::BTreeMap<(&str, &str), (f32, f32, f32)> = Default::default();
    for ((clip, pred), &t) in rows.iter().zip(truth) {
        let e = by_text
            .entry((clip.target.as_str(), clip.text.as_str()))
            .or_default();
        e.0 += pred;
        e.1 += t;
        e.2 += 1.0;
    }
    let answers: Vec<(&str, f32, f32)> = by_text
        .iter()
        .map(|(&(target, _), &(p, t, n))| (target, p / n, t / n))
        .collect();
    let (mut right, mut pairs) = (0, 0);
    for (i, a) in answers.iter().enumerate() {
        for b in &answers[i + 1..] {
            if a.0 == b.0 && (a.2 - b.2).abs() >= RANK_MARGIN {
                pairs += 1;
                right += usize::from((a.1 - b.1) * (a.2 - b.2) > 0.0);
            }
        }
    }
    (right, pairs)
}

/// Distinct answers (phrase, text) with the mean predicted fidelity and mean Whisper similarity
/// of their clips. A text replayed in several experiments is one answer.
fn answer_means(rows: &[(&Clip, f32, f32)]) -> Vec<(f32, f32)> {
    let mut by_text: std::collections::BTreeMap<(&str, &str), (f32, f32, f32)> = Default::default();
    for &(clip, pred, whisper) in rows {
        let e = by_text
            .entry((clip.target.as_str(), clip.text.as_str()))
            .or_default();
        e.0 += pred;
        e.1 += whisper;
        e.2 += 1.0;
    }
    by_text.values().map(|&(p, w, n)| (p / n, w / n)).collect()
}

/// How a predicted-fidelity cut splits answers into heard well (Whisper ≥ the `heard_well`
/// level) and not.
#[derive(Debug, PartialEq)]
struct Cut {
    cut: f32,
    above: usize,
    above_well: usize,
    below: usize,
    below_well: usize,
}

impl Cut {
    fn of(answers: &[(f32, f32)], heard_well: f32, cut: f32) -> Cut {
        let (mut c, well) = (
            Cut {
                cut,
                above: 0,
                above_well: 0,
                below: 0,
                below_well: 0,
            },
            |w: f32| usize::from(w >= heard_well),
        );
        for &(pred, whisper) in answers {
            if pred >= cut {
                c.above += 1;
                c.above_well += well(whisper);
            } else {
                c.below += 1;
                c.below_well += well(whisper);
            }
        }
        c
    }

    /// Share of the heard-well answers above the cut.
    fn recall(&self) -> f32 {
        self.above_well as f32 / (self.above_well + self.below_well).max(1) as f32
    }

    /// Youden's J: share of heard-well answers above the cut minus share of the rest above it.
    fn youden(&self) -> f32 {
        let bad_above = self.above - self.above_well;
        let bad = bad_above + self.below - self.below_well;
        self.recall() - bad_above as f32 / bad.max(1) as f32
    }
}

/// Cuts 0.80, 0.82, … 0.98 and the profile's own, with the one that best separates heard-well
/// answers (highest Youden's J; the lowest cut on a tie).
fn close_enough_table(answers: &[(f32, f32)], heard_well: f32, own: f32) -> (Vec<Cut>, f32) {
    let mut cuts: Vec<f32> = (40..=49).map(|i| i as f32 / 50.0).collect();
    if !cuts.contains(&own) {
        cuts.push(own);
        cuts.sort_by(f32::total_cmp);
    }
    let table: Vec<Cut> = cuts
        .iter()
        .map(|&c| Cut::of(answers, heard_well, c))
        .collect();
    let best = table
        .iter()
        .fold(None::<&Cut>, |b, c| match b {
            Some(b) if b.youden() >= c.youden() => Some(b),
            _ => Some(c),
        })
        .map_or(own, |c| c.cut);
    (table, best)
}

fn print_close_enough(
    answers: &[(f32, f32)],
    heard_well: f32,
    profile: &phono_core::model::ModelProfile,
) {
    let well = answers.iter().filter(|a| a.1 >= heard_well).count();
    println!(
        "\nclose enough: {} answers (phrase and text, clips averaged), {well} heard well (Whisper ≥ {heard_well:.2})",
        answers.len()
    );
    println!(
        "  {:>5} {:>6} {:>10} {:>6} {:>10} {:>7} {:>6}",
        "cut", "above", "heard well", "below", "heard well", "recall", "J"
    );
    let share = |k: usize, n: usize| {
        if n == 0 {
            "—".to_string()
        } else {
            format!("{:.0}%", 100.0 * k as f32 / n as f32)
        }
    };
    let (table, best) = close_enough_table(answers, heard_well, profile.close_enough);
    for c in &table {
        let mark = if c.cut == profile.close_enough {
            "  ← profile"
        } else {
            ""
        };
        println!(
            "  {:>5.2} {:>6} {:>10} {:>6} {:>10} {:>7.2} {:>6.2}{mark}",
            c.cut,
            c.above,
            share(c.above_well, c.above),
            c.below,
            share(c.below_well, c.below),
            c.recall(),
            c.youden()
        );
    }
    println!(
        "best split (Youden's J): {best:.2}; {} profile: {:.2}",
        profile.id(),
        profile.close_enough
    );
}

/// What `phono calibrate` prints and writes besides its summary.
struct Report {
    worst: usize,
    dump: Option<PathBuf>,
    by_script: bool,
    by_word: bool,
    heard_well: f32,
    /// Fit script risks with these options.
    fit_risks: Option<risks::Opts>,
}

fn calibrate(
    engine: &Engine,
    path: &PathBuf,
    heard: &phono_core::score::Heard,
    predict: &phono_core::Predict,
    report: &Report,
) -> Result<(), String> {
    let clips = load_clips(path, engine.profile)?;
    // Truth is measured with the hand-set weights whatever the engine predicts with.
    let scorer =
        phono_core::score::Scorer::with_weights(&engine.dist, phono_core::score::Weights::HAND);
    struct Row<'a> {
        clip: &'a Clip,
        predicted: f32,
        actual: f32,
        whisper: f32,
    }
    let mut rows = Vec::new();
    for c in &clips {
        let (actual, whisper) = clip_truth(engine, &scorer, c, heard);
        rows.push(Row {
            clip: c,
            predicted: engine.predict(&c.text, &engine.target_of(&c.target), predict),
            actual,
            whisper,
        });
    }
    let col = |f: fn(&Row) -> f32| rows.iter().map(f).collect::<Vec<f32>>();
    let (pred, act, wh) = (col(|r| r.predicted), col(|r| r.actual), col(|r| r.whisper));
    // Ground truth: mean of the phone-level and Whisper-text measures (independent errors).
    let truth: Vec<f32> = act.iter().zip(&wh).map(|(a, w)| (a + w) / 2.0).collect();
    if let Some(path) = &report.dump {
        let mut out = String::from(
            "exp\tcase\ttext\ttarget\tpredicted\tactual\twhisper\trisk\twords\tspell\tshare\n",
        );
        for r in &rows {
            let target = engine.target_words(&r.clip.target);
            let (_, read) = engine.read(&r.clip.text);
            let words: Vec<String> = phono_core::words::judge(&scorer, &target, &read)
                .iter()
                .map(|j| {
                    let flag = |b: bool, c: char| if b { c } else { '-' };
                    format!(
                        "{:.3}{}{}{}{}",
                        j.fidelity,
                        flag(j.lexical, 'L'),
                        flag(j.joined_prev, 'P'),
                        flag(j.joined_next, 'N'),
                        flag(j.dictionary_hit, 'H')
                    )
                })
                .collect();
            let risk = engine.risk(&r.clip.text);
            let (spell, share) = phono_core::words::spelling(&target, &read).unwrap_or((0.0, 0.0));
            out += &format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                r.clip.exp,
                r.clip.case,
                r.clip.text,
                r.clip.target,
                r.predicted,
                r.actual,
                r.whisper,
                risk,
                words.join(" "),
                spell,
                share
            );
        }
        std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    println!("{} clips", rows.len());
    println!(
        "predicted vs truth (mean of both): pearson {:.3}  spearman {:.3}",
        pearson(&pred, &truth),
        pearson(&ranks(&pred), &ranks(&truth))
    );
    println!(
        "predicted vs actual phones:  pearson {:.3}  spearman {:.3}",
        pearson(&pred, &act),
        pearson(&ranks(&pred), &ranks(&act))
    );
    println!(
        "predicted vs whisper text:   pearson {:.3}",
        pearson(&pred, &wh)
    );
    println!(
        "actual phones vs whisper:    pearson {:.3}  (do the two ground truths agree?)",
        pearson(&act, &wh)
    );
    let (right, pairs) = within_phrase_ranking(
        &rows
            .iter()
            .map(|r| (r.clip, r.predicted))
            .collect::<Vec<_>>(),
        &truth,
    );
    println!(
        "within-phrase ranking: {right}/{pairs} pairs of different answers ({:.1}%) ordered like truth (|Δ| ≥ {RANK_MARGIN})",
        100.0 * right as f32 / pairs.max(1) as f32
    );
    let mut exps: Vec<&str> = clips.iter().map(|c| c.exp.as_str()).collect();
    exps.dedup();
    for e in exps {
        let sel: Vec<&Row> = rows.iter().filter(|r| r.clip.exp == e).collect();
        let mean = |f: fn(&Row) -> f32| sel.iter().map(|r| f(r)).sum::<f32>() / sel.len() as f32;
        println!(
            "  {e}: n={:3}  predicted {:.3}  actual phones {:.3}  whisper {:.3}",
            sel.len(),
            mean(|r| r.predicted),
            mean(|r| r.actual),
            mean(|r| r.whisper)
        );
    }
    let answers = answer_means(
        &rows
            .iter()
            .map(|r| (r.clip, r.predicted, r.whisper))
            .collect::<Vec<_>>(),
    );
    print_close_enough(&answers, report.heard_well, engine.profile);
    if report.by_script {
        // Penalty as the predictor applies it: risk_scale × the profile's per-codepoint risk.
        let mut stats = ByScript::<3>::new(); // predicted, risk penalty, truth
        for (r, t) in rows.iter().zip(&truth) {
            let penalty = predict.risk_scale * engine.risk(&r.clip.text);
            stats.add(&r.clip.text, [r.predicted, penalty, *t]);
        }
        println!(
            "\n{:<20} {:>5} {:>9} {:>7} {:>6} {:>6} {:>12}",
            "script", "clips", "predicted", "penalty", "truth", "gap", "gap w/o risk"
        );
        for (script, n, [p, k, t]) in stats.rows() {
            println!(
                "{:<20} {n:>5} {p:>9.3} {k:>7.3} {t:>6.3} {:>+6.3} {:>+12.3}",
                script.name(),
                p - t,
                p + k - t
            );
        }
    }
    if let Some(opts) = &report.fit_risks {
        let mut phrases = std::collections::BTreeMap::new();
        let items: Vec<risks::Item> = rows
            .iter()
            .zip(&truth)
            .map(|(r, &t)| {
                let n = phrases.len();
                risks::Item {
                    cluster: *phrases.entry(r.clip.target.as_str()).or_insert(n),
                    ..risk_item(engine, predict, r.clip, t, 1.0)
                }
            })
            .collect();
        print_risk_fit(engine, &items, predict.risk_scale, opts);
    }
    if report.by_word {
        by_word::report(
            engine,
            &clips,
            &scorer,
            heard,
            phono_core::reading::READING_NBEST,
            report.worst,
        );
    }
    rows.sort_by(|a, b| (b.predicted - b.actual).total_cmp(&(a.predicted - a.actual)));
    println!("\nworst over-predictions (predicted − actual):");
    for r in rows.iter().take(report.worst) {
        let (latin, words) = engine.read(&r.clip.text);
        println!(
            "  {:+.2} {:5} {:7} {:<10} {:<22} pred {:.2} act {:.2}  reads {:?} → heard {:?}  whisper {:?}",
            r.predicted - r.actual,
            r.clip.exp,
            r.clip.case,
            r.clip.text,
            truncate(&r.clip.target, 22),
            r.predicted,
            r.actual,
            latin.trim(),
            r.clip.phones,
            r.clip.whisper
        );
        let _ = words;
    }
    Ok(())
}

/// A clip (or an answer averaged over `weight` clips) as an item for [`risks::fit`], in
/// cluster 0 (the caller sets the phrase).
fn risk_item(
    engine: &Engine,
    predict: &phono_core::Predict,
    clip: &Clip,
    truth: f32,
    weight: f32,
) -> risks::Item {
    let target = engine.target_of(&clip.target);
    let (_, words) = engine.read(&clip.text);
    let no_risk = phono_core::Predict {
        risk_scale: 0.0,
        ..*predict
    };
    let share = match phono_core::words::spelling(&target.words, &words) {
        Some((_, share)) if predict.spelling > 0.0 => 1.0 - predict.spelling * share,
        _ => 1.0,
    };
    risks::Item {
        counts: risks::Item::counts(&clip.text),
        share,
        excess: engine.predict_read(&clip.text, &words, &target, &no_risk) - truth,
        source: risks::Source::of_case(&clip.exp, &clip.case),
        weight,
        cluster: 0,
    }
}

/// Fit script risks on `items` and print the table.
fn print_risk_fit(engine: &Engine, items: &[risks::Item], risk_scale: f32, opts: &risks::Opts) {
    let f = risks::fit(items, &engine.risks, risk_scale, opts);
    let boot = risks::bootstrap(items, &engine.risks, risk_scale, opts, 200);
    let held: Vec<&str> = opts.hold.iter().map(|s| s.name()).collect();
    println!(
        "\nscript risks fitted on {} items (risk_scale {risk_scale}; held: {}; fitted if ≥ {} clips; 90% interval by resampling phrases):",
        items.len(),
        held.join(", "),
        opts.min_clips
    );
    let base: Vec<String> = risks::Source::ALL
        .iter()
        .zip(f.baseline)
        .filter_map(|(s, b)| b.map(|b| format!("{} {b:+.3}", s.name())))
        .collect();
    println!("baseline over-prediction: {}", base.join(", "));
    println!(
        "{:<20} {:>6} {:>6} {:>6} {:>8} {:>8} {:>8} {:>17}",
        "script", "comm.", "ours", "probe", "cp/clip", "current", "fitted", "90% interval"
    );
    for (i, script) in Script::ALL.iter().enumerate() {
        let mut clips = [0f32; 3];
        let mut cps = 0f32;
        for it in items.iter().filter(|it| it.counts[i] > 0.0) {
            clips[it.source as usize] += it.weight;
            cps += it.weight * it.counts[i];
        }
        let total: f32 = clips.iter().sum();
        if total == 0.0 {
            continue;
        }
        let interval = if f.free[i] {
            let mut v: Vec<f32> = boot.iter().map(|b| b[i]).collect();
            v.sort_by(f32::total_cmp);
            let q = |p: f32| v[((v.len() - 1) as f32 * p).round() as usize];
            format!("[{:+.3}, {:+.3}]", q(0.05), q(0.95))
        } else {
            "held".to_string()
        };
        let fitted = if f.free[i] {
            format!("{:+.3}", f.raw[i])
        } else {
            "-".to_string()
        };
        println!(
            "{:<20} {:>6} {:>6} {:>6} {:>8.2} {:>8.3} {:>8} {:>17}",
            script.name(),
            clips[0],
            clips[1],
            clips[2],
            cps / total,
            engine.risks[i],
            fitted,
            interval
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ByScript, Clip, Cut, Summary, answer_means, close_enough_table, no_good_result, parse_keys,
        parse_messages, scripts_in,
    };
    use phono_core::chunks::Script;
    use phono_core::model::FLASH_V2_5;
    use phono_core::search::Candidate;

    #[test]
    fn messages_prefer_target_and_fall_back_to_spoken() {
        let text = r#"[
            {"8": {"id": "x", "unicode": "ab", "spoken": "a/b", "target": "a", "notes": ""}},
            {"u1": {"unicode": "cd", "spoken": "c d", "characters": [{"unicodeName": "C"}]}},
            {"3": {"unicode": "ef", "spoken": "e f", "target": ""}}
        ]"#;
        let got = parse_messages(text).unwrap();
        let want = [("8", "ab", "a"), ("u1", "cd", "c d"), ("3", "ef", "e f")];
        assert_eq!(got.len(), want.len());
        for ((k, u, p), (wk, wu, wp)) in got.iter().zip(want) {
            assert_eq!((k.as_str(), u.as_str(), p.as_str()), (wk, wu, wp));
        }
    }

    #[test]
    fn messages_need_unicode_and_a_phrase() {
        assert!(parse_messages(r#"[{"1": {"spoken": "a"}}]"#).is_err());
        assert!(parse_messages(r#"[{"1": {"unicode": "a"}}]"#).is_err());
        assert!(parse_messages("not json").is_err());
    }

    #[test]
    fn subset_keys_take_the_last_tab_field_and_skip_comments() {
        let keys = parse_keys("# header\n\n8\nabcd-1234\t10\n  u1a2b3c4d  \n");
        let mut got: Vec<_> = keys.into_iter().collect();
        got.sort();
        assert_eq!(got, ["10", "8", "u1a2b3c4d"]);
    }

    #[test]
    fn summary_means_and_counts_ties_as_wins() {
        let s = Summary::of(&[("a", 0.8, 0.9), ("b", 0.6, 0.6), ("c", 1.0, 0.5)]);
        assert_eq!((s.n, s.wins), (3, 2));
        assert!((s.community - 0.8).abs() < 1e-6);
        assert!((s.ours - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn scripts_ignore_spaces_and_punctuation() {
        assert_eq!(scripts_in("ꀠ 딧-잇!"), [Script::Hangul, Script::Yi]);
        assert_eq!(
            scripts_in("Z Q丁낫x"),
            [Script::Latin, Script::Hangul, Script::CjkUnified]
        );
        assert!(scripts_in(" -!? ").is_empty());
    }

    #[test]
    fn by_script_counts_each_item_once_per_script() {
        let mut stats = ByScript::<2>::new();
        stats.add("ꀠ딧", [0.5, 0.9]);
        stats.add("딧잇", [0.7, 0.6]);
        let rows = stats.rows();
        assert_eq!(rows.len(), 2);
        let (script, n, [a, b]) = rows[0];
        assert_eq!((script, n), (Script::Hangul, 2));
        assert!((a - 0.6).abs() < 1e-6 && (b - 0.75).abs() < 1e-6);
        assert_eq!((rows[1].0, rows[1].1), (Script::Yi, 1));
        assert_eq!(rows[1].2, [0.5, 0.9]);
    }

    fn clip(target: &str, text: &str) -> Clip {
        Clip {
            exp: "exp".into(),
            case: "c".into(),
            text: text.into(),
            target: target.into(),
            whisper: String::new(),
            phones: String::new(),
        }
    }

    #[test]
    fn answers_average_their_clips() {
        let (a, b, c) = (clip("hi", "ꀠ"), clip("hi", "ꀠ"), clip("hi", "딧"));
        let means = answer_means(&[(&a, 0.9, 1.0), (&b, 0.8, 0.5), (&c, 0.6, 0.2)]);
        assert_eq!(means.len(), 2);
        let (p, w) = means[0]; // "ꀠ" (U+A020) sorts before "딧"
        assert!((p - 0.85).abs() < 1e-6 && (w - 0.75).abs() < 1e-6);
        assert_eq!(means[1], (0.6, 0.2));
    }

    #[test]
    fn cut_counts_heard_well_on_each_side() {
        let answers = [(0.95, 0.9), (0.92, 0.5), (0.85, 0.85), (0.7, 0.1)];
        let c = Cut::of(&answers, 0.8, 0.9);
        assert_eq!(
            c,
            Cut {
                cut: 0.9,
                above: 2,
                above_well: 1,
                below: 2,
                below_well: 1,
            }
        );
        assert!((c.recall() - 0.5).abs() < 1e-6);
        assert!((c.youden() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn table_picks_the_cut_that_separates_best_and_includes_the_profiles() {
        // Heard well exactly when predicted ≥ 0.9.
        let answers = [(0.97, 0.9), (0.91, 0.85), (0.89, 0.4), (0.82, 0.3)];
        let (table, best) = close_enough_table(&answers, 0.8, 0.91);
        assert_eq!(best, 0.9);
        assert!(table.iter().any(|c| c.cut == 0.91));
        assert!(table.windows(2).all(|w| w[0].cut < w[1].cut));
        assert_eq!(table.len(), 11);
    }

    #[test]
    fn note_only_when_no_guess_is_close_enough() {
        let guess = |fidelity| Candidate {
            text: "x".into(),
            latin: "x".into(),
            words: Vec::new(),
            fidelity,
            codepoints: 1,
            gen_cost: 0.0,
        };
        assert_eq!(
            no_good_result(&[guess(0.8), guess(0.9)], 6, &FLASH_V2_5),
            None
        );
        let note = no_good_result(&[guess(0.8)], 6, &FLASH_V2_5).unwrap();
        assert!(
            note.starts_with("No string of 6 codepoints or fewer scores well"),
            "{note}"
        );
        assert!(no_good_result(&[], 6, &FLASH_V2_5).is_some());
    }
}
