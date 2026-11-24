//! EM many-to-many alignment of letters to phones.
//!
//! Each word is a lattice over (letter position, phone position). Edges consume 1–2 letters
//! and 0–2 phones (no 2:2, no phones without letters). EM learns a probability for each
//! (letters, phones) unit; Viterbi then picks each word's best segmentation.

use phono_core::g2p::Unit;
use phono_core::phone::{ARPABET, Phone};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

const EDGES: [(usize, usize); 5] = [(1, 0), (1, 1), (1, 2), (2, 0), (2, 1)];

/// Fast hasher for the packed u64 unit keys (Fx-style multiply).
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

type Map<V> = HashMap<u64, V, BuildHasherDefault<KeyHasher>>;

/// Pack a unit (≤ 2 letters, ≤ 2 phones) into a u64.
fn key(letters: &[u8], phones: &[Phone]) -> u64 {
    let mut k = (letters.len() as u64) << 40 | (phones.len() as u64) << 44;
    for (i, &l) in letters.iter().enumerate() {
        k |= (l as u64) << (8 * i);
    }
    for (i, p) in phones.iter().enumerate() {
        k |= (p.code() as u64) << (16 + 8 * i);
    }
    k
}

fn unkey(k: u64) -> Unit {
    let nl = ((k >> 40) & 0xF) as usize;
    let np = ((k >> 44) & 0xF) as usize;
    let letters = (0..nl).map(|i| ((k >> (8 * i)) & 0xFF) as u8 as char).collect();
    let phones = (0..np).map(|i| Phone::from_code(((k >> (16 + 8 * i)) & 0xFF) as u8)).collect();
    Unit { letters, phones }
}

/// Two phones from one letter only in shapes English spelling actually has: two consonants
/// (`x` → K S) or a glide plus a vowel (`u` → Y UW, `o` → W AH). Without this, EM drifts into
/// degenerate alignments like `t` → T AE, `a` → ∅.
fn allowed(phones: &[Phone]) -> bool {
    match phones {
        [a, b] => {
            let glide = |p: &Phone| matches!(ARPABET[p.base() as usize], "Y" | "W");
            match (a.is_vowel(), b.is_vowel()) {
                (false, false) => true,
                (true, true) => false,
                (false, true) => glide(a),
                (true, false) => glide(b),
            }
        }
        _ => true,
    }
}

fn edge_ok(x: &[u8], y: &[Phone], i: usize, j: usize, a: usize, b: usize) -> bool {
    i + a <= x.len() && j + b <= y.len() && allowed(&y[j..j + b])
}

/// Visit every lattice edge of a word in increasing i: (i, j, i', j', unit key).
fn edges(x: &[u8], y: &[Phone], mut f: impl FnMut(usize, usize, usize, usize, u64)) {
    for i in 0..x.len() {
        for j in 0..=y.len() {
            for &(a, b) in &EDGES {
                if edge_ok(x, y, i, j, a, b) {
                    f(i, j, i + a, j + b, key(&x[i..i + a], &y[j..j + b]));
                }
            }
        }
    }
}

/// Align words; returns unit sequences for words that have at least one alignment.
pub fn align(words: &[(&str, &[Phone])], iters: usize) -> Vec<Vec<Unit>> {
    // Initialize with how often each unit appears in any lattice.
    let mut prob: Map<f64> = Map::default();
    for &(w, y) in words {
        edges(w.as_bytes(), y, |_, _, _, _, k| *prob.entry(k).or_default() += 1.0);
    }
    normalize(&mut prob);

    for it in 0..iters {
        let mut counts: Map<f64> = Map::default();
        let mut loglik = 0.0;
        for &(w, y) in words {
            let x = w.as_bytes();
            let (n, m) = (x.len(), y.len());
            let idx = |i: usize, j: usize| i * (m + 1) + j;
            let mut alpha = vec![0.0f64; (n + 1) * (m + 1)];
            let mut beta = vec![0.0f64; (n + 1) * (m + 1)];
            alpha[0] = 1.0;
            // Edges go forward in i, so iterating i ascending gives a valid order.
            edges(x, y, |i, j, i2, j2, k| alpha[idx(i2, j2)] += alpha[idx(i, j)] * prob[&k]);
            let total = alpha[idx(n, m)];
            if total <= 0.0 {
                continue;
            }
            loglik += total.ln();
            beta[idx(n, m)] = 1.0;
            for i in (0..n).rev() {
                for j in (0..=m).rev() {
                    let mut s = 0.0;
                    for &(a, b) in &EDGES {
                        if edge_ok(x, y, i, j, a, b) {
                            s += prob[&key(&x[i..i + a], &y[j..j + b])] * beta[idx(i + a, j + b)];
                        }
                    }
                    beta[idx(i, j)] = s;
                }
            }
            edges(x, y, |i, j, i2, j2, k| {
                let c = alpha[idx(i, j)] * prob[&k] * beta[idx(i2, j2)] / total;
                if c > 0.0 {
                    *counts.entry(k).or_default() += c;
                }
            });
        }
        normalize(&mut counts);
        // Keep zero entries so every lattice edge still has a probability to look up.
        for p in prob.values_mut() {
            *p = 0.0;
        }
        for (k, p) in counts {
            prob.insert(k, p);
        }
        eprintln!("  EM iteration {}: log-likelihood {:.0}", it + 1, loglik);
    }

    let mut out = Vec::with_capacity(words.len());
    for &(w, y) in words {
        let x = w.as_bytes();
        let (n, m) = (x.len(), y.len());
        let idx = |i: usize, j: usize| i * (m + 1) + j;
        let mut best = vec![(f64::NEG_INFINITY, usize::MAX, 0u64); (n + 1) * (m + 1)];
        best[0].0 = 0.0;
        edges(x, y, |i, j, i2, j2, k| {
            let p = prob[&k];
            if p > 0.0 && best[idx(i, j)].0 > f64::NEG_INFINITY {
                let s = best[idx(i, j)].0 + p.ln();
                if s > best[idx(i2, j2)].0 {
                    best[idx(i2, j2)] = (s, idx(i, j), k);
                }
            }
        });
        if best[idx(n, m)].0 == f64::NEG_INFINITY {
            continue;
        }
        let mut units = Vec::new();
        let mut at = idx(n, m);
        while at != 0 {
            let (_, prev, k) = best[at];
            units.push(unkey(k));
            at = prev;
        }
        units.reverse();
        out.push(units);
    }
    out
}

fn normalize(m: &mut Map<f64>) {
    let total: f64 = m.values().sum();
    if total > 0.0 {
        for v in m.values_mut() {
            *v /= total;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phono_core::phone::parse_pron;

    #[test]
    fn key_roundtrip() {
        let u = Unit { letters: "ph".into(), phones: parse_pron("F").unwrap() };
        assert_eq!(unkey(key(u.letters.as_bytes(), &u.phones)), u);
        let x = Unit { letters: "x".into(), phones: parse_pron("K S").unwrap() };
        assert_eq!(unkey(key(x.letters.as_bytes(), &x.phones)), x);
    }

    #[test]
    fn aligns_a_small_corpus() {
        let corpus = [
            ("cat", "K AE1 T"), ("cab", "K AE1 B"), ("tax", "T AE1 K S"), ("box", "B AA1 K S"),
            ("phat", "F AE1 T"), ("photo", "F OW1 T OW0"), ("bat", "B AE1 T"), ("fat", "F AE1 T"),
        ];
        let prons: Vec<(String, Vec<Phone>)> =
            corpus.iter().map(|(w, p)| (w.to_string(), parse_pron(p).unwrap())).collect();
        let words: Vec<(&str, &[Phone])> = prons.iter().map(|(w, p)| (w.as_str(), p.as_slice())).collect();
        let aligned = align(&words, 5);
        let show = |u: &[Unit]| u.iter().map(|u| format!("{}:{}", u.letters, phono_core::phone::fmt_phones(&u.phones))).collect::<Vec<_>>().join(" ");
        let tax = show(&aligned[2]);
        assert_eq!(tax, "t:T a:AE1 x:K S", "{tax}");
        let phat = show(&aligned[4]);
        assert!(phat.starts_with("ph:F"), "{phat}");
    }
}
