//! Phoible distinctive features and a weighted distance between segments.
//!
//! Source: `phoible/raw-data/FEATURES/phoible-segments-features.tsv` (CC-BY 4.0,
//! Moran & McCloy 2019). Each of the 2,000+ IPA segments has ~37 ternary features
//! (`+`, `-`, `0`), with contours such as `-,+` for diphthongs.

use crate::phone::{PHONE_CODES, Phone};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Feature value encoding: 0 = `0` (not applicable), 1 = `+`, 2 = `-`.
/// Each cell packs (start, end) as `start | end << 2`; non-contours have start == end.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FeatureTable {
    pub names: Vec<String>,
    pub segments: Vec<String>,
    /// Row-major: `values[seg * names.len() + feature]`.
    pub values: Vec<u8>,
}

fn parse_value(v: &str) -> Option<u8> {
    match v {
        "0" => Some(0),
        "+" => Some(1),
        "-" => Some(2),
        _ => None,
    }
}

impl FeatureTable {
    pub fn parse_tsv(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let header: Vec<&str> = lines.next().ok_or("empty feature table")?.split('\t').collect();
        if header.first() != Some(&"segment") {
            return Err(format!("unexpected header: {:?}", header.first()));
        }
        let names: Vec<String> = header[1..].iter().map(|s| s.to_string()).collect();
        let mut table = FeatureTable { names, ..Default::default() };
        for (n, line) in lines.enumerate() {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != table.names.len() + 1 {
                return Err(format!("line {}: {} columns", n + 2, cols.len()));
            }
            table.segments.push(cols[0].to_string());
            for v in &cols[1..] {
                let parts: Vec<&str> = v.split(',').collect();
                let first = parse_value(parts[0]);
                let last = parse_value(parts[parts.len() - 1]);
                match (first, last) {
                    (Some(a), Some(b)) => table.values.push(a | b << 2),
                    _ => return Err(format!("line {}: bad value {v:?}", n + 2)),
                }
            }
        }
        Ok(table)
    }
}

/// Relative importance of each feature for perceived similarity. Unlisted features weigh 0.5.
/// Major class and manner dominate; place and vowel quality are next; laryngeal detail
/// (aspiration, ejectives) matters little to a listener across languages. Tone and stress
/// are ignored here — stress is handled by the scorer.
fn feature_weight(name: &str) -> f32 {
    match name {
        "tone" | "stress" => 0.0,
        "syllabic" | "consonantal" | "sonorant" => 3.0,
        "continuant" | "approximant" | "nasal" => 2.0,
        "delayedRelease" | "tap" | "trill" | "lateral" | "strident" => 1.5,
        "labial" | "coronal" | "dorsal" | "anterior" | "distributed" | "labiodental" => 1.5,
        "high" | "low" | "front" | "back" | "round" => 1.5,
        "tense" => 1.0,
        // Voicing: raised from 1.0 after exp2 (g/k, d/t swaps misheard). Calibrated on exp1+exp2.
        "periodicGlottalSource" => 3.0,
        "short" | "long" | "spreadGlottis" => 0.3,
        _ => 0.5,
    }
}

fn value_mismatch(a: u8, b: u8) -> f32 {
    if a == b {
        0.0
    } else if a == 0 || b == 0 {
        0.5
    } else {
        1.0
    }
}

fn cell_mismatch(x: u8, y: u8) -> f32 {
    (value_mismatch(x & 3, y & 3) + value_mismatch(x >> 2, y >> 2)) * 0.5
}

/// Feature table plus weights and lookup indices.
pub struct FeatureSpace {
    table: FeatureTable,
    index: HashMap<String, usize>,
    weights: Vec<f32>,
    vowel_weights: Vec<f32>,
    syllabic: Option<usize>,
}

/// Rhoticity isn't a Phoible feature (ɚ = ə + a diacritic), so rhotic vs plain vowels gets this.
const RHOTIC_MISMATCH: f32 = 0.3;

/// Spellings the phoneme recognizer (espeak IPA) uses that Phoible writes differently.
const IPA_ALIASES: &[(&str, &str)] = &[("ɚ", "ə˞"), ("ɝ", "ə˞"), ("g", "ɡ"), ("ᵻ", "ɨ")];

impl FeatureSpace {
    pub fn new(table: FeatureTable) -> Self {
        let index = table.segments.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
        let weights: Vec<f32> = table.names.iter().map(|n| feature_weight(n)).collect();
        let vowel_weights = table
            .names
            .iter()
            .map(|n| match n.as_str() {
                "high" | "low" => 2.0,
                "front" | "back" => 1.5,
                "round" | "tense" => 1.0,
                _ => 0.0,
            })
            .collect();
        let syllabic = table.names.iter().position(|n| n == "syllabic");
        FeatureSpace { table, index, weights, vowel_weights, syllabic }
    }

    pub fn segment(&self, ipa: &str) -> Option<usize> {
        self.index.get(ipa).copied()
    }

    /// Override one feature's weight (for calibration experiments).
    pub fn set_weight(&mut self, name: &str, weight: f32) -> Result<(), String> {
        let i = self.table.names.iter().position(|n| n == name).ok_or_else(|| format!("no feature {name:?}"))?;
        self.weights[i] = weight;
        Ok(())
    }

    pub fn segment_name(&self, id: usize) -> &str {
        &self.table.segments[id]
    }

    pub fn len(&self) -> usize {
        self.table.segments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.segments.is_empty()
    }

    /// Syllabic (+ at the start): vowels and syllabic consonants.
    pub fn is_vowel(&self, seg: usize) -> bool {
        self.syllabic.is_some_and(|f| self.table.values[seg * self.table.names.len() + f] & 3 == 1)
    }

    pub fn is_rhotic(&self, seg: usize) -> bool {
        self.table.segments[seg].contains('˞')
    }

    /// Weighted feature distance in [0, 1], normalized over the features at least one of the
    /// two segments specifies (so place features of vowels don't dilute consonant comparisons).
    pub fn distance(&self, a: usize, b: usize) -> f32 {
        self.distance_with(a, b, &self.weights)
    }

    /// Distance over vowel quality only: height, backness, rounding, tenseness (with contours).
    /// All vowels agree on major-class features, which would otherwise swamp these.
    pub fn vowel_distance(&self, a: usize, b: usize) -> f32 {
        self.distance_with(a, b, &self.vowel_weights)
    }

    /// The distance the scorer uses: vowel quality (plus rhoticity) between two vowels,
    /// otherwise the full feature distance.
    pub fn segment_distance(&self, a: usize, b: usize) -> f32 {
        self.segment_distance_with(a, b, RHOTIC_MISMATCH)
    }

    /// [`FeatureSpace::segment_distance`] with a custom rhotic-mismatch cost.
    pub fn segment_distance_with(&self, a: usize, b: usize, rhotic_mismatch: f32) -> f32 {
        if self.is_vowel(a) && self.is_vowel(b) {
            let rhotic = if self.is_rhotic(a) != self.is_rhotic(b) { rhotic_mismatch } else { 0.0 };
            (self.vowel_distance(a, b) + rhotic).min(1.0)
        } else {
            self.distance(a, b)
        }
    }

    fn distance_with(&self, a: usize, b: usize, weights: &[f32]) -> f32 {
        if a == b {
            return 0.0;
        }
        let n = self.table.names.len();
        let (ra, rb) = (&self.table.values[a * n..(a + 1) * n], &self.table.values[b * n..(b + 1) * n]);
        let (mut d, mut total) = (0.0, 0.0);
        for ((&x, &y), &w) in ra.iter().zip(rb).zip(weights) {
            if x != 0 || y != 0 {
                d += w * cell_mismatch(x, y);
                total += w;
            }
        }
        if total > 0.0 { d / total } else { 0.0 }
    }

    /// Segments for one token of recognizer output (`tʃ`, `ɑː`, `ɑu5`, `əl`). Tone digits and
    /// stray punctuation are dropped; unknown multi-character tokens are split greedily into
    /// the longest known segments. Characters that match nothing are skipped.
    pub fn parse_ipa_token(&self, token: &str) -> Vec<usize> {
        let cleaned: String = token.chars().filter(|c| !c.is_ascii_digit() && !matches!(c, '.' | 'ˈ' | 'ˌ')).collect();
        let lookup = |s: &str| {
            let s = IPA_ALIASES.iter().find(|&&(from, _)| from == s).map(|&(_, to)| to).unwrap_or(s);
            self.segment(s)
        };
        if let Some(id) = lookup(&cleaned) {
            return vec![id];
        }
        let chars: Vec<char> = cleaned.chars().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            let found = (i + 1..=chars.len()).rev().find_map(|j| {
                let piece: String = chars[i..j].iter().collect();
                lookup(&piece).map(|id| (id, j))
            });
            match found {
                Some((id, j)) => {
                    out.push(id);
                    i = j;
                }
                None => i += 1,
            }
        }
        out
    }
}

/// Precomputed distance between every pair of English phones (stress ignored).
pub struct PhoneDistance {
    d: Vec<f32>,
    seg: Vec<usize>,
}

impl PhoneDistance {
    pub fn new(space: &FeatureSpace) -> Result<Self, String> {
        let mut seg = vec![usize::MAX; PHONE_CODES];
        for p in Phone::all() {
            seg[p.code() as usize] = space
                .segment(p.ipa())
                .ok_or_else(|| format!("phone {p} ({}) not in Phoible feature table", p.ipa()))?;
        }
        let mut d = vec![1.0; PHONE_CODES * PHONE_CODES];
        for a in Phone::all() {
            for b in Phone::all() {
                d[a.code() as usize * PHONE_CODES + b.code() as usize] =
                    space.segment_distance(seg[a.code() as usize], seg[b.code() as usize]);
            }
        }
        Ok(PhoneDistance { d, seg })
    }

    pub fn get(&self, a: Phone, b: Phone) -> f32 {
        self.d[a.code() as usize * PHONE_CODES + b.code() as usize]
    }

    /// Phoible segment id of a phone.
    pub fn segment(&self, p: Phone) -> usize {
        self.seg[p.code() as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TSV: &str = "segment\tsyllabic\tconsonantal\tsonorant\tperiodicGlottalSource\tcoronal\tlabial\tfront\n\
        p\t-\t+\t-\t-\t-\t+\t0\n\
        b\t-\t+\t-\t+\t-\t+\t0\n\
        t\t-\t+\t-\t-\t+\t-\t0\n\
        a\t+\t-\t+\t+\t-\t-\t-\n\
        aɪ\t+\t-\t+\t+\t-\t-\t-,+\n";

    #[test]
    fn parses_and_ranks_distances() {
        let s = FeatureSpace::new(FeatureTable::parse_tsv(TSV).unwrap());
        let id = |x| s.segment(x).unwrap();
        assert_eq!(s.distance(id("p"), id("p")), 0.0);
        let pb = s.distance(id("p"), id("b"));
        let pt = s.distance(id("p"), id("t"));
        let pa = s.distance(id("p"), id("a"));
        assert!(pb < pa && pt < pa, "voicing and place both well below major class: {pb} {pt} {pa}");
        let contour = s.distance(id("a"), id("aɪ"));
        assert!(contour > 0.0 && contour < pb, "a diphthong is close to its first vowel: {contour}");
    }

    #[test]
    fn rejects_bad_values() {
        assert!(FeatureTable::parse_tsv("segment\tx\np\t?\n").is_err());
    }
}
