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
    Number(String),
    Symbol(char),
}

fn tokenize(latin: &str) -> Vec<Token> {
    let chars: Vec<char> = latin.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() || (c == '\'' && i > 0 && chars[i - 1].is_ascii_alphabetic()) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphabetic() || chars[i] == '\'') {
                i += 1;
            }
            // Apostrophes stay for the dictionary lookup (`you're`); `letters` drops them otherwise.
            let w: String = chars[start..i].iter().collect::<String>().trim_end_matches('\'').to_string();
            out.push(Token::Letters(w.to_ascii_lowercase()));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || (matches!(chars[i], '.' | ',') && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())))
            {
                i += 1;
            }
            out.push(Token::Number(chars[start..i].iter().collect()));
        } else {
            if symbol_word(c).is_some() {
                out.push(Token::Symbol(c));
            }
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
        let mut best: Vec<(f32, usize, bool)> = vec![(f32::INFINITY, 0, false); n + 1];
        best[0].0 = 0.0;
        for i in 0..n {
            if best[i].0.is_infinite() {
                continue;
            }
            for j in i + 1..=n {
                let piece = &w[i..j];
                // A lone leftover letter ("d" + "isis") is rarely how a reader splits a word.
                let lone = if piece.len() == 1 && n > 1 { SPLIT_LONE_LETTER } else { 0.0 };
                let g2p_cost = 1.0 + SPLIT_G2P_LETTER * piece.len() as f32 + lone;
                let dict_cost = (piece.len() >= 3 && has_vowel(piece) && self.lexicon.first(piece).is_some())
                    .then_some(1.0 + if piece.len() == 3 { SPLIT_SHORT_WORD } else { 0.0 });
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
        let mut out = Vec::new();
        for tok in tokenize(latin) {
            match tok {
                Token::Letters(w) => self.letters(&w, &mut out),
                Token::Number(n) => {
                    for w in number_token_words(&n) {
                        out.push(Word { text: w.to_string(), phones: self.word(w), source: Source::Dictionary, alternatives: vec![] });
                    }
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

    #[test]
    fn tokenizes() {
        let toks: Vec<String> = tokenize("*100.000*apartment's @ x-y")
            .into_iter()
            .map(|t| match t {
                Token::Letters(w) => format!("L:{w}"),
                Token::Number(n) => format!("N:{n}"),
                Token::Symbol(c) => format!("S:{c}"),
            })
            .collect();
        assert_eq!(toks, ["N:100.000", "L:apartment's", "S:@", "L:x", "L:y"]);
    }
}
