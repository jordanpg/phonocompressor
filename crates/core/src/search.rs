//! Find ≤ N codepoints whose TTS reading sounds like a target phrase.
//!
//! Two stages:
//! 1. **Generate.** A beam search over target phone positions. Each step either spells the next
//!    1–2 target phones with English spelling options from the G2P units (`K` → c, k, ck;
//!    `K S` → x), walking those letters through the chunk trie, where a chunk can close and a
//!    new one start between any two letters; or drops a target phone at its scorer cost; or
//!    matches a whole phonetic chunk (`҈` = "one hundred thousand", `㌀` = "apartment") against a
//!    span of target phones. Spaces are optional chunks, free at word boundaries.
//! 2. **Re-rank.** Every finished candidate is re-read the way the TTS would read it
//!    ([`crate::reading`]: unidecode, then English) and scored against the target
//!    ([`crate::score`]). Generation costs only decide what gets re-read.

use crate::Engine;
use crate::chunks::{BOUNDARY, ChunkKind, ChunkSet, Trie};
use crate::phone::{PHONE_CODES, Phone};
use crate::reading::Word;
use crate::score::{Scorer, TargetPhone};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct SearchConfig {
    /// Codepoint budget, spaces included (Twitch counts codepoints).
    pub max_codepoints: usize,
    pub top_n: usize,
    /// Entries kept per target position.
    pub beam: usize,
    /// Entries kept per (trie node, codepoints used, flags) at a position, inside a chunk.
    pub per_state: usize,
    /// Same, at chunk boundaries (trie root). Every path that just closed a chunk lands in the
    /// same few root states, so a small cap here would collapse the candidate set to a handful.
    pub per_root: usize,
    /// Spelling options tried per phone sequence.
    pub spellings_per_phone: usize,
    /// Finished candidates re-read and scored.
    pub rerank: usize,
    /// Candidates that tile the phrase's real spelling ([`crate::tile`]), added to the rerank,
    /// and the letter edits they may make.
    pub spelled: usize,
    pub spelled_edits: usize,
    /// Ranking cost per codepoint, so near-ties go to the shorter answer. Predictions are
    /// nowhere near precise to 0.005, and filler letters the scorer finds almost free (`e` in
    /// `gigamaxedong`) are still heard (exp3/exp5: "gigamaxidong"). Not part of `fidelity`.
    pub codepoint_cost: f32,
    /// How candidates are scored; `None` uses the engine's model profile.
    pub predict: Option<crate::Predict>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            max_codepoints: 6,
            top_n: 10,
            beam: 20000,
            per_state: 2,
            per_root: 16,
            spellings_per_phone: 10,
            rerank: 600,
            spelled: 400,
            spelled_edits: 2,
            codepoint_cost: 0.005,
            predict: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub text: String,
    pub latin: String,
    pub words: Vec<Word>,
    pub fidelity: f32,
    pub codepoints: usize,
    pub gen_cost: f32,
}

// Generation cost weights (relative to scorer deletion costs of ~0.35–1.0).
const SPELL_W: f32 = 0.25;
const CLOSE_COST: f32 = 0.03;
const SPACE_MID_WORD: f32 = 0.4;
const SILENT_COST: f32 = 0.3;
const NEAR_MAX_COST: f32 = 0.4;
const NEAR_PER_PHONE: usize = 4;

const FLAG_SPACE: u8 = 1; // last emitted symbol was a space
const FLAG_SILENT: u8 = 2; // a silent-letter unit was used at this position

struct PhoneticChunk {
    chunk: u32,
    phones: Vec<Phone>,
}

/// Precomputed search tables for an engine. Build once, reuse across queries.
pub struct Searcher<'a> {
    engine: &'a Engine,
    scorer: Scorer<'a>,
    near: Vec<Vec<(Phone, f32)>>,
    phonetic: Vec<PhoneticChunk>,
    space_chunk: Option<u32>,
}

#[derive(Clone, Copy)]
struct Entry {
    cost: f32,
    node: u32,
    k: u8,
    flags: u8,
    path: u32,
}

const NO_PATH: u32 = u32::MAX;

/// Arena of closed chunks: each step points at its predecessor.
#[derive(Default)]
struct Paths {
    steps: Vec<(u32, u32)>, // (parent, chunk)
}

impl Paths {
    fn push(&mut self, parent: u32, chunk: u32) -> u32 {
        self.steps.push((parent, chunk));
        (self.steps.len() - 1) as u32
    }

    /// Last two letters of the closed chunks on this path: the orthographic context English
    /// reading depends on (`to|c…` vs `toe|c…`), which the trie node alone doesn't capture.
    fn context(&self, chunks: &ChunkSet, mut at: u32) -> u16 {
        let mut letters = [0u8; 2];
        let mut n = 0;
        while at != NO_PATH && n < 2 {
            let (parent, chunk) = self.steps[at as usize];
            for &b in chunks.get(chunk).latin.as_bytes().iter().rev() {
                if n < 2 {
                    letters[n] = b;
                    n += 1;
                }
            }
            at = parent;
        }
        u16::from(letters[0]) | u16::from(letters[1]) << 8
    }

    fn chunks(&self, mut at: u32) -> Vec<u32> {
        let mut out = Vec::new();
        while at != NO_PATH {
            let (parent, chunk) = self.steps[at as usize];
            out.push(chunk);
            at = parent;
        }
        out.reverse();
        out
    }
}

/// Entries at one target position, at most `per_state` per (node, k, flags, context).
struct Layer {
    states: HashMap<(u32, u8, u8, u16), Vec<Entry>>,
}

impl Layer {
    fn new() -> Self {
        Layer {
            states: HashMap::new(),
        }
    }

    /// Insert if it ranks among the best for its state. Returns whether it was kept.
    fn offer(&mut self, e: Entry, cfg: &SearchConfig, paths: &Paths, chunks: &ChunkSet) -> bool {
        let cap = if e.node == Trie::ROOT {
            cfg.per_root
        } else {
            cfg.per_state
        };
        let ctx = paths.context(chunks, e.path);
        let v = self.states.entry((e.node, e.k, e.flags, ctx)).or_default();
        if v.len() >= cap {
            let worst = v
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.cost.total_cmp(&b.1.cost))
                .map(|(i, w)| (i, w.cost));
            match worst {
                Some((i, c)) if e.cost < c => v[i] = e,
                _ => return false,
            }
        } else {
            v.push(e);
        }
        true
    }

    fn drain_sorted(&mut self, beam: usize) -> Vec<Entry> {
        let mut all: Vec<Entry> = self.states.drain().flat_map(|(_, v)| v).collect();
        // Full tie-break: HashMap order must not decide what survives the beam.
        all.sort_by(|a, b| {
            a.cost
                .total_cmp(&b.cost)
                .then_with(|| (a.node, a.k, a.flags, a.path).cmp(&(b.node, b.k, b.flags, b.path)))
        });
        all.truncate(beam);
        all
    }
}

impl<'a> Searcher<'a> {
    pub fn new(engine: &'a Engine) -> Self {
        let scorer = engine.scorer();
        let mut near = vec![Vec::new(); PHONE_CODES];
        for p in Phone::all() {
            let mut v: Vec<(Phone, f32)> = Phone::all()
                .filter(|q| q.base() != p.base())
                .map(|q| (q, scorer.sub_cost(p, q)))
                .filter(|&(_, c)| c <= NEAR_MAX_COST)
                .collect();
            v.sort_by(|a, b| a.1.total_cmp(&b.1));
            // One entry per base: spellings ignore stress anyway.
            v.dedup_by_key(|(q, _)| q.base());
            v.truncate(NEAR_PER_PHONE);
            near[p.code() as usize] = v;
        }

        let reader = engine.reader();
        let mut phonetic = Vec::new();
        let mut space_chunk = None;
        for (id, c) in engine.chunks.chunks.iter().enumerate() {
            let word = c.latin.trim();
            let use_phones = match c.kind {
                ChunkKind::Phonetic => true,
                // Long whole words (㌀ apartment, ㌖ kilometer) are hard to reach letter by letter.
                ChunkKind::Spelling => {
                    word.len() >= 6 && !word.contains(' ') && engine.lexicon.first(word).is_some()
                }
            };
            if use_phones {
                let phones: Vec<Phone> = reader
                    .read(word)
                    .into_iter()
                    .flat_map(|w| w.phones)
                    .collect();
                if !phones.is_empty() {
                    phonetic.push(PhoneticChunk {
                        chunk: id as u32,
                        phones,
                    });
                }
            }
            if c.latin == " " {
                space_chunk = Some(id as u32);
            }
        }
        Searcher {
            engine,
            scorer,
            near,
            phonetic,
            space_chunk,
        }
    }

    pub fn phonetic_chunks(&self) -> usize {
        self.phonetic.len()
    }

    fn trie(&self) -> &Trie {
        &self.engine.chunks.trie
    }

    /// Walk `letters` from (node, k), optionally closing the current chunk between letters.
    #[allow(clippy::too_many_arguments)]
    fn walk(
        &self,
        paths: &mut Paths,
        max: usize,
        node: u32,
        k: u8,
        path: u32,
        cost: f32,
        letters: &[u8],
        out: &mut Vec<(u32, u8, u32, f32)>,
    ) {
        let Some((&sym, rest)) = letters.split_first() else {
            out.push((node, k, path, cost));
            return;
        };
        let trie = self.trie();
        if let Some(next) = trie.step(node, sym)
            && (k as usize) < max
        {
            self.walk(paths, max, next, k, path, cost, rest, out);
        }
        if node != Trie::ROOT
            && let Some(chunk) = trie.chunk_at(node)
            && (k as usize) + 2 <= max
            && let Some(next) = trie.step(Trie::ROOT, sym)
        {
            let p = paths.push(path, chunk);
            self.walk(paths, max, next, k + 1, p, cost + CLOSE_COST, rest, out);
        }
    }

    /// Find the best-sounding codepoint strings for a phrase.
    pub fn search(&self, phrase: &str, cfg: &SearchConfig) -> Vec<Candidate> {
        let target = self.engine.target_of(phrase);
        let generated = self.generate(&target.phones, cfg);
        let spelled = crate::tile::tile(
            &self.engine.chunks,
            self.engine.profile,
            phrase,
            cfg.max_codepoints,
            cfg.spelled_edits,
            cfg.spelled,
        );
        // Phonetic candidates first, then spelled ones not already among them.
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let all = generated
            .into_iter()
            .take(cfg.rerank)
            .chain(spelled.into_iter().map(|(t, e)| (t, 1e6 + e as f32)))
            .filter(|(t, _)| seen.insert(t.clone()));
        self.rerank(&target, all, cfg)
    }

    /// Debugging aid: generation rank (0-based) of the candidate whose transliteration matches
    /// `text`'s, and the number of distinct candidates generated.
    pub fn generation_rank(
        &self,
        phrase: &str,
        text: &str,
        cfg: &SearchConfig,
    ) -> (Option<usize>, usize) {
        let target = self.engine.target_of(phrase);
        let want = self.engine.translit.latin(text).to_lowercase();
        let generated = self.generate_traced(&target.phones, cfg, Some(&want));
        let rank = generated
            .iter()
            .position(|(t, _)| self.engine.translit.latin(t).to_lowercase() == want);
        (rank, generated.len())
    }

    /// Stage 1: distinct candidate strings (one per transliteration, fewest codepoints), cheapest
    /// generation cost first.
    fn generate(&self, target: &[TargetPhone], cfg: &SearchConfig) -> Vec<(String, f32)> {
        self.generate_traced(target, cfg, None)
    }

    fn generate_traced(
        &self,
        target: &[TargetPhone],
        cfg: &SearchConfig,
        trace: Option<&str>,
    ) -> Vec<(String, f32)> {
        let node_strings = trace.map(|_| self.trie().node_strings());
        let n = target.len();
        if n == 0 {
            return Vec::new();
        }
        let max = cfg.max_codepoints;
        let g2p = &self.engine.g2p;
        let chunks = &self.engine.chunks;
        let mut paths = Paths::default();
        let mut layers: Vec<Layer> = (0..=n).map(|_| Layer::new()).collect();
        layers[0].offer(
            Entry {
                cost: 0.0,
                node: Trie::ROOT,
                k: 0,
                flags: 0,
                path: NO_PATH,
            },
            cfg,
            &paths,
            chunks,
        );
        let mut goals: Vec<Entry> = Vec::new();
        let mut walked = Vec::new();
        let phonetic_matches = self.phonetic_matches(target);

        for i in 0..=n {
            // Non-advancing moves: close the open chunk, emit a space, emit silent letters.
            let mut work = layers[i].drain_sorted(cfg.beam);
            let mut layer = Layer::new();
            for e in &work {
                layer.offer(*e, cfg, &paths, chunks);
            }
            // At the last position nothing follows, so state merging would only discard finished
            // candidates that differ from cheaper ones in ways only re-ranking can judge.
            if i == n {
                goals.extend(
                    work.iter()
                        .filter(|e| e.node == Trie::ROOT && e.k > 0)
                        .copied(),
                );
            }
            while let Some(e) = work.pop() {
                let mut next = Vec::new();
                if e.node != Trie::ROOT
                    && let Some(chunk) = self.trie().chunk_at(e.node)
                {
                    let ends_space = self.engine.chunks.get(chunk).latin.ends_with(' ');
                    let flags = if ends_space {
                        e.flags | FLAG_SPACE
                    } else {
                        e.flags & !FLAG_SPACE
                    };
                    let path = paths.push(e.path, chunk);
                    next.push(Entry {
                        cost: e.cost + CLOSE_COST,
                        node: Trie::ROOT,
                        k: e.k + 1,
                        flags,
                        path,
                    });
                }
                let at_boundary = i > 0 && i < n && target[i].word_start;
                if e.flags & FLAG_SPACE == 0 && i > 0 && i < n {
                    let space = if at_boundary { 0.0 } else { SPACE_MID_WORD };
                    walked.clear();
                    self.walk(
                        &mut paths,
                        max,
                        e.node,
                        e.k,
                        e.path,
                        e.cost + space,
                        &[BOUNDARY],
                        &mut walked,
                    );
                    for &(node, k, path, cost) in &walked {
                        next.push(Entry {
                            cost,
                            node,
                            k,
                            flags: e.flags | FLAG_SPACE,
                            path,
                        });
                    }
                }
                if e.flags & FLAG_SILENT == 0 {
                    for s in g2p.spellings(&[]).iter().take(3) {
                        walked.clear();
                        let cost = e.cost + s.cost * SPELL_W + SILENT_COST;
                        self.walk(
                            &mut paths,
                            max,
                            e.node,
                            e.k,
                            e.path,
                            cost,
                            s.letters.as_bytes(),
                            &mut walked,
                        );
                        for &(node, k, path, cost) in &walked {
                            next.push(Entry {
                                cost,
                                node,
                                k,
                                flags: (e.flags | FLAG_SILENT) & !FLAG_SPACE,
                                path,
                            });
                        }
                    }
                }
                for ne in next {
                    if (ne.k as usize) + usize::from(ne.node != Trie::ROOT) > max {
                        continue;
                    }
                    if i == n && ne.node == Trie::ROOT {
                        goals.push(ne);
                    }
                    if layer.offer(ne, cfg, &paths, chunks) {
                        work.push(ne);
                    }
                }
            }
            let entries = layer.drain_sorted(cfg.beam);
            if let (Some(want), Some(ns)) = (trace, &node_strings) {
                let mut best: Option<(String, f32, u8)> = None;
                for e in &entries {
                    let closed: String = paths
                        .chunks(e.path)
                        .iter()
                        .map(|&c| chunks.get(c).latin.to_lowercase())
                        .collect();
                    let s = closed + &ns[e.node as usize];
                    if want.starts_with(&s) && best.as_ref().is_none_or(|b| s.len() > b.0.len()) {
                        best = Some((s, e.cost, e.k));
                    }
                }
                let worst = entries.last().map(|e| e.cost).unwrap_or(0.0);
                eprintln!(
                    "layer {i:2} ({} entries, worst cost {worst:.2}): longest wanted prefix {best:?}",
                    entries.len()
                );
            }
            if i == n {
                break;
            }

            // Advancing moves.
            let t = target[i];
            for e in entries {
                let flags = e.flags & !(FLAG_SPACE | FLAG_SILENT);
                // Drop the target phone.
                let del = Entry {
                    cost: e.cost + self.scorer.del_cost(&t),
                    flags,
                    ..e
                };
                layers[i + 1].offer(del, cfg, &paths, chunks);
                // Spell the next one or two phones (or a near neighbour of a single phone).
                for len in 1..=g2p.max_phones_per_unit().min(n - i) {
                    let phones: Vec<Phone> = target[i..i + len].iter().map(|t| t.phone).collect();
                    let mut options: Vec<(Vec<Phone>, f32)> = vec![(phones.clone(), 0.0)];
                    if len == 1 {
                        options.extend(
                            self.near[t.phone.code() as usize]
                                .iter()
                                .map(|&(q, c)| (vec![q], c)),
                        );
                    }
                    for (ph, extra) in options {
                        for s in g2p.spellings(&ph).iter().take(cfg.spellings_per_phone) {
                            walked.clear();
                            let cost = e.cost + extra + s.cost * SPELL_W;
                            self.walk(
                                &mut paths,
                                max,
                                e.node,
                                e.k,
                                e.path,
                                cost,
                                s.letters.as_bytes(),
                                &mut walked,
                            );
                            for &(node, k, path, cost) in &walked {
                                layers[i + len].offer(
                                    Entry {
                                        cost,
                                        node,
                                        k,
                                        flags,
                                        path,
                                    },
                                    cfg,
                                    &paths,
                                    chunks,
                                );
                            }
                        }
                    }
                }
                // Whole phonetic chunks.
                if e.node == Trie::ROOT && (e.k as usize) < max {
                    for &(j, chunk, c) in &phonetic_matches[i] {
                        let path = paths.push(e.path, chunk);
                        let ne = Entry {
                            cost: e.cost + c,
                            node: Trie::ROOT,
                            k: e.k + 1,
                            flags,
                            path,
                        };
                        layers[j].offer(ne, cfg, &paths, chunks);
                    }
                }
            }
        }

        goals.sort_by(|a, b| a.cost.total_cmp(&b.cost).then_with(|| a.path.cmp(&b.path)));
        // Many tilings spell the same Latin string; they read identically, so keep the one with
        // the fewest codepoints and spend the re-ranking budget on distinct spellings.
        let mut by_latin: HashMap<String, usize> = HashMap::new();
        let mut out: Vec<(String, f32)> = Vec::new();
        for g in goals {
            let mut chunks = paths.chunks(g.path);
            // A trailing space is wasted budget.
            while chunks.last().is_some_and(|&c| Some(c) == self.space_chunk) {
                chunks.pop();
            }
            let text: String = chunks
                .iter()
                .map(|&c| self.engine.chunks.get(c).ch)
                .collect();
            if text.is_empty() {
                continue;
            }
            let key = self.engine.translit.latin(&text).to_lowercase();
            match by_latin.get(&key) {
                Some(&i) if out[i].0.chars().count() > text.chars().count() => out[i].0 = text,
                Some(_) => {}
                None => {
                    by_latin.insert(key, out.len());
                    out.push((text, g.cost));
                }
            }
        }
        out
    }

    /// For each start position: (end position, chunk, cost) of phonetic chunks that fit the span
    /// at less than half the cost of deleting it.
    fn phonetic_matches(&self, target: &[TargetPhone]) -> Vec<Vec<(usize, u32, f32)>> {
        let n = target.len();
        (0..n)
            .map(|i| {
                let mut v = Vec::new();
                for pc in &self.phonetic {
                    let lo = i + pc.phones.len().saturating_sub(2).max(1);
                    let hi = (i + pc.phones.len() + 2).min(n);
                    for j in lo..=hi {
                        let span = &target[i..j];
                        let c = self.scorer.cost(span, &pc.phones);
                        let budget: f32 = span.iter().map(|t| self.scorer.del_cost(t)).sum();
                        if c < 0.5 * budget {
                            v.push((j, pc.chunk, c));
                        }
                    }
                }
                v
            })
            .collect()
    }

    /// Stage 2: read each candidate the way the TTS would and score it against the target.
    fn rerank(
        &self,
        target: &crate::Target,
        generated: impl Iterator<Item = (String, f32)>,
        cfg: &SearchConfig,
    ) -> Vec<Candidate> {
        let predict = cfg.predict.unwrap_or(self.engine.profile.predict);
        let mut out: Vec<Candidate> = Vec::new();
        for (text, gen_cost) in generated {
            let (latin, words) = self.engine.read(&text);
            let fidelity = self.engine.predict_read(&text, &words, target, &predict);
            out.push(Candidate {
                codepoints: text.chars().count(),
                text,
                latin,
                words,
                fidelity,
                gen_cost,
            });
        }
        let rank = |c: &Candidate| c.fidelity - cfg.codepoint_cost * c.codepoints as f32;
        out.sort_by(|a, b| {
            rank(b)
                .total_cmp(&rank(a))
                .then(a.codepoints.cmp(&b.codepoints))
                .then(a.gen_cost.total_cmp(&b.gen_cost))
                .then_with(|| a.text.cmp(&b.text))
        });
        // Strings with the same transliteration read identically; keep the shortest.
        let mut readings = std::collections::HashSet::new();
        out.retain(|c| readings.insert(c.latin.to_lowercase()));
        out.truncate(cfg.top_n);
        out
    }
}
