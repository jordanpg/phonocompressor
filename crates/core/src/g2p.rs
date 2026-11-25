//! English letter-to-sound for arbitrary (pseudo-)words: a joint-sequence "graphone" model.
//!
//! A graphone unit pairs 1–2 letters with 0–2 phones (`ph`→F, `x`→K S, `e`→∅). Words are
//! aligned into unit sequences offline (see the builder), and an n-gram over units scores
//! segmentations (Bisani & Ney 2008). The n-gram is smoothed with interpolated modified
//! Kneser-Ney and its order is tuned on held-out words, following the pair n-gram recipe of
//! Novak et al. (Phonetisaurus) as used by wikipron-modeling. Order 5 cut word error on
//! wikipron-modeling's CMUdict split from 34.0% (our old Witten-Bell trigram) to about 26.5%.
//!
//! Probabilities are precomputed in backoff form: every seen n-gram stores its interpolated
//! probability and every seen context its backoff weight γ, so a lookup usually stops at the
//! longest matching context.
//!
//! The same units read backwards give phone→spelling options ([`G2p::spellings`]), which
//! the search uses to propose spellings for target phones.

use crate::phone::Phone;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Unit {
    pub letters: String,
    pub phones: Vec<Phone>,
}

/// Unit id 0 is the word-boundary sentinel.
pub const BOUNDARY: u32 = 0;

/// Default n-gram order (best accuracy for its size and speed on the CMUdict benchmark).
pub const DEFAULT_ORDER: usize = 5;

/// Decoder beam: states kept per letter position. Order-5 histories make many distinct
/// states, so this bounds the cost of a word.
const BEAM: usize = 48;

/// Memoized pronunciations. A search re-reads the same pseudo-words across hundreds of
/// candidates (one search: 1,430 calls, 372 distinct words); cleared when it grows past this.
const CACHE_MAX: usize = 100_000;

type Pronunciations = Vec<(Vec<Phone>, f64)>;

#[derive(Serialize, Deserialize, Default)]
pub struct G2pModel {
    pub units: Vec<Unit>,
    /// Raw unit counts (phone→spelling costs).
    pub uni: Vec<u32>,
    /// N-gram order (context of `order - 1` units).
    pub order: u32,
    /// ln P(w) for every unit (lowest order, interpolated with uniform).
    pub unigram: Vec<f32>,
    /// `grams[k - 1]`: n-grams with k context units, as (packed key, ln P(w | h)), sorted.
    pub grams: Vec<Vec<(u64, f32)>>,
    /// `backoff[k - 1]`: contexts of k units, as (packed h, ln γ(h)), sorted.
    pub backoff: Vec<Vec<(u64, f32)>>,
}

/// Bits per unit id in packed keys. Ids are packed oldest first, so the newest unit sits in
/// the low bits and the last k units of a history are `hist & mask(k)`.
fn id_bits(num_units: usize) -> u32 {
    (usize::BITS - num_units.saturating_sub(1).leading_zeros()).max(1)
}

/// Fast hasher for packed u64 keys (Fx-style multiply).
#[derive(Default)]
struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

type KeyMap<V> = HashMap<u64, V, BuildHasherDefault<KeyHasher>>;

/// Modified Kneser-Ney discounts (Chen & Goodman 1998) for counts 1, 2 and ≥3, from the
/// number of n-grams seen exactly 1–4 times. Falls back to one absolute discount when the
/// counts are too sparse (toy models).
fn discounts(n: [f64; 5]) -> [f64; 3] {
    if n[1..].iter().all(|&x| x > 0.0) {
        let y = n[1] / (n[1] + 2.0 * n[2]);
        let d = [
            1.0 - 2.0 * y * n[2] / n[1],
            2.0 - 3.0 * y * n[3] / n[2],
            3.0 - 4.0 * y * n[4] / n[3],
        ];
        if d.iter()
            .enumerate()
            .all(|(i, &x)| x > 0.0 && x < (i + 1) as f64)
        {
            return d;
        }
    }
    let d = if n[1] > 0.0 && n[2] > 0.0 {
        n[1] / (n[1] + 2.0 * n[2])
    } else {
        0.5
    };
    [d; 3]
}

impl G2pModel {
    /// Train an order-`order` Kneser-Ney model over aligned words.
    pub fn train<'a>(
        words: impl IntoIterator<Item = &'a [Unit]>,
        order: usize,
    ) -> Result<Self, String> {
        if order < 2 {
            return Err(format!("g2p order must be ≥ 2, got {order}"));
        }
        let mut units = vec![Unit {
            letters: String::new(),
            phones: vec![],
        }];
        let mut uni = vec![0u32];
        let mut ids: HashMap<Unit, u32> = HashMap::new();
        let mut seqs: Vec<Vec<u32>> = Vec::new();
        for word in words {
            let mut seq = vec![BOUNDARY; order - 1];
            for u in word {
                let id = *ids.entry(u.clone()).or_insert_with(|| {
                    units.push(u.clone());
                    uni.push(0);
                    (units.len() - 1) as u32
                });
                seq.push(id);
            }
            seq.push(BOUNDARY);
            for &id in &seq[order - 1..] {
                uni[id as usize] += 1;
            }
            seqs.push(seq);
        }
        let bits = id_bits(units.len());
        if order as u32 * bits > 64 {
            return Err(format!(
                "g2p order {order} with {} units needs {} key bits (> 64)",
                units.len(),
                order as u32 * bits
            ));
        }
        let mask = |k: usize| {
            if k == 0 {
                0
            } else {
                u64::MAX >> (64 - k as u32 * bits)
            }
        };

        // counts[k]: n-grams with k context units. Raw counts at the top order; below it,
        // continuation counts (how many distinct units precede the n-gram).
        let mut counts: Vec<KeyMap<u32>> = (0..order).map(|_| KeyMap::default()).collect();
        for seq in &seqs {
            for i in order - 1..seq.len() {
                let key = seq[i + 1 - order..=i]
                    .iter()
                    .fold(0u64, |k, &id| k << bits | id as u64);
                *counts[order - 1].entry(key).or_default() += 1;
            }
        }
        for k in (1..order).rev() {
            let higher: Vec<u64> = counts[k].keys().copied().collect();
            for key in higher {
                *counts[k - 1].entry(key & mask(k)).or_default() += 1;
            }
        }

        let v = units.len() as f64;
        let mut unigram = vec![0f32; units.len()];
        let mut grams: Vec<KeyMap<f32>> = Vec::new();
        let mut backoff: Vec<KeyMap<f32>> = Vec::new();
        for (k, level) in counts.iter().enumerate() {
            let mut n = [0f64; 5];
            // Per context: (count sum, followers seen once, twice, ≥3 times).
            let mut ctx: KeyMap<(u32, u32, u32, u32)> = KeyMap::default();
            for (&key, &c) in level {
                if (c as usize) < n.len() {
                    n[c as usize] += 1.0;
                }
                let e = ctx.entry(key >> bits).or_default();
                e.0 += c;
                match c {
                    1 => e.1 += 1,
                    2 => e.2 += 1,
                    _ => e.3 += 1,
                }
            }
            let d = discounts(n);
            let gamma = |&(total, n1, n2, n3): &(u32, u32, u32, u32)| {
                (d[0] * n1 as f64 + d[1] * n2 as f64 + d[2] * n3 as f64) / total as f64
            };
            // P(w | h) for the level below, which already holds every suffix of this level's n-grams.
            let lower = |key: u64| -> f64 {
                let w = (key & mask(1)) as usize;
                if k == 0 {
                    1.0 / v
                } else if k == 1 {
                    (unigram[w] as f64).exp()
                } else {
                    let suffix = key & mask(k);
                    (grams[k - 2][&suffix] as f64).exp()
                }
            };
            let mut probs: KeyMap<f32> = KeyMap::default();
            for (&key, &c) in level {
                let stats = &ctx[&(key >> bits)];
                let p = (c as f64 - d[(c as usize).min(3) - 1]) / stats.0 as f64
                    + gamma(stats) * lower(key);
                probs.insert(key, p.ln() as f32);
            }
            if k == 0 {
                // Every unit is seen, but keep unseen ids well defined anyway.
                let floor = ctx.get(&0).map_or(1.0, gamma) / v;
                for (w, slot) in unigram.iter_mut().enumerate() {
                    *slot = probs.get(&(w as u64)).copied().unwrap_or(floor.ln() as f32);
                }
            } else {
                backoff.push(
                    ctx.iter()
                        .map(|(&h, s)| (h, gamma(s).ln() as f32))
                        .collect(),
                );
                grams.push(probs);
            }
        }
        let sorted = |m: KeyMap<f32>| {
            let mut v: Vec<(u64, f32)> = m.into_iter().collect();
            v.sort_unstable_by_key(|e| e.0);
            v
        };
        Ok(G2pModel {
            units,
            uni,
            order: order as u32,
            unigram,
            grams: grams.into_iter().map(sorted).collect(),
            backoff: backoff.into_iter().map(sorted).collect(),
        })
    }

    pub fn num_ngrams(&self) -> usize {
        self.unigram.len() + self.grams.iter().map(Vec::len).sum::<usize>()
    }
}

/// A spelling option for a phone sequence: unit id, its letters, and cost = −ln P(unit | phones).
#[derive(Clone, Debug)]
pub struct Spelling {
    pub unit: u32,
    pub letters: String,
    pub cost: f32,
}

pub struct G2p {
    /// The model with its n-gram tables moved into `grams` / `backoff` below.
    model: G2pModel,
    by_letters: HashMap<String, Vec<u32>>,
    order: usize,
    bits: u32,
    /// `grams[k - 1]` / `backoff[k - 1]`: lookup tables for contexts of k units.
    grams: Vec<KeyMap<f32>>,
    backoff: Vec<KeyMap<f32>>,
    max_letters: usize,
    /// Stress-collapsed phones → spellings, cheapest first.
    p2g: HashMap<Vec<Phone>, Vec<Spelling>>,
    max_phones: usize,
    cache: Mutex<HashMap<(String, usize), Pronunciations>>,
}

impl G2p {
    pub fn new(mut model: G2pModel) -> Self {
        let mut by_letters: HashMap<String, Vec<u32>> = HashMap::new();
        for (id, u) in model.units.iter().enumerate().skip(1) {
            by_letters
                .entry(u.letters.clone())
                .or_default()
                .push(id as u32);
        }
        let order = (model.order as usize).max(1);
        let bits = id_bits(model.units.len());
        let to_map = |v: Vec<(u64, f32)>| v.into_iter().collect::<KeyMap<f32>>();
        let grams: Vec<KeyMap<f32>> = std::mem::take(&mut model.grams)
            .into_iter()
            .map(to_map)
            .collect();
        let backoff: Vec<KeyMap<f32>> = std::mem::take(&mut model.backoff)
            .into_iter()
            .map(to_map)
            .collect();
        let max_letters = model
            .units
            .iter()
            .map(|u| u.letters.chars().count())
            .max()
            .unwrap_or(1);

        let mut groups: HashMap<Vec<Phone>, Vec<(u32, u32)>> = HashMap::new();
        for (id, u) in model.units.iter().enumerate().skip(1) {
            let key: Vec<Phone> = u.phones.iter().map(|p| p.collapse_stress()).collect();
            groups
                .entry(key)
                .or_default()
                .push((id as u32, model.uni[id]));
        }
        let mut p2g = HashMap::new();
        for (key, members) in groups {
            let sum: f64 = members.iter().map(|&(_, c)| c as f64).sum();
            // Units differing only in vowel stress (o:OW1, o:OW2) collapse to one spelling;
            // keep the most frequent unit id and pool the counts.
            let mut by_letters: HashMap<&str, (u32, u32, u32)> = HashMap::new();
            for (id, c) in members {
                let e = by_letters
                    .entry(model.units[id as usize].letters.as_str())
                    .or_insert((id, 0, 0));
                e.2 += c;
                if c > e.1 {
                    *e = (id, c, e.2);
                }
            }
            let mut spellings: Vec<Spelling> = by_letters
                .into_iter()
                .filter(|&(_, (_, _, total))| total > 0)
                .map(|(letters, (id, _, total))| Spelling {
                    unit: id,
                    letters: letters.to_string(),
                    cost: -((total as f64 / sum).ln()) as f32,
                })
                .collect();
            spellings.sort_by(|a, b| {
                a.cost
                    .total_cmp(&b.cost)
                    .then_with(|| a.letters.cmp(&b.letters))
            });
            p2g.insert(key, spellings);
        }
        let max_phones = model
            .units
            .iter()
            .map(|u| u.phones.len())
            .max()
            .unwrap_or(1);
        G2p {
            model,
            by_letters,
            order,
            bits,
            grams,
            backoff,
            max_letters,
            p2g,
            max_phones,
            cache: Mutex::default(),
        }
    }

    /// Give back the underlying model (e.g. to serialize it).
    pub fn into_model(self) -> G2pModel {
        let sorted = |m: KeyMap<f32>| {
            let mut v: Vec<(u64, f32)> = m.into_iter().collect();
            v.sort_unstable_by_key(|e| e.0);
            v
        };
        let mut model = self.model;
        model.grams = self.grams.into_iter().map(sorted).collect();
        model.backoff = self.backoff.into_iter().map(sorted).collect();
        model
    }

    pub fn order(&self) -> usize {
        self.order
    }

    pub fn unit(&self, id: u32) -> &Unit {
        &self.model.units[id as usize]
    }

    pub fn num_units(&self) -> usize {
        self.model.units.len()
    }

    pub fn max_phones_per_unit(&self) -> usize {
        self.max_phones
    }

    fn mask(&self, k: usize) -> u64 {
        if k == 0 {
            0
        } else {
            u64::MAX >> (64 - k as u32 * self.bits)
        }
    }

    /// History after appending unit `w`: the last `order - 1` units, newest in the low bits.
    fn push(&self, hist: u64, w: u32) -> u64 {
        (hist << self.bits | w as u64) & self.mask(self.order - 1)
    }

    /// ln P(w | hist): the longest seen context's probability times the backoff weights of
    /// the longer contexts that were seen without `w`.
    fn logp(&self, hist: u64, w: u32) -> f64 {
        let mut bow = 0f32;
        for k in (1..self.order).rev() {
            let h = hist & self.mask(k);
            if let Some(&p) = self.grams[k - 1].get(&(h << self.bits | w as u64)) {
                return (bow + p) as f64;
            }
            if let Some(&g) = self.backoff[k - 1].get(&h) {
                bow += g;
            }
        }
        (bow + self.model.unigram[w as usize]) as f64
    }

    /// N-best pronunciations of a lowercase word, with log-probabilities. Letters the model
    /// has never seen are skipped.
    pub fn pronounce(&self, word: &str, nbest: usize) -> Pronunciations {
        let key = (word.to_string(), nbest);
        if let Some(hit) = self.cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
            return hit;
        }
        let out = self.decode(word, nbest);
        if let Ok(mut cache) = self.cache.lock() {
            if cache.len() >= CACHE_MAX {
                cache.clear();
            }
            cache.insert(key, out.clone());
        }
        out
    }

    fn decode(&self, word: &str, nbest: usize) -> Pronunciations {
        let letters: Vec<char> = word
            .chars()
            .flat_map(|c| c.to_lowercase())
            .filter(|c| self.by_letters.contains_key(c.to_string().as_str()))
            .collect();
        if letters.is_empty() {
            return vec![(vec![], 0.0)];
        }
        let n = letters.len();
        // Arena of (parent, unit); index 0 is the root.
        let mut arena: Vec<(usize, u32)> = vec![(usize::MAX, BOUNDARY)];
        // State: the last order-1 units (all BOUNDARY at the start, as in training).
        let mut states: Vec<KeyMap<(f64, usize)>> = vec![KeyMap::default(); n + 1];
        states[0].insert(0, (0.0, 0));
        let mut key = String::new();
        for i in 0..n {
            let mut cur: Vec<(u64, (f64, usize))> = states[i].drain().collect();
            cur.sort_by(|a, b| b.1.0.total_cmp(&a.1.0));
            cur.truncate(BEAM);
            for (hist, (lp, node)) in cur {
                for len in 1..=self.max_letters.min(n - i) {
                    key.clear();
                    key.extend(&letters[i..i + len]);
                    let Some(units) = self.by_letters.get(&key) else {
                        continue;
                    };
                    for &w in units {
                        let nlp = lp + self.logp(hist, w);
                        let slot = states[i + len]
                            .entry(self.push(hist, w))
                            .or_insert((f64::NEG_INFINITY, 0));
                        if nlp > slot.0 {
                            arena.push((node, w));
                            *slot = (nlp, arena.len() - 1);
                        }
                    }
                }
            }
        }
        let mut finals: Vec<(f64, usize)> = states[n]
            .iter()
            .map(|(&hist, &(lp, node))| (lp + self.logp(hist, BOUNDARY), node))
            .collect();
        finals.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut out: Vec<(Vec<Phone>, f64)> = Vec::new();
        for (lp, mut node) in finals {
            let mut units = Vec::new();
            while node != 0 {
                units.push(arena[node].1);
                node = arena[node].0;
            }
            let phones: Vec<Phone> = units
                .iter()
                .rev()
                .flat_map(|&u| self.model.units[u as usize].phones.iter().copied())
                .collect();
            if !out.iter().any(|(p, _)| *p == phones) {
                out.push((phones, lp));
                if out.len() >= nbest {
                    break;
                }
            }
        }
        out
    }

    /// Spelling options for a phone sequence (stress collapsed), cheapest first.
    pub fn spellings(&self, phones: &[Phone]) -> &[Spelling] {
        let key: Vec<Phone> = phones.iter().map(|p| p.collapse_stress()).collect();
        self.p2g.get(&key).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phone::{fmt_phones, parse_pron};

    fn u(letters: &str, phones: &str) -> Unit {
        Unit {
            letters: letters.into(),
            phones: parse_pron(phones).unwrap(),
        }
    }

    fn toy() -> G2p {
        // "cat" K AE1 T, "kit" K IH1 T, "sax" S AE1 K S, "cake" K EY1 K, "tax" T AE1 K S
        let words = [
            vec![u("c", "K"), u("a", "AE1"), u("t", "T")],
            vec![u("k", "K"), u("i", "IH1"), u("t", "T")],
            vec![u("s", "S"), u("a", "AE1"), u("x", "K S")],
            vec![u("c", "K"), u("a", "EY1"), u("k", "K"), u("e", "")],
            vec![u("t", "T"), u("a", "AE1"), u("x", "K S")],
        ];
        G2p::new(G2pModel::train(words.iter().map(|w| w.as_slice()), DEFAULT_ORDER).unwrap())
    }

    #[test]
    fn pronounces_seen_and_unseen_words() {
        let g = toy();
        assert_eq!(fmt_phones(&g.pronounce("cat", 1)[0].0), "K AE1 T");
        // Unseen combination: built from units.
        assert_eq!(fmt_phones(&g.pronounce("tat", 1)[0].0), "T AE1 T");
        assert_eq!(fmt_phones(&g.pronounce("kax", 1)[0].0), "K AE1 K S");
        // Unknown letters are skipped rather than failing.
        assert_eq!(fmt_phones(&g.pronounce("c?at", 1)[0].0), "K AE1 T");
    }

    #[test]
    fn spellings_for_phones() {
        let g = toy();
        let k = g.spellings(&parse_pron("K").unwrap());
        let mut letters: Vec<&str> = k.iter().map(|s| s.letters.as_str()).collect();
        letters.sort();
        assert_eq!(letters, ["c", "k"]);
        assert!(
            (k[0].cost - 2f32.ln()).abs() < 1e-6,
            "c and k are equally likely for K"
        );
        assert_eq!(g.spellings(&parse_pron("K S").unwrap())[0].letters, "x");
        // Stress collapses: AE2 finds AE1 spellings.
        assert_eq!(g.spellings(&parse_pron("AE2").unwrap())[0].letters, "a");
    }
}
