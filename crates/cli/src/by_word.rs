//! `phono calibrate --by-word`: how each word of the predicted reading was heard.
//!
//! The recognized phones of a clip are aligned to the reader's words as one sequence
//! ([`Scorer::heard_words`]), so each word gets the segments heard for it and its own fidelity.
//! Words left to the G2P are also checked against a wide n-best: if the heard segments match a
//! lower-ranked reading better than the best one, the G2P had the TTS's reading but ranked it
//! low; if no reading matches better, the reading is missing from the n-best.
//!
//! The heard span comes from aligning against the best reading, which favours it a little.
//! Recognized vowels are noisy, so small gains from another reading mean little: only a gain of
//! at least [`GAIN`] counts.

use crate::Clip;
use crate::risks::Source as Answer;
use phono_core::Engine;
use phono_core::phone::{Phone, fmt_phones};
use phono_core::reading::Source;
use phono_core::score::{Heard, Scorer, flatten_target};
use std::collections::BTreeMap;
use std::path::Path;

/// Readings checked per G2P word.
pub const WIDE_NBEST: usize = 10;
/// Fidelity gain over the best reading from which another reading counts as heard instead.
pub const GAIN: f32 = 0.1;
/// Letters up to which a G2P word counts as short.
const SHORT: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Dictionary,
    Letters,
    G2pShort,
    G2pLong,
}

impl Kind {
    fn of(source: Source, text: &str) -> Kind {
        match source {
            Source::Dictionary => Kind::Dictionary,
            Source::Letters => Kind::Letters,
            Source::G2p if text.chars().count() <= SHORT => Kind::G2pShort,
            Source::G2p => Kind::G2pLong,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Dictionary => "dictionary",
            Kind::Letters => "letters",
            Kind::G2pShort => "g2p ≤4",
            Kind::G2pLong => "g2p 5+",
        }
    }
}

/// Where the reading that matched the heard segments best sits in the G2P n-best.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    /// The best reading, or none better by [`GAIN`].
    First,
    /// Rank 2–`n` with `n` the reader's n-best: already in the expectation.
    InReader,
    /// Past the reader's n-best but within [`WIDE_NBEST`].
    Wide,
}

#[derive(Default)]
struct Tally {
    n: usize,
    fidelity: f32,
}

#[derive(Default)]
struct WordStats {
    clips: usize,
    fidelity: f32,
    predicted: String,
    /// Better-matching readings by how often they won, with their rank.
    better: BTreeMap<String, (usize, usize)>,
    /// One clip's heard segments and Whisper text (the worst-heard).
    example: (f32, String, String),
}

pub fn report(
    engine: &Engine,
    clips: &[Clip],
    scorer: &Scorer,
    heard: &Heard,
    reader_nbest: usize,
    worst: usize,
    dump: Option<&Path>,
) -> Result<(), String> {
    let space = &engine.features;
    let mut rows = String::from(
        "exp\tcase\tanswer\ttext\twhisper\tword\tkind\treads\theard\tfidelity\tbetter\n",
    );
    let mut by_kind: BTreeMap<(Answer, Kind), Tally> = BTreeMap::new();
    let mut ranks: BTreeMap<(Answer, bool, Rank), usize> = BTreeMap::new();
    let mut words: BTreeMap<String, WordStats> = BTreeMap::new();
    for c in clips {
        let answer = Answer::of_case(&c.exp, &c.case);
        let (_, read) = engine.read(&c.text);
        let segs: Vec<usize> = c
            .phones
            .split_whitespace()
            .flat_map(|t| space.parse_ipa_token(t))
            .collect();
        let phones: Vec<&[Phone]> = read.iter().map(|w| w.phones.as_slice()).collect();
        let heard_words = scorer.heard_words(space, &phones, &segs, heard);
        for (w, hw) in read.iter().zip(&heard_words) {
            let kind = Kind::of(w.source, &w.text);
            let f = hw.fidelity();
            let ipa: Vec<&str> = hw.segments.iter().map(|&g| space.segment_name(g)).collect();
            let ipa = ipa.join(" ");
            let row = |better: &str| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{}\n",
                    c.exp,
                    c.case,
                    answer.name(),
                    c.text,
                    c.whisper,
                    w.text,
                    kind.name(),
                    fmt_phones(&w.phones),
                    ipa,
                    f,
                    better
                )
            };
            let t = by_kind.entry((answer, kind)).or_default();
            t.n += 1;
            t.fidelity += f;
            let control = w.source == Source::Dictionary;
            if w.source != Source::G2p && !control {
                rows += &row("");
                continue;
            }
            let fid = |p: &[Phone]| {
                scorer.fidelity_segments(space, &flatten_target(&[p.to_vec()]), &hw.segments, heard)
            };
            // Dictionary words (the control): their own reading first, then the G2P's others.
            let mut nbest: Vec<Vec<Phone>> = engine
                .g2p
                .pronounce(&w.text, WIDE_NBEST)
                .into_iter()
                .map(|(p, _)| p)
                .collect();
            if control {
                nbest.retain(|p| *p != w.phones);
                nbest.insert(0, w.phones.clone());
                nbest.truncate(WIDE_NBEST);
            }
            let top = nbest.first().map(|p| fid(p)).unwrap_or(0.0);
            let best = nbest
                .iter()
                .enumerate()
                .skip(1)
                .map(|(k, p)| (k, fid(p), p))
                .filter(|&(_, v, _)| v >= top + GAIN)
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
            let rank = match best {
                None => Rank::First,
                Some((k, _, _)) if k < reader_nbest => Rank::InReader,
                Some(_) => Rank::Wide,
            };
            *ranks.entry((answer, control, rank)).or_default() += 1;
            rows += &row(&best
                .map(|(k, _, p)| format!("#{} {}", k + 1, fmt_phones(p)))
                .unwrap_or_default());
            if control {
                continue;
            }
            let s = words.entry(w.text.clone()).or_default();
            s.clips += 1;
            s.fidelity += f;
            s.predicted = fmt_phones(&w.phones);
            if let Some((k, _, p)) = best {
                s.better.entry(fmt_phones(p)).or_insert((0, k + 1)).0 += 1;
            }
            if s.clips == 1 || f < s.example.0 {
                s.example = (f, ipa, c.whisper.clone());
            }
        }
    }

    println!("\nby word: fidelity of each reader word to the phones heard for it");
    println!(
        "  {:<10} {:<11} {:>6} {:>9}",
        "answers", "word", "words", "fidelity"
    );
    for ((a, k), t) in &by_kind {
        println!(
            "  {:<10} {:<11} {:>6} {:>9.3}",
            a.name(),
            k.name(),
            t.n,
            t.fidelity / t.n as f32
        );
    }
    println!(
        "\nG2P words: which of the {WIDE_NBEST}-best readings the heard phones match (better than the best by ≥ {GAIN}; the reader uses {reader_nbest})"
    );
    println!("(control: dictionary words, their own reading first and the G2P's others after it)");
    println!(
        "  {:<10} {:<11} {:>6} {:>9} {:>9} {:>9}",
        "answers", "word", "words", "best", "in reader", "wider"
    );
    for control in [false, true] {
        for a in Answer::ALL {
            let get = |r| ranks.get(&(a, control, r)).copied().unwrap_or(0);
            let (f, i, w) = (get(Rank::First), get(Rank::InReader), get(Rank::Wide));
            let n = f + i + w;
            if n == 0 {
                continue;
            }
            let pct = |v: usize| format!("{:.0}%", 100.0 * v as f32 / n as f32);
            println!(
                "  {:<10} {:<11} {:>6} {:>9} {:>9} {:>9}",
                a.name(),
                if control { "dictionary" } else { "g2p" },
                n,
                pct(f),
                pct(i),
                pct(w)
            );
        }
    }
    let mut list: Vec<(&String, &WordStats)> = words.iter().filter(|(_, s)| s.clips >= 2).collect();
    list.sort_by(|a, b| {
        let m = |s: &WordStats| s.fidelity / s.clips as f32;
        m(a.1).total_cmp(&m(b.1)).then(a.0.cmp(b.0))
    });
    println!("\nworst-heard G2P words (2+ clips):");
    for (w, s) in list.iter().take(worst) {
        let better = s
            .better
            .iter()
            .max_by_key(|(p, (n, _))| (*n, std::cmp::Reverse(p.as_str())))
            .map(|(p, (n, k))| format!("  better: #{k} {p} ×{n}"))
            .unwrap_or_default();
        println!(
            "  {:<9} {:>2} clips  fidelity {:.2}  reads {:<16} heard {:?} whisper {:?}{better}",
            w,
            s.clips,
            s.fidelity / s.clips as f32,
            s.predicted,
            s.example.1,
            s.example.2
        );
    }
    if let Some(path) = dump {
        std::fs::write(path, rows).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}
