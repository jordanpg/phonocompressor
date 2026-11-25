//! `phono fit`: tune the scorer's hand-set weights on recorded harness clips.
//!
//! The search's real decision is which of several answers for the same phrase to return, so
//! the objective is pairwise: for two recorded answers to one phrase, the one the TTS rendered
//! better (truth, as in `phono calibrate`) should be predicted higher. The loss is a logistic
//! (RankNet) loss on the predicted difference, weighted by the truth difference, plus an L2
//! pull towards the hand-set weights in log space.
//!
//! Generalization is checked two ways:
//! - k-fold cross-validation by phrase: the held-out phrases' answers are never seen in fitting;
//! - "new in the last experiment": fit without the answers that first appeared in the last
//!   experiment, test on pairs involving them (the situation of a new harness run).

use crate::{Clip, RANK_MARGIN, clip_truth, load_clips};
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

/// Log-space coordinates of `p`.
fn coords(p: &Params) -> Vec<f32> {
    (0..NAMES.len()).map(|i| get(p, i).max(1e-4).ln()).collect()
}

fn from_coords(base: &Params, x: &[f32]) -> Params {
    let mut p = base.clone();
    for (i, &v) in x.iter().enumerate() {
        let mut v = v.exp();
        if NAMES[i] == "spelling" {
            v = v.min(1.0); // a blend weight
        }
        *field(&mut p, i) = v;
    }
    p
}

fn predictions(engine: &mut Engine, answers: &[Answer], p: &Params) -> Vec<f32> {
    engine.weights = p.weights.clone();
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

fn splitmix(mut x: u64) -> u64 {
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
    /// Cross-validation runs, each with a different assignment of phrases to folds.
    pub repeats: usize,
    pub lambdas: Vec<f32>,
    pub truth: TruthKind,
    /// Parameters held at their hand-set values.
    pub fix: Vec<String>,
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
    for name in &opts.fix {
        if !NAMES.contains(&name.as_str()) {
            return Err(format!(
                "--fix: no parameter {name:?} (have {})",
                NAMES.join(", ")
            ));
        }
    }
    let free: Vec<bool> = NAMES
        .iter()
        .map(|n| !opts.fix.iter().any(|f| f == n))
        .collect();
    let mut by_answer: BTreeMap<(String, String), ([f32; 3], usize, String)> = BTreeMap::new();
    for c in &clips {
        let (actual, whisper) = clip_truth(engine, &truth_scorer, c, heard);
        let e = by_answer
            .entry((c.target.clone(), c.text.clone()))
            .or_insert(([0.0; 3], 0, c.exp.clone()));
        for (t, v) in
            e.0.iter_mut()
                .zip([(actual + whisper) / 2.0, actual, whisper])
        {
            *t += v;
        }
        e.1 += 1;
        if c.exp < e.2 {
            e.2 = c.exp.clone();
        }
    }
    let mut phrases: Vec<String> = by_answer.keys().map(|(t, _)| t.clone()).collect();
    phrases.dedup();
    let mut answers = Vec::new();
    for ((target, text), (sums, n, first_exp)) in by_answer {
        let (_, words) = engine.read(&text);
        answers.push(Answer {
            phrase: phrases.binary_search(&target).unwrap(),
            target: engine.target_of(&target),
            text,
            words,
            truths: sums.map(|v| v / n as f32),
            first_exp,
        });
    }
    let pairs = make_pairs(&answers, opts.truth);
    let all: Vec<&Pair> = pairs.iter().collect();
    let hand = Params {
        weights: Weights::HAND,
        predict: Predict::HAND,
    };
    let truths: Vec<f32> = answers
        .iter()
        .map(|a| a.truths[opts.truth as usize])
        .collect();
    let hand_pred = predictions(engine, &answers, &hand);
    let (r, n) = ranking(&hand_pred, &all);
    println!(
        "{} clips → {} distinct answers to {} phrases; {} within-phrase pairs (|Δ| ≥ {MIN_PAIR}), {n} at |Δ| ≥ {RANK_MARGIN}",
        clips.len(),
        answers.len(),
        phrases.len(),
        pairs.len()
    );
    println!(
        "hand-set: ranking {r}/{n} ({:.1}%), loss {:.4}, pearson over answers {:.3}",
        100.0 * r as f32 / n as f32,
        loss(&hand_pred, &all),
        pearson(&hand_pred, &truths)
    );

    // Folds by phrase: every k-th phrase in a scrambled order, a different scramble per repeat.
    let fold_maps: Vec<Vec<usize>> = (0..opts.repeats)
        .map(|r| {
            let mut order: Vec<usize> = (0..phrases.len()).collect();
            order.sort_by_key(|&i| splitmix(splitmix(r as u64) ^ i as u64));
            let mut fold = vec![0; phrases.len()];
            for (pos, &i) in order.iter().enumerate() {
                fold[i] = pos % opts.folds;
            }
            fold
        })
        .collect();

    let mut best_lambda = (opts.lambdas[0], f32::MAX);
    println!(
        "\ncross-validation, {} folds by phrase × {} repeats (held-out pairs only; hand-set: {:.1}%, loss {:.4}):",
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
                let (fitted, e) = fit(engine, &answers, &train, &hand, lambda, &free);
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
        if test_loss < best_lambda.1 {
            best_lambda = (lambda, test_loss);
        }
    }
    let lambda = best_lambda.0;

    let last_exp = answers
        .iter()
        .map(|a| a.first_exp.clone())
        .max()
        .unwrap_or_default();
    let is_new = |i: usize| answers[i].first_exp == last_exp;
    let train: Vec<&Pair> = pairs
        .iter()
        .filter(|p| !is_new(p.a) && !is_new(p.b))
        .collect();
    let test: Vec<&Pair> = pairs
        .iter()
        .filter(|p| is_new(p.a) || is_new(p.b))
        .collect();
    let (fitted, _) = fit(engine, &answers, &train, &hand, lambda, &free);
    let pred = predictions(engine, &answers, &fitted);
    let (hr, hn) = ranking(&hand_pred, &test);
    let (fr, fn_) = ranking(&pred, &test);
    println!(
        "\nnew in {last_exp} (fit on the rest, λ {lambda}): hand-set {hr}/{hn} ({:.1}%) loss {:.4}; fitted {fr}/{fn_} ({:.1}%) loss {:.4}",
        100.0 * hr as f32 / hn.max(1) as f32,
        loss(&hand_pred, &test),
        100.0 * fr as f32 / fn_.max(1) as f32,
        loss(&pred, &test)
    );

    let (fitted, _) = fit(engine, &answers, &all, &hand, lambda, &free);
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
            "  {kind:?}: hand-set {hr}/{hn} ({:.1}%), fitted {fr}/{fn_} ({:.1}%)",
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
    };
    for (i, name) in NAMES.iter().enumerate() {
        println!(
            "{name:<16} {:>7.3} {:>7.3} {:>8.3}",
            get(&hand, i),
            get(&fitted, i),
            get(&current, i)
        );
    }
    let (r, n) = ranking(&predictions(engine, &answers, &current), &all);
    println!(
        "current defaults (in-sample): ranking {r}/{n} ({:.1}%)",
        100.0 * r as f32 / n as f32
    );
    engine.weights = engine.profile.weights.clone();
    Ok(())
}
