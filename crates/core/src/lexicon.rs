//! English pronunciation dictionary (CMUdict).

use crate::phone::{Phone, parse_pron};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Default)]
pub struct Lexicon {
    /// Lowercase word → pronunciations, most common first (CMUdict order).
    pub words: HashMap<String, Vec<Vec<Phone>>>,
}

impl Lexicon {
    /// Parse `cmudict.dict`: `word P1 P2 …`, variants as `word(2) …`, optional `# comment`.
    pub fn parse_cmudict(text: &str) -> Result<Self, String> {
        let mut lex = Lexicon::default();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (head, pron) = line.split_once(' ').ok_or_else(|| format!("line {}: no pronunciation", n + 1))?;
            let word = head.split('(').next().unwrap_or(head).to_lowercase();
            let phones = parse_pron(pron).ok_or_else(|| format!("line {}: bad phones {pron:?}", n + 1))?;
            lex.words.entry(word).or_default().push(phones);
        }
        Ok(lex)
    }

    pub fn get(&self, word: &str) -> Option<&[Vec<Phone>]> {
        self.words.get(word).map(|v| v.as_slice())
    }

    /// First (most common) pronunciation.
    pub fn first(&self, word: &str) -> Option<&[Phone]> {
        self.words.get(word).and_then(|v| v.first()).map(|p| p.as_slice())
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_variants_and_comments() {
        let lex = Lexicon::parse_cmudict("read R EH1 D\nread(2) R IY1 D\naalen AE1 L AH0 N # place, german\n").unwrap();
        assert_eq!(lex.get("read").unwrap().len(), 2);
        assert_eq!(crate::phone::fmt_phones(lex.first("aalen").unwrap()), "AE1 L AH0 N");
    }
}
