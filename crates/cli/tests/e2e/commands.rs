//! `phones`, `read`, `score` and `spell`, the global options, and the errors found after the
//! data file loads.

use crate::common::{compress, phono};
use phono_core::phone::parse_pron;

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn phones_marks_dictionary_words_and_names() {
    let run = phono(&["phones", "zorblax is here"]).ok();
    let rows: Vec<Vec<&str>> = run
        .lines()
        .iter()
        .map(|l| l.split_whitespace().collect())
        .collect();
    let words: Vec<(&str, &str)> = rows.iter().map(|r| (r[0], r[1])).collect();
    assert_eq!(
        words,
        [("zorblax", "name"), ("is", "word"), ("here", "word")]
    );
    for r in &rows {
        assert!(parse_pron(&r[2..].join(" ")).is_some(), "{r:?}");
    }
    phono(&["phones", "is here"])
        .ok()
        .has_line("is               word  IH1 Z");
}

/// `read` and `score` on the strings `compress` returns give the same reading and score as the
/// JSON did: the commands share one reader and one predictor.
#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn read_and_score_agree_with_compress() {
    let phrase = "hundred thousand apartments";
    for g in compress(phrase, &["--top", "3"]) {
        let read = phono(&["read", &g.text]).ok();
        read.has_line(&format!("latin: {:?}", g.latin));
        let reads = read.line("reads: ").expect("a reads line");
        let phones: Vec<&str> = reads["reads: ".len()..]
            .trim_end_matches(']')
            .split("] ")
            .map(|w| w.split_once('[').expect("word[phones]").1)
            .collect();
        assert_eq!(phones.join(" | "), g.phones, "{reads}");

        let score = phono(&["score", &g.text, phrase]).ok();
        score.has_line(&format!("latin:    {:?}", g.latin));
        score.has_line(&format!("predict:  {:.3} …", g.score));
        score.has_line(
            "target:   HH AH1 N D R AH0 D | TH AW1 Z AH0 N D | AH0 P AA1 R T M AH0 N T S",
        );
        score.has_line("fidelity: …").has_line("align:    …");
    }
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn plain_english_scores_perfectly_against_itself() {
    let run = phono(&["score", "bad dream", "bad dream"]).ok();
    run.has_line("latin:    \"bad dream\"")
        .has_line("reads:    bad[B AE1 D] dream[D R IY1 M]")
        .has_line("fidelity: 1.000 (cost 0.00 / …")
        .has_line("align:    B AE1 D D R IY1 M");
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn spell_lists_spellings_cheapest_first() {
    let run = phono(&["spell", "B", "-n", "3"]).ok();
    let rows: Vec<(&str, f32)> = run
        .lines()
        .iter()
        .map(|l| {
            let (letters, cost) = l.split_once(' ').unwrap();
            (letters, cost.trim().parse().unwrap())
        })
        .collect();
    assert_eq!(rows.len(), 3, "{}", run.stdout);
    assert_eq!(rows[0].0, "b");
    assert!(rows.windows(2).all(|w| w[0].1 <= w[1].1), "{rows:?}");
    // The README's example.
    assert!(phono(&["spell", "K S"]).ok().stdout.starts_with("x "));
    // An empty string asks for silent letters.
    assert!(!phono(&["spell", ""]).ok().stdout.is_empty());
    // Spellings exist only for single G2P units, and the command says nothing otherwise.
    assert_eq!(phono(&["spell", "B AE1"]).ok().stdout, "");
    phono(&["spell", "XX"]).fails(1, &["bad ARPAbet"]);
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn g2p_options_are_accepted() {
    phono(&[
        "--g2p-nbest",
        "1",
        "--g2p-temperature",
        "2",
        "--g2p-merge-stress",
        "false",
        "phones",
        "zorblax",
    ])
    .ok()
    .has_line("zorblax …");
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn bad_options_found_after_loading_are_errors() {
    let known = "known scripts: latin, hangul, yi";
    phono(&["--scripts", "klingon", "phones", "x"]).fails(1, &["unknown script `klingon`", known]);
    phono(&["--scripts", "devanagari", "phones", "x"]).fails(
        1,
        &[
            "script `devanagari` is read natively by eleven_flash_v2_5",
            "scripts it can use: latin, hangul",
        ],
    );
    phono(&["--model", "eleven_v3", "phones", "x"]).fails(
        1,
        &[
            "no calibrated profile for model `eleven_v3`",
            "eleven_flash_v2_5",
            "eleven_multilingual_v2",
        ],
    );
    phono(&["--g2p-nbest", "0", "phones", "x"]).fails(1, &["--g2p-nbest must be at least 1"]);
    phono(&["--g2p-temperature", "0", "phones", "x"])
        .fails(1, &["--g2p-temperature must be above 0"]);
}
