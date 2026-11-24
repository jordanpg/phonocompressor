//! Per-word judgement: does a candidate let the listener pick out each word of the phrase?
//!
//! The phone-level fidelity in [`crate::score`] averages over the whole phrase, so one mangled
//! word costs little. Listeners (and Whisper) hear words: a phrase with one unrecognizable word
//! fails. Two things decide whether a target word survives:
//!
//! - its own fidelity (the alignment cost of its phones);
//! - whether the TTS reads it as a word of its own. With no space between them, a run like
//!   `bipbipamtruc` gets one stress pattern and is heard as a new word ("Bipipeng Drug").
//!
//! Real words (dictionary, numbers, symbols) are what listeners expect to recognize. Usernames
//! and coinages (`xqc`, `chungus`) are not words to them, so only their sound is judged.

use crate::phone::Phone;
use crate::reading::{Source, Word};
use crate::score::{Op, Scorer, TargetPhone, flatten_target};

/// One word of the input phrase.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetWord {
    pub text: String,
    pub phones: Vec<Phone>,
    /// A dictionary word, number word or symbol word. False for usernames and coinages.
    pub lexical: bool,
}

/// How one target word fares in a candidate's reading.
#[derive(Clone, Debug, PartialEq)]
pub struct WordJudgement {
    /// 1 − (alignment cost of this word's phones) / (cost of deleting them).
    pub fidelity: f32,
    pub lexical: bool,
    /// A candidate word also covers the previous / next target word (they run together).
    pub joined_prev: bool,
    pub joined_next: bool,
    /// Covered by exactly one candidate word, which covers nothing else and is a dictionary word.
    pub dictionary_hit: bool,
}

/// Target phones of the words, flattened, with each phone's word index.
pub fn flatten(words: &[TargetWord]) -> (Vec<TargetPhone>, Vec<usize>) {
    let phones: Vec<Vec<Phone>> = words.iter().map(|w| w.phones.clone()).collect();
    let owner = words.iter().enumerate().flat_map(|(i, w)| std::iter::repeat_n(i, w.phones.len())).collect();
    (flatten_target(&phones), owner)
}

/// Judge each target word against a candidate read as `cand` (the reader's words).
pub fn judge(scorer: &Scorer, target: &[TargetWord], cand: &[Word]) -> Vec<WordJudgement> {
    let (tphones, towner) = flatten(target);
    let cphones: Vec<Phone> = cand.iter().flat_map(|w| w.phones.iter().copied()).collect();
    let cowner: Vec<usize> = cand.iter().enumerate().flat_map(|(i, w)| std::iter::repeat_n(i, w.phones.len())).collect();
    let n = target.len();
    let mut cost = vec![0f32; n];
    let mut covered: Vec<Vec<usize>> = vec![Vec::new(); n];
    let (mut i, mut j) = (0usize, 0usize);
    // Insertions go to the word being read when they occur (the first word before any phone).
    let mut current = 0usize;
    for op in scorer.align(&tphones, &cphones).ops {
        match op {
            Op::Match(_, _, c) => {
                current = towner[i];
                cost[current] += c;
                covered[current].push(cowner[j]);
                i += 1;
                j += 1;
            }
            Op::Delete(_, c) => {
                current = towner[i];
                cost[current] += c;
                i += 1;
            }
            Op::Insert(_, c) => {
                if n > 0 {
                    cost[current] += c;
                }
                j += 1;
            }
        }
    }
    let mut out = Vec::with_capacity(n);
    for (w, tw) in target.iter().enumerate() {
        let max: f32 = tphones.iter().zip(&towner).filter(|&(_, &o)| o == w).map(|(t, _)| scorer.del_cost(t)).sum();
        let fidelity = if max > 0.0 { (1.0 - cost[w] / max).max(0.0) } else { 1.0 };
        let shares = |other: Option<usize>| {
            other.is_some_and(|o| covered[w].iter().any(|c| covered[o].contains(c)))
        };
        let joined_prev = shares(w.checked_sub(1));
        let joined_next = shares((w + 1 < n).then_some(w + 1));
        let mut cw = covered[w].clone();
        cw.dedup();
        let dictionary_hit =
            cw.len() == 1 && !joined_prev && !joined_next && cand[cw[0]].source == Source::Dictionary;
        out.push(WordJudgement { fidelity, lexical: tw.lexical, joined_prev, joined_next, dictionary_hit });
    }
    out
}

/// How closely the candidate's reading is *spelled* like the phrase's real words.
///
/// An English TTS says a word the way it says that spelling: letters that look like "check"
/// come out as "check", while `chep` comes out "shef" whatever our letter-to-sound predicts.
/// Across 272 recorded clips, the spelling of a candidate's reading predicted what listeners
/// (Whisper) heard about as well as phone fidelity did, and the two together ordered answers
/// for the same phrase far better than either.
///
/// Only real words count: a username or coinage has no spelling the listener knows, so it is
/// judged by sound alone. Each real word is looked for in the candidate's letters (after the
/// reader's normalization: numbers and symbols spelled out, case and punctuation dropped) by
/// best-substring edit distance; the letters around it are free, since the phone score
/// already charges extra sounds.
///
/// Returns (letter-weighted mean similarity over real words, share of the target's letters
/// that belong to real words), or `None` when the phrase has no real words.
pub fn spelling(target: &[TargetWord], cand: &[Word]) -> Option<(f32, f32)> {
    let letters = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect::<Vec<char>>();
    let hay: Vec<char> = cand.iter().flat_map(|w| letters(&w.text)).collect();
    let (mut sim, mut real, mut total) = (0f32, 0f32, 0f32);
    for w in target {
        let word = letters(&w.text);
        let len = word.len() as f32;
        total += len;
        if !w.lexical || word.is_empty() {
            continue;
        }
        // Semi-global edit distance: free start and end in the candidate.
        let mut prev = vec![0u32; hay.len() + 1];
        for (i, &x) in word.iter().enumerate() {
            let mut cur = vec![i as u32 + 1; hay.len() + 1];
            for (j, &y) in hay.iter().enumerate() {
                cur[j + 1] = (prev[j] + u32::from(x != y)).min(prev[j + 1] + 1).min(cur[j] + 1);
            }
            prev = cur;
        }
        let edits = prev.iter().copied().min().unwrap_or(word.len() as u32) as f32;
        sim += len * (1.0 - edits / len).max(0.0);
        real += len;
    }
    (real > 0.0).then(|| (sim / real, real / total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{FeatureSpace, FeatureTable, PhoneDistance};
    use crate::phone::parse_pron;

    fn dist() -> PhoneDistance {
        let tsv = include_str!("../../../phoible/raw-data/FEATURES/phoible-segments-features.tsv");
        PhoneDistance::new(&FeatureSpace::new(FeatureTable::parse_tsv(tsv).unwrap())).unwrap()
    }

    fn tw(text: &str, pron: &str, lexical: bool) -> TargetWord {
        TargetWord { text: text.into(), phones: parse_pron(pron).unwrap(), lexical }
    }

    fn cw(text: &str, pron: &str, source: Source) -> Word {
        Word { text: text.into(), phones: parse_pron(pron).unwrap(), source, alternatives: vec![] }
    }

    #[test]
    fn separate_words_land_and_runs_join() {
        let d = dist();
        let s = Scorer::new(&d);
        let target = [tw("bad", "B AE1 D", true), tw("dream", "D R IY1 M", true)];
        // "bad dream" read as two dictionary words: both land.
        let two = judge(&s, &target, &[cw("bad", "B AE1 D", Source::Dictionary), cw("dream", "D R IY1 M", Source::Dictionary)]);
        assert!(two.iter().all(|j| j.dictionary_hit && !j.joined_next && !j.joined_prev && j.fidelity > 0.99));
        // One pseudo-word over both: same sound, but the words run together.
        let one = judge(&s, &target, &[cw("baddrim", "B AE1 D R IY1 M", Source::G2p)]);
        assert!(one[0].joined_next && one[1].joined_prev);
        assert!(!one[0].dictionary_hit && !one[1].dictionary_hit);
    }

    #[test]
    fn spelling_counts_real_words_only() {
        let target = [tw("check", "CH EH1 K", true), tw("xqc", "EH1 K S K Y UW1 S IY1", false)];
        // Exact spelling of the real word; the username respelled by sound is not held against it.
        let (sim, share) =
            spelling(&target, &[cw("check", "CH EH1 K", Source::Dictionary), cw("ekskyusee", "", Source::G2p)]).unwrap();
        assert!((sim - 1.0).abs() < 1e-6, "{sim}");
        assert!((share - 5.0 / 8.0).abs() < 1e-6);
        // A misspelled real word scores lower.
        let (sim, _) = spelling(&target, &[cw("chep", "CH EH1 P", Source::G2p), cw("xqc", "", Source::Letters)]).unwrap();
        assert!(sim < 0.7, "{sim}");
        // No real words: nothing to say.
        assert!(spelling(&target[1..], &[cw("xqc", "", Source::Letters)]).is_none());
    }

    #[test]
    fn per_word_fidelity_isolates_the_bad_word() {
        let d = dist();
        let s = Scorer::new(&d);
        let target = [tw("time", "T AY1 M", true), tw("loop", "L UW1 P", true)];
        let cand = [cw("time", "T AY1 M", Source::Dictionary), cw("lag", "L AE1 G", Source::Dictionary)];
        let j = judge(&s, &target, &cand);
        assert!(j[0].fidelity > 0.99);
        assert!(j[1].fidelity < 0.6, "{}", j[1].fidelity);
    }
}
