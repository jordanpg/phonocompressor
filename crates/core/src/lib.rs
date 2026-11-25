//! phonocompressor core: find ≤ N Unicode codepoints that an English TTS reads like a phrase.
//!
//! Pipeline (docs/PLAN.md): phrase → target phones (CMUdict / G2P) → search for codepoint
//! sequences whose transliteration spells something with those sounds → re-read each candidate
//! the way the TTS would ([`reading`]) → score against the target ([`score`]).
//!
//! What depends on the TTS model (how it reads each script, reader constants, fitted weights)
//! is in a [`model::ModelProfile`]; the [`Engine`] is built for one.

pub mod chunks;
pub mod data;
pub mod features;
pub mod g2p;
pub mod lexicon;
pub mod model;
pub mod phone;
pub mod reading;
pub mod score;
pub mod search;
pub mod tile;
pub mod words;

use chunks::{ChunkSet, Scripts, Translit};
use features::{FeatureSpace, PhoneDistance};
use g2p::G2p;
use lexicon::Lexicon;
use model::ModelProfile;
use phone::Phone;
use reading::{Reader, Source, Word};
use score::{Alignment, Scorer, TargetPhone, Weights, flatten_target};

/// How candidates are scored before we trust them (see [`Engine::predict`]).
#[derive(Clone, Copy, Debug)]
pub struct Predict {
    /// Average over the G2P n-best readings of unknown words, not just the best one.
    pub nbest: bool,
    /// Multiplier on the per-script risk penalty ([`model::ScriptProfile::risk`]); 0 disables it.
    pub risk_scale: f32,
    /// Weight of spelling closeness to the phrase's real words ([`words::spelling`]), scaled by
    /// the share of the phrase that is real words; 0 disables it.
    pub spelling: f32,
}

impl Predict {
    /// v1–v5's values, for comparison (`phono calibrate --hand`). Each model's fitted values
    /// are in its profile ([`model::ModelProfile::predict`]).
    pub const HAND: Predict = Predict {
        nbest: true,
        risk_scale: 1.0,
        spelling: 0.4,
    };
}

/// A phrase to compress: its words (real or not) and their phones, flattened for alignment.
#[derive(Clone, Debug)]
pub struct Target {
    pub words: Vec<words::TargetWord>,
    pub phones: Vec<TargetPhone>,
}

pub struct Engine {
    /// The TTS model the engine predicts readings for. Switch it with [`Engine::configure`].
    pub profile: &'static ModelProfile,
    /// The scripts [`Engine::chunks`] was built from.
    scripts: Scripts,
    pub features: FeatureSpace,
    pub dist: PhoneDistance,
    pub lexicon: Lexicon,
    pub g2p: G2p,
    pub translit: Translit,
    pub chunks: ChunkSet,
    /// Alignment weights for prediction and generation ([`Engine::scorer`]). Starts as the
    /// profile's; calibration and fitting change it.
    pub weights: Weights,
}

impl Engine {
    pub fn new(data: data::Data, profile: &'static ModelProfile) -> Result<Self, String> {
        let features = FeatureSpace::new(data.features);
        let dist = PhoneDistance::new(&features)?;
        let scripts = Scripts::Default;
        let chunks = ChunkSet::for_scripts(&data.translit, profile, &scripts)?;
        Ok(Engine {
            profile,
            scripts,
            features,
            dist,
            lexicon: data.lexicon,
            g2p: G2p::new(data.g2p),
            translit: data.translit,
            chunks,
            weights: profile.weights.clone(),
        })
    }

    /// Switch the model and the scripts the search uses, without decoding the data file again.
    ///
    /// Rebuilds [`Engine::chunks`] when either changes. A new profile also brings its own
    /// [`Engine::weights`]; switching scripts alone keeps the current ones (which `phono fit`
    /// and `calibrate` may have changed). On error, nothing changes.
    pub fn configure(
        &mut self,
        profile: &'static ModelProfile,
        scripts: Scripts,
    ) -> Result<(), String> {
        let scripts = scripts.normalized();
        let new_profile = !std::ptr::eq(self.profile, profile);
        if new_profile || scripts != self.scripts {
            self.chunks = ChunkSet::for_scripts(&self.translit, profile, &scripts)?;
        }
        if new_profile {
            self.weights = profile.weights.clone();
        }
        self.profile = profile;
        self.scripts = scripts;
        Ok(())
    }

    /// The scripts the search uses ([`Engine::configure`]).
    pub fn scripts(&self) -> &Scripts {
        &self.scripts
    }

    /// Let the search use every script the model transliterates, including the ones off by
    /// default ([`model::ScriptProfile::default_on`]).
    pub fn allow_all_scripts(&mut self) {
        self.configure(self.profile, Scripts::All)
            .expect("Scripts::All is valid for every profile");
    }

    pub fn from_bytes(bytes: &[u8], profile: &'static ModelProfile) -> Result<Self, String> {
        Self::new(data::Data::from_bytes(bytes)?, profile)
    }

    pub fn reader(&self) -> Reader<'_> {
        Reader {
            lexicon: &self.lexicon,
            g2p: &self.g2p,
            params: self.profile.reader,
        }
    }

    pub fn scorer(&self) -> Scorer<'_> {
        Scorer::with_weights(&self.dist, self.weights.clone())
    }

    /// Target pronunciation of an English phrase, one phone list per word.
    pub fn target(&self, phrase: &str) -> Vec<Vec<Phone>> {
        self.reader()
            .read(phrase)
            .into_iter()
            .map(|w| w.phones)
            .collect()
    }

    /// The phrase's words, each marked real (dictionary, number, symbol) or not (usernames,
    /// coinages). A token that is not a dictionary word stays one word even when the reader
    /// pronounces it in pieces (`chungus` → chun + gus); a number token becomes its number
    /// words (`100,000` → one hundred thousand).
    pub fn target_words(&self, phrase: &str) -> Vec<words::TargetWord> {
        let reader = self.reader();
        let mut out = Vec::new();
        for token in phrase.split_whitespace() {
            let token = token.replace(['\u{2019}', '\u{2018}'], "'");
            let read = reader.read(&token);
            let has_letters = token.chars().any(|c| c.is_alphabetic());
            let single_dictionary = read.len() == 1 && read[0].source == Source::Dictionary;
            if single_dictionary
                || (!has_letters && read.iter().all(|w| w.source == Source::Dictionary))
            {
                out.extend(read.into_iter().map(|w| words::TargetWord {
                    text: w.text,
                    phones: w.phones,
                    lexical: true,
                }));
            } else if !read.is_empty() {
                let phones = read.into_iter().flat_map(|w| w.phones).collect();
                out.push(words::TargetWord {
                    text: token.to_lowercase(),
                    phones,
                    lexical: false,
                });
            }
        }
        out
    }

    /// The phrase as a search target.
    pub fn target_of(&self, phrase: &str) -> Target {
        let words = self.target_words(phrase);
        let (phones, _) = words::flatten(&words);
        Target { words, phones }
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

    /// Predicted fidelity of `text` as `target`:
    /// - the expected phone fidelity over each unknown word's alternative readings
    ///   (first-order: one word swapped at a time, weighted by its probability),
    /// - minus a per-codepoint script risk,
    /// - blended with how closely the reading is spelled like the phrase's real words
    ///   (usernames and coinages are judged by sound only).
    pub fn predict(&self, text: &str, target: &Target, opts: &Predict) -> f32 {
        let (_, words) = self.read(text);
        self.predict_read(text, &words, target, opts)
    }

    /// [`Engine::predict`] for a `text` already read into `words` (saves re-reading it).
    pub fn predict_read(&self, text: &str, words: &[Word], target: &Target, opts: &Predict) -> f32 {
        let sound = self.predict_sound(text, words, &target.phones, opts);
        match words::spelling(&target.words, words) {
            Some((spell, share)) if opts.spelling > 0.0 => {
                let w = opts.spelling * share;
                (1.0 - w) * sound + w * spell
            }
            _ => sound,
        }
    }

    /// The sound part of [`Engine::predict`]: expected phone fidelity minus script risk.
    fn predict_sound(
        &self,
        text: &str,
        words: &[Word],
        target: &[TargetPhone],
        opts: &Predict,
    ) -> f32 {
        let scorer = self.scorer();
        let max = scorer.max_cost(target);
        if max <= 0.0 {
            return 0.0;
        }
        let fidelity = |phones: &[Phone]| (1.0 - scorer.cost(target, phones) / max).max(0.0);
        let base: Vec<Phone> = words
            .iter()
            .flat_map(|w| w.phones.iter().copied())
            .collect();
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
        (expected - opts.risk_scale * self.profile.risk(text)).max(0.0)
    }

    /// Score how well `text` (any Unicode) would be heard as `phrase`.
    pub fn score(&self, text: &str, phrase: &str) -> Alignment {
        let target = flatten_target(&self.target(phrase));
        let (_, words) = self.read(text);
        let cand: Vec<Phone> = words.into_iter().flat_map(|w| w.phones).collect();
        self.scorer().align(&target, &cand)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chunks::Script;
    use features::FeatureTable;
    use model::{FLASH_V2_5, ScriptProfile, ScriptReading};

    /// Only Latin and Hangul, both transliterated and on by default; weights differ from Flash.
    static LATIN_HANGUL: ModelProfile = ModelProfile {
        model_ids: &["test_latin_hangul"],
        scripts: |s| ScriptProfile {
            reading: if matches!(s, Script::Latin | Script::Hangul) {
                ScriptReading::Transliterated
            } else {
                ScriptReading::Unmeasured
            },
            risk: 0.0,
            default_on: true,
        },
        script_preference: &[Script::Latin, Script::Hangul],
        reader: reading::ReaderParams {
            split_short_word: 0.5,
            split_g2p_letter: 0.3,
            split_lone_letter: 0.7,
            split_onset: 1.0,
            glued_number_drop: 1.0,
        },
        weights: Weights::HAND,
        predict: Predict::HAND,
    };

    fn engine() -> Engine {
        let tsv = include_str!("../../../phoible/raw-data/FEATURES/phoible-segments-features.tsv");
        let translit = [
            ('a', "a"),
            (' ', " "),
            ('바', "ba"),
            ('ꀠ', "ba"),
            ('ሀ', "ha"),
        ];
        let data = data::Data {
            version: data::FORMAT_VERSION,
            features: FeatureTable::parse_tsv(tsv).unwrap(),
            translit: Translit {
                map: translit.iter().map(|&(c, s)| (c, s.to_string())).collect(),
            },
            ..Default::default()
        };
        Engine::new(data, &FLASH_V2_5).unwrap()
    }

    fn chars(e: &Engine) -> Vec<char> {
        let mut c: Vec<char> = e.chunks.chunks.iter().map(|c| c.ch).collect();
        c.sort();
        c
    }

    #[test]
    fn switching_scripts_and_back_restores_the_default() {
        let mut e = engine();
        let default = chars(&e);
        assert_eq!(default, [' ', 'a', '바'], "Ethiopic `ሀ` is off by default");
        assert_eq!(e.scripts(), &Scripts::Default);

        e.allow_all_scripts();
        assert_eq!(chars(&e), [' ', 'a', 'ሀ', '바']);
        assert_eq!(e.scripts(), &Scripts::All);

        e.configure(
            &FLASH_V2_5,
            Scripts::from_names(&["yi", "ethiopic"]).unwrap(),
        )
        .unwrap();
        assert_eq!(chars(&e), ['ሀ', 'ꀠ']);

        e.configure(&FLASH_V2_5, Scripts::Default).unwrap();
        assert_eq!(chars(&e), default);
    }

    #[test]
    fn failed_switch_leaves_the_engine_unchanged() {
        let mut e = engine();
        e.allow_all_scripts();
        let before = chars(&e);
        let err = e
            .configure(&FLASH_V2_5, Scripts::from_names(&["devanagari"]).unwrap())
            .unwrap_err();
        assert!(err.contains("natively"), "{err}");
        let err = e
            .configure(&LATIN_HANGUL, Scripts::from_names(&["yi"]).unwrap())
            .unwrap_err();
        assert!(
            err.contains("test_latin_hangul"),
            "checked against the new profile: {err}"
        );
        assert_eq!(chars(&e), before);
        assert_eq!(e.scripts(), &Scripts::All);
        assert!(std::ptr::eq(e.profile, &FLASH_V2_5));
    }

    #[test]
    fn weights_follow_the_profile_not_the_scripts() {
        let mut e = engine();
        e.weights.sub_scale = 9.0; // as `phono fit` or `calibrate --hand` would
        e.configure(&FLASH_V2_5, Scripts::All).unwrap();
        assert_eq!(
            e.weights.sub_scale, 9.0,
            "a script switch keeps fitted weights"
        );

        e.configure(&LATIN_HANGUL, Scripts::Default).unwrap();
        assert!(std::ptr::eq(e.profile, &LATIN_HANGUL));
        assert_eq!(e.weights.sub_scale, Weights::HAND.sub_scale);
        assert_eq!(chars(&e), [' ', 'a', '바']);
    }
}
