//! How an English TTS reads a Latin string (stage 2 of reading model T).
//!
//! Tokens: letter runs are words (dictionary pronunciation if known, else G2P; vowel-less runs
//! like `tm` are spelled out as letters), digit runs are numbers (`100.000` → one hundred
//! thousand), and a few symbols are words (`@` → at). Everything else separates words.

use crate::chunks::symbol_word;
use crate::g2p::G2p;
use crate::lexicon::Lexicon;
use crate::phone::Phone;

/// Where a word's pronunciation came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A dictionary word (CMUdict), including number words and symbols read as words.
    Dictionary,
    /// A vowel-less run spelled out letter by letter (`tm` → T M).
    Letters,
    /// Letter-to-sound for a string that is not a dictionary word.
    G2p,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub text: String,
    pub phones: Vec<Phone>,
    pub source: Source,
    /// Other plausible readings with their probability relative to `phones` (G2P n-best, only
    /// for words not in the dictionary). Used to score how much a candidate depends on the
    /// TTS reading an ambiguous spelling our way (`ge`: G or JH?).
    pub alternatives: Vec<(Vec<Phone>, f32)>,
}

/// How many G2P readings to consider per unknown word.
pub const READING_NBEST: usize = 3;

enum Token {
    Letters(String),
    /// A digit run. `glued`: letters follow with no separator (`3the`), which the TTS's text
    /// normalizer handles unpredictably (see [`GLUED_NUMBER_DROP`]).
    Number { text: String, glued: bool },
    /// A lone `s` right after a number or an initialism (`2s`, `PCs`): a plural ending.
    Plural,
    Symbol(char),
}

/// Split a letter run around capitals the TTS reads as their own word:
/// - a run of two or more capitals inside a mixed-case run (`checkhimPC` → checkhim · PC, as
///   exp1 heard it);
/// - a single capital ending a run, after two or more lowercase letters: it is read as its
///   letter name. `timE` → "Tim A" (exp2), `nexT` → "next to" (exp4), `yurE` → "you're E"
///   (exp6): 6 of 6 clips. After one letter it stays in the word (`nO` → "no", 4 clips).
///
/// A single capital inside a run stays put: exp5 heard `tHe` as "the". Returns
/// (piece, is_capitals) spans.
fn split_caps(run: &[char]) -> Vec<(usize, usize, bool)> {
    let mixed = run.iter().any(|c| c.is_ascii_lowercase()) && run.iter().any(|c| c.is_ascii_uppercase());
    if !mixed {
        return vec![(0, run.len(), false)];
    }
    let mut out = Vec::new();
    let (mut i, mut start) = (0, 0);
    while i < run.len() {
        if run[i].is_ascii_uppercase() {
            let j = (i..run.len()).find(|&k| !run[k].is_ascii_uppercase()).unwrap_or(run.len());
            let trailing_letter = j - i == 1 && j == run.len() && i >= 2;
            if j - i >= 2 || trailing_letter {
                if i > start {
                    out.push((start, i, false));
                }
                out.push((i, j, true));
                start = j;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    if start < run.len() {
        out.push((start, run.len(), false));
    }
    out
}

fn tokenize(latin: &str) -> Vec<Token> {
    // Curly apostrophes read like straight ones (`you’re`).
    let chars: Vec<char> = latin.chars().map(|c| if matches!(c, '\u{2019}' | '\u{2018}') { '\'' } else { c }).collect();
    let mut out = Vec::new();
    let mut i = 0;
    // The previous token was a number or capitals and nothing separates it from here.
    let mut after_count = false;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() || (c == '\'' && i > 0 && chars[i - 1].is_ascii_alphabetic()) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphabetic() || chars[i] == '\'') {
                i += 1;
            }
            for (a, b, caps) in split_caps(&chars[start..i]) {
                let piece: String = chars[start + a..start + b].iter().collect();
                // Apostrophes stay for the dictionary lookup (`you're`); `letters` drops them otherwise.
                let w = piece.trim_end_matches('\'').to_ascii_lowercase();
                if after_count && piece == "s" {
                    out.push(Token::Plural);
                } else if !w.is_empty() {
                    out.push(Token::Letters(w));
                }
                after_count = caps;
            }
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || (matches!(chars[i], '.' | ',') && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())))
            {
                i += 1;
            }
            let plural = chars.get(i) == Some(&'s') && !chars.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic());
            let glued = chars.get(i).is_some_and(|c| c.is_ascii_alphabetic()) && !plural;
            out.push(Token::Number { text: chars[start..i].iter().collect(), glued });
            after_count = true;
        } else {
            if symbol_word(c).is_some() {
                out.push(Token::Symbol(c));
            }
            after_count = false;
            i += 1;
        }
    }
    out
}

const ONES: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve",
    "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
];
const TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

fn below_thousand(n: u64, out: &mut Vec<&'static str>) {
    if n >= 100 {
        out.push(ONES[(n / 100) as usize]);
        out.push("hundred");
    }
    let r = n % 100;
    if r >= 20 {
        out.push(TENS[(r / 10) as usize]);
        if !r.is_multiple_of(10) {
            out.push(ONES[(r % 10) as usize]);
        }
    } else if r > 0 || n == 0 {
        out.push(ONES[r as usize]);
    }
}

/// English words for a non-negative integer.
pub fn number_words(n: u64) -> Vec<&'static str> {
    let mut out = Vec::new();
    if n == 0 {
        out.push("zero");
        return out;
    }
    for (scale, name) in [(1_000_000_000_000, "trillion"), (1_000_000_000, "billion"), (1_000_000, "million"), (1_000, "thousand"), (1, "")] {
        let part = (n / scale) % 1000;
        if part > 0 {
            below_thousand(part, &mut out);
            if !name.is_empty() {
                out.push(name);
            }
        }
    }
    out
}

/// Words for a digit run. Separators followed by exactly three digits group thousands
/// (`100.000`, `100,000`); any other separator is read as "point" with digits spelled out.
fn number_token_words(tok: &str) -> Vec<&'static str> {
    let groups: Vec<&str> = tok.split(['.', ',']).collect();
    let grouped = groups.len() > 1 && groups[1..].iter().all(|g| g.len() == 3);
    if grouped || groups.len() == 1 {
        let digits: String = tok.chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = digits.parse::<u64>() {
            return number_words(n);
        }
    }
    let mut out = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            out.push("point");
        }
        if i == 0 {
            out.extend(g.parse::<u64>().map(number_words).unwrap_or_default());
        } else {
            out.extend(g.chars().map(|d| ONES[d.to_digit(10).unwrap_or(0) as usize]));
        }
    }
    out
}

pub struct Reader<'a> {
    pub lexicon: &'a Lexicon,
    pub g2p: &'a G2p,
}

// Word splitting costs: each piece costs 1; a dictionary piece of only 3 letters costs extra
// (CMUdict is full of short names and acronyms); each letter left to G2P costs SPLIT_G2P_LETTER.
const SPLIT_SHORT_WORD: f32 = 0.5;
const SPLIT_G2P_LETTER: f32 = 0.3;
const SPLIT_LONE_LETTER: f32 = 0.7;
/// A split between a consonant letter and a vowel letter (`mold|ifrog`, `is|epic`) costs as
/// much as an extra piece. Readers carry the consonant over to the vowel (maximal onset): the
/// TTS read "varymoldifrog" as "very moldy frog" (exp1), "thisisepicswang" as "this is Sepik
/// swang" (exp2) and "biepbiepamtruck" as "beep beep'em truck" (exp5). 0.3–1.0 all help
/// offline; 1.0 also keeps `gigamaxdong` from splitting as gig|amax|dong.
const SPLIT_ONSET: f32 = 1.0;

/// How often a number glued to following letters (`3the`, `2eut`) is dropped, relative to being
/// read: exp2–exp5 heard the digit in 5 of 10 such clips. Plural `2s` was read both times.
const GLUED_NUMBER_DROP: f32 = 1.0;

fn has_vowel(w: &str) -> bool {
    w.bytes().any(|b| b"aeiouy".contains(&b))
}

impl Reader<'_> {
    /// G2P reading plus alternatives weighted by probability relative to the best one.
    fn g2p_word_nbest(&self, w: &str) -> Word {
        let nbest = self.g2p.pronounce(w, READING_NBEST);
        let best_lp = nbest.first().map(|(_, lp)| *lp).unwrap_or(0.0);
        let mut it = nbest.into_iter();
        let phones = it.next().map(|(p, _)| p).unwrap_or_default();
        let alternatives = it.map(|(p, lp)| (p, (lp - best_lp).exp() as f32)).collect();
        Word { text: w.to_string(), phones, source: Source::G2p, alternatives }
    }

    /// Split a letter run into dictionary words where that covers it better than reading it
    /// whole. The TTS does this: exp1 heard "iowasux" as "Iowa sucks", "baddream" as "bad dream".
    /// Returns (piece, is_dictionary_word).
    pub fn segment<'w>(&self, w: &'w str) -> Vec<(&'w str, bool)> {
        let n = w.len();
        let bytes = w.as_bytes();
        let mut best: Vec<(f32, usize, bool)> = vec![(f32::INFINITY, 0, false); n + 1];
        best[0].0 = 0.0;
        for i in 0..n {
            if best[i].0.is_infinite() {
                continue;
            }
            let onset = if i > 0 && !b"aeiouy".contains(&bytes[i - 1]) && b"aeiou".contains(&bytes[i]) {
                SPLIT_ONSET
            } else {
                0.0
            };
            for j in i + 1..=n {
                let piece = &w[i..j];
                // A lone leftover letter ("d" + "isis") is rarely how a reader splits a word.
                let lone = if piece.len() == 1 && n > 1 { SPLIT_LONE_LETTER } else { 0.0 };
                let g2p_cost = 1.0 + SPLIT_G2P_LETTER * piece.len() as f32 + lone + onset;
                let dict_cost = (piece.len() >= 3 && has_vowel(piece) && self.lexicon.first(piece).is_some())
                    .then_some(1.0 + if piece.len() == 3 { SPLIT_SHORT_WORD } else { 0.0 } + onset);
                let (cost, dict) = match dict_cost {
                    Some(d) if d <= g2p_cost => (d, true),
                    _ => (g2p_cost, false),
                };
                if best[i].0 + cost < best[j].0 {
                    best[j] = (best[i].0 + cost, i, dict);
                }
            }
        }
        let mut pieces = Vec::new();
        let mut j = n;
        while j > 0 {
            let (_, i, dict) = best[j];
            pieces.push((&w[i..j], dict));
            j = i;
        }
        pieces.reverse();
        pieces
    }

    fn letters(&self, w: &str, out: &mut Vec<Word>) {
        if w.contains('\'') {
            if let Some(p) = self.lexicon.first(w) {
                out.push(Word { text: w.to_string(), phones: p.to_vec(), source: Source::Dictionary, alternatives: vec![] });
                return;
            }
            let bare: String = w.chars().filter(|&c| c != '\'').collect();
            return self.letters(&bare, out);
        }
        if let Some(p) = self.lexicon.first(w) {
            out.push(Word { text: w.to_string(), phones: p.to_vec(), source: Source::Dictionary, alternatives: vec![] });
        } else if !has_vowel(w) {
            // No vowel letters: read as an initialism (`tm` → T M).
            let phones = w
                .chars()
                .flat_map(|c| self.lexicon.first(&c.to_string()).map(|p| p.to_vec()).unwrap_or_default())
                .collect();
            out.push(Word { text: w.to_string(), phones, source: Source::Letters, alternatives: vec![] });
        } else {
            for (piece, dict) in self.segment(w) {
                if dict {
                    let phones = self.lexicon.first(piece).unwrap_or_default().to_vec();
                    out.push(Word { text: piece.to_string(), phones, source: Source::Dictionary, alternatives: vec![] });
                } else {
                    out.push(self.g2p_word_nbest(piece));
                }
            }
        }
    }

    fn word(&self, w: &str) -> Vec<Phone> {
        let mut out = Vec::new();
        self.letters(w, &mut out);
        out.into_iter().flat_map(|w| w.phones).collect()
    }

    /// Read a Latin string as English words.
    pub fn read(&self, latin: &str) -> Vec<Word> {
        let mut out: Vec<Word> = Vec::new();
        for tok in tokenize(latin) {
            match tok {
                Token::Letters(w) => self.letters(&w, &mut out),
                Token::Number { text, glued } => {
                    let words = number_token_words(&text);
                    if glued {
                        // One word, so the n-best expectation can drop it as a whole.
                        let phones = words.iter().flat_map(|w| self.word(w)).collect();
                        let text = words.join(" ");
                        out.push(Word { text, phones, source: Source::Dictionary, alternatives: vec![(vec![], GLUED_NUMBER_DROP)] });
                    } else {
                        for w in words {
                            out.push(Word { text: w.to_string(), phones: self.word(w), source: Source::Dictionary, alternatives: vec![] });
                        }
                    }
                }
                Token::Plural => {
                    let last = out.last().and_then(|w| w.phones.last().copied());
                    out.push(Word { text: "s".into(), phones: plural_ending(last), source: Source::Dictionary, alternatives: vec![] });
                }
                Token::Symbol(c) => {
                    let w = symbol_word(c).unwrap_or_default();
                    out.push(Word { text: w.to_string(), phones: self.word(w), source: Source::Dictionary, alternatives: vec![] });
                }
            }
        }
        out.retain(|w| !w.phones.is_empty());
        out
    }
}

/// English plural ending after a word ending in `last`: /ɪz/ after sibilants, /s/ after other
/// voiceless consonants, /z/ otherwise.
fn plural_ending(last: Option<Phone>) -> Vec<Phone> {
    let p = |s: &str| Phone::parse(s).expect("ARPAbet");
    match last.map(|l| crate::phone::ARPABET[l.base() as usize]) {
        Some("S" | "Z" | "SH" | "ZH" | "CH" | "JH") => vec![p("IH0"), p("Z")],
        Some("P" | "T" | "K" | "F" | "TH") => vec![p("S")],
        _ => vec![p("Z")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(number_words(100_000).join(" "), "one hundred thousand");
        assert_eq!(number_words(21).join(" "), "twenty one");
        assert_eq!(number_words(1_000_005).join(" "), "one million five");
        assert_eq!(number_token_words("100.000").join(" "), "one hundred thousand");
        assert_eq!(number_token_words("3.14").join(" "), "three point one four");
    }

    fn toks(s: &str) -> Vec<String> {
        tokenize(s)
            .into_iter()
            .map(|t| match t {
                Token::Letters(w) => format!("L:{w}"),
                Token::Number { text, glued } => format!("N:{text}{}", if glued { "+" } else { "" }),
                Token::Plural => "P".into(),
                Token::Symbol(c) => format!("S:{c}"),
            })
            .collect()
    }

    #[test]
    fn tokenizes() {
        assert_eq!(toks("*100.000*apartment's @ x-y"), ["N:100.000", "L:apartment's", "S:@", "L:x", "L:y"]);
    }

    #[test]
    fn capitals_and_glued_numbers() {
        assert_eq!(toks("checkhimPC"), ["L:checkhim", "L:pc"]);
        assert_eq!(toks("tHe PCs"), ["L:the", "L:pc", "P"]);
        assert_eq!(toks("CHECK"), ["L:check"]);
        assert_eq!(toks("yurE nexT nO aM"), ["L:yur", "L:e", "L:nex", "L:t", "L:no", "L:am"]);
        assert_eq!(toks("are2s"), ["L:are", "N:2", "P"]);
        assert_eq!(toks("3the 2sa no2"), ["N:3+", "L:the", "N:2+", "L:sa", "L:no", "N:2"]);
    }
}
