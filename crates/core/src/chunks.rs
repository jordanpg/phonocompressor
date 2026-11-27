//! Transliteration table and the chunk inventory the search tiles with.
//!
//! Reading model T (docs/PLAN.md): the TTS turns each codepoint into a Latin chunk (as Python
//! `unidecode` and Rust `deunicode` do) and reads the concatenation as English. A *chunk* is the Latin string of
//! one codepoint; the search picks one representative codepoint per distinct chunk.

use crate::model::{ModelProfile, ScriptReading};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Raw transliteration of each codepoint (the builder's table, from `deunicode`), e.g. `嬶` → `"Kaka "`.
#[derive(Serialize, Deserialize, Default)]
pub struct Translit {
    pub map: HashMap<char, String>,
}

impl Translit {
    /// Parse a TSV table: `HEX<TAB>"json string"` (what `phono-build --dump-translit` writes).
    pub fn parse_tsv(text: &str) -> Result<Self, String> {
        let mut map = HashMap::new();
        for (n, line) in text.lines().enumerate() {
            let (hex, json) = line
                .split_once('\t')
                .ok_or_else(|| format!("line {}: no tab", n + 1))?;
            let cp = u32::from_str_radix(hex, 16).map_err(|e| format!("line {}: {e}", n + 1))?;
            let ch = char::from_u32(cp).ok_or_else(|| format!("line {}: bad codepoint", n + 1))?;
            map.insert(
                ch,
                unescape_json(json).ok_or_else(|| format!("line {}: bad string {json}", n + 1))?,
            );
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

/// Script blocks from the community list. How each is read depends on the TTS model
/// ([`crate::model::ModelProfile::scripts`]).
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
    Devanagari,
    Cyrillic,
    Other,
}

impl Script {
    /// How many scripts there are.
    pub const COUNT: usize = 18;

    /// Every script, in declaration order.
    pub const ALL: [Script; Script::COUNT] = [
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

    /// Position in [`Script::ALL`], for per-script tables.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The script's name in options and messages (`hangul`, `cjk-compat`).
    pub fn name(self) -> &'static str {
        match self {
            Script::Latin => "latin",
            Script::Hangul => "hangul",
            Script::Yi => "yi",
            Script::CjkCompat => "cjk-compat",
            Script::Cherokee => "cherokee",
            Script::Ucas => "ucas",
            Script::Ethiopic => "ethiopic",
            Script::HangulCompatJamo => "hangul-compat-jamo",
            Script::HangulJamo => "hangul-jamo",
            Script::Runic => "runic",
            Script::Armenian => "armenian",
            Script::Hiragana => "hiragana",
            Script::Katakana => "katakana",
            Script::CjkUnified => "cjk-unified",
            Script::CyrillicNumber => "cyrillic-number",
            Script::Devanagari => "devanagari",
            Script::Cyrillic => "cyrillic",
            Script::Other => "other",
        }
    }

    /// The script called `name` ([`Script::name`], any case).
    pub fn from_name(name: &str) -> Result<Script, String> {
        Script::ALL
            .into_iter()
            .find(|s| s.name().eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| {
                format!(
                    "unknown script `{name}`; known scripts: {}",
                    names(Script::ALL)
                )
            })
    }

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
}

fn names(scripts: impl IntoIterator<Item = Script>) -> String {
    scripts
        .into_iter()
        .map(Script::name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Which scripts the search may use. Only scripts the model reads by transliteration
/// ([`ScriptReading::Transliterated`]) are ever searched.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Scripts {
    /// The model's defaults ([`crate::model::ScriptProfile::default_on`]).
    #[default]
    Default,
    /// Every script the model reads by transliteration, including the ones off by default.
    All,
    /// Exactly these. Naming one the model doesn't read by transliteration is an error.
    Only(Vec<Script>),
}

impl Scripts {
    /// [`Scripts::Only`] from script names ([`Script::name`]).
    pub fn from_names<S: AsRef<str>>(names: &[S]) -> Result<Scripts, String> {
        let scripts = names
            .iter()
            .map(|n| Script::from_name(n.as_ref()))
            .collect::<Result<_, _>>()?;
        Ok(Scripts::Only(scripts))
    }

    /// The same selection with [`Scripts::Only`] sorted and deduplicated, so equal sets compare
    /// equal.
    pub fn normalized(self) -> Scripts {
        match self {
            Scripts::Only(mut v) => {
                v.sort();
                v.dedup();
                Scripts::Only(v)
            }
            other => other,
        }
    }

    /// Whether the search may use `s` under `profile`, before [`ChunkSet::new`]'s own
    /// transliteration check.
    fn allows(&self, profile: &ModelProfile, s: Script) -> bool {
        match self {
            Scripts::Default => profile.script(s).default_on,
            Scripts::All => true,
            Scripts::Only(v) => v.contains(&s),
        }
    }

    /// Reject an empty list, or a script `profile`'s model doesn't read by transliteration.
    fn check(&self, profile: &ModelProfile) -> Result<(), String> {
        let Scripts::Only(v) = self else {
            return Ok(());
        };
        let usable = || {
            names(
                Script::ALL
                    .into_iter()
                    .filter(|&s| profile.script(s).reading == ScriptReading::Transliterated),
            )
        };
        if v.is_empty() {
            return Err(format!(
                "no scripts given; name at least one of: {}",
                usable()
            ));
        }
        for &s in v {
            let why = match profile.script(s).reading {
                ScriptReading::Transliterated => continue,
                ScriptReading::Native => "is read natively by",
                ScriptReading::Unmeasured => "has not been measured on",
            };
            return Err(format!(
                "script `{}` {why} {}, so the search can't use it; scripts it can use: {}",
                s.name(),
                profile.id(),
                usable()
            ));
        }
        Ok(())
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

/// Symbols some model reads as a word. A chunk containing one is read through the reader (see
/// `normalize`); whether a model actually voices it is [`crate::reading::ReaderParams::symbol_words`].
pub const SYMBOL_WORDS: &[(char, &str)] =
    &[('@', "at"), ('&', "and"), ('+', "plus"), ('%', "percent")];

/// Word a symbol is read as by some model, if any.
pub fn symbol_word(c: char) -> Option<&'static str> {
    SYMBOL_WORDS.iter().find(|&&(s, _)| s == c).map(|&(_, w)| w)
}

/// Normalize a raw transliteration. Returns None for chunks the search can't use.
fn normalize(raw: &str) -> Option<(String, ChunkKind)> {
    if raw
        .chars()
        .any(|c| c.is_ascii_digit() || symbol_word(c).is_some())
    {
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
        Trie {
            nodes: vec![TrieNode::default()],
        }
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
        kids.binary_search_by_key(&symbol, |&(c, _)| c)
            .ok()
            .map(|i| kids[i].1)
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
    /// Chunks from the scripts `scripts` selects under `profile`. Errors on an empty
    /// [`Scripts::Only`] or a script the model doesn't read by transliteration.
    pub fn for_scripts(
        translit: &Translit,
        profile: &ModelProfile,
        scripts: &Scripts,
    ) -> Result<Self, String> {
        scripts.check(profile)?;
        Ok(Self::new(translit, profile, |s| scripts.allows(profile, s)))
    }

    /// Chunks from the scripts `profile` reads by transliteration and `allowed` accepts (for
    /// the model's defaults, `|s| profile.script(s).default_on`).
    pub fn new(
        translit: &Translit,
        profile: &ModelProfile,
        allowed: impl Fn(Script) -> bool,
    ) -> Self {
        let mut best: HashMap<String, Chunk> = HashMap::new();
        for (&ch, raw) in &translit.map {
            let script = Script::of(ch);
            if profile.script(script).reading != ScriptReading::Transliterated
                || !allowed(script)
                || ch.is_ascii_uppercase()
            {
                continue;
            }
            let Some((latin, kind)) = normalize(raw) else {
                continue;
            };
            let cand = Chunk {
                ch,
                latin: latin.clone(),
                script,
                kind,
            };
            // Prefer the model's preferred scripts, then lowercase forms (Armenian `ճ` "ch" over `Ճ` "Ch").
            let caps = |c: char| translit.get(c).chars().any(|x| x.is_ascii_uppercase());
            let rank = |s: Script| profile.preference(s);
            match best.get(&latin) {
                Some(cur)
                    if (rank(cur.script), caps(cur.ch), cur.ch) <= (rank(script), caps(ch), ch) => {
                }
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
        Translit {
            map: pairs.iter().map(|&(c, s)| (c, s.to_string())).collect(),
        }
    }

    #[test]
    fn parses_tsv_with_escapes() {
        let t =
            Translit::parse_tsv("5B36\t\"Kaka \"\n0022\t\"\\\"\"\n0488\t\"*100.000*\"\n").unwrap();
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
        let p = &crate::model::FLASH_V2_5;
        let set = ChunkSet::new(&t, p, |s| p.script(s).default_on);
        let by_latin: HashMap<&str, char> = set
            .chunks
            .iter()
            .map(|c| (c.latin.as_str(), c.ch))
            .collect();
        assert_eq!(by_latin["a"], 'a', "lowercase Latin, never uppercase");
        assert_eq!(by_latin["ba"], '바', "Hangul preferred over Yi");
        assert_eq!(
            by_latin["kaka "], '嬶',
            "CJK keeps its trailing word boundary"
        );
        assert_eq!(
            by_latin[" "], ' ',
            "space beats comma as the boundary chunk"
        );
        assert!(
            !by_latin.contains_key("k"),
            "Devanagari is read natively, so excluded"
        );
        assert_eq!(
            set.chunks.iter().find(|c| c.ch == '҈').unwrap().kind,
            ChunkKind::Phonetic
        );

        let walk = |s: &str| s.bytes().try_fold(Trie::ROOT, |n, b| set.trie.step(n, b));
        let ba = walk("ba").unwrap();
        assert_eq!(set.get(set.trie.chunk_at(ba).unwrap()).ch, '바');
        assert!(set.trie.chunk_at(walk("b").unwrap()).is_none());
        assert!(walk("100").is_none(), "phonetic chunks aren't in the trie");
    }

    #[test]
    fn script_names_round_trip() {
        for s in Script::ALL {
            assert_eq!(Script::from_name(s.name()), Ok(s));
        }
        assert_eq!(Script::from_name("Hangul"), Ok(Script::Hangul));
        assert_eq!(Script::from_name("cjk-compat"), Ok(Script::CjkCompat));
        let mut names: Vec<&str> = Script::ALL.iter().map(|s| s.name()).collect();
        names.dedup();
        assert_eq!(names.len(), Script::ALL.len(), "names are unique");
    }

    #[test]
    fn unknown_script_name_lists_valid_names() {
        let err = Script::from_name("klingon").unwrap_err();
        assert!(err.contains("unknown script `klingon`"), "{err}");
        assert!(
            err.contains("hangul") && err.contains("cjk-compat"),
            "{err}"
        );
        let err = Scripts::from_names(&["hangul", "klingon"]).unwrap_err();
        assert!(err.contains("`klingon`"), "{err}");
    }

    fn fixture() -> Translit {
        translit(&[
            ('a', "a"),
            (' ', " "),
            ('바', "ba"),
            ('ꀠ', "ba"),
            ('ꀊ', "a"),
            ('ሀ', "ha"),
            ('क', "k"),
        ])
    }

    fn scripts_of(set: &ChunkSet) -> Vec<Script> {
        let mut s: Vec<Script> = set.chunks.iter().map(|c| c.script).collect();
        s.sort();
        s.dedup();
        s
    }

    #[test]
    fn keeps_only_allowed_scripts() {
        let p = &crate::model::FLASH_V2_5;
        let t = fixture();

        let only_yi = ChunkSet::for_scripts(&t, p, &Scripts::from_names(&["yi"]).unwrap()).unwrap();
        assert_eq!(scripts_of(&only_yi), [Script::Yi]);
        let by_latin: HashMap<&str, char> = only_yi
            .chunks
            .iter()
            .map(|c| (c.latin.as_str(), c.ch))
            .collect();
        assert_eq!(
            by_latin["ba"], 'ꀠ',
            "Yi represents `ba` once Hangul is out"
        );

        let default = ChunkSet::for_scripts(&t, p, &Scripts::Default).unwrap();
        assert_eq!(
            scripts_of(&default),
            [Script::Latin, Script::Hangul],
            "Ethiopic is off by default; Yi's `a` and `ba` lose to Latin and Hangul"
        );
        let all = ChunkSet::for_scripts(&t, p, &Scripts::All).unwrap();
        assert!(scripts_of(&all).contains(&Script::Ethiopic));
        assert!(!scripts_of(&all).contains(&Script::Devanagari));
    }

    #[test]
    fn rejects_scripts_the_model_cannot_use() {
        let p = &crate::model::FLASH_V2_5;
        let t = fixture();
        let err = |names: &[&str]| {
            ChunkSet::for_scripts(&t, p, &Scripts::from_names(names).unwrap())
                .err()
                .unwrap()
        };
        let native = err(&["hangul", "devanagari"]);
        assert!(
            native.contains("`devanagari`") && native.contains("natively"),
            "{native}"
        );
        assert!(
            native.contains("hangul") && !native.contains("cyrillic,"),
            "{native}"
        );
        let unmeasured = err(&["other"]);
        assert!(unmeasured.contains("not been measured"), "{unmeasured}");
        let none = err(&[]);
        assert!(none.contains("no scripts"), "{none}");
    }
}
