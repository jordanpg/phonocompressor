//! `phono fit`: tune the scorer's hand-set weights on recorded harness clips.
//!
//! The search's real decision is which of several answers for the same phrase to return, so
//! the objective is pairwise: for two recorded answers to one phrase, the one the TTS rendered
//! better (truth, as in `phono calibrate`) should be predicted higher. The loss is a logistic
//! (RankNet) loss on the predicted difference, weighted by the truth difference, plus an L2
//! pull towards the hand-set weights in log space.
//!
//! Generalization is checked two ways:
//! - k-fold cross-validation by phrase: the held-out phrases' answers are never seen in fitting.
//!   By default phrases whose answers share a non-Latin chunk of two or more glyphs stay in one
//!   fold ([`Grouping::Chunk`]), since a chunk's reading learned in one phrase carries to the next;
//! - "new in the last experiment": fit without the answers that first appeared in the last
//!   experiment, test on pairs involving them (the situation of a new harness run).
//!
//! With `--refit-risks`, each fit first sets the per-script risks from its own training answers
//! ([`crate::risks`], least squares of over-prediction), then fits the weights with those risks
//! held, so the held-out numbers test the whole procedure.

use crate::risks;
use crate::{Clip, RANK_MARGIN, clip_truth, load_clips};
use phono_core::chunks::Script;
use phono_core::reading::Word;
use phono_core::score::{Heard, Scorer, Weights};
use phono_core::{Engine, Predict, Target};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Pairs whose truths differ by less than this carry no signal worth fitting.
const MIN_PAIR: f32 = 0.02;
/// Logistic slope: a predicted difference of 0.05 is one logit.
const SLOPE: f32 = 20.0;

/// One distinct answer: a (phrase, text) pair with its truth averaged over all its clips.
struct Answer {
    phrase: usize,
    text: String,
    target: Target,
    words: Vec<Word>,
    /// Truth by each measure, indexed by [`TruthKind`] as usize.
    truths: [f32; 3],
    first_exp: String,
    source: risks::Source,
    clips: usize,
}

struct Pair {
    a: usize,
    b: usize,
    /// truth[a] − truth[b]
    dt: f32,
    phrase: usize,
}

#[derive(Clone, Debug)]
pub struct Params {
    pub weights: Weights,
    pub predict: Predict,
    /// Per-codepoint script risks ([`Engine::risks`]).
    pub risks: [f32; Script::COUNT],
}

/// A risk coordinate of 1 is this much risk. Risks move in linear space (several are 0, which
/// log space can't leave); with this unit the first step is ±0.035.
const RISK_UNIT: f32 = 0.05;

/// Every coordinate: [`NAMES`], then one risk per script (`risk.<script>`).
const PARAMS: usize = NAMES.len() + Script::COUNT;

fn param_name(i: usize) -> String {
    match NAMES.get(i) {
        Some(n) => n.to_string(),
        None => format!("risk.{}", Script::ALL[i - NAMES.len()].name()),
    }
}

/// Fitted parameters. `del_primary` stays at 1.0: scaling every cost together leaves fidelity
/// unchanged, so it anchors the scale.
const NAMES: [&str; 15] = [
    "del_secondary",
    "del_unstressed",
    "del_schwa",
    "del_onset",
    "del_cluster",
    "del_consonant",
    "ins_stressed",
    "ins_unstressed",
    "ins_consonant",
    "sub_scale",
    "vowel_sub_scale",
    "stress_mismatch",
    "class_mismatch",
    "spelling",
    "risk_scale",
];

fn field(p: &mut Params, i: usize) -> &mut f32 {
    if i >= NAMES.len() {
        return &mut p.risks[i - NAMES.len()];
    }
    let w = &mut p.weights;
    match i {
        0 => &mut w.del_secondary,
        1 => &mut w.del_unstressed,
        2 => &mut w.del_schwa,
        3 => &mut w.del_onset,
        4 => &mut w.del_cluster,
        5 => &mut w.del_consonant,
        6 => &mut w.ins_stressed,
        7 => &mut w.ins_unstressed,
        8 => &mut w.ins_consonant,
        9 => &mut w.sub_scale,
        10 => &mut w.vowel_sub_scale,
        11 => &mut w.stress_mismatch,
        12 => &mut w.class_mismatch,
        13 => &mut p.predict.spelling,
        14 => &mut p.predict.risk_scale,
        _ => unreachable!(),
    }
}

fn get(p: &Params, i: usize) -> f32 {
    *field(&mut p.clone(), i)
}

/// Coordinates of `p`: log space for weights, units of [`RISK_UNIT`] for risks.
fn coords(p: &Params) -> Vec<f32> {
    (0..PARAMS)
        .map(|i| {
            if i < NAMES.len() {
                get(p, i).max(1e-4).ln()
            } else {
                get(p, i) / RISK_UNIT
            }
        })
        .collect()
}

fn from_coords(base: &Params, x: &[f32]) -> Params {
    let mut p = base.clone();
    for (i, &v) in x.iter().enumerate() {
        let v = if i >= NAMES.len() {
            (v * RISK_UNIT).max(0.0)
        } else if NAMES[i] == "spelling" {
            v.exp().min(1.0) // a blend weight
        } else {
            v.exp()
        };
        *field(&mut p, i) = v;
    }
    p
}

fn predictions(engine: &mut Engine, answers: &[Answer], p: &Params) -> Vec<f32> {
    engine.weights = p.weights.clone();
    engine.risks = p.risks;
    answers
        .iter()
        .map(|a| engine.predict_read(&a.text, &a.words, &a.target, &p.predict))
        .collect()
}

fn softplus(x: f32) -> f32 {
    if x > 20.0 { x } else { x.exp().ln_1p() }
}

/// Mean truth-weighted logistic loss over `pairs`.
fn loss(pred: &[f32], pairs: &[&Pair]) -> f32 {
    let (mut l, mut w) = (0.0, 0.0);
    for p in pairs {
        let dp = pred[p.a] - pred[p.b];
        l += p.dt.abs() * softplus(-SLOPE * p.dt.signum() * dp);
        w += p.dt.abs();
    }
    if w > 0.0 { l / w } else { 0.0 }
}

/// (pairs ordered like truth, pairs), counting only pairs at least [`RANK_MARGIN`] apart, as
/// `phono calibrate` does.
fn ranking(pred: &[f32], pairs: &[&Pair]) -> (usize, usize) {
    let sel: Vec<&&Pair> = pairs.iter().filter(|p| p.dt.abs() >= RANK_MARGIN).collect();
    let right = sel
        .iter()
        .filter(|p| (pred[p.a] - pred[p.b]) * p.dt > 0.0)
        .count();
    (right, sel.len())
}

fn pearson(x: &[f32], y: &[f32]) -> f32 {
    let n = x.len() as f32;
    let (mx, my) = (x.iter().sum::<f32>() / n, y.iter().sum::<f32>() / n);
    let cov: f32 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let vx: f32 = x.iter().map(|a| (a - mx).powi(2)).sum();
    let vy: f32 = y.iter().map(|b| (b - my).powi(2)).sum();
    cov / (vx * vy).sqrt()
}

/// Within-phrase pairs of answers whose truths (by `kind`) differ by at least [`MIN_PAIR`].
fn make_pairs(answers: &[Answer], kind: TruthKind) -> Vec<Pair> {
    let mut pairs = Vec::new();
    for (a, x) in answers.iter().enumerate() {
        for (b, y) in answers.iter().enumerate().skip(a + 1) {
            let dt = x.truths[kind as usize] - y.truths[kind as usize];
            if x.phrase == y.phrase && dt.abs() >= MIN_PAIR {
                pairs.push(Pair {
                    a,
                    b,
                    dt,
                    phrase: x.phrase,
                });
            }
        }
    }
    pairs
}

/// Sort key for experiment names, so that `exp16` comes after `exp9`.
fn exp_order(name: &str) -> (usize, &str) {
    (name.len(), name)
}

pub(crate) fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Coordinate descent in log space from `start` over the `free` coordinates, halving the step
/// when none improves.
fn fit(
    engine: &mut Engine,
    answers: &[Answer],
    pairs: &[&Pair],
    start: &Params,
    lambda: f32,
    free: &[bool],
) -> (Params, usize) {
    let x0 = coords(start);
    let objective = |engine: &mut Engine, x: &[f32]| {
        let pred = predictions(engine, answers, &from_coords(start, x));
        let reg: f32 = x.iter().zip(&x0).map(|(a, b)| (a - b).powi(2)).sum();
        loss(&pred, pairs) + lambda * reg
    };
    let mut x = x0.clone();
    let mut best = objective(engine, &x);
    let mut evals = 1;
    let mut step = 2f32.ln();
    while step > 1.04f32.ln() {
        let mut improved = false;
        for i in (0..x.len()).filter(|&i| free[i]) {
            for dir in [1.0, -1.0] {
                loop {
                    let mut y = x.clone();
                    y[i] += dir * step;
                    let v = objective(engine, &y);
                    evals += 1;
                    if v < best - 1e-6 {
                        best = v;
                        x = y;
                        improved = true;
                    } else {
                        break;
                    }
                }
            }
        }
        if !improved {
            step /= 2.0;
        }
    }
    (from_coords(start, &x), evals)
}

pub struct FitOpts {
    pub folds: usize,
    /// What the cross-validation folds keep together.
    pub group: Grouping,
    /// Cross-validation runs, each with a different assignment of phrases to folds.
    pub repeats: usize,
    pub lambdas: Vec<f32>,
    pub truth: TruthKind,
    /// Parameters held at their starting values (see `prior`).
    pub fix: Vec<String>,
    /// Where the fit starts and what the L2 term pulls towards.
    pub prior: Prior,
    /// Refit the script risks on each fit's training answers first (`risk_scale` is then held).
    pub refit_risks: Option<risks::Opts>,
    /// Scripts whose risks are free parameters of the ranking fit,
    /// if at least `risk_min_clips` clips use them.
    pub free_risks: Vec<Script>,
    pub risk_min_clips: f32,
}

/// Starting point and regularization centre of a fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Prior {
    /// v1–v5's hand-set weights (`Weights::HAND`, `Predict::HAND`), as v6 was fitted.
    Hand,
    /// The model profile's current weights, e.g. Flash v2.5's for a new model's first fit.
    Profile,
}

/// What a cross-validation fold keeps together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Grouping {
    /// Each phrase on its own (v6's fit).
    Phrase,
    /// Phrases connected by a shared non-Latin chunk of two or more glyphs ([`chunk_keys`]).
    Chunk,
}

/// The two-glyph chunks in `text` outside Latin: consecutive non-whitespace characters from
/// U+0250 (past Latin Extended-B) up. Two texts that share a longer chunk share one of these.
fn chunk_keys(text: &str) -> Vec<(char, char)> {
    let glyph = |c: char| c as u32 >= 0x250 && !c.is_whitespace();
    let chars: Vec<char> = text.chars().collect();
    chars
        .windows(2)
        .filter(|w| glyph(w[0]) && glyph(w[1]))
        .map(|w| (w[0], w[1]))
        .collect()
}

/// A group id for each of `n` phrases: phrases whose texts share a key are connected, and each
/// connected component is one group. Ids are the smallest phrase index in the group.
fn groups<K: Ord>(n: usize, keyed: impl IntoIterator<Item = (usize, K)>) -> Vec<usize> {
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut parent: Vec<usize> = (0..n).collect();
    let mut first: BTreeMap<K, usize> = BTreeMap::new();
    for (phrase, key) in keyed {
        let other = *first.entry(key).or_insert(phrase);
        let (a, b) = (find(&mut parent, phrase), find(&mut parent, other));
        parent[a.max(b)] = a.min(b);
    }
    (0..n).map(|i| find(&mut parent, i)).collect()
}

/// Fold of each phrase: groups in a scrambled order (a different one per `repeat`), each put in
/// the fold with the fewest phrases so far, so a large group doesn't unbalance the folds.
fn assign_folds(group: &[usize], folds: usize, repeat: usize) -> Vec<usize> {
    let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
    for &g in group {
        *sizes.entry(g).or_default() += 1;
    }
    let mut order: Vec<(usize, usize)> = sizes.into_iter().collect();
    order.sort_by_key(|&(g, n)| {
        (
            std::cmp::Reverse(n),
            splitmix(splitmix(repeat as u64) ^ g as u64),
        )
    });
    let mut load = vec![0; folds];
    let mut fold_of_group = BTreeMap::new();
    for (g, n) in order {
        let k = (0..folds).min_by_key(|&k| (load[k], k)).unwrap();
        load[k] += n;
        fold_of_group.insert(g, k);
    }
    group.iter().map(|g| fold_of_group[g]).collect()
}

/// Which measure of what the TTS produced to fit against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum TruthKind {
    /// Mean of recognized-phone fidelity and Whisper text similarity (as `phono calibrate`).
    Mean,
    /// Recognized-phone fidelity only. It is computed with the hand-set alignment costs.
    Phones,
    /// Whisper text similarity only: independent of every scorer weight.
    Whisper,
}

pub fn run(
    engine: &mut Engine,
    path: &PathBuf,
    heard: &Heard,
    opts: &FitOpts,
) -> Result<(), String> {
    let clips: Vec<Clip> = load_clips(path, engine.profile)?;
    let truth_scorer = Scorer::with_weights(&engine.dist, Weights::HAND);
    // Distinct answers, truth averaged over seeds and over experiments that reused a clip.
    let fix_all = opts.fix.iter().any(|f| f == "all");
    if opts.refit_risks.is_some() && !opts.free_risks.is_empty() {
        return Err("--refit-risks and --free-risks are alternatives".into());
    }
    for name in opts.fix.iter().filter(|f| *f != "all") {
        if !NAMES.contains(&name.as_str()) {
            return Err(format!(
                "--fix: no parameter {name:?} (have {})",
                NAMES.join(", ")
            ));
        }
    }
    // Refitted risks and a free `risk_scale` would trade off against each other: hold the scale.
    let mut script_clips = [0f32; Script::COUNT];
    for c in &clips {
        for (n, k) in script_clips.iter_mut().zip(risks::Item::counts(&c.text)) {
            if k > 0.0 {
                *n += 1.0;
            }
        }
    }
    let free_risks: Vec<Script> = opts
        .free_risks
        .iter()
        .copied()
        .filter(|s| script_clips[s.index()] >= opts.risk_min_clips)
        .collect();
    if !opts.free_risks.is_empty() {
        let names: Vec<&str> = free_risks.iter().map(|s| s.name()).collect();
        println!("free risks: {}", names.join(", "));
    }
    // Least-squares risks and a free `risk_scale` would trade off against each other, so
    // `--refit-risks` holds the scale. With `--free-risks` the L2 term separates the two (the
    // scale is a shared cut, the risks each script's departure from it); `--fix risk_scale`
    // holds it.
    let free: Vec<bool> = (0..PARAMS)
        .map(|i| match NAMES.get(i) {
            Some(n) => {
                !fix_all
                    && !opts.fix.iter().any(|f| f == n)
                    && !(opts.refit_risks.is_some() && *n == "risk_scale")
            }
            None => free_risks.contains(&Script::ALL[i - NAMES.len()]),
        })
        .collect();
    /// An answer's clips so far: truth sums, count, earliest experiment, source.
    struct Tally {
        sums: [f32; 3],
        n: usize,
        first_exp: String,
        source: risks::Source,
    }
    let mut by_answer: BTreeMap<(String, String), Tally> = BTreeMap::new();
    for c in &clips {
        let (actual, whisper) = clip_truth(engine, &truth_scorer, c, heard);
        let e = by_answer
            .entry((c.target.clone(), c.text.clone()))
            .or_insert(Tally {
                sums: [0.0; 3],
                n: 0,
                first_exp: c.exp.clone(),
                source: risks::Source::of_case(&c.exp, &c.case),
            });
        for (t, v) in e
            .sums
            .iter_mut()
            .zip([(actual + whisper) / 2.0, actual, whisper])
        {
            *t += v;
        }
        e.n += 1;
        if exp_order(&c.exp) < exp_order(&e.first_exp) {
            e.first_exp = c.exp.clone();
        }
    }
    let mut phrases: Vec<String> = by_answer.keys().map(|(t, _)| t.clone()).collect();
    phrases.dedup();
    let mut answers = Vec::new();
    for ((target, text), t) in by_answer {
        let Tally {
            sums,
            n,
            first_exp,
            source,
        } = t;
        let (_, words) = engine.read(&text);
        answers.push(Answer {
            phrase: phrases.binary_search(&target).unwrap(),
            target: engine.target_of(&target),
            text,
            words,
            truths: sums.map(|v| v / n as f32),
            first_exp,
            source,
            clips: n,
        });
    }
    let pairs = make_pairs(&answers, opts.truth);
    let all: Vec<&Pair> = pairs.iter().collect();
    // The risks every fit starts from: the profile's, or `--risks`.
    let base_risks = engine.risks;
    let hand = Params {
        weights: Weights::HAND,
        predict: Predict::HAND,
        risks: base_risks,
    };
    let base = match opts.prior {
        Prior::Hand => hand.clone(),
        Prior::Profile => Params {
            weights: engine.profile.weights.clone(),
            predict: engine.profile.predict,
            risks: base_risks,
        },
    };
    let truths: Vec<f32> = answers
        .iter()
        .map(|a| a.truths[opts.truth as usize])
        .collect();
    // Where each fit starts: `base`, with `--refit-risks` setting the risks from its training
    // answers.
    let start = |engine: &mut Engine, train: &dyn Fn(usize) -> bool| {
        let mut p = base.clone();
        if let Some(ro) = &opts.refit_risks {
            let items = risk_items(engine, &answers, &base, opts.truth, train);
            p.risks = risks::fit(&items, &base_risks, base.predict.risk_scale, ro).risks;
        }
        p
    };
    let label = match opts.prior {
        Prior::Hand => "hand-set",
        Prior::Profile => "profile",
    };
    let hand_pred = predictions(engine, &answers, &base);
    let (r, n) = ranking(&hand_pred, &all);
    println!(
        "{} clips → {} distinct answers to {} phrases; {} within-phrase pairs (|Δ| ≥ {MIN_PAIR}), {n} at |Δ| ≥ {RANK_MARGIN}",
        clips.len(),
        answers.len(),
        phrases.len(),
        pairs.len()
    );
    println!(
        "{label}: ranking {r}/{n} ({:.1}%), loss {:.4}, pearson over answers {:.3}",
        100.0 * r as f32 / n as f32,
        loss(&hand_pred, &all),
        pearson(&hand_pred, &truths)
    );

    let group = match opts.group {
        Grouping::Phrase => (0..phrases.len()).collect(),
        Grouping::Chunk => groups(
            phrases.len(),
            answers
                .iter()
                .flat_map(|a| chunk_keys(&a.text).into_iter().map(|k| (a.phrase, k))),
        ),
    };
    let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
    for &g in &group {
        *sizes.entry(g).or_default() += 1;
    }
    let shared = sizes.values().filter(|&&n| n > 1).count();
    let largest = sizes.values().max().copied().unwrap_or(0);
    let fold_maps: Vec<Vec<usize>> = (0..opts.repeats)
        .map(|r| match opts.group {
            // v6's assignment: every k-th phrase in a scrambled order.
            Grouping::Phrase => {
                let mut order: Vec<usize> = (0..phrases.len()).collect();
                order.sort_by_key(|&i| splitmix(splitmix(r as u64) ^ i as u64));
                let mut fold = vec![0; phrases.len()];
                for (pos, &i) in order.iter().enumerate() {
                    fold[i] = pos % opts.folds;
                }
                fold
            }
            Grouping::Chunk => assign_folds(&group, opts.folds, r),
        })
        .collect();

    // The λ with the best held-out ranking (the search's decision), ties going to lower loss.
    let mut best_lambda = (opts.lambdas[0], 0.0, f32::MAX);
    if opts.group == Grouping::Chunk {
        println!(
            "chunk groups: {} phrases in {} groups; {shared} groups join 2+ phrases, the largest {largest}",
            phrases.len(),
            sizes.len()
        );
    }
    let by = match opts.group {
        Grouping::Phrase => "phrase",
        Grouping::Chunk => "chunk group",
    };
    println!(
        "\ncross-validation, {} folds by {by} × {} repeats (held-out pairs only; {label}: {:.1}%, loss {:.4}):",
        opts.folds,
        opts.repeats,
        100.0 * r as f32 / n as f32,
        loss(&hand_pred, &all)
    );
    let total_w: f32 = pairs.iter().map(|p| p.dt.abs()).sum();
    for &lambda in &opts.lambdas {
        let (mut right, mut total, mut test_loss, mut evals) = (0, 0, 0.0, 0);
        let mut per_repeat = Vec::new();
        for fold_of in &fold_maps {
            let (mut rr, mut rt) = (0, 0);
            for k in 0..opts.folds {
                let train: Vec<&Pair> = pairs.iter().filter(|p| fold_of[p.phrase] != k).collect();
                let test: Vec<&Pair> = pairs.iter().filter(|p| fold_of[p.phrase] == k).collect();
                let s = start(engine, &|i| fold_of[answers[i].phrase] != k);
                let (fitted, e) = fit(engine, &answers, &train, &s, lambda, &free);
                evals += e;
                let pred = predictions(engine, &answers, &fitted);
                let (r, n) = ranking(&pred, &test);
                rr += r;
                rt += n;
                test_loss +=
                    loss(&pred, &test) * test.iter().map(|p| p.dt.abs()).sum::<f32>() / total_w;
            }
            per_repeat.push(100.0 * rr as f32 / rt.max(1) as f32);
            right += rr;
            total += rt;
        }
        test_loss /= opts.repeats as f32;
        let spread = per_repeat
            .iter()
            .map(|v| format!("{v:.1}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "  λ {lambda:<6} held-out ranking {:.1}% [{spread}], loss {test_loss:.4}  ({evals} evaluations)",
            100.0 * right as f32 / total.max(1) as f32
        );
        let held_out = right as f32 / total.max(1) as f32;
        if (held_out, -test_loss) > (best_lambda.1, -best_lambda.2) {
            best_lambda = (lambda, held_out, test_loss);
        }
    }
    let lambda = best_lambda.0;

    let last_exp = answers
        .iter()
        .map(|a| a.first_exp.as_str())
        .max_by_key(|e| exp_order(e))
        .unwrap_or_default()
        .to_string();
    let is_new = |i: usize| answers[i].first_exp == last_exp;
    let train: Vec<&Pair> = pairs
        .iter()
        .filter(|p| !is_new(p.a) && !is_new(p.b))
        .collect();
    let test: Vec<&Pair> = pairs
        .iter()
        .filter(|p| is_new(p.a) || is_new(p.b))
        .collect();
    let s = start(engine, &|i| !is_new(i));
    let (fitted, _) = fit(engine, &answers, &train, &s, lambda, &free);
    let pred = predictions(engine, &answers, &fitted);
    let (hr, hn) = ranking(&hand_pred, &test);
    let (fr, fn_) = ranking(&pred, &test);
    println!(
        "\nnew in {last_exp} (fit on the rest, λ {lambda}): {label} {hr}/{hn} ({:.1}%) loss {:.4}; fitted {fr}/{fn_} ({:.1}%) loss {:.4}",
        100.0 * hr as f32 / hn.max(1) as f32,
        loss(&hand_pred, &test),
        100.0 * fr as f32 / fn_.max(1) as f32,
        loss(&pred, &test)
    );

    let s = start(engine, &|_| true);
    let (fitted, _) = fit(engine, &answers, &all, &s, lambda, &free);
    let pred = predictions(engine, &answers, &fitted);
    let (r, n) = ranking(&pred, &all);
    println!(
        "\nall pairs (λ {lambda}, in-sample): ranking {r}/{n} ({:.1}%), loss {:.4}, pearson over answers {:.3}",
        100.0 * r as f32 / n as f32,
        loss(&pred, &all),
        pearson(&pred, &truths)
    );
    println!(
        "\nin-sample ranking of the same two weight sets under each truth (pairs at |Δ| ≥ {RANK_MARGIN}):"
    );
    for kind in [TruthKind::Mean, TruthKind::Phones, TruthKind::Whisper] {
        let kp = make_pairs(&answers, kind);
        let kp: Vec<&Pair> = kp.iter().collect();
        let (hr, hn) = ranking(&hand_pred, &kp);
        let (fr, fn_) = ranking(&pred, &kp);
        println!(
            "  {kind:?}: {label} {hr}/{hn} ({:.1}%), fitted {fr}/{fn_} ({:.1}%)",
            100.0 * hr as f32 / hn.max(1) as f32,
            100.0 * fr as f32 / fn_.max(1) as f32
        );
    }
    println!(
        "\n{:<16} {:>7} {:>7} {:>8}",
        "parameter", "hand", "fitted", "default"
    );
    let current = Params {
        weights: engine.profile.weights.clone(),
        predict: engine.profile.predict,
        risks: engine.profile.risks(),
    };
    for (i, name) in NAMES.iter().enumerate() {
        println!(
            "{name:<16} {:>7.3} {:>7.3} {:>8.3}",
            get(&hand, i),
            get(&fitted, i),
            get(&current, i)
        );
    }
    // Risks: only those that moved or differ from the profile.
    for i in NAMES.len()..PARAMS {
        let (h, f, c) = (get(&hand, i), get(&fitted, i), get(&current, i));
        if h != f || h != c {
            println!("{:<16} {h:>7.3} {f:>7.3} {c:>8.3}", param_name(i));
        }
    }
    let (r, n) = ranking(&predictions(engine, &answers, &current), &all);
    println!(
        "current defaults (in-sample): ranking {r}/{n} ({:.1}%)",
        100.0 * r as f32 / n as f32
    );
    engine.weights = engine.profile.weights.clone();
    engine.risks = base_risks;
    Ok(())
}

/// The answers `keep` selects as items for [`risks::fit`], predicted with `p`'s weights.
fn risk_items(
    engine: &mut Engine,
    answers: &[Answer],
    p: &Params,
    kind: TruthKind,
    keep: &dyn Fn(usize) -> bool,
) -> Vec<risks::Item> {
    engine.weights = p.weights.clone();
    engine.risks = p.risks;
    let no_risk = Predict {
        risk_scale: 0.0,
        ..p.predict
    };
    answers
        .iter()
        .enumerate()
        .filter(|&(i, _)| keep(i))
        .map(|(_, a)| {
            let share = match phono_core::words::spelling(&a.target.words, &a.words) {
                Some((_, share)) if p.predict.spelling > 0.0 => 1.0 - p.predict.spelling * share,
                _ => 1.0,
            };
            risks::Item {
                counts: risks::Item::counts(&a.text),
                share,
                excess: engine.predict_read(&a.text, &a.words, &a.target, &no_risk)
                    - a.truths[kind as usize],
                source: a.source,
                weight: a.clips as f32,
                cluster: a.phrase,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experiments_order_by_number() {
        assert!(exp_order("exp9") < exp_order("exp16"));
        assert!(exp_order("exp15") < exp_order("exp16"));
    }

    #[test]
    fn chunk_keys_skip_latin_and_spaces() {
        assert_eq!(
            chunk_keys("ab \u{13E3}\u{13B3}"),
            vec![('\u{13E3}', '\u{13B3}')]
        );
        assert_eq!(chunk_keys("\u{13E3} \u{13B3}"), vec![]);
        assert_eq!(chunk_keys("éa"), vec![]);
        assert_eq!(chunk_keys("ア이ウ"), vec![('ア', '이'), ('이', 'ウ')]);
    }

    #[test]
    fn groups_join_phrases_transitively() {
        // 0–1 share "x", 1–2 share "y", 3 stands alone.
        let g = groups(4, [(0, "x"), (1, "x"), (1, "y"), (2, "y"), (3, "z")]);
        assert_eq!(g, vec![0, 0, 0, 3]);
    }

    #[test]
    fn folds_keep_groups_together_and_balance() {
        let group = vec![0, 0, 0, 0, 4, 5, 6, 7];
        for repeat in 0..3 {
            let fold = assign_folds(&group, 2, repeat);
            assert!(fold[..4].iter().all(|&k| k == fold[0]));
            let in_first = fold.iter().filter(|&&k| k == fold[0]).count();
            assert_eq!(in_first, 4);
        }
    }
}
