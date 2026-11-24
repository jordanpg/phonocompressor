//! Candidates that spell the phrase: tile its real spelling with chunks.
//!
//! The phonetic generator in [`crate::search`] spells phone by phone, so it rarely produces a
//! word's actual spelling. But an English TTS reads real spellings most reliably, and the
//! community's best messages tile the English spelling directly: `ꃬꏜ몰ꄂf록` is
//! ve·ry·mol·dy·f·rog, `ㅊ헻ㅋ힘` is c·hec·k·him. For a username, tiling its letters makes the TTS
//! read it exactly as it would read the name itself.
//!
//! This tiles the phrase's letters with chunks within the codepoint budget. A space between
//! words is optional (it costs a codepoint). Up to `max_edits` letter edits are allowed:
//! dropping a target letter (silent or doubled letters: `doug` → `dug`) or swapping it for a
//! similar letter (vowels for vowels, c/k/q, s/z, f/v, g/j). Number tokens are matched by
//! number chunks with the same digits (`100,000` → `҈` "*100.000*"). The reranker decides
//! which tilings actually sound right.

use crate::chunks::{BOUNDARY, ChunkKind, ChunkSet};
use std::collections::HashMap;

/// One piece of the phrase to cover.
enum Seg {
    /// Lowercase letters of a word.
    Letters(Vec<u8>),
    /// A number or symbol token: the chunks whose digits/symbols match it.
    Fixed(Vec<u32>),
}

/// Letters that can stand in for each other at the cost of one edit.
fn similar(a: u8, b: u8) -> bool {
    const CLASSES: [&[u8]; 6] = [b"aeiouy", b"ckq", b"sz", b"fv", b"gj", b"iy"];
    a != b && CLASSES.iter().any(|c| c.contains(&a) && c.contains(&b))
}

fn alnum(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

struct Tiler<'a> {
    chunks: &'a ChunkSet,
    segs: Vec<Seg>,
    max_cp: usize,
    max_edits: usize,
    limit: usize,
    out: Vec<(String, usize)>,
    /// Longest chunk, in letters (for pruning).
    max_letters: usize,
}

impl Tiler<'_> {
    /// Letters left to cover from (segment, offset); a number token counts as one.
    fn remaining(&self, seg: usize, off: usize) -> usize {
        self.segs[seg..]
            .iter()
            .enumerate()
            .map(|(k, s)| match s {
                Seg::Letters(l) => l.len() - if k == 0 { off.min(l.len()) } else { 0 },
                Seg::Fixed(_) => 1,
            })
            .sum()
    }

    fn go(&mut self, seg: usize, off: usize, path: &mut Vec<u32>, edits: usize) {
        if self.out.len() >= self.limit {
            return;
        }
        if seg == self.segs.len() {
            let text: String = path.iter().map(|&c| self.chunks.get(c).ch).collect();
            self.out.push((text, edits));
            return;
        }
        let left = self.max_cp - path.len();
        let need = self.remaining(seg, off);
        if need > 0 && (left == 0 || need > left * self.max_letters) {
            return;
        }
        match &self.segs[seg] {
            Seg::Fixed(options) => {
                for c in options.clone() {
                    if path.len() < self.max_cp {
                        path.push(c);
                        self.boundary(seg + 1, path, edits);
                        path.pop();
                    }
                }
            }
            Seg::Letters(letters) => {
                if off == letters.len() {
                    return self.boundary(seg + 1, path, edits);
                }
                let letters = letters.clone();
                // Longest chunks first: fewer codepoints.
                let mut found: Vec<(usize, u32, usize)> = Vec::new();
                self.walk(&letters, off, (0, 0, 0), self.max_edits - edits, &mut found);
                found.sort_by_key(|&(len, _, e)| (e, std::cmp::Reverse(len)));
                for (len, chunk, e) in found {
                    path.push(chunk);
                    self.go(seg, off + len, path, edits + e);
                    path.pop();
                }
                // Drop this letter.
                if edits < self.max_edits {
                    self.go(seg, off + 1, path, edits + 1);
                }
            }
        }
    }

    /// Chunks matching `letters[start..]`, found by walking the trie from `node` (`depth`
    /// letters in), as (letters covered, chunk, similar-letter swaps ≤ `budget`).
    fn walk(&self, letters: &[u8], start: usize, (node, depth, swaps): (u32, usize, usize), budget: usize, found: &mut Vec<(usize, u32, usize)>) {
        if depth > 0
            && let Some(c) = self.chunks.trie.chunk_at(node)
        {
            found.push((depth, c, swaps));
        }
        let Some(&want) = letters.get(start + depth) else { return };
        if let Some(next) = self.chunks.trie.step(node, want) {
            self.walk(letters, start, (next, depth + 1, swaps), budget, found);
        }
        if swaps < budget {
            for alt in (b'a'..=b'z').filter(|&alt| similar(want, alt)) {
                if let Some(next) = self.chunks.trie.step(node, alt) {
                    self.walk(letters, start, (next, depth + 1, swaps + 1), budget, found);
                }
            }
        }
    }

    /// Between words: run on, or spend a codepoint on a space.
    fn boundary(&mut self, seg: usize, path: &mut Vec<u32>, edits: usize) {
        self.go(seg, 0, path, edits);
        if seg < self.segs.len()
            && seg > 0
            && path.len() < self.max_cp
            && let Some(space) = self.chunks.trie.step(0, BOUNDARY).and_then(|n| self.chunks.trie.chunk_at(n))
        {
            path.push(space);
            self.go(seg, 0, path, edits);
            path.pop();
        }
    }
}

/// Tilings of `phrase`'s spelling in ≤ `max_cp` codepoints with ≤ `max_edits` letter edits,
/// fewest edits first (at most `limit`). Returns (text, edits).
pub fn tile(chunks: &ChunkSet, phrase: &str, max_cp: usize, max_edits: usize, limit: usize) -> Vec<(String, usize)> {
    let mut fixed: HashMap<String, Vec<u32>> = HashMap::new();
    for (i, c) in chunks.chunks.iter().enumerate() {
        if c.kind == ChunkKind::Phonetic {
            fixed.entry(alnum(&c.latin)).or_default().push(i as u32);
        }
    }
    let mut segs = Vec::new();
    for token in phrase.split_whitespace() {
        let letters: Vec<u8> = token.bytes().filter(u8::is_ascii_alphabetic).map(|b| b.to_ascii_lowercase()).collect();
        if !letters.is_empty() {
            segs.push(Seg::Letters(letters));
        } else if let Some(options) = fixed.get(&alnum(token)) {
            segs.push(Seg::Fixed(options.clone()));
        } else if !alnum(token).is_empty() {
            return Vec::new(); // A number or symbol we have no chunk for.
        }
    }
    if segs.is_empty() {
        return Vec::new();
    }
    let max_letters = chunks.chunks.iter().map(|c| c.latin.trim().len()).max().unwrap_or(1);
    let mut t = Tiler { chunks, segs, max_cp, max_edits, limit, out: Vec::new(), max_letters };
    // Exact tilings first, then one edit, then two: the cap keeps the closest spellings.
    for budget in 0..=max_edits {
        t.max_edits = budget;
        t.go(0, 0, &mut Vec::new(), 0);
    }
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<(String, usize)> = t.out.into_iter().filter(|(s, _)| seen.insert(s.clone())).collect();
    out.sort_by_key(|(s, e)| (*e, s.chars().count()));
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunks::{Script, Translit};

    fn set() -> ChunkSet {
        let pairs = [
            ('b', "b"),
            ('a', "a"),
            ('d', "d"),
            ('r', "r"),
            ('e', "e"),
            ('m', "m"),
            ('바', "ba"),
            ('드', "deu"),
            ('림', "rim"),
            ('ꀊ', "a"),
            ('ꏒ', "re"),
            ('҈', "*100.000*"),
            (' ', " "),
        ];
        let t = Translit { map: pairs.iter().map(|&(c, s)| (c, s.to_string())).collect() };
        ChunkSet::new(&t, Script::allowed_by_default)
    }

    #[test]
    fn tiles_exact_spelling_first() {
        let out = tile(&set(), "bad dream", 6, 2, 100);
        assert!(!out.is_empty());
        assert_eq!(out[0].1, 0, "an exact tiling exists");
        let texts: Vec<&str> = out.iter().map(|(t, _)| t.as_str()).collect();
        assert!(texts.contains(&"바ddream") || texts.contains(&"바d dream") || texts.iter().any(|t| t.starts_with('바')), "{texts:?}");
        assert!(out.iter().all(|(t, _)| t.chars().count() <= 6));
    }

    #[test]
    fn numbers_use_number_chunks() {
        let out = tile(&set(), "100,000 bad", 6, 0, 100);
        assert!(out.iter().any(|(t, _)| t.starts_with('҈')), "{out:?}");
    }
}
