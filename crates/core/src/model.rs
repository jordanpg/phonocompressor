//! Everything that depends on which TTS model reads the output.
//!
//! The target side (phrase → phones), the phone distances and the search are the same for any
//! model. What differs is how the model reads each script, how reliably it does so, how it
//! splits run-together letters, and the scorer weights fitted to its clips. A [`ModelProfile`]
//! holds those. Each profile comes from harness runs on its own model, so a new model needs an
//! exp0-style probe and a community-message run before it gets a profile.
//!
//! Only Flash v2.5 is calibrated (exp0–exp8). Some reader behavior is still code, not profile
//! data, because only Flash v2.5 has been measured: the capital-letter rules and plurals in
//! [`crate::reading`] and number grouping. Models that read scripts natively (exp0: v3 and v4)
//! also need reading model N, which doesn't exist yet ([`ScriptReading::Native`] scripts are
//! never searched).

use crate::Predict;
use crate::chunks::Script;
use crate::reading::ReaderParams;
use crate::score::Weights;

/// How a model reads a script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptReading {
    /// Reading model T: unidecode's Latin, read as English. The search can use it.
    Transliterated,
    /// Read in its own language (reading model N), which the reader doesn't model yet.
    Native,
    /// Not measured on this model.
    Unmeasured,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptProfile {
    pub reading: ScriptReading,
    /// Penalty per codepoint for how unpredictable the model's reading of this script has been
    /// in the harness. Scaled by [`Predict::risk_scale`].
    pub risk: f32,
    /// The search uses it unless told otherwise (`--all-scripts`). Only meaningful for
    /// [`ScriptReading::Transliterated`] scripts.
    pub default_on: bool,
}

pub struct ModelProfile {
    /// ElevenLabs model ids this profile covers. Harness clips from other models are ignored.
    pub model_ids: &'static [&'static str],
    pub scripts: fn(Script) -> ScriptProfile,
    /// When several codepoints transliterate to the same chunk, the search uses the one whose
    /// script comes first here (then lowercase forms). Unlisted scripts come last.
    pub script_preference: &'static [Script],
    pub reader: ReaderParams,
    /// Alignment weights for prediction and generation. `phono calibrate` measures truth with
    /// [`Weights::HAND`] whatever the model, so its numbers stay comparable.
    pub weights: Weights,
    pub predict: Predict,
}

impl ModelProfile {
    /// The main model id (the first of [`ModelProfile::model_ids`]), for messages.
    pub fn id(&self) -> &'static str {
        self.model_ids
            .first()
            .expect("every profile covers at least one model id")
    }

    pub fn script(&self, s: Script) -> ScriptProfile {
        (self.scripts)(s)
    }

    /// Script risk summed over a string's codepoints.
    pub fn risk(&self, text: &str) -> f32 {
        text.chars().map(|c| self.script(Script::of(c)).risk).sum()
    }

    /// Every script's risk, indexed by [`Script::index`].
    pub fn risks(&self) -> [f32; Script::COUNT] {
        Script::ALL.map(|s| self.script(s).risk)
    }

    /// Rank of a script among representatives for the same chunk (lower is preferred).
    pub fn preference(&self, s: Script) -> usize {
        self.script_preference
            .iter()
            .position(|&p| p == s)
            .unwrap_or(self.script_preference.len())
    }
}

/// Every calibrated profile.
pub static PROFILES: &[&ModelProfile] = &[&FLASH_V2_5, &MULTILINGUAL_V2];

/// The profile used when none is named: the model the community writes for.
pub static DEFAULT: &ModelProfile = &FLASH_V2_5;

/// The profile covering an ElevenLabs model id.
pub fn for_model(id: &str) -> Option<&'static ModelProfile> {
    PROFILES.iter().copied().find(|p| p.model_ids.contains(&id))
}

/// The profile covering a model id, or an error that names the calibrated models.
pub fn profile(id: &str) -> Result<&'static ModelProfile, String> {
    for_model(id).ok_or_else(|| {
        format!(
            "no calibrated profile for model `{id}`; calibrated models: {}",
            known_model_ids().join(", ")
        )
    })
}

/// Model ids with a profile, for error messages.
pub fn known_model_ids() -> Vec<&'static str> {
    PROFILES
        .iter()
        .flat_map(|p| p.model_ids.iter().copied())
        .collect()
}

/// `eleven_flash_v2_5`, calibrated on exp0–exp8. exp0 found `eleven_turbo_v2_5` reads the same
/// way, and ElevenLabs calls the two functionally equivalent.
pub static FLASH_V2_5: ModelProfile = ModelProfile {
    model_ids: &["eleven_flash_v2_5", "eleven_turbo_v2_5"],
    scripts: flash_v2_5_scripts,
    script_preference: SCRIPT_PREFERENCE,
    reader: FLASH_READER,
    weights: FLASH_WEIGHTS,
    predict: FLASH_PREDICT,
};

/// Representative order shared by the calibrated profiles: when several codepoints
/// transliterate to the same chunk, the search uses the first script here.
const SCRIPT_PREFERENCE: &[Script] = &[
    Script::Latin,
    Script::Hangul,
    Script::Yi,
    Script::CjkCompat,
    Script::Cherokee,
    Script::Ucas,
    Script::Ethiopic,
    Script::HangulCompatJamo,
    Script::HangulJamo,
    Script::Runic,
    Script::Armenian,
    Script::Hiragana,
    Script::Katakana,
    Script::CjkUnified,
    Script::CyrillicNumber,
    Script::Devanagari,
    Script::Cyrillic,
    Script::Other,
];

const FLASH_READER: ReaderParams = ReaderParams {
    split_short_word: 0.5,
    split_g2p_letter: 0.3,
    split_lone_letter: 0.7,
    // The TTS read "varymoldifrog" as "very moldy frog" (exp1), "thisisepicswang" as "this
    // is Sepik swang" (exp2) and "biepbiepamtruck" as "beep beep'em truck" (exp5). 0.3–1.0
    // all help offline; 1.0 also keeps `gigamaxdong` from splitting as gig|amax|dong.
    split_onset: 1.0,
    // exp2–exp5 heard the digit in 5 of 10 clips with a number glued to letters (`3the`,
    // `2eut`). Plural `2s` was read both times.
    glued_number_drop: 1.0,
    // exp9: `&` → "and". `@` between words → "at" in 11 of 14 harness clips (clips-all9);
    // at the end of a text it was silent.
    symbol_words: crate::chunks::SYMBOL_WORDS,
};

// v6: deletion and insertion costs fitted by `phono fit` on 408 harness clips (exp1–exp6),
// ranking pairs of answers to the same phrase (λ 0.01, substitution scales held at their
// hand-set values). Losing a sound costs more than v1–v5 assumed, most of all an unstressed
// full vowel (0.45 → 0.94), a singleton consonant (0.7 → 1.23) and an onset (0.9 → 1.22).
// Held-out phrases: within-phrase ranking 80.0% → 83.1% (Whisper-only truth: 74.9% →
// 78.8%). Letting the substitution scales move too scored ~86%, mostly by making vowel
// swaps cheap (3.0 → 1.3); the search avoided vowel swaps so far, so the clips can't say
// whether it would exploit them, and that fit is not used.
const FLASH_WEIGHTS: Weights = Weights {
    del_primary: 1.0,
    del_secondary: 0.84,
    del_unstressed: 0.94,
    del_schwa: 0.37,
    del_onset: 1.22,
    del_cluster: 0.5,
    del_consonant: 1.23,
    ins_stressed: 0.62,
    ins_unstressed: 0.26,
    ins_consonant: 0.75,
    sub_scale: 2.0,
    vowel_sub_scale: 3.0,
    stress_mismatch: 0.18,
    class_mismatch: 0.57,
};

// Fitted with the weights above. The spelling weight was 0.4 in v4 (fit on exp1–exp3).
const FLASH_PREDICT: Predict = Predict {
    nbest: true,
    risk_scale: 1.3,
    spelling: 0.52,
};

/// `eleven_multilingual_v2`, probed in exp11 and calibrated on exp11–exp13 (338 clips). It reads
/// most scripts by transliteration like Flash v2.5, but less clearly (mean truth 0.70 vs 0.78
/// on the same 160 texts).
pub static MULTILINGUAL_V2: ModelProfile = ModelProfile {
    model_ids: &["eleven_multilingual_v2"],
    scripts: multilingual_v2_scripts,
    // exp11 found no transliterated script that reads worse than its neighbours in Flash's
    // order, so the order is the same.
    script_preference: SCRIPT_PREFERENCE,
    reader: ReaderParams {
        // exp11–exp13: a digit glued to letters was read in 11 of 12 clips (`3the` "three the"
        // ×2, `3ᕞ ጔ` "three the gwee"), the 12th unclear. Flash dropped it in about half.
        glued_number_drop: 0.1,
        // `@` was never read "at": 0 of 14 clips across 7 texts, against most mid-phrase clips
        // on Flash (11 of 14 between words: `chuxdung@joMama` "dung at Joe Mama" there,
        // "Dunga Joe Mama" here).
        // `&` → "and" in 5 of 6 clips and `%` → "percent" ×2, as on Flash. `+` was read once
        // in 2 clips (`you+me` /j p s m e/, "You, me"): too few to move it.
        symbol_words: &[('&', "and"), ('+', "plus"), ('%', "percent")],
        // Split costs: moving each one down and up (e.g. onset 0.5, 1.5) left `calibrate`
        // within 158–162 of 210 within-phrase pairs, so they stay Flash's.
        ..FLASH_READER
    },
    // Flash's weights. `phono fit --model eleven_multilingual_v2 --prior profile --fix
    // sub_scale,vowel_sub_scale` on the 338 clips: held-out within-phrase ranking 78.8–80.8%
    // (5 folds by phrase × 4, λ 0–0.03) against 79.5% for these weights (measured on the
    // same clips the risks above were set from). Fitted weights lowered the pooled correlation
    // (0.574 → 0.536) and made losing an onset cost 2.9 (here 1.22), so they are not used.
    weights: FLASH_WEIGHTS,
    predict: FLASH_PREDICT,
};

/// Multilingual v2's scripts, from exp11–exp13:
/// - Read natively: Cyrillic (`Привет` in Russian), Devanagari (`कमल` "Kamal", not unidecode's
///   `kml`) and kana (`フリ`, `ふり` "Free"). Kana is the change from Flash, which searches it.
/// - Everything else is read by transliteration, with the same defaults as Flash.
/// - Risks are Flash's except four. Least squares of Flash-profile over-prediction (predicted −
///   truth) on per-script codepoint counts, Multilingual v2's clips against Flash's own:
///   - `҈`: +0.29 per codepoint vs +0.06. Risk 0.18 × `risk_scale` 1.3 covers the difference.
///     exp11 heard it as "Hander a thousand" and exp12's worst messages contain it.
///   - Canadian syllabics, Ethiopic, conjoining jamo: −0.04, −0.08, −0.09 vs +0.03, +0.03,
///     −0.01 (20 or so codepoints each), so their risks come down (0.05 → 0.02, 0.05 → 0,
///     0.03 → 0). These are this model's own values: Flash's syllabics risk later fell to 0.01
///     (#50), and this one stays at 0.02. `phono fit` pointed the same way: without this it cut `risk_scale` to 0.14,
///     because answers in these scripts beat the risk's prediction within phrases (8 pairs).
///     All three stay off by default, so this changes scoring, not the default search.
///   - Every other script's excess was within the ±0.03 that Flash's in-sample check shows.
fn multilingual_v2_scripts(s: Script) -> ScriptProfile {
    use Script::*;
    use ScriptReading::*;
    match s {
        Hiragana | Katakana => ScriptProfile {
            reading: Native,
            risk: 0.1,
            default_on: false,
        },
        CyrillicNumber => ScriptProfile {
            risk: 0.18,
            ..flash_v2_5_scripts(s)
        },
        Ucas => ScriptProfile {
            risk: 0.02,
            ..flash_v2_5_scripts(s)
        },
        Ethiopic | HangulJamo => ScriptProfile {
            risk: 0.0,
            ..flash_v2_5_scripts(s)
        },
        _ => flash_v2_5_scripts(s),
    }
}

/// Flash v2.5's scripts, from exp0–exp4 (risks for kana and syllabics revised on exp1–exp16):
/// - Latin, Hangul and Yi behaved; CJK, kana, syllabics, Ethiopic and conjoining jamo produced
///   most misreadings (the risk values).
/// - Kana and syllabics read better than those risks said (#50). `phono calibrate --fit-risks`
///   on exp1–exp16's 1764 clips puts Katakana and Hiragana at 0 per codepoint (0.03 before) and
///   syllabics at 0.01 (0.05). Set from each cross-validation fold's training clips, these three
///   raise held-out within-phrase ranking from 76.4% to 77.2% with the weights below.
/// - Ethiopic, Canadian syllabics and conjoining Hangul jamo were misread often (mean outcome
///   0.59, 0.69 and 0.64 across clips vs 0.76–0.80 for Hangul, Latin, Yi), and Runic came out
///   silent or as "Yeah." (exp0 `ᚠ`), so they are off by default.
/// - Devanagari (exp0: `क` → "ka") and Cyrillic are read natively, except the Cyrillic number
///   signs (`҈` → "*100.000*").
fn flash_v2_5_scripts(s: Script) -> ScriptProfile {
    use Script::*;
    use ScriptReading::*;
    let (reading, risk) = match s {
        Latin | Hangul | Yi | CjkCompat | CyrillicNumber | Hiragana | Katakana => {
            (Transliterated, 0.0)
        }
        HangulCompatJamo | Cherokee | Ucas => (Transliterated, 0.01),
        CjkUnified | HangulJamo => (Transliterated, 0.03),
        Runic | Armenian => (Transliterated, 0.04),
        Ethiopic => (Transliterated, 0.05),
        Devanagari | Cyrillic => (Native, 0.1),
        Other => (Unmeasured, 0.1),
    };
    let default_on =
        reading == Transliterated && !matches!(s, Ethiopic | Ucas | HangulJamo | Runic);
    ScriptProfile {
        reading,
        risk,
        default_on,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_profiles_by_model_id() {
        assert!(std::ptr::eq(
            for_model("eleven_flash_v2_5").unwrap(),
            &FLASH_V2_5
        ));
        assert!(std::ptr::eq(
            for_model("eleven_turbo_v2_5").unwrap(),
            &FLASH_V2_5
        ));
        assert!(
            for_model("eleven_v4").is_none(),
            "v4 reads Hangul natively: not calibrated"
        );
        assert!(std::ptr::eq(
            profile("eleven_turbo_v2_5").unwrap(),
            &FLASH_V2_5
        ));
        let err = profile("eleven_v4").err().unwrap();
        assert!(
            err.contains("`eleven_v4`") && err.contains("eleven_flash_v2_5"),
            "{err}"
        );
    }

    #[test]
    fn finds_multilingual_v2() {
        let p = for_model("eleven_multilingual_v2").unwrap();
        assert!(std::ptr::eq(p, &MULTILINGUAL_V2));
        assert_eq!(p.id(), "eleven_multilingual_v2");
        assert!(
            std::ptr::eq(DEFAULT, &FLASH_V2_5),
            "Flash stays the default"
        );
        let ids = known_model_ids();
        assert!(ids.contains(&"eleven_flash_v2_5") && ids.contains(&"eleven_multilingual_v2"));
    }

    #[test]
    fn multilingual_v2_scripts() {
        let p = &MULTILINGUAL_V2;
        // exp11: kana, Cyrillic and Devanagari are read natively.
        for s in [
            Script::Katakana,
            Script::Hiragana,
            Script::Cyrillic,
            Script::Devanagari,
        ] {
            assert_eq!(p.script(s).reading, ScriptReading::Native, "{s:?}");
            assert!(!p.script(s).default_on, "{s:?}");
        }
        for s in [
            Script::Latin,
            Script::Hangul,
            Script::Yi,
            Script::CjkCompat,
            Script::CjkUnified,
            Script::CyrillicNumber,
        ] {
            assert_eq!(p.script(s).reading, ScriptReading::Transliterated, "{s:?}");
            assert!(p.script(s).default_on, "{s:?}");
        }
        for s in [
            Script::Runic,
            Script::HangulJamo,
            Script::Ucas,
            Script::Ethiopic,
        ] {
            assert!(!p.script(s).default_on, "{s:?}");
        }
        // `҈` is read unreliably on this model (exp11–exp13), unlike on Flash.
        assert!(
            p.script(Script::CyrillicNumber).risk > FLASH_V2_5.script(Script::CyrillicNumber).risk
        );
        assert!(p.preference(Script::Hangul) < p.preference(Script::Yi));
        // Ethiopic and conjoining jamo read better here than Flash's risks say. Syllabics keep
        // this model's own risk (exp11–exp13), whatever Flash's is.
        for s in [Script::Ethiopic, Script::HangulJamo] {
            assert!(p.script(s).risk < FLASH_V2_5.script(s).risk, "{s:?}");
        }
        assert_eq!(p.script(Script::Ucas).risk, 0.02);
    }

    #[test]
    fn reader_symbols_are_known_symbols() {
        for p in PROFILES {
            for &(c, w) in p.reader.symbol_words {
                assert_eq!(crate::chunks::symbol_word(c), Some(w), "{}: {c}", p.id());
            }
        }
    }

    #[test]
    fn multilingual_v2_reads_glued_numbers() {
        // exp11–exp13: a digit glued to letters was read in 11 of 12 clips, the 12th unclear
        // (Flash: about half).
        assert!(MULTILINGUAL_V2.reader.glued_number_drop < 0.2);
        assert_eq!(FLASH_V2_5.reader.glued_number_drop, 1.0);
    }

    #[test]
    fn flash_scripts() {
        let p = &FLASH_V2_5;
        assert_eq!(p.script(Script::Devanagari).reading, ScriptReading::Native);
        assert!(p.script(Script::Hangul).default_on);
        assert!(!p.script(Script::Runic).default_on);
        assert!(p.preference(Script::Hangul) < p.preference(Script::Yi));
        assert!((p.risk("ᐊ가") - 0.01).abs() < 1e-6);
        // #50: kana and syllabics read about as well as Latin once picked by hand.
        assert_eq!(p.script(Script::Katakana).risk, 0.0);
        assert_eq!(p.script(Script::Hiragana).risk, 0.0);
        assert!(p.script(Script::Hiragana).default_on);
        assert!(
            !p.script(Script::Ucas).default_on,
            "syllabics stay off by default"
        );
    }
}
