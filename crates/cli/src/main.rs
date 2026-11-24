//! `phono`: try the phonocompressor engine from the command line.

use clap::{Parser, Subcommand};
use phono_core::Engine;
use phono_core::phone::fmt_phones;
use phono_core::reading::Word;
use phono_core::score::Op;
use phono_core::search::{SearchConfig, Searcher};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(about = "Phonetic compression: phrases → a few Unicode characters a TTS reads back")]
struct Cli {
    /// Data file from `mise run data`.
    #[arg(long, global = true, default_value = "data/generated/phono.bin")]
    data: PathBuf,
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
    },
    /// Compare predicted fidelity with what the TTS actually produced (phones recognized in
    /// harness clips, from `mise run harness:export`).
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
        #[arg(long, default_value_t = phono_core::Predict::default().risk_scale)]
        risk_scale: f32,
        /// Override the Phoible voicing weight (periodicGlottalSource).
        #[arg(long)]
        voicing: Option<f32>,
    },
    /// Score the community sample messages and compress their phrases.
    Eval {
        #[arg(default_value = "sample/messages.json")]
        messages: PathBuf,
        /// Also write our top answers in the same JSON shape, for a harness experiment
        /// (`cases_from`). It contains the sample phrases, so keep it out of git.
        #[arg(long)]
        export: Option<PathBuf>,
    },
}

fn words_str(words: &[Word]) -> String {
    words.iter().map(|w| format!("{}[{}]", w.text, fmt_phones(&w.phones))).collect::<Vec<_>>().join(" ")
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    let t = Instant::now();
    let bytes = std::fs::read(&cli.data).map_err(|e| format!("{}: {e} (run `mise run data`)", cli.data.display()))?;
    let engine = Engine::from_bytes(&bytes)?;
    eprintln!("loaded {} in {:.2}s", cli.data.display(), t.elapsed().as_secs_f32());

    match cli.command {
        Command::Phones { phrase } => {
            for (w, p) in phrase.split_whitespace().zip(engine.target(&phrase)) {
                println!("{w:16} {}", fmt_phones(&p));
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
            println!("target:   {}", engine.target(&phrase).iter().map(|w| fmt_phones(w)).collect::<Vec<_>>().join(" | "));
            println!("fidelity: {:.3} (cost {:.2} / {:.2})", a.fidelity, a.cost, a.max_cost);
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
        Command::Compress { phrase, top, max, beam, per_state, per_root, want } => {
            let searcher = Searcher::new(&engine);
            let cfg = SearchConfig { max_codepoints: max, top_n: top, beam, per_state, per_root, ..Default::default() };
            let t = Instant::now();
            let results = searcher.search(&phrase, &cfg);
            eprintln!("searched in {:.2}s", t.elapsed().as_secs_f32());
            if let Some(w) = want {
                let (rank, total) = searcher.generation_rank(&phrase, &w, &cfg);
                let a = engine.score(&w, &phrase);
                match rank {
                    Some(r) => println!("want {w}: generated at rank {r} of {total} (re-rank budget {}), fidelity {:.3}", cfg.rerank, a.fidelity),
                    None => println!("want {w}: not generated ({total} candidates), fidelity {:.3}", a.fidelity),
                }
            }
            println!("target: {}", engine.target(&phrase).iter().map(|w| fmt_phones(w)).collect::<Vec<_>>().join(" | "));
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
        }
        Command::Calibrate { clips, worst, heard_vowel_scale, heard_rhotic, no_nbest, risk_scale, voicing } => {
            let heard = phono_core::score::Heard { vowel_scale: heard_vowel_scale, rhotic: heard_rhotic };
            let predict = phono_core::Predict { nbest: !no_nbest, risk_scale };
            let mut engine = engine;
            if let Some(v) = voicing {
                engine.set_feature_weight("periodicGlottalSource", v)?;
            }
            calibrate(&engine, &clips, worst, &heard, &predict)?
        }
        Command::Eval { messages, export } => eval(&engine, &messages, export.as_ref())?,
    }
    Ok(())
}

/// Minimal reader for the community messages file: [{"<id>": {"unicode": .., "spoken": ..}}, ..].
fn load_messages(path: &PathBuf) -> Result<Vec<(String, String, String)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    // Pull "unicode" and "spoken" string fields in order; ids are the keys before them.
    let mut rest = text.as_str();
    while let Some(pos) = rest.find("\"unicode\"") {
        let before = &rest[..pos];
        let id = before.rfind("\": {").and_then(|e| before[..e].rfind('"').map(|s| before[s + 1..e].to_string()));
        rest = &rest[pos..];
        let unicode = json_string_field(rest, "unicode").ok_or("bad unicode field")?;
        let spoken = json_string_field(rest, "spoken").ok_or("bad spoken field")?;
        out.push((id.unwrap_or_default(), unicode, spoken));
        rest = &rest[9..];
    }
    Ok(out)
}

fn json_string_field(s: &str, name: &str) -> Option<String> {
    let start = s.find(&format!("\"{name}\""))?;
    let after = &s[start + name.len() + 2..];
    let q = after.find('"')?;
    let mut out = String::new();
    let mut chars = after[q + 1..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

fn eval(engine: &Engine, path: &PathBuf, export: Option<&PathBuf>) -> Result<(), String> {
    let messages = load_messages(path)?;
    let searcher = Searcher::new(engine);
    let cfg = SearchConfig { top_n: 1, ..Default::default() };
    let (mut theirs, mut ours, mut wins) = (0.0, 0.0, 0);
    let mut exported = Vec::new();
    println!("{:<6} {:<28} {:>6} {:<10} {:>6} {:<10}", "id", "phrase", "theirs", "", "ours", "");
    for (id, unicode, spoken) in &messages {
        let target = phono_core::score::flatten_target(&engine.target(spoken));
        let their = engine.predict(unicode, &target, &cfg.predict);
        let best = searcher.search(spoken, &cfg);
        let (our_text, our) = best.first().map(|c| (c.text.clone(), c.fidelity)).unwrap_or_default();
        theirs += their;
        ours += our;
        if our >= their {
            wins += 1;
        }
        println!("{id:<6} {:<28} {their:>6.3} {unicode:<10} {our:>6.3} {our_text:<10}", truncate(spoken, 28));
        exported.push(format!(
            "  {{\"{id}\": {{\"unicode\": {}, \"spoken\": {}, \"notes\": \"phono v2 top-1, predicted {our:.3} (community {their:.3})\"}}}}",
            json_quote(&our_text),
            json_quote(spoken)
        ));
    }
    if let Some(out) = export {
        std::fs::write(out, format!("[\n{}\n]\n", exported.join(",\n"))).map_err(|e| format!("{}: {e}", out.display()))?;
        eprintln!("wrote {}", out.display());
    }
    let n = messages.len() as f32;
    println!(
        "\nmean fidelity: community {:.3}, ours {:.3}; ours ≥ community on {wins}/{}",
        theirs / n,
        ours / n,
        messages.len()
    );
    Ok(())
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
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}

struct Clip {
    exp: String,
    case: String,
    text: String,
    target: String,
    whisper: String,
    phones: String,
}

fn load_clips(path: &PathBuf) -> Result<Vec<Clip>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e} (run `mise run harness:export`)", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 8 {
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
    let norm = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect::<Vec<char>>();
    let (a, b) = (norm(a), norm(b));
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(x != y)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    1.0 - prev[b.len()] as f32 / a.len().max(b.len()) as f32
}

fn calibrate(
    engine: &Engine,
    path: &PathBuf,
    worst: usize,
    heard: &phono_core::score::Heard,
    predict: &phono_core::Predict,
) -> Result<(), String> {
    let clips = load_clips(path)?;
    let scorer = engine.scorer();
    struct Row<'a> {
        clip: &'a Clip,
        predicted: f32,
        actual: f32,
        whisper: f32,
    }
    let mut rows = Vec::new();
    for c in &clips {
        let target = phono_core::score::flatten_target(&engine.target(&c.target));
        let heard_phones: Vec<usize> = c.phones.split_whitespace().flat_map(|t| engine.features.parse_ipa_token(t)).collect();
        rows.push(Row {
            clip: c,
            predicted: engine.predict(&c.text, &target, predict),
            actual: scorer.fidelity_segments(&engine.features, &target, &heard_phones, heard),
            whisper: text_sim(&c.whisper, &c.target),
        });
    }
    let col = |f: fn(&Row) -> f32| rows.iter().map(f).collect::<Vec<f32>>();
    let (pred, act, wh) = (col(|r| r.predicted), col(|r| r.actual), col(|r| r.whisper));
    // Ground truth: mean of the phone-level and Whisper-text measures (independent errors).
    let truth: Vec<f32> = act.iter().zip(&wh).map(|(a, w)| (a + w) / 2.0).collect();
    println!("{} clips", rows.len());
    println!(
        "predicted vs truth (mean of both): pearson {:.3}  spearman {:.3}",
        pearson(&pred, &truth),
        pearson(&ranks(&pred), &ranks(&truth))
    );
    println!("predicted vs actual phones:  pearson {:.3}  spearman {:.3}", pearson(&pred, &act), pearson(&ranks(&pred), &ranks(&act)));
    println!("predicted vs whisper text:   pearson {:.3}", pearson(&pred, &wh));
    println!("actual phones vs whisper:    pearson {:.3}  (do the two ground truths agree?)", pearson(&act, &wh));
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
    rows.sort_by(|a, b| (b.predicted - b.actual).total_cmp(&(a.predicted - a.actual)));
    println!("\nworst over-predictions (predicted − actual):");
    for r in rows.iter().take(worst) {
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
