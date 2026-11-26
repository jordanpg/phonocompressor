//! Candidates that spell the phrase: tile its real spelling with chunks.
//!
//! The phonetic generator in [`crate::search`] spells phone by phone, so it rarely produces a
//! word's actual spelling. But an English TTS reads real spellings most reliably, and the
//! community's best messages tile the English spelling directly: `ꃬꏜ몰ꄂf록` is
//! ve·ry·mol·dy·f·rog, `ㅊ헻ㅋ힘` is c·hec·k·him. For a username, tiling its letters makes the TTS
//! read it exactly as it would read the name itself.
//!
//! This tiles the phrase's letters with chunks within the codepoint budget. A space between
//! words is optional (it costs a codepoint), and a chunk that ends in a word boundary (CJK
//! `嬶` "Kaka ") can end a word and supply that space itself. Up to `max_edits` letter edits are
//! allowed: dropping a target letter (silent or doubled letters: `doug` → `dug`), swapping it
//! for a similar letter (vowels for vowels, c/k/q, s/z, f/v, g/j), or spelling `ks`/`cks`/`cs`
//! as `x` (`chucks` → `chux`). Number tokens are matched by number chunks with the same digits
//! (`100,000` → `҈` "*100.000*"), and the words `at`, `and`, `plus`, `percent` may be spelled
//! by their symbol. The reranker decides which tilings actually sound right.

use crate::chunks::{BOUNDARY, ChunkKind, ChunkSet, Script};
use crate::model::ModelProfile;
use std::collections::HashMap;

/// One piece of the phrase to cover.
enum Seg {
    /// Lowercase letters of a word, and chunks that may stand for the whole word (`@` for "at").
    Letters(Vec<u8>, Vec<u32>),
    /// A number or symbol token: the chunks whose digits/symbols match it.
    Fixed(Vec<u32>),
}

/// Tilings enumerated per tiling kept, before ranking.
const POOL_FACTOR: usize = 25;

/// Letter clusters that `x` spells, at the cost of one edit.
const X_CLUSTERS: [&[u8]; 3] = [b"cks", b"ks", b"cs"];

/// A chunk that covers `len` letters from the current offset with `edits` edits. `spaced`: it
/// also ends with a word boundary, so it closes the word (`kaka ` for "kaka").
struct Found {
    len: usize,
    chunk: u32,
    edits: usize,
    spaced: bool,
}

/// Letters that can stand in for each other at the cost of one edit.
fn similar(a: u8, b: u8) -> bool {
    const CLASSES: [&[u8]; 6] = [b"aeiouy", b"ckq", b"sz", b"fv", b"gj", b"iy"];
    a != b && CLASSES.iter().any(|c| c.contains(&a) && c.contains(&b))
}

fn alnum(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

struct Tiler<'a> {
    chunks: &'a ChunkSet,
    segs: Vec<Seg>,
    max_cp: usize,
    max_edits: usize,
    limit: usize,
    /// (text, edits, letters spelled with boundaries as spaces).
    out: Vec<(String, usize, String)>,
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
                Seg::Letters(l, _) => l.len() - if k == 0 { off.min(l.len()) } else { 0 },
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
            let spelled: String = path
                .iter()
                .map(|&c| self.chunks.get(c).latin.as_str())
                .collect();
            self.out.push((text, edits, spelled.trim_end().to_string()));
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
            Seg::Letters(letters, whole) => {
                if off == letters.len() {
                    return self.boundary(seg + 1, path, edits);
                }
                let (letters, whole) = (letters.clone(), whole.clone());
                if off == 0 && path.len() < self.max_cp {
                    for c in whole {
                        path.push(c);
                        self.boundary(seg + 1, path, edits);
                        path.pop();
                    }
                }
                // Longest chunks first: fewer codepoints.
                let mut found: Vec<Found> = Vec::new();
                self.walk(&letters, off, (0, 0, 0), self.max_edits - edits, &mut found);
                found.sort_by_key(|f| (f.edits, std::cmp::Reverse(f.len), f.spaced));
                for f in found {
                    path.push(f.chunk);
                    if f.spaced {
                        self.go(seg + 1, 0, path, edits + f.edits);
                    } else {
                        self.go(seg, off + f.len, path, edits + f.edits);
                    }
                    path.pop();
                }
                // Drop this letter.
                if edits < self.max_edits {
                    self.go(seg, off + 1, path, edits + 1);
                }
            }
        }
    }

    /// Chunks matching `letters[start..]`, found by walking the trie from `node` with `used`
    /// letters covered and `edits` ≤ `budget` edits so far.
    fn walk(
        &self,
        letters: &[u8],
        start: usize,
        (node, used, edits): (u32, usize, usize),
        budget: usize,
        found: &mut Vec<Found>,
    ) {
        let trie = &self.chunks.trie;
        if used > 0 {
            if let Some(chunk) = trie.chunk_at(node) {
                found.push(Found {
                    len: used,
                    chunk,
                    edits,
                    spaced: false,
                });
            }
            if start + used == letters.len()
                && let Some(chunk) = trie.step(node, BOUNDARY).and_then(|n| trie.chunk_at(n))
            {
                found.push(Found {
                    len: used,
                    chunk,
                    edits,
                    spaced: true,
                });
            }
        }
        let rest = &letters[start + used..];
        let Some(&want) = rest.first() else { return };
        if let Some(next) = trie.step(node, want) {
            self.walk(letters, start, (next, used + 1, edits), budget, found);
        }
        if edits < budget {
            for alt in (b'a'..=b'z').filter(|&alt| similar(want, alt)) {
                if let Some(next) = trie.step(node, alt) {
                    self.walk(letters, start, (next, used + 1, edits + 1), budget, found);
                }
            }
            if let Some(next) = trie.step(node, b'x') {
                for cluster in X_CLUSTERS.iter().filter(|c| rest.starts_with(c)) {
                    self.walk(
                        letters,
                        start,
                        (next, used + cluster.len(), edits + 1),
                        budget,
                        found,
                    );
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
            && let Some(space) = self
                .chunks
                .trie
                .step(0, BOUNDARY)
                .and_then(|n| self.chunks.trie.chunk_at(n))
        {
            path.push(space);
            self.go(seg, 0, path, edits);
            path.pop();
        }
    }
}

/// Tilings of `phrase`'s spelling in ≤ `max_cp` codepoints with ≤ `max_edits` letter edits,
/// fewest edits first (at most `limit`). Returns (text, edits).
pub fn tile(
    chunks: &ChunkSet,
    profile: &ModelProfile,
    phrase: &str,
    max_cp: usize,
    max_edits: usize,
    limit: usize,
) -> Vec<(String, usize)> {
    let mut fixed: HashMap<String, Vec<u32>> = HashMap::new();
    let mut symbols: HashMap<&str, Vec<u32>> = HashMap::new();
    for (i, c) in chunks.chunks.iter().enumerate() {
        if c.kind == ChunkKind::Phonetic {
            fixed.entry(alnum(&c.latin)).or_default().push(i as u32);
            let mut it = c.latin.trim().chars();
            if let (Some(sym), None) = (it.next(), it.next())
                && let Some(word) = profile.reader.symbol_word(sym)
            {
                symbols.entry(word).or_default().push(i as u32);
            }
        }
    }
    let mut segs = Vec::new();
    for token in phrase.split_whitespace() {
        let letters: Vec<u8> = token
            .bytes()
            .filter(u8::is_ascii_alphabetic)
            .map(|b| b.to_ascii_lowercase())
            .collect();
        if !letters.is_empty() {
            let whole = std::str::from_utf8(&letters)
                .ok()
                .and_then(|w| symbols.get(w))
                .cloned()
                .unwrap_or_default();
            segs.push(Seg::Letters(letters, whole));
        } else if alnum(token).is_empty() {
            // Punctuation (`-`, `!`): nothing to say. (v4 matched it with every symbol chunk,
            // so "joe mama - ha ha" came out with an `@` that the TTS read as "at".)
        } else if let Some(options) = fixed.get(&alnum(token)) {
            segs.push(Seg::Fixed(options.clone()));
        } else {
            return Vec::new(); // A number or symbol we have no chunk for.
        }
    }
    if segs.is_empty() {
        return Vec::new();
    }
    let max_letters = chunks
        .chunks
        .iter()
        .map(|c| c.latin.trim().len())
        .max()
        .unwrap_or(1);
    // Enumerate a wide pool, then keep the best `limit`. Depth-first order alone would fill the
    // cap with variants of whatever prefix it tries first (`푼Ճ…` before `푼ch…`).
    let pool = limit.saturating_mul(POOL_FACTOR);
    let mut t = Tiler {
        chunks,
        segs,
        max_cp,
        max_edits,
        limit: pool,
        out: Vec::new(),
        max_letters,
    };
    // Exact tilings first, then one edit, then two: if the pool fills, it holds the closest.
    for budget in 0..=max_edits {
        t.max_edits = budget;
        t.go(0, 0, &mut Vec::new(), 0);
    }
    // Fewest edits, then the most reliable scripts, then the fewest codepoints.
    let risk = |s: &str| {
        s.chars()
            .map(|c| (profile.script(Script::of(c)).risk * 1000.0).round() as u32)
            .sum::<u32>()
    };
    let mut all: Vec<(String, usize, String)> = t.out;
    all.sort_by_cached_key(|(s, e, _)| (*e, risk(s), s.chars().count(), s.clone()));
    // Tilings that spell the same letters read alike: keep the best of each, so the rerank
    // budget goes to distinct spellings rather than re-chunkings of one.
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<(String, usize)> = all
        .into_iter()
        .filter(|(_, _, spelled)| seen.insert(spelled.clone()))
        .map(|(s, e, _)| (s, e))
        .collect();
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunks::Translit;
    use crate::model::FLASH_V2_5;

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
            ('嬶', "Kaka "),
            ('ꍱ', "chux"),
            ('@', "@"),
        ];
        let t = Translit {
            map: pairs.iter().map(|&(c, s)| (c, s.to_string())).collect(),
        };
        ChunkSet::new(&t, &FLASH_V2_5, |s| FLASH_V2_5.script(s).default_on)
    }

    #[test]
    fn tiles_exact_spelling_first() {
        let out = tile(&set(), &FLASH_V2_5, "bad dream", 6, 2, 100);
        assert!(!out.is_empty());
        assert_eq!(out[0].1, 0, "an exact tiling exists");
        let texts: Vec<&str> = out.iter().map(|(t, _)| t.as_str()).collect();
        assert!(
            texts.contains(&"바ddream")
                || texts.contains(&"바d dream")
                || texts.iter().any(|t| t.starts_with('바')),
            "{texts:?}"
        );
        assert!(out.iter().all(|(t, _)| t.chars().count() <= 6));
    }

    #[test]
    fn spaced_chunks_x_and_symbols() {
        let out = tile(&set(), &FLASH_V2_5, "kaka chucks at bad", 6, 1, 1000);
        let texts: Vec<&str> = out.iter().map(|(t, _)| t.as_str()).collect();
        assert!(texts.contains(&"嬶ꍱ@바d"), "{texts:?}");
        assert!(out.iter().any(|(t, e)| t == "嬶ꍱ@바d" && *e == 1));
    }

    #[test]
    fn punctuation_is_skipped() {
        let out = tile(&set(), &FLASH_V2_5, "bad - bad", 6, 0, 100);
        assert!(
            !out.is_empty() && out.iter().all(|(t, _)| !t.contains('@')),
            "{out:?}"
        );
    }

    #[test]
    fn numbers_use_number_chunks() {
        let out = tile(&set(), &FLASH_V2_5, "100,000 bad", 6, 0, 100);
        assert!(out.iter().any(|(t, _)| t.starts_with('҈')), "{out:?}");
    }
}
