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

    /// Rank of a script among representatives for the same chunk (lower is preferred).
    pub fn preference(&self, s: Script) -> usize {
        self.script_preference
            .iter()
            .position(|&p| p == s)
            .unwrap_or(self.script_preference.len())
    }
}

/// Every calibrated profile.
pub static PROFILES: &[&ModelProfile] = &[&FLASH_V2_5];

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
    script_preference: &[
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
    ],
    reader: ReaderParams {
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
    },
    // v6: deletion and insertion costs fitted by `phono fit` on 408 harness clips (exp1–exp6),
    // ranking pairs of answers to the same phrase (λ 0.01, substitution scales held at their
    // hand-set values). Losing a sound costs more than v1–v5 assumed, most of all an unstressed
    // full vowel (0.45 → 0.94), a singleton consonant (0.7 → 1.23) and an onset (0.9 → 1.22).
    // Held-out phrases: within-phrase ranking 80.0% → 83.1% (Whisper-only truth: 74.9% →
    // 78.8%). Letting the substitution scales move too scored ~86%, mostly by making vowel
    // swaps cheap (3.0 → 1.3); the search avoided vowel swaps so far, so the clips can't say
    // whether it would exploit them, and that fit is not used.
    weights: Weights {
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
    },
    // Fitted with the weights above. The spelling weight was 0.4 in v4 (fit on exp1–exp3).
    predict: Predict {
        nbest: true,
        risk_scale: 1.3,
        spelling: 0.52,
    },
};

/// Flash v2.5's scripts, from exp0–exp4:
/// - Latin, Hangul and Yi behaved; CJK, kana, syllabics, Ethiopic and conjoining jamo produced
///   most misreadings (the risk values).
/// - Ethiopic, Canadian syllabics and conjoining Hangul jamo were misread often (mean outcome
///   0.59, 0.69 and 0.64 across clips vs 0.76–0.80 for Hangul, Latin, Yi), and Runic came out
///   silent or as "Yeah." (exp0 `ᚠ`), so they are off by default.
/// - Devanagari (exp0: `क` → "ka") and Cyrillic are read natively, except the Cyrillic number
///   signs (`҈` → "*100.000*").
fn flash_v2_5_scripts(s: Script) -> ScriptProfile {
    use Script::*;
    use ScriptReading::*;
    let (reading, risk) = match s {
        Latin | Hangul | Yi | CjkCompat | CyrillicNumber => (Transliterated, 0.0),
        HangulCompatJamo | Cherokee => (Transliterated, 0.01),
        CjkUnified | Hiragana | Katakana | HangulJamo => (Transliterated, 0.03),
        Runic | Armenian => (Transliterated, 0.04),
        Ucas | Ethiopic => (Transliterated, 0.05),
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
    fn flash_scripts() {
        let p = &FLASH_V2_5;
        assert_eq!(p.script(Script::Devanagari).reading, ScriptReading::Native);
        assert!(p.script(Script::Hangul).default_on);
        assert!(!p.script(Script::Runic).default_on);
        assert!(p.preference(Script::Hangul) < p.preference(Script::Yi));
        assert!((p.risk("ᐊ가") - 0.05).abs() < 1e-6);
    }
}
