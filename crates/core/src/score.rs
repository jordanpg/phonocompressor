//! How close a candidate pronunciation is to the target: a weighted alignment.
//!
//! Substitutions cost the Phoible feature distance (plus a stress mismatch term for vowels).
//! Deleting a target phone costs more when listeners rely on it: stressed vowels and
//! word-initial consonants are expensive; schwas and consonants inside clusters are cheap.
//! Fidelity = 1 − cost / (cost of deleting the whole target), floored at 0.

use crate::features::{FeatureSpace, PhoneDistance};
use crate::phone::Phone;

/// Base ids of AH, ER, IH: when unstressed these are reduced vowels (ə, ɚ, ɨ), the cheapest to lose.
const REDUCED_VOWELS: [u8; 3] = [2, 11, 16];

/// A target phone with its position in the phrase.
#[derive(Clone, Copy, Debug)]
pub struct TargetPhone {
    pub phone: Phone,
    pub word_start: bool,
    pub word_end: bool,
    /// Adjacent (within the word) to another consonant.
    pub in_cluster: bool,
}

pub fn flatten_target(words: &[Vec<Phone>]) -> Vec<TargetPhone> {
    let mut out = Vec::new();
    for w in words {
        for (i, &p) in w.iter().enumerate() {
            let cons = |j: Option<usize>| j.and_then(|j| w.get(j)).is_some_and(|q| !q.is_vowel());
            out.push(TargetPhone {
                phone: p,
                word_start: i == 0,
                word_end: i + 1 == w.len(),
                in_cluster: !p.is_vowel() && (cons(i.checked_sub(1)) || cons(Some(i + 1))),
            });
        }
    }
    out
}

#[derive(Clone, Debug)]
pub struct Weights {
    pub del_primary: f32,
    pub del_secondary: f32,
    pub del_unstressed: f32,
    pub del_schwa: f32,
    pub del_onset: f32,
    pub del_cluster: f32,
    pub del_consonant: f32,
    pub ins_stressed: f32,
    pub ins_unstressed: f32,
    pub ins_consonant: f32,
    pub sub_scale: f32,
    /// Vowel-for-vowel substitutions: listeners track vowel identity closely ("time" ≠ "tem").
    pub vowel_sub_scale: f32,
    pub stress_mismatch: f32,
    /// Extra cost when a vowel stands in for a consonant or vice versa. Feature distance alone
    /// (~0.45 for K vs AA) would make that cheaper than deleting one and inserting the other.
    pub class_mismatch: f32,
}

impl Weights {
    /// The hand-set weights of v1–v5. `phono calibrate` measures truth (what the TTS actually
    /// said) with these, so its numbers stay comparable across scorer versions. Each model's
    /// fitted weights are in its profile ([`crate::model::ModelProfile::weights`]).
    pub const HAND: Weights = Weights {
        del_primary: 1.0,
        del_secondary: 0.8,
        del_unstressed: 0.45,
        del_schwa: 0.35,
        del_onset: 0.9,
        del_cluster: 0.5,
        del_consonant: 0.7,
        ins_stressed: 0.7,
        ins_unstressed: 0.35,
        ins_consonant: 0.6,
        sub_scale: 2.0,
        vowel_sub_scale: 3.0,
        stress_mismatch: 0.2,
        class_mismatch: 0.5,
    };
}

/// One step of the alignment, for display.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Match(Phone, Phone, f32),
    Delete(Phone, f32),
    Insert(Phone, f32),
}

#[derive(Clone, Debug)]
pub struct Alignment {
    pub cost: f32,
    pub max_cost: f32,
    pub fidelity: f32,
    pub ops: Vec<Op>,
}

/// Leniency for comparing recognizer output with targets (see [`Scorer::fidelity_segments`]).
#[derive(Clone, Copy, Debug)]
pub struct Heard {
    pub vowel_scale: f32,
    pub rhotic: f32,
}

impl Default for Heard {
    fn default() -> Self {
        Heard { vowel_scale: 2.0, rhotic: 0.0 }
    }
}

pub struct Scorer<'a> {
    pub dist: &'a PhoneDistance,
    pub w: Weights,
}

impl<'a> Scorer<'a> {
    /// A scorer with the hand-set weights ([`Weights::HAND`]).
    pub fn new(dist: &'a PhoneDistance) -> Self {
        Scorer { dist, w: Weights::HAND }
    }

    pub fn with_weights(dist: &'a PhoneDistance, w: Weights) -> Self {
        Scorer { dist, w }
    }

    pub fn del_cost(&self, t: &TargetPhone) -> f32 {
        let w = &self.w;
        match t.phone.stress() {
            Some(1) => w.del_primary,
            Some(2) => w.del_secondary,
            Some(_) if REDUCED_VOWELS.contains(&t.phone.base()) => w.del_schwa,
            Some(_) => w.del_unstressed,
            None if t.word_start => w.del_onset,
            None if t.in_cluster => w.del_cluster,
            None => w.del_consonant,
        }
    }

    pub fn ins_cost(&self, c: Phone) -> f32 {
        match c.stress() {
            Some(0) => self.w.ins_unstressed,
            Some(_) => self.w.ins_stressed,
            None => self.w.ins_consonant,
        }
    }

    pub fn sub_cost(&self, t: Phone, c: Phone) -> f32 {
        let scale = if t.is_vowel() && c.is_vowel() { self.w.vowel_sub_scale } else { self.w.sub_scale };
        let mut cost = self.dist.get(t, c) * scale;
        if t.is_vowel() != c.is_vowel() {
            cost += self.w.class_mismatch;
        } else if t.is_vowel() && t.is_stressed() != c.is_stressed() {
            cost += self.w.stress_mismatch;
        }
        cost
    }

    /// Cost of deleting the whole target (the denominator of fidelity).
    pub fn max_cost(&self, target: &[TargetPhone]) -> f32 {
        target.iter().map(|t| self.del_cost(t)).sum()
    }

    /// Fidelity of *recognized* phones (Phoible segment ids from a phoneme recognizer run on TTS
    /// audio) against the target, with the same costs as [`Scorer::align`] except for vowels.
    /// The recognizer's vowel labels follow its own conventions (English ʌ comes out `a`,
    /// schwas often `ɚ`), so vowel-for-vowel differences are scaled by `heard.vowel_scale` and
    /// rhoticity by `heard.rhotic`. Recognized phones carry no stress: vowels are inserted at
    /// the mean of the stressed/unstressed costs and no stress mismatch is charged.
    pub fn fidelity_segments(&self, space: &FeatureSpace, target: &[TargetPhone], cand: &[usize], heard: &Heard) -> f32 {
        let ins = |c: usize| {
            if space.is_vowel(c) { (self.w.ins_stressed + self.w.ins_unstressed) / 2.0 } else { self.w.ins_consonant }
        };
        let sub = |t: Phone, c: usize| {
            let ts = self.dist.segment(t);
            let (tv, cv) = (t.is_vowel(), space.is_vowel(c));
            let scale = if tv && cv { heard.vowel_scale } else { self.w.sub_scale };
            space.segment_distance_with(ts, c, heard.rhotic) * scale + if tv != cv { self.w.class_mismatch } else { 0.0 }
        };
        let m = cand.len();
        let mut prev: Vec<f32> = (0..=m).map(|j| cand[..j].iter().map(|&c| ins(c)).sum()).collect();
        let mut cur = vec![0.0; m + 1];
        for t in target {
            cur[0] = prev[0] + self.del_cost(t);
            for j in 1..=m {
                cur[j] = (prev[j - 1] + sub(t.phone, cand[j - 1]))
                    .min(prev[j] + self.del_cost(t))
                    .min(cur[j - 1] + ins(cand[j - 1]));
            }
            std::mem::swap(&mut prev, &mut cur);
        }
        let max = self.max_cost(target);
        if max > 0.0 { (1.0 - prev[m] / max).max(0.0) } else { 0.0 }
    }

    /// Alignment cost only (no traceback), for the search's inner loop.
    pub fn cost(&self, target: &[TargetPhone], cand: &[Phone]) -> f32 {
        let m = cand.len();
        let mut prev: Vec<f32> = (0..=m).map(|j| cand[..j].iter().map(|&c| self.ins_cost(c)).sum()).collect();
        let mut cur = vec![0.0; m + 1];
        for t in target {
            cur[0] = prev[0] + self.del_cost(t);
            for j in 1..=m {
                cur[j] = (prev[j - 1] + self.sub_cost(t.phone, cand[j - 1]))
                    .min(prev[j] + self.del_cost(t))
                    .min(cur[j - 1] + self.ins_cost(cand[j - 1]));
            }
            std::mem::swap(&mut prev, &mut cur);
        }
        prev[m]
    }

    /// Full alignment with traceback.
    pub fn align(&self, target: &[TargetPhone], cand: &[Phone]) -> Alignment {
        let (n, m) = (target.len(), cand.len());
        let mut d = vec![vec![0f32; m + 1]; n + 1];
        for j in 1..=m {
            d[0][j] = d[0][j - 1] + self.ins_cost(cand[j - 1]);
        }
        for i in 1..=n {
            d[i][0] = d[i - 1][0] + self.del_cost(&target[i - 1]);
            for j in 1..=m {
                d[i][j] = (d[i - 1][j - 1] + self.sub_cost(target[i - 1].phone, cand[j - 1]))
                    .min(d[i - 1][j] + self.del_cost(&target[i - 1]))
                    .min(d[i][j - 1] + self.ins_cost(cand[j - 1]));
            }
        }
        let mut ops = Vec::new();
        let (mut i, mut j) = (n, m);
        while i > 0 || j > 0 {
            if i > 0 && j > 0 {
                let s = self.sub_cost(target[i - 1].phone, cand[j - 1]);
                if (d[i][j] - (d[i - 1][j - 1] + s)).abs() < 1e-5 {
                    ops.push(Op::Match(target[i - 1].phone, cand[j - 1], s));
                    i -= 1;
                    j -= 1;
                    continue;
                }
            }
            if i > 0 && (j == 0 || (d[i][j] - (d[i - 1][j] + self.del_cost(&target[i - 1]))).abs() < 1e-5) {
                ops.push(Op::Delete(target[i - 1].phone, self.del_cost(&target[i - 1])));
                i -= 1;
            } else {
                ops.push(Op::Insert(cand[j - 1], self.ins_cost(cand[j - 1])));
                j -= 1;
            }
        }
        ops.reverse();
        let cost = d[n][m];
        let max_cost = self.max_cost(target);
        let fidelity = if max_cost > 0.0 { (1.0 - cost / max_cost).max(0.0) } else { 0.0 };
        Alignment { cost, max_cost, fidelity, ops }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{FeatureSpace, FeatureTable};
    use crate::phone::parse_pron;

    fn dist() -> PhoneDistance {
        let tsv = include_str!("../../../phoible/raw-data/FEATURES/phoible-segments-features.tsv");
        PhoneDistance::new(&FeatureSpace::new(FeatureTable::parse_tsv(tsv).unwrap())).unwrap()
    }

    fn target(s: &str) -> Vec<TargetPhone> {
        let words: Vec<Vec<Phone>> = s.split('|').map(|w| parse_pron(w).unwrap()).collect();
        flatten_target(&words)
    }

    #[test]
    fn english_phone_distances_are_sensible() {
        let d = dist();
        let p = |s| Phone::parse(s).unwrap();
        assert!(d.get(p("P"), p("B")) < d.get(p("P"), p("K")), "voicing < place");
        assert!(d.get(p("TH"), p("F")) < d.get(p("TH"), p("M")));
        assert!(d.get(p("IY1"), p("IH1")) < d.get(p("IY1"), p("AA1")));
        assert!(d.get(p("K"), p("AA1")) > 0.4, "consonant vs vowel is far");
        assert!(d.get(p("W"), p("UW1")) < d.get(p("W"), p("V")), "glide is near its vowel");
        let s = Scorer::new(&d);
        let k = TargetPhone { phone: p("K"), word_start: false, word_end: false, in_cluster: false };
        assert!(s.sub_cost(p("K"), p("AA1")) > s.del_cost(&k), "vowel-for-consonant beats deleting");
    }

    #[test]
    fn fidelity_orders_candidates() {
        let d = dist();
        let s = Scorer::new(&d);
        let t = target("B AE1 D|D R IY1 M");
        let exact = s.align(&t, &parse_pron("B AE1 D D R IY1 M").unwrap());
        let close = s.align(&t, &parse_pron("B AE1 D R IY1 M").unwrap()); // one D lost in a cluster
        let worse = s.align(&t, &parse_pron("B AH0 D R AH0 M").unwrap()); // stressed vowels reduced
        assert!((exact.fidelity - 1.0).abs() < 1e-6);
        assert!(close.fidelity > worse.fidelity, "{} vs {}", close.fidelity, worse.fidelity);
        assert!((s.cost(&t, &parse_pron("B AE1 D R IY1 M").unwrap()) - close.cost).abs() < 1e-5);
    }

    #[test]
    fn deletion_costs_follow_position_and_stress() {
        let d = dist();
        let s = Scorer::new(&d);
        let t = target("S T R IY1 M AH0");
        let costs: Vec<f32> = t.iter().map(|x| s.del_cost(x)).collect();
        assert_eq!(costs[0], s.w.del_onset);
        assert_eq!(costs[1], s.w.del_cluster);
        assert_eq!(costs[3], s.w.del_primary);
        assert_eq!(costs[5], s.w.del_schwa);
    }
}

#[cfg(test)]
mod probe {
    /// `cargo test -p phono-core print_distances -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn print_distances() {
        use crate::features::*;
        use crate::phone::Phone;
        let tsv = include_str!("../../../phoible/raw-data/FEATURES/phoible-segments-features.tsv");
        let d = PhoneDistance::new(&FeatureSpace::new(FeatureTable::parse_tsv(tsv).unwrap())).unwrap();
        let p = |s| Phone::parse(s).unwrap();
        for (a, b) in [
            ("P", "B"), ("P", "K"), ("P", "T"), ("TH", "F"), ("TH", "S"), ("TH", "M"), ("DH", "D"), ("DH", "Z"),
            ("IY1", "IH1"), ("IY1", "AA1"), ("K", "AA1"), ("S", "AA1"), ("S", "Z"), ("AE1", "EH1"), ("R", "L"),
            ("W", "V"), ("W", "UW1"), ("Y", "IY1"), ("AY1", "EH1"), ("UW1", "OW1"), ("ER1", "AH1"), ("AE1", "AA1"),
        ] {
            println!("{a:>4} {b:>4} {:.3}", d.get(p(a), p(b)));
        }
    }
}
