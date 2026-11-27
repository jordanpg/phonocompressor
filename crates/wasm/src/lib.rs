//! Browser binding for phonocompressor (wasm-bindgen).
//!
//! The JavaScript side fetches `phono.bin` and passes its bytes to [`Engine::new`], then calls
//! [`Engine::compress`] per phrase. This crate only checks options and marshals between
//! JavaScript and [`phono_core`], which does the work. The checking and mapping are plain Rust
//! ([`Request`], [`compress`], [`Guess`]) so they have native tests; the `#[wasm_bindgen]` layer
//! only converts types and errors. Build with `mise run //crates/wasm:build`.
//!
//! TypeScript types for the options and results come from `tsify`'s derive (`Ts<T>` at the
//! boundary). Options are read field by field rather than through `Ts::to_rust`:
//! serde-wasm-bindgen ignores unknown keys, so a typo like `maxCodepoint` would be dropped
//! silently, and its type errors don't name the field.

use phono_core::chunks::Scripts;
use phono_core::model::{self, ModelProfile};
use phono_core::phone::fmt_word_phones;
use phono_core::search::{Candidate, SearchConfig, Searcher};
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

/// Codepoint budgets [`Engine::compress`] accepts. Search time grows slowly with the budget
/// (native, "hundred thousand apartments": 3.1 s at 6, 3.8 s at 30).
pub const MAX_CODEPOINTS: RangeInclusive<u32> = 1..=30;

/// Result counts [`Engine::compress`] accepts.
pub const TOP_N: RangeInclusive<u32> = 1..=50;

/// Options for `compress`. Every field is optional; the defaults are `phono compress`'s.
#[derive(Tsify, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CompressOptions {
    /// ElevenLabs model id. Default: `eleven_flash_v2_5`.
    #[tsify(optional)]
    pub model: Option<String>,
    /// Codepoint budget, spaces included: a whole number from 1 to 30. Default: 6.
    #[tsify(optional)]
    pub max_codepoints: Option<f64>,
    /// How many guesses to return at most: a whole number from 1 to 50. Default: 10.
    #[tsify(optional)]
    pub top_n: Option<f64>,
    /// Scripts the search may use, by name (`latin`, `hangul`, `yi`, …). Default: the model's
    /// default scripts.
    #[tsify(optional)]
    pub allowed_scripts: Option<Vec<String>>,
}

/// The option names `compress` accepts: [`CompressOptions`]'s fields in camelCase.
pub const OPTION_NAMES: [&str; 4] = ["model", "maxCodepoints", "topN", "allowedScripts"];

/// Reject option names `compress` doesn't know, so a typo is an error rather than ignored.
pub fn check_option_names<S: AsRef<str>>(names: &[S]) -> Result<(), String> {
    match names.iter().find(|n| !OPTION_NAMES.contains(&n.as_ref())) {
        Some(n) => Err(format!(
            "unknown option `{}`; options are: {}",
            n.as_ref(),
            OPTION_NAMES.join(", ")
        )),
        None => Ok(()),
    }
}

/// One guess: a string to send to the TTS, how it is predicted to read it, and how close that is
/// to the phrase.
#[derive(Tsify, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Guess {
    /// The string to send.
    pub text: String,
    /// Its transliteration, which the TTS reads as English.
    pub latin: String,
    /// The predicted reading in ARPAbet, word by word (`B AE1 D | D R IY1 M`).
    pub phones: String,
    /// Predicted fidelity to the phrase, 0–1. Results are ranked by it.
    pub score: f32,
    /// `score` reaches the model's close-enough cut, measured on its harness clips (0.90 on
    /// Flash v2.5). When no guess is close enough, tell the user no result scores well.
    pub close_enough: bool,
}

impl Guess {
    pub fn new(c: &Candidate, profile: &ModelProfile) -> Self {
        Guess {
            text: c.text.clone(),
            latin: c.latin.clone(),
            phones: fmt_word_phones(c.words.iter().map(|w| w.phones.as_slice())),
            score: c.fidelity,
            close_enough: profile.is_close_enough(c.fidelity),
        }
    }
}

/// A compress call, checked and filled in with the defaults.
pub struct Request {
    pub profile: &'static ModelProfile,
    pub scripts: Scripts,
    pub config: SearchConfig,
}

impl Request {
    /// Check `phrase` and `options`. Script names are checked here; whether the model can use
    /// them is checked when the engine switches to them ([`phono_core::Engine::configure`]).
    pub fn new(phrase: &str, options: CompressOptions) -> Result<Request, String> {
        if phrase.trim().is_empty() {
            return Err("the phrase is empty; type an English phrase to compress".into());
        }
        let profile = match &options.model {
            Some(id) => model::profile(id)?,
            None => model::DEFAULT,
        };
        let scripts = match &options.allowed_scripts {
            Some(names) => Scripts::from_names(names)?,
            None => Scripts::Default,
        };
        let defaults = SearchConfig::default();
        let config = SearchConfig {
            max_codepoints: in_range(
                "maxCodepoints",
                options.max_codepoints,
                MAX_CODEPOINTS,
                defaults.max_codepoints,
            )?,
            top_n: in_range("topN", options.top_n, TOP_N, defaults.top_n)?,
            ..defaults
        };
        Ok(Request {
            profile,
            scripts,
            config,
        })
    }
}

/// A whole number within `range` (JavaScript numbers arrive as `f64`), or `default` if unset.
fn in_range(
    name: &str,
    value: Option<f64>,
    range: RangeInclusive<u32>,
    default: usize,
) -> Result<usize, String> {
    let (lo, hi) = (*range.start(), *range.end());
    match value {
        None => Ok(default),
        Some(v) if v.fract() == 0.0 && v >= f64::from(lo) && v <= f64::from(hi) => Ok(v as usize),
        Some(v) => Err(format!(
            "{name} must be a whole number from {lo} to {hi}, not {v}"
        )),
    }
}

/// `compress` in plain Rust: switch the engine's model and scripts if needed, search, and map
/// the results. On an error the engine keeps its previous model and scripts.
pub fn compress(
    engine: &mut phono_core::Engine,
    phrase: &str,
    options: CompressOptions,
) -> Result<Vec<Guess>, String> {
    let request = Request::new(phrase, options)?;
    engine.configure(request.profile, request.scripts)?;
    if engine.target(phrase).iter().all(Vec::is_empty) {
        return Err(format!(
            "nothing to pronounce in {phrase:?}; type English words or numbers"
        ));
    }
    let results = Searcher::new(engine).search(phrase, &request.config);
    Ok(results
        .iter()
        .map(|c| Guess::new(c, request.profile))
        .collect())
}

/// The phonocompressor engine, decoded once from the data file and reused for every phrase.
#[wasm_bindgen]
pub struct Engine {
    inner: phono_core::Engine,
}

#[wasm_bindgen]
impl Engine {
    /// Decode the data file (`phono.bin`, fetched by the caller). The bytes are copied in, so
    /// the caller can drop its buffer afterwards.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8]) -> Result<Engine, JsError> {
        let inner = phono_core::Engine::from_bytes(data, model::DEFAULT)
            .map_err(|e| JsError::new(&format!("can't load the data file: {e}")))?;
        Ok(Engine { inner })
    }

    /// Short strings the TTS should read as `phrase`, best first. Throws an `Error` saying what
    /// to change when the phrase or an option is invalid.
    pub fn compress(
        &mut self,
        phrase: &str,
        options: Option<Ts<CompressOptions>>,
    ) -> Result<Vec<Ts<Guess>>, JsError> {
        let options = match options {
            Some(o) => options_from_js(&o.js_value()).map_err(|e| JsError::new(&e))?,
            None => CompressOptions::default(),
        };
        let guesses = compress(&mut self.inner, phrase, options).map_err(|e| JsError::new(&e))?;
        guesses
            .iter()
            .map(|g| {
                g.into_ts()
                    .map_err(|e| JsError::new(&format!("can't return a guess: {e}")))
            })
            .collect()
    }
}

/// Read `compress`'s options object: known names only, each field checked for its type.
fn options_from_js(js: &JsValue) -> Result<CompressOptions, String> {
    if js.is_undefined() || js.is_null() {
        return Ok(CompressOptions::default());
    }
    let obj = js
        .dyn_ref::<js_sys::Object>()
        .filter(|_| !js_sys::Array::is_array(js))
        .ok_or("options must be an object such as { topN: 5 }")?;
    let names: Vec<String> = js_sys::Object::keys(obj)
        .iter()
        .filter_map(|k| k.as_string())
        .collect();
    check_option_names(&names)?;
    Ok(CompressOptions {
        model: field(obj, "model", "a model id such as \"eleven_flash_v2_5\"")?,
        max_codepoints: field(obj, "maxCodepoints", "a number")?,
        top_n: field(obj, "topN", "a number")?,
        allowed_scripts: field(
            obj,
            "allowedScripts",
            "an array of script names such as [\"latin\", \"hangul\"]",
        )?,
    })
}

/// One option field: `None` if absent (or `undefined`/`null`), else `what` it must be.
fn field<T: serde::de::DeserializeOwned>(
    obj: &js_sys::Object,
    name: &str,
    what: &str,
) -> Result<Option<T>, String> {
    let v = js_sys::Reflect::get(obj, &JsValue::from_str(name))
        .map_err(|_| format!("can't read option {name}"))?;
    if v.is_undefined() || v.is_null() {
        return Ok(None);
    }
    serde_wasm_bindgen::from_value(v)
        .map(Some)
        .map_err(|_| format!("{name} must be {what}"))
}

/// The binding's crate version and the data file format it reads, e.g. `0.1.0 (data v2)`.
#[wasm_bindgen]
pub fn version() -> String {
    format!(
        "{} (data v{})",
        env!("CARGO_PKG_VERSION"),
        phono_core::data::FORMAT_VERSION
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use phono_core::chunks::{Script, Translit};
    use phono_core::features::FeatureTable;
    use phono_core::phone::parse_pron;
    use phono_core::reading::{Source, Word};

    fn options(json: &str) -> CompressOptions {
        serde_json::from_str(json).unwrap()
    }

    fn err(phrase: &str, json: &str) -> String {
        Request::new(phrase, options(json)).err().unwrap()
    }

    #[test]
    fn defaults_are_the_clis() {
        let r = Request::new("bad dream", CompressOptions::default()).unwrap();
        assert!(std::ptr::eq(r.profile, model::DEFAULT));
        assert_eq!(r.scripts, Scripts::Default);
        assert_eq!(r.config.max_codepoints, 6);
        assert_eq!(r.config.top_n, 10);
        assert_eq!(r.config.beam, SearchConfig::default().beam);
    }

    #[test]
    fn maps_options() {
        let r = Request::new(
            "bad dream",
            options(
                r#"{"model": "eleven_turbo_v2_5", "maxCodepoints": 4, "topN": 3,
                    "allowedScripts": ["latin", "Hangul"]}"#,
            ),
        )
        .unwrap();
        assert!(std::ptr::eq(r.profile, &model::FLASH_V2_5));
        assert_eq!(r.config.max_codepoints, 4);
        assert_eq!(r.config.top_n, 3);
        assert_eq!(
            r.scripts,
            Scripts::Only(vec![Script::Latin, Script::Hangul])
        );
        let r = Request::new("x", options(r#"{"maxCodepoints": 30, "topN": 1}"#)).unwrap();
        assert_eq!((r.config.max_codepoints, r.config.top_n), (30, 1));
    }

    #[test]
    fn rejects_bad_input_saying_what_to_change() {
        assert!(err(" \t", "{}").contains("phrase is empty"));
        for (json, want) in [
            (
                r#"{"maxCodepoints": 0}"#,
                "maxCodepoints must be a whole number from 1 to 30, not 0",
            ),
            (
                r#"{"maxCodepoints": 31}"#,
                "maxCodepoints must be a whole number from 1 to 30, not 31",
            ),
            (r#"{"maxCodepoints": 2.5}"#, "not 2.5"),
            (
                r#"{"topN": 0}"#,
                "topN must be a whole number from 1 to 50, not 0",
            ),
            (r#"{"topN": 51}"#, "not 51"),
            (r#"{"topN": -1}"#, "not -1"),
            (
                r#"{"model": "eleven_v4"}"#,
                "no calibrated profile for model `eleven_v4`; calibrated models: eleven_flash_v2_5",
            ),
            (
                r#"{"allowedScripts": ["hangul", "klingon"]}"#,
                "unknown script `klingon`; known scripts: latin, hangul",
            ),
        ] {
            let e = err("bad dream", json);
            assert!(e.contains(want), "{json}: {e}");
        }
    }

    #[test]
    fn option_names_are_checked() {
        assert_eq!(check_option_names(&OPTION_NAMES), Ok(()));
        let e = check_option_names(&["topN", "maxCodepoint"]).unwrap_err();
        assert_eq!(
            e,
            "unknown option `maxCodepoint`; options are: model, maxCodepoints, topN, allowedScripts"
        );
        // OPTION_NAMES are the struct's serde names: each one fills its field.
        let json = format!(
            r#"{{"{}": "m", "{}": 1, "{}": 2, "{}": ["latin"]}}"#,
            OPTION_NAMES[0], OPTION_NAMES[1], OPTION_NAMES[2], OPTION_NAMES[3]
        );
        let o = options(&json);
        assert!(
            o.model.is_some()
                && o.max_codepoints.is_some()
                && o.top_n.is_some()
                && o.allowed_scripts.is_some()
        );
    }

    /// An engine with Flash's profile, real phone features and a few chunks, but no lexicon or
    /// G2P, so no phrase can be pronounced.
    fn engine() -> phono_core::Engine {
        let tsv = include_str!("../../../phoible/raw-data/FEATURES/phoible-segments-features.tsv");
        let data = phono_core::data::Data {
            version: phono_core::data::FORMAT_VERSION,
            features: FeatureTable::parse_tsv(tsv).unwrap(),
            translit: Translit {
                map: [('a', "a"), ('바', "ba")]
                    .iter()
                    .map(|&(c, s)| (c, s.to_string()))
                    .collect(),
            },
            ..Default::default()
        };
        phono_core::Engine::new(data, model::DEFAULT).unwrap()
    }

    #[test]
    fn compress_reports_errors_and_keeps_the_engine() {
        let mut e = engine();
        let unusable = options(r#"{"allowedScripts": ["devanagari"]}"#);
        let msg = compress(&mut e, "bad dream", unusable).unwrap_err();
        assert!(msg.contains("`devanagari` is read natively"), "{msg}");
        assert_eq!(e.scripts(), &Scripts::Default);

        let msg = compress(&mut e, "!!!", CompressOptions::default()).unwrap_err();
        assert!(msg.contains("nothing to pronounce in \"!!!\""), "{msg}");
    }

    #[test]
    fn guess_from_candidate() {
        let word = |text: &str, pron: &str| Word {
            text: text.into(),
            phones: parse_pron(pron).unwrap(),
            source: Source::Dictionary,
            alternatives: Vec::new(),
        };
        let c = Candidate {
            text: "받 d레암".into(),
            latin: "bad dream".into(),
            words: vec![word("bad", "B AE1 D"), word("dream", "D R IY1 M")],
            fidelity: 0.9341593,
            codepoints: 5,
            gen_cost: 0.0,
        };
        assert_eq!(
            Guess::new(&c, model::DEFAULT),
            Guess {
                text: "받 d레암".into(),
                latin: "bad dream".into(),
                phones: "B AE1 D | D R IY1 M".into(),
                score: 0.9341593,
                close_enough: true,
            }
        );
        let weak = Candidate {
            fidelity: 0.89,
            ..c
        };
        assert!(!Guess::new(&weak, &model::FLASH_V2_5).close_enough);
        assert!(Guess::new(&weak, &model::MULTILINGUAL_V2).close_enough);
    }

    #[test]
    fn version_names_the_data_format() {
        let v = version();
        let data = format!(" (data v{})", phono_core::data::FORMAT_VERSION);
        assert!(v.ends_with(&data), "{v}");
    }
}
