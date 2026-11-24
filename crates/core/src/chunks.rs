//! Transliteration table and the chunk inventory the search tiles with.
//!
//! Reading model T (docs/PLAN.md): the TTS turns each codepoint into a Latin chunk (as Python
//! `unidecode` does) and reads the concatenation as English. A *chunk* is the Latin string of
//! one codepoint; the search picks one representative codepoint per distinct chunk.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Raw transliteration of each codepoint (unidecode output), e.g. `嬶` → `"Kaka "`.
#[derive(Serialize, Deserialize, Default)]
pub struct Translit {
    pub map: HashMap<char, String>,
}

impl Translit {
    /// Parse the TSV written by `phono-harness translit-table`: `HEX<TAB>"json string"`.
    pub fn parse_tsv(text: &str) -> Result<Self, String> {
        let mut map = HashMap::new();
        for (n, line) in text.lines().enumerate() {
            let (hex, json) = line.split_once('\t').ok_or_else(|| format!("line {}: no tab", n + 1))?;
            let cp = u32::from_str_radix(hex, 16).map_err(|e| format!("line {}: {e}", n + 1))?;
            let ch = char::from_u32(cp).ok_or_else(|| format!("line {}: bad codepoint", n + 1))?;
            map.insert(ch, unescape_json(json).ok_or_else(|| format!("line {}: bad string {json}", n + 1))?);
        }
        Ok(Translit { map })
    }

    pub fn get(&self, c: char) -> &str {
        self.map.get(&c).map(|s| s.as_str()).unwrap_or("")
    }

    /// Transliterate a whole string.
    pub fn latin(&self, text: &str) -> String {
        text.chars().map(|c| self.get(c)).collect()
    }
}

fn unescape_json(s: &str) -> Option<String> {
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

/// Script blocks from the community list, in representative-preference order.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Script {
    Latin,
    Hangul,
    Yi,
    CjkCompat,
    Cherokee,
    Ucas,
    Ethiopic,
    HangulCompatJamo,
    HangulJamo,
    Runic,
    Armenian,
    Hiragana,
    Katakana,
    CjkUnified,
    /// U+0483–0489 combining Cyrillic number signs (`҈` → "*100.000*").
    CyrillicNumber,
    /// Read natively on Flash v2.5 (exp0: `क` → "ka"), so not a transliteration chunk.
    Devanagari,
    /// Read natively (Russian is supported), except the number signs above.
    Cyrillic,
    Other,
}

impl Script {
    pub fn of(c: char) -> Script {
        match c as u32 {
            0x20..=0x7E => Script::Latin,
            0x0483..=0x0489 => Script::CyrillicNumber,
            0x0400..=0x04FF => Script::Cyrillic,
            0x0530..=0x058F => Script::Armenian,
            0x0900..=0x097F => Script::Devanagari,
            0x1100..=0x11FF => Script::HangulJamo,
            0x1200..=0x137F => Script::Ethiopic,
            0x13A0..=0x13FF => Script::Cherokee,
            0x1400..=0x167F => Script::Ucas,
            0x16A0..=0x16FF => Script::Runic,
            0x3040..=0x309F => Script::Hiragana,
            0x30A0..=0x30FF => Script::Katakana,
            0x3130..=0x318F => Script::HangulCompatJamo,
            0x3300..=0x33FF => Script::CjkCompat,
            0x4E00..=0x9FFF => Script::CjkUnified,
            0xA000..=0xA48F => Script::Yi,
            0xAC00..=0xD7A3 => Script::Hangul,
            _ => Script::Other,
        }
    }

    /// Penalty per codepoint for how unpredictable this script's reading has been in the
    /// harness (exp0–exp2): Latin, Hangul and Yi behaved; CJK, kana, syllabics, Ethiopic and
    /// conjoining jamo produced most misreadings. Scaled by `Predict::risk_scale`.
    pub fn risk(self) -> f32 {
        match self {
            Script::Latin | Script::Hangul | Script::Yi | Script::CjkCompat | Script::CyrillicNumber => 0.0,
            Script::HangulCompatJamo | Script::Cherokee => 0.01,
            Script::CjkUnified | Script::Hiragana | Script::Katakana | Script::HangulJamo => 0.03,
            Script::Runic | Script::Armenian => 0.04,
            Script::Ucas | Script::Ethiopic => 0.05,
            Script::Devanagari | Script::Cyrillic | Script::Other => 0.1,
        }
    }

    /// Can the search emit characters of this script at all? (Devanagari and Cyrillic are
    /// read natively, not transliterated.)
    pub fn usable(self) -> bool {
        !matches!(self, Script::Devanagari | Script::Cyrillic | Script::Other)
    }

    /// Does the search use this script unless told otherwise? Off by default, on the evidence
    /// of exp0–exp4 (Flash v2.5):
    /// - Ethiopic, Canadian syllabics and conjoining Hangul jamo were misread often (mean
    ///   outcome 0.59, 0.69 and 0.64 across clips vs 0.76–0.80 for Hangul, Latin, Yi).
    /// - Runic came out silent or as "Yeah." (exp0 `ᚠ`).
    pub fn allowed_by_default(self) -> bool {
        self.usable() && !matches!(self, Script::Ethiopic | Script::Ucas | Script::HangulJamo | Script::Runic)
    }
}

/// How a chunk is used by the search.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChunkKind {
    /// Letters a–z and word boundaries (space): tiled letter by letter through the trie.
    Spelling,
    /// Contains digits or symbols read as words (`*100.000*`, `@`): matched by its phones.
    Phonetic,
}

#[derive(Clone, Debug)]
pub struct Chunk {
    pub ch: char,
    /// Normalized Latin: lowercase letters, `' '` for any boundary; or the raw text for phonetic chunks.
    pub latin: String,
    pub script: Script,
    pub kind: ChunkKind,
}

const SYMBOL_WORDS: &[(char, &str)] = &[('@', "at"), ('&', "and"), ('+', "plus"), ('%', "percent")];

/// Word a symbol is read as, if any.
pub fn symbol_word(c: char) -> Option<&'static str> {
    SYMBOL_WORDS.iter().find(|&&(s, _)| s == c).map(|&(_, w)| w)
}

/// Normalize a raw transliteration. Returns None for chunks the search can't use.
fn normalize(raw: &str) -> Option<(String, ChunkKind)> {
    if raw.chars().any(|c| c.is_ascii_digit() || symbol_word(c).is_some()) {
        return Some((raw.to_string(), ChunkKind::Phonetic));
    }
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphabetic() {
            out.push(c.to_ascii_lowercase());
        } else if c == '\'' {
            // Apostrophes don't change the reading.
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    (!out.is_empty()).then_some((out, ChunkKind::Spelling))
}

/// Byte symbols in the trie: a–z and space.
pub const BOUNDARY: u8 = b' ';

#[derive(Default)]
pub struct Trie {
    nodes: Vec<TrieNode>,
}

#[derive(Default)]
struct TrieNode {
    children: Vec<(u8, u32)>,
    chunk: Option<u32>,
}

impl Trie {
    pub const ROOT: u32 = 0;

    pub fn new() -> Self {
        Trie { nodes: vec![TrieNode::default()] }
    }

    pub fn insert(&mut self, symbols: &[u8], chunk: u32) {
        let mut node = Self::ROOT;
        for &s in symbols {
            node = match self.step(node, s) {
                Some(n) => n,
                None => {
                    self.nodes.push(TrieNode::default());
                    let n = (self.nodes.len() - 1) as u32;
                    let kids = &mut self.nodes[node as usize].children;
                    let pos = kids.partition_point(|&(c, _)| c < s);
                    kids.insert(pos, (s, n));
                    n
                }
            };
        }
        self.nodes[node as usize].chunk = Some(chunk);
    }

    pub fn step(&self, node: u32, symbol: u8) -> Option<u32> {
        let kids = &self.nodes[node as usize].children;
        kids.binary_search_by_key(&symbol, |&(c, _)| c).ok().map(|i| kids[i].1)
    }

    /// Chunk that ends exactly at this node, if any.
    pub fn chunk_at(&self, node: u32) -> Option<u32> {
        self.nodes[node as usize].chunk
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// The letters spelled by each node (for debugging).
    pub fn node_strings(&self) -> Vec<String> {
        let mut out = vec![String::new(); self.nodes.len()];
        let mut stack = vec![Self::ROOT];
        while let Some(n) = stack.pop() {
            for &(c, child) in &self.nodes[n as usize].children {
                out[child as usize] = format!("{}{}", out[n as usize], c as char);
                stack.push(child);
            }
        }
        out
    }
}

/// One representative codepoint per distinct chunk, plus a trie over spelling chunks.
pub struct ChunkSet {
    pub chunks: Vec<Chunk>,
    pub trie: Trie,
}

impl ChunkSet {
    /// Chunks from the scripts `allowed` accepts (see [`Script::allowed_by_default`]).
    pub fn new(translit: &Translit, allowed: impl Fn(Script) -> bool) -> Self {
        let mut best: HashMap<String, Chunk> = HashMap::new();
        for (&ch, raw) in &translit.map {
            let script = Script::of(ch);
            if !script.usable() || !allowed(script) || ch.is_ascii_uppercase() {
                continue;
            }
            let Some((latin, kind)) = normalize(raw) else { continue };
            let cand = Chunk { ch, latin: latin.clone(), script, kind };
            // Prefer reliable scripts, then lowercase forms (Armenian `ճ` "ch" over `Ճ` "Ch").
            let caps = |c: char| translit.get(c).chars().any(|x| x.is_ascii_uppercase());
            match best.get(&latin) {
                Some(cur) if (cur.script, caps(cur.ch), cur.ch) <= (script, caps(ch), ch) => {}
                _ => {
                    best.insert(latin, cand);
                }
            }
        }
        let mut chunks: Vec<Chunk> = best.into_values().collect();
        chunks.sort_by(|a, b| a.latin.cmp(&b.latin));
        let mut trie = Trie::new();
        for (i, c) in chunks.iter().enumerate() {
            if c.kind == ChunkKind::Spelling {
                trie.insert(c.latin.as_bytes(), i as u32);
            }
        }
        ChunkSet { chunks, trie }
    }

    pub fn get(&self, id: u32) -> &Chunk {
        &self.chunks[id as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translit(pairs: &[(char, &str)]) -> Translit {
        Translit { map: pairs.iter().map(|&(c, s)| (c, s.to_string())).collect() }
    }

    #[test]
    fn parses_tsv_with_escapes() {
        let t = Translit::parse_tsv("5B36\t\"Kaka \"\n0022\t\"\\\"\"\n0488\t\"*100.000*\"\n").unwrap();
        assert_eq!(t.get('嬶'), "Kaka ");
        assert_eq!(t.get('"'), "\"");
        assert_eq!(t.latin("嬶҈"), "Kaka *100.000*");
    }

    #[test]
    fn picks_representatives_and_builds_trie() {
        let t = translit(&[
            ('a', "a"),
            ('A', "A"),
            ('바', "ba"),
            ('ꀠ', "ba"),
            ('嬶', "Kaka "),
            ('क', "k"),
            ('҈', "*100.000*"),
            (' ', " "),
            (',', ","),
        ]);
        let set = ChunkSet::new(&t, Script::allowed_by_default);
        let by_latin: HashMap<&str, char> = set.chunks.iter().map(|c| (c.latin.as_str(), c.ch)).collect();
        assert_eq!(by_latin["a"], 'a', "lowercase Latin, never uppercase");
        assert_eq!(by_latin["ba"], '바', "Hangul preferred over Yi");
        assert_eq!(by_latin["kaka "], '嬶', "CJK keeps its trailing word boundary");
        assert_eq!(by_latin[" "], ' ', "space beats comma as the boundary chunk");
        assert!(!by_latin.contains_key("k"), "Devanagari is read natively, so excluded");
        assert_eq!(set.chunks.iter().find(|c| c.ch == '҈').unwrap().kind, ChunkKind::Phonetic);

        let walk = |s: &str| s.bytes().try_fold(Trie::ROOT, |n, b| set.trie.step(n, b));
        let ba = walk("ba").unwrap();
        assert_eq!(set.get(set.trie.chunk_at(ba).unwrap()).ch, '바');
        assert!(set.trie.chunk_at(walk("b").unwrap()).is_none());
        assert!(walk("100").is_none(), "phonetic chunks aren't in the trie");
    }
}
