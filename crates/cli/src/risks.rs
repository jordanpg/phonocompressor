//! Per-script risks estimated from harness clips (`phono calibrate --fit-risks`, and per fold in
//! `phono fit --refit-risks`).
//!
//! For each item (a clip, or an answer averaged over its clips), a weighted least-squares fit of
//!
//! ```text
//! predicted without risk − truth = b[source] + Σ_s k_s · share · n_s
//! ```
//!
//! - `n_s` is the item's codepoints in script `s`.
//! - `share` is the part of the prediction the sound score carries (1 − the spelling blend
//!   weight), since the risk is subtracted from the sound score only.
//! - `b[source]` is the over-prediction that has nothing to do with scripts, per source. Our
//!   answers are the top of many predictions, so they are over-predicted more than community
//!   entries (exp16: +0.08 to +0.10 against +0.03 in Latin, Hangul and Yi), and charging that to
//!   the scripts the search likes would be wrong.
//! - `k_s` is the penalty per codepoint the clips ask for; the risk is `k_s / risk_scale`.
//!
//! Scripts with too few clips, and the reference scripts (Latin, Hangul and Yi by default), keep
//! their current risk. A small ridge pulls each fitted script towards its current risk.

use phono_core::chunks::Script;

/// Where an item's text came from: each has its own baseline over-prediction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// A community entry (exp1, and the library side of exp15–exp16).
    Community,
    /// One of our answers (exp2–exp8, the `_ours-…` side of exp15–exp16).
    Ours,
    /// Anything else: probes such as exp9 and exp10.
    Probe,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Community, Source::Ours, Source::Probe];

    pub fn name(self) -> &'static str {
        match self {
            Source::Community => "community",
            Source::Ours => "ours",
            Source::Probe => "probe",
        }
    }

    /// The source of a harness case. Community entries have ids `msg<key>`; read from several
    /// files, our answers get the file's stem as a suffix (`msgu1a2b3c4d_ours-exp16`). exp2–exp8
    /// read only our answers, under plain `msg<key>` ids. Anything else is a probe.
    pub fn of_case(exp: &str, case: &str) -> Source {
        let n: Option<u32> = exp.strip_prefix("exp").and_then(|n| n.parse().ok());
        if case.contains("_ours-") || matches!(n, Some(2..=8)) {
            Source::Ours
        } else if case.starts_with("msg") && !case.contains('_') {
            Source::Community
        } else {
            Source::Probe
        }
    }
}

pub struct Item {
    /// Codepoints per script, indexed by [`Script::index`].
    pub counts: [f32; Script::COUNT],
    /// Share of the prediction the sound score carries (the risk's reach).
    pub share: f32,
    /// Predicted fidelity with no script risk, minus truth.
    pub excess: f32,
    pub source: Source,
    /// Clips behind the item.
    pub weight: f32,
    /// Resampling unit for intervals (the phrase).
    pub cluster: usize,
}

impl Item {
    pub fn counts(text: &str) -> [f32; Script::COUNT] {
        let mut out = [0.0; Script::COUNT];
        for c in text.chars() {
            out[Script::of(c).index()] += 1.0;
        }
        out
    }
}

pub struct Opts {
    /// Scripts held at their current risk whatever the clips say.
    pub hold: Vec<Script>,
    /// Fit a script only if at least this many clips use it.
    pub min_clips: f32,
    /// Ridge strength towards the current risk, in clips of one codepoint.
    pub ridge: f32,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            hold: vec![Script::Latin, Script::Hangul, Script::Yi],
            min_clips: 10.0,
            ridge: 2.0,
        }
    }
}

pub struct Fit {
    /// The new risks: fitted scripts clamped at 0, the rest as they were.
    pub risks: [f32; Script::COUNT],
    /// Fitted per-codepoint risk before clamping (only meaningful where `free`).
    pub raw: [f32; Script::COUNT],
    pub free: [bool; Script::COUNT],
    /// Baseline over-prediction per [`Source::ALL`] entry, `None` where there are no items.
    pub baseline: [Option<f32>; 3],
}

/// Weighted least squares of `items` (see the module docs).
pub fn fit(items: &[Item], current: &[f32; Script::COUNT], risk_scale: f32, opts: &Opts) -> Fit {
    let mut clips = [0f32; Script::COUNT];
    for it in items {
        for (c, &n) in clips.iter_mut().zip(&it.counts) {
            if n > 0.0 {
                *c += it.weight;
            }
        }
    }
    let free: [bool; Script::COUNT] = std::array::from_fn(|i| {
        clips[i] >= opts.min_clips && !opts.hold.contains(&Script::ALL[i]) && risk_scale > 0.0
    });
    let sources: Vec<Source> = Source::ALL
        .into_iter()
        .filter(|s| items.iter().any(|it| it.source == *s))
        .collect();
    let scripts: Vec<usize> = (0..Script::COUNT).filter(|&i| free[i]).collect();
    let dim = sources.len() + scripts.len();
    // Normal equations (XᵀWX + R) θ = XᵀWy + R θ₀, in f64.
    let mut a = vec![vec![0f64; dim]; dim];
    let mut b = vec![0f64; dim];
    for it in items {
        let mut x = vec![0f64; dim];
        x[sources.iter().position(|&s| s == it.source).unwrap()] = 1.0;
        for (j, &i) in scripts.iter().enumerate() {
            x[sources.len() + j] = (it.share * it.counts[i]) as f64;
        }
        // Held scripts' penalties are known: move them to the left-hand side.
        let held: f32 = (0..Script::COUNT)
            .filter(|&i| !free[i])
            .map(|i| risk_scale * current[i] * it.share * it.counts[i])
            .sum();
        let y = (it.excess - held) as f64;
        let w = it.weight as f64;
        for r in 0..dim {
            b[r] += w * x[r] * y;
            for c in 0..dim {
                a[r][c] += w * x[r] * x[c];
            }
        }
    }
    for (j, &i) in scripts.iter().enumerate() {
        let r = sources.len() + j;
        a[r][r] += opts.ridge as f64;
        b[r] += (opts.ridge * risk_scale * current[i]) as f64;
    }
    let theta = solve(a, b);
    let mut baseline = [None; 3];
    for (k, s) in sources.iter().enumerate() {
        baseline[Source::ALL.iter().position(|x| x == s).unwrap()] = Some(theta[k] as f32);
    }
    let mut raw = *current;
    let mut risks = *current;
    for (j, &i) in scripts.iter().enumerate() {
        raw[i] = theta[sources.len() + j] as f32 / risk_scale;
        risks[i] = raw[i].max(0.0);
    }
    Fit {
        risks,
        raw,
        free,
        baseline,
    }
}

/// Gaussian elimination with partial pivoting; a singular column solves to 0.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for col in 0..n {
        let p = (col..n)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap();
        if a[p][col].abs() < 1e-12 {
            continue;
        }
        a.swap(col, p);
        b.swap(col, p);
        for r in 0..n {
            if r != col {
                let f = a[r][col] / a[col][col];
                if f != 0.0 {
                    for c in col..n {
                        a[r][c] -= f * a[col][c];
                    }
                    b[r] -= f * b[col];
                }
            }
        }
    }
    (0..n)
        .map(|i| {
            if a[i][i].abs() < 1e-12 {
                0.0
            } else {
                b[i] / a[i][i]
            }
        })
        .collect()
}

/// Raw per-codepoint risks refitted on `reps` resamples of the clusters (with replacement), for
/// intervals.
pub fn bootstrap(
    items: &[Item],
    current: &[f32; Script::COUNT],
    risk_scale: f32,
    opts: &Opts,
    reps: usize,
) -> Vec<[f32; Script::COUNT]> {
    let clusters = items.iter().map(|it| it.cluster).max().map_or(0, |m| m + 1);
    let mut members = vec![Vec::new(); clusters];
    for (i, it) in items.iter().enumerate() {
        members[it.cluster].push(i);
    }
    let mut rng = 0x5EED_u64;
    (0..reps)
        .map(|_| {
            let mut sample = Vec::with_capacity(items.len());
            for _ in 0..clusters {
                rng = crate::fit::splitmix(rng);
                for &i in &members[(rng % clusters as u64) as usize] {
                    let it = &items[i];
                    sample.push(Item {
                        counts: it.counts,
                        share: it.share,
                        excess: it.excess,
                        source: it.source,
                        weight: it.weight,
                        cluster: it.cluster,
                    });
                }
            }
            fit(&sample, current, risk_scale, opts).raw
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(text: &str, excess: f32, source: Source, cluster: usize) -> Item {
        Item {
            counts: Item::counts(text),
            share: 1.0,
            excess,
            source,
            weight: 1.0,
            cluster,
        }
    }

    #[test]
    fn recovers_per_codepoint_risks_and_baselines() {
        // Truth: community baseline +0.03, ours +0.09, Katakana 0.01 and Ethiopic 0.08 per
        // codepoint at risk_scale 1 (Latin held at 0).
        let mut items = Vec::new();
        for (i, (text, kata, eth)) in [
            ("ab", 0.0, 0.0),
            ("アイ", 2.0, 0.0),
            ("アイウ", 3.0, 0.0),
            ("ሀ", 0.0, 1.0),
            ("ሀለa", 0.0, 2.0),
            ("アሀ", 1.0, 1.0),
        ]
        .into_iter()
        .enumerate()
        {
            for (source, b) in [(Source::Community, 0.03), (Source::Ours, 0.09)] {
                for _ in 0..5 {
                    items.push(item(text, b + 0.01 * kata + 0.08 * eth, source, i));
                }
            }
        }
        let current = [0.05; Script::COUNT];
        let opts = Opts {
            hold: vec![Script::Latin],
            ridge: 0.0,
            ..Opts::default()
        };
        let mut current_latin_zero = current;
        current_latin_zero[Script::Latin.index()] = 0.0;
        let f = fit(&items, &current_latin_zero, 1.0, &opts);
        assert!((f.risks[Script::Katakana.index()] - 0.01).abs() < 1e-4);
        assert!((f.risks[Script::Ethiopic.index()] - 0.08).abs() < 1e-4);
        assert!((f.baseline[0].unwrap() - 0.03).abs() < 1e-4);
        assert!((f.baseline[1].unwrap() - 0.09).abs() < 1e-4);
        assert_eq!(f.baseline[2], None);
        // Scripts without clips keep their risk, and so do held ones.
        assert!(!f.free[Script::Cherokee.index()]);
        assert_eq!(f.risks[Script::Cherokee.index()], 0.05);
        assert_eq!(f.risks[Script::Latin.index()], 0.0);
    }

    #[test]
    fn risk_scale_and_share_divide_out() {
        // Half the prediction is spelling: a 0.04 excess per codepoint at share 0.5 and
        // risk_scale 2 is a risk of 0.04.
        let mut items = Vec::new();
        for k in 0..12 {
            let n = (k % 3 + 1) as f32;
            let text: String = "ア".repeat(n as usize);
            let mut it = item(&text, 0.02 + 0.5 * 2.0 * 0.04 * n, Source::Community, k);
            it.share = 0.5;
            items.push(it);
            items.push(item("ab", 0.02, Source::Community, k));
        }
        let no_ridge = Opts {
            ridge: 0.0,
            ..Opts::default()
        };
        let f = fit(&items, &[0.0; Script::COUNT], 2.0, &no_ridge);
        assert!((f.risks[Script::Katakana.index()] - 0.04).abs() < 1e-4);
    }

    #[test]
    fn negative_estimates_clamp_to_zero_and_ridge_pulls_to_current() {
        let items: Vec<Item> = (0..10)
            .flat_map(|k| {
                [
                    item("アア", -0.1, Source::Community, k),
                    item("ab", 0.0, Source::Community, k),
                ]
            })
            .collect();
        let f = fit(&items, &[0.0; Script::COUNT], 1.0, &Opts::default());
        assert!(f.raw[Script::Katakana.index()] < 0.0);
        assert_eq!(f.risks[Script::Katakana.index()], 0.0);
        let mut current = [0.0; Script::COUNT];
        current[Script::Katakana.index()] = 0.05;
        let strong = Opts {
            ridge: 1e6,
            ..Opts::default()
        };
        let f = fit(&items, &current, 1.0, &strong);
        assert!((f.risks[Script::Katakana.index()] - 0.05).abs() < 1e-3);
    }

    #[test]
    fn sources_of_harness_cases() {
        assert_eq!(Source::of_case("exp1", "msg8"), Source::Community);
        assert_eq!(Source::of_case("exp6", "msg8"), Source::Ours);
        assert_eq!(
            Source::of_case("exp16", "msgu1a2b3c4d_ours-exp16"),
            Source::Ours
        );
        assert_eq!(Source::of_case("exp16", "msgu1a2b3c4d"), Source::Community);
        assert_eq!(Source::of_case("exp15", "thousand_sign"), Source::Probe);
        assert_eq!(Source::of_case("exp9", "doeg_E"), Source::Probe);
    }
}
