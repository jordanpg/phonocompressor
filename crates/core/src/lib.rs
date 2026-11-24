//! phonocompressor core: find ≤ N Unicode codepoints that an English TTS reads like a phrase.
//!
//! Pipeline (docs/PLAN.md): phrase → target phones (CMUdict / G2P) → search for codepoint
//! sequences whose transliteration spells something with those sounds → re-read each candidate
//! the way the TTS would ([`reading`]) → score against the target ([`score`]).

pub mod chunks;
pub mod data;
pub mod features;
pub mod g2p;
pub mod lexicon;
pub mod phone;
pub mod reading;
pub mod score;
pub mod search;

use chunks::{ChunkSet, Translit};
use features::{FeatureSpace, PhoneDistance};
use g2p::G2p;
use lexicon::Lexicon;
use phone::Phone;
use reading::{Reader, Word};
use score::{Alignment, Scorer, TargetPhone, flatten_target};

/// How candidates are scored before we trust them (see [`Engine::predict`]).
#[derive(Clone, Copy, Debug)]
pub struct Predict {
    /// Average over the G2P n-best readings of unknown words, not just the best one.
    pub nbest: bool,
    /// Multiplier on the per-script risk penalty ([`chunks::Script::risk`]); 0 disables it.
    pub risk_scale: f32,
}

impl Default for Predict {
    fn default() -> Self {
        Predict { nbest: true, risk_scale: 1.0 }
    }
}

pub struct Engine {
    pub features: FeatureSpace,
    pub dist: PhoneDistance,
    pub lexicon: Lexicon,
    pub g2p: G2p,
    pub translit: Translit,
    pub chunks: ChunkSet,
}

impl Engine {
    pub fn new(data: data::Data) -> Result<Self, String> {
        let features = FeatureSpace::new(data.features);
        let dist = PhoneDistance::new(&features)?;
        let chunks = ChunkSet::new(&data.translit);
        Ok(Engine { features, dist, lexicon: data.lexicon, g2p: G2p::new(data.g2p), translit: data.translit, chunks })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        Self::new(data::Data::from_bytes(bytes)?)
    }

    pub fn reader(&self) -> Reader<'_> {
        Reader { lexicon: &self.lexicon, g2p: &self.g2p }
    }

    pub fn scorer(&self) -> Scorer<'_> {
        Scorer::new(&self.dist)
    }

    /// Target pronunciation of an English phrase, one phone list per word.
    pub fn target(&self, phrase: &str) -> Vec<Vec<Phone>> {
        self.reader().read(phrase).into_iter().map(|w| w.phones).collect()
    }

    /// How the TTS reads a Unicode string under reading model T.
    pub fn read(&self, text: &str) -> (String, Vec<Word>) {
        let latin = self.translit.latin(text);
        let words = self.reader().read(&latin);
        (latin, words)
    }

    /// Override a Phoible feature weight and recompute phone distances (calibration).
    pub fn set_feature_weight(&mut self, name: &str, weight: f32) -> Result<(), String> {
        self.features.set_weight(name, weight)?;
        self.dist = PhoneDistance::new(&self.features)?;
        Ok(())
    }

    /// Predicted fidelity of `text` as `target`, accounting for reading uncertainty:
    /// the expected fidelity over each unknown word's alternative readings (first-order: one
    /// word swapped at a time, weighted by its probability), minus a per-codepoint script risk.
    pub fn predict(&self, text: &str, target: &[TargetPhone], opts: &Predict) -> f32 {
        let (_, words) = self.read(text);
        self.predict_read(text, &words, target, opts)
    }

    /// [`Engine::predict`] for a `text` already read into `words` (saves re-reading it).
    pub fn predict_read(&self, text: &str, words: &[Word], target: &[TargetPhone], opts: &Predict) -> f32 {
        let scorer = self.scorer();
        let max = scorer.max_cost(target);
        if max <= 0.0 {
            return 0.0;
        }
        let fidelity = |phones: &[Phone]| (1.0 - scorer.cost(target, phones) / max).max(0.0);
        let base: Vec<Phone> = words.iter().flat_map(|w| w.phones.iter().copied()).collect();
        let f0 = fidelity(&base);
        let mut expected = f0;
        if opts.nbest {
            let mut offset = 0;
            for w in words {
                let total: f32 = 1.0 + w.alternatives.iter().map(|(_, r)| r).sum::<f32>();
                for (alt, r) in &w.alternatives {
                    let mut phones = base[..offset].to_vec();
                    phones.extend_from_slice(alt);
                    phones.extend_from_slice(&base[offset + w.phones.len()..]);
                    expected += (r / total) * (fidelity(&phones) - f0);
                }
                offset += w.phones.len();
            }
        }
        let risk: f32 = text.chars().map(|c| chunks::Script::of(c).risk()).sum();
        (expected - opts.risk_scale * risk).max(0.0)
    }

    /// Score how well `text` (any Unicode) would be heard as `phrase`.
    pub fn score(&self, text: &str, phrase: &str) -> Alignment {
        let target = flatten_target(&self.target(phrase));
        let (_, words) = self.read(text);
        let cand: Vec<Phone> = words.into_iter().flat_map(|w| w.phones).collect();
        self.scorer().align(&target, &cand)
    }
}
