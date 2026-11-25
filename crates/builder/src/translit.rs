//! The transliteration table the chunk inventory is built from.
//!
//! Source: the `deunicode` crate (BSD-3), over the BMP, plus [`OVERRIDES`] where the TTS has
//! been observed to disagree with it. See docs/PLAN.md, "Transliteration source".

use phono_core::chunks::Translit;
use std::collections::HashMap;

/// Codepoints where we replace `deunicode`'s output. Each entry must be backed by observed TTS
/// behaviour or Unicode data (a harness clip, a Unicode name), never copied from another
/// transliteration library's table.
///
/// All from exp10 (Flash v2.5, wav2vec2 phones, 2 seeds), where `deunicode` and the TTS disagreed:
/// - し シ → `ʃ i` both seeds (deunicode: `si`); ち チ → `tʃ i` both seeds (`ti`); つ ツ → `s`/`ts`
///   then a swallowed vowel, never a bare `t` (`tu`).
/// - 么 → `j aː`, `j ɑ`: Yao, not Me. 什 → `s. i`: Shi, not Shen. 旅 → `l uː`: Lu, not Lv.
///   术 → `dʒ uː`: Zhu, not Shu.
///
/// 价 is left as `deunicode` has it: the seeds split between `dʒ aː` and `dʒ i`.
pub const OVERRIDES: &[(char, &str)] = &[
    ('し', "shi"),
    ('シ', "shi"),
    ('ち', "chi"),
    ('チ', "chi"),
    ('つ', "tsu"),
    ('ツ', "tsu"),
    ('么', "Yao "),
    ('什', "Shi "),
    ('旅', "Lu "),
    ('术', "Zhu "),
];

/// `deunicode`'s output for every BMP codepoint it knows, with [`OVERRIDES`] applied.
pub fn from_deunicode() -> Translit {
    let mut map: HashMap<char, String> = HashMap::new();
    for cp in 0x20u32..0x10000 {
        let Some(c) = char::from_u32(cp) else { continue };
        let Some(t) = deunicode::deunicode_char(c) else { continue };
        if !t.is_empty() && t != "[?]" {
            map.insert(c, t.to_string());
        }
    }
    for &(c, t) in OVERRIDES {
        map.insert(c, t.to_string());
    }
    Translit { map }
}

/// Write `t` as the TSV `Translit::parse_tsv` reads (`HEX<TAB>"json"`), sorted by codepoint.
pub fn to_tsv(t: &Translit) -> String {
    let mut keys: Vec<char> = t.map.keys().copied().collect();
    keys.sort();
    let mut out = String::new();
    for c in keys {
        out.push_str(&format!("{:04X}\t{}\n", c as u32, json_quote(&t.map[&c])));
    }
    out
}

fn json_quote(s: &str) -> String {
    let mut o = String::from('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    o.push_str(&format!("\\u{u:04x}"));
                }
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
