//! phono-build: compile Phoible + CMUdict + the transliteration table into one data file.
//!
//! Also trains the G2P model: EM many-to-many alignment of CMUdict letters to phones
//! (Jiampojamarn et al. 2007, simplified), then a Kneser-Ney unit n-gram (phono_core::g2p). 2% of words
//! are held out to report accuracy; they stay in the lexicon, so nothing is lost at runtime.

mod align;

use clap::Parser;
use phono_core::chunks::{ChunkKind, ChunkSet, Translit};
use phono_core::data::{Data, FORMAT_VERSION};
use phono_core::features::{FeatureSpace, FeatureTable, PhoneDistance};
use phono_core::g2p::{G2p, G2pModel};
use phono_core::lexicon::Lexicon;
use phono_core::phone::Phone;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(about = "Build the phonocompressor data file")]
struct Args {
    #[arg(long, default_value = "phoible/raw-data/FEATURES/phoible-segments-features.tsv")]
    phoible: PathBuf,
    #[arg(long, default_value = "cmudict/cmudict.dict")]
    cmudict: PathBuf,
    /// Written by `mise run data:translit` (Python unidecode; not committed, GPL).
    #[arg(long, default_value = "data/generated/translit.tsv")]
    translit: PathBuf,
    #[arg(long, default_value = "data/generated/phono.bin")]
    out: PathBuf,
    /// EM iterations for the letter–phone aligner.
    #[arg(long, default_value_t = 6)]
    em_iters: usize,
    /// N-gram order of the G2P model.
    #[arg(long, default_value_t = phono_core::g2p::DEFAULT_ORDER)]
    g2p_order: usize,
}

fn read(path: &PathBuf) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    let t0 = Instant::now();

    let features = FeatureTable::parse_tsv(&read(&args.phoible)?)?;
    PhoneDistance::new(&FeatureSpace::new(features.clone()))?; // every English phone must resolve
    println!("phoible: {} segments × {} features", features.segments.len(), features.names.len());

    let lexicon = Lexicon::parse_cmudict(&read(&args.cmudict)?)?;
    println!("cmudict: {} words", lexicon.len());

    let translit = Translit::parse_tsv(&read(&args.translit)?)?;
    let chunks = ChunkSet::new(&translit);
    let spelling = chunks.chunks.iter().filter(|c| c.kind == ChunkKind::Spelling).count();
    println!(
        "translit: {} codepoints → {} chunks ({} spelling, {} phonetic), trie {} nodes",
        translit.map.len(),
        chunks.chunks.len(),
        spelling,
        chunks.chunks.len() - spelling,
        chunks.trie.len()
    );

    // Training data: alphabetic words, first (most common) pronunciation, sorted for determinism.
    let mut words: Vec<(&str, &[Phone])> = lexicon
        .words
        .iter()
        .filter(|(w, _)| w.bytes().all(|b| b.is_ascii_lowercase() || b == b'\''))
        .map(|(w, p)| (w.as_str(), p[0].as_slice()))
        .collect();
    words.sort();
    let (held_out, train): (Vec<_>, Vec<_>) = words.iter().enumerate().partition(|(i, _)| i % 50 == 7);
    let train: Vec<(&str, &[Phone])> = train.into_iter().map(|(_, w)| *w).collect();
    let held_out: Vec<(&str, &[Phone])> = held_out.into_iter().map(|(_, w)| *w).collect();

    let t = Instant::now();
    let aligned = align::align(&train, args.em_iters);
    println!("aligned {} / {} training words in {:.1}s", aligned.len(), train.len(), t.elapsed().as_secs_f32());

    let model = G2pModel::train(aligned.iter().map(|w| w.as_slice()), args.g2p_order)?;
    let per_order: Vec<String> = model.grams.iter().map(|g| g.len().to_string()).collect();
    println!(
        "g2p: {} units, order-{} Kneser-Ney, {} n-grams ({} by context length)",
        model.units.len(),
        model.order,
        model.num_ngrams(),
        per_order.join("/")
    );

    let g2p = G2p::new(model);
    evaluate(&g2p, &held_out, &lexicon);
    let model = g2p.into_model();

    let data = Data { version: FORMAT_VERSION, features, lexicon, g2p: model, translit };
    let bytes = data.to_bytes()?;
    if let Some(dir) = args.out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&args.out, &bytes).map_err(|e| format!("{}: {e}", args.out.display()))?;
    println!(
        "wrote {} ({:.1} MB) in {:.1}s",
        args.out.display(),
        bytes.len() as f64 / 1e6,
        t0.elapsed().as_secs_f32()
    );
    Ok(())
}

/// Word accuracy (top-1 matches any dictionary pronunciation) and phone error rate.
fn evaluate(g2p: &G2p, held_out: &[(&str, &[Phone])], lexicon: &Lexicon) {
    let t = Instant::now();
    let (mut correct, mut errors, mut ref_len) = (0usize, 0usize, 0usize);
    for &(word, reference) in held_out {
        let hyp = g2p.pronounce(word, 1).into_iter().next().map(|(p, _)| p).unwrap_or_default();
        if lexicon.get(word).is_some_and(|prons| prons.contains(&hyp)) {
            correct += 1;
        }
        errors += edit_distance(reference, &hyp);
        ref_len += reference.len();
    }
    println!(
        "g2p held-out ({} words): word accuracy {:.1}%, phone error rate {:.1}% ({:.1}s)",
        held_out.len(),
        100.0 * correct as f64 / held_out.len() as f64,
        100.0 * errors as f64 / ref_len as f64,
        t.elapsed().as_secs_f32()
    );
    for w in ["sux", "baddream", "timeloop", "huridakui", "gigamax", "kutabireru", "disis"] {
        let p = g2p.pronounce(w, 1).into_iter().next().map(|(p, _)| p).unwrap_or_default();
        println!("  {w:12} {}", phono_core::phone::fmt_phones(&p));
    }
}

fn edit_distance(a: &[Phone], b: &[Phone]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(x != y)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}
