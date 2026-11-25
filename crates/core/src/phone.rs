//! English phones in ARPAbet (the CMUdict symbol set), with vowel stress.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The 39 CMUdict base phones. Index = base id.
pub const ARPABET: [&str; 39] = [
    "AA", "AE", "AH", "AO", "AW", "AY", "B", "CH", "D", "DH", "EH", "ER", "EY", "F", "G", "HH",
    "IH", "IY", "JH", "K", "L", "M", "N", "NG", "OW", "OY", "P", "R", "S", "SH", "T", "TH", "UH",
    "UW", "V", "W", "Y", "Z", "ZH",
];

/// IPA for each base phone, as spelled in Phoible's feature table.
/// AH depends on stress (see [`Phone::ipa`]); ER uses the rhotic schwa Phoible lists.
const IPA: [&str; 39] = [
    "ɑ", "æ", "ʌ", "ɔ", "aʊ", "aɪ", "b", "tʃ", "d", "ð", "ɛ", "ə˞", "eɪ", "f", "ɡ", "h", "ɪ", "i",
    "dʒ", "k", "l", "m", "n", "ŋ", "oʊ", "ɔɪ", "p", "ɹ", "s", "ʃ", "t", "θ", "ʊ", "u", "v", "w",
    "j", "z", "ʒ",
];

const AH: u8 = 2;

/// A phone: base id (0..39) plus stress for vowels.
/// Packed as `base << 2 | s`, where s = 0 for consonants and 1 + stress (0, 1, 2) for vowels.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Phone(u8);

/// Number of distinct packed phone codes (for lookup tables indexed by [`Phone::code`]).
pub const PHONE_CODES: usize = 39 << 2;

impl Phone {
    pub fn consonant(base: u8) -> Self {
        debug_assert!(!is_vowel_base(base));
        Phone(base << 2)
    }

    pub fn vowel(base: u8, stress: u8) -> Self {
        debug_assert!(is_vowel_base(base) && stress <= 2);
        Phone(base << 2 | (stress + 1))
    }

    pub fn from_code(code: u8) -> Self {
        Phone(code)
    }

    pub fn code(self) -> u8 {
        self.0
    }

    pub fn base(self) -> u8 {
        self.0 >> 2
    }

    pub fn is_vowel(self) -> bool {
        self.0 & 3 != 0
    }

    /// CMUdict stress for vowels: 0 unstressed, 1 primary, 2 secondary.
    pub fn stress(self) -> Option<u8> {
        match self.0 & 3 {
            0 => None,
            s => Some(s - 1),
        }
    }

    pub fn is_stressed(self) -> bool {
        matches!(self.stress(), Some(1 | 2))
    }

    /// Same phone with stress collapsed to stressed (1) / unstressed (0). Identity for consonants.
    pub fn collapse_stress(self) -> Self {
        match self.stress() {
            Some(2) => Phone::vowel(self.base(), 1),
            _ => self,
        }
    }

    /// Parse a CMUdict symbol such as `AH0` or `K`.
    pub fn parse(s: &str) -> Option<Self> {
        let (sym, stress) = match s.as_bytes().last()? {
            d @ b'0'..=b'2' => (&s[..s.len() - 1], Some(d - b'0')),
            _ => (s, None),
        };
        let base = ARPABET.iter().position(|&a| a == sym)? as u8;
        match (is_vowel_base(base), stress) {
            (true, Some(st)) => Some(Phone::vowel(base, st)),
            (false, None) => Some(Phone::consonant(base)),
            _ => None,
        }
    }

    /// IPA string as it appears in Phoible's feature table.
    pub fn ipa(self) -> &'static str {
        if self.base() == AH && self.stress() == Some(0) {
            "ə"
        } else {
            IPA[self.base() as usize]
        }
    }

    /// Every valid phone (39 bases, vowels in three stresses).
    pub fn all() -> impl Iterator<Item = Phone> {
        (0..39u8).flat_map(|b| {
            let v: Vec<Phone> = if is_vowel_base(b) {
                (0..3).map(|s| Phone::vowel(b, s)).collect()
            } else {
                vec![Phone::consonant(b)]
            };
            v
        })
    }
}

pub fn is_vowel_base(base: u8) -> bool {
    matches!(
        ARPABET[base as usize].as_bytes()[0],
        b'A' | b'E' | b'I' | b'O' | b'U'
    )
}

impl fmt::Display for Phone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(ARPABET[self.base() as usize])?;
        if let Some(s) = self.stress() {
            write!(f, "{s}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Phone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Parse a space-separated CMUdict pronunciation.
pub fn parse_pron(s: &str) -> Option<Vec<Phone>> {
    s.split_whitespace().map(Phone::parse).collect()
}

/// Format phones as space-separated ARPAbet.
pub fn fmt_phones(phones: &[Phone]) -> String {
    phones
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format phones word by word, as `B AE1 D | D R IY1 M`: a target's words, or a guess's
/// reading (`phono compress`, its `--json` and the wasm binding all use this).
pub fn fmt_word_phones<'a>(words: impl IntoIterator<Item = &'a [Phone]>) -> String {
    words
        .into_iter()
        .map(fmt_phones)
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_phones_by_word() {
        let bad = parse_pron("B AE1 D").unwrap();
        let dream = parse_pron("D R IY1 M").unwrap();
        assert_eq!(fmt_phones(&bad), "B AE1 D");
        assert_eq!(
            fmt_word_phones([bad.as_slice(), dream.as_slice()]),
            "B AE1 D | D R IY1 M"
        );
        assert_eq!(fmt_word_phones([dream.as_slice()]), "D R IY1 M");
        assert_eq!(fmt_word_phones(std::iter::empty::<&[Phone]>()), "");
    }

    #[test]
    fn parse_and_display_roundtrip() {
        for s in ["AH0", "AA1", "ER2", "K", "ZH", "NG"] {
            assert_eq!(Phone::parse(s).unwrap().to_string(), s);
        }
        assert!(Phone::parse("AH").is_none(), "vowels need stress");
        assert!(Phone::parse("K1").is_none(), "consonants take no stress");
        assert!(Phone::parse("XX").is_none());
    }

    #[test]
    fn ipa_depends_on_stress_for_ah() {
        assert_eq!(Phone::parse("AH0").unwrap().ipa(), "ə");
        assert_eq!(Phone::parse("AH1").unwrap().ipa(), "ʌ");
        assert_eq!(Phone::parse("G").unwrap().ipa(), "ɡ");
    }

    #[test]
    fn all_phones() {
        assert_eq!(Phone::all().count(), 15 * 3 + 24);
        assert!(Phone::all().all(|p| (p.code() as usize) < PHONE_CODES));
    }
}
