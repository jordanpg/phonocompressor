//! `calibrate`, `fit` and `eval` on small made-up fixtures (`tests/fixtures/`): they run, print
//! their sections and write their files. The numbers mean nothing at this size.

use crate::common::{fixture, out_dir, phono};
use serde_json::Value;

const DUMP_HEADER: &str =
    "exp\tcase\ttext\ttarget\tpredicted\tactual\twhisper\trisk\twords\tspell\tshare";
const WORD_DUMP_HEADER: &str =
    "exp\tcase\tanswer\ttext\twhisper\tword\tkind\treads\theard\tfidelity\tbetter";

/// Flash clips in `clips.tsv` (one more is a Multilingual v2 clip).
const FLASH_CLIPS: usize = 12;

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn calibrate_reports_and_dumps() {
    let dir = out_dir("calibrate_reports_and_dumps");
    let (dump, words) = (dir.join("dump.tsv"), dir.join("words.tsv"));
    let run = phono(&[
        "calibrate",
        &fixture("clips.tsv"),
        "--by-script",
        "--by-word",
        "--dump",
        &dump.display().to_string(),
        "--word-dump",
        &words.display().to_string(),
    ])
    .ok();
    assert!(
        run.stderr
            .contains("skipped 1 clips from models other than eleven_flash_v2_5"),
        "{}",
        run.stderr
    );
    run.has_line(&format!("{FLASH_CLIPS} clips"))
        .has_line("predicted vs truth (mean of both): pearson …")
        .has_line("within-phrase ranking: …")
        .has_line("close enough: …")
        .has_line("best split (Youden's J): …")
        .has_line("script               clips predicted penalty  truth    gap gap w/o risk")
        .has_line("by word: …")
        .has_line("worst over-predictions (predicted − actual):");
    assert!(run.stdout.contains("← profile"), "{}", run.stdout);

    let dump = read(&dump);
    let rows: Vec<&str> = dump.lines().collect();
    assert_eq!(rows[0], DUMP_HEADER);
    assert_eq!(rows.len(), 1 + FLASH_CLIPS);
    let columns = DUMP_HEADER.split('\t').count();
    assert!(rows.iter().all(|r| r.split('\t').count() == columns));

    let words = read(&words);
    let rows: Vec<&str> = words.lines().collect();
    assert_eq!(rows[0], WORD_DUMP_HEADER);
    assert!(rows.len() > 1 + FLASH_CLIPS, "{words}");
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn calibrate_options() {
    let clips = fixture("clips.tsv");
    phono(&["calibrate", &clips, "--model", "eleven_multilingual_v2"])
        .ok()
        .has_line("1 clips");
    phono(&[
        "calibrate",
        &clips,
        "--hand",
        "--no-nbest",
        "--voicing",
        "1.5",
    ])
    .ok();
    phono(&["calibrate", &clips, "--risks", "katakana=0.01,ucas=0.02"]).ok();
    phono(&["calibrate", &clips, "--fit-risks", "--risk-min-clips", "1"])
        .ok()
        .has_line("script risks fitted on 12 items …")
        .has_line("cjk-compat …");
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn calibrate_errors() {
    let clips = fixture("clips.tsv");
    phono(&["calibrate", &clips, "--risks", "katakana"])
        .fails(1, &["--risks: expected script=value"]);
    phono(&["calibrate", &clips, "--risks", "katakana=x"])
        .fails(1, &["katakana=x", "invalid float literal"]);
    phono(&["calibrate", &clips, "--risks", "klingon=1"]).fails(1, &["unknown script `klingon`"]);

    let dir = out_dir("calibrate_errors");
    let missing = dir.join("clips.tsv").display().to_string();
    phono(&["calibrate", &missing]).fails(1, &[&missing, "mise run //tools:export"]);
    let header_only = dir.join("header.tsv");
    std::fs::write(
        &header_only,
        "exp\tcase\tseed\tmodel\ttext\ttarget\twhisper_en\tphones\n",
    )
    .unwrap();
    phono(&["calibrate", &header_only.display().to_string()])
        .fails(1, &["no clips from eleven_flash_v2_5"]);
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn fit_cross_validates_and_prints_weights() {
    let clips = fixture("clips.tsv");
    let args = [
        "fit",
        &clips,
        "--folds",
        "2",
        "--repeats",
        "1",
        "--lambdas",
        "0",
    ];
    let run = phono(&args).ok();
    run.has_line("parameter           hand  fitted  default")
        .has_line("current defaults (in-sample): ranking …");
    for param in [
        "del_secondary",
        "ins_stressed",
        "sub_scale",
        "stress_mismatch",
        "spelling",
        "risk_scale",
    ] {
        run.has_line(&format!("{param} …"));
    }

    for extra in [
        &[
            "--group", "phrase", "--truth", "phones", "--prior", "profile",
        ][..],
        &["--fix", "all", "--refit-risks", "--risk-min-clips", "1"],
        &["--free-risks", "cjk-compat", "--risk-min-clips", "1"],
    ] {
        phono(&[&args[..], extra].concat()).ok();
    }
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn eval_scores_the_library_and_exports_answers() {
    let dir = out_dir("eval_scores_the_library_and_exports_answers");
    let export = dir.join("export.json");
    let run = phono(&[
        "eval",
        &fixture("library.json"),
        "--export",
        &export.display().to_string(),
        "--by-script",
        "--named-subset",
        &fixture("keys.tsv"),
    ])
    .ok();
    run.has_line("id     phrase …")
        .has_line("e2e-1  bad dream …")
        .has_line("e2e-2  good morning …")
        // `target` wins over `spoken` when it is set.
        .has_line("e2e-3  free kilo …")
        .has_line("lowest community scores:")
        .has_line("mean fidelity: community …");
    assert!(
        run.stdout.contains("keys.tsv (2 of 2 keys): community "),
        "{}",
        run.stdout
    );

    let exported: Vec<serde_json::Map<String, Value>> =
        serde_json::from_str(&read(&export)).unwrap();
    let keys: Vec<&str> = exported
        .iter()
        .flat_map(|e| e.keys().map(String::as_str))
        .collect();
    assert_eq!(keys, ["e2e-1", "e2e-2", "e2e-3"]);
    for (entry, phrase) in exported
        .iter()
        .zip(["bad dream", "good morning", "free kilo"])
    {
        let e = entry.values().next().unwrap();
        assert_eq!(e["spoken"], phrase);
        let unicode = e["unicode"].as_str().unwrap();
        assert!((1..=6).contains(&unicode.chars().count()), "{e}");
        assert!(
            e["notes"].as_str().unwrap().starts_with("phono top-1"),
            "{e}"
        );
    }
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn eval_subset_scores_only_the_listed_keys() {
    // The default --named-subset (tools/library-keys.tsv) doesn't exist in the empty working
    // directory, so there is no summary for it.
    let run = phono(&[
        "eval",
        &fixture("library.json"),
        "--subset",
        &fixture("keys.tsv"),
    ])
    .ok();
    run.has_line("e2e-1  …").has_line("e2e-3  …");
    assert!(run.line("e2e-2").is_none(), "{}", run.stdout);
    assert!(!run.stdout.contains("keys)"), "{}", run.stdout);
    run.has_line("mean fidelity: community …");
    assert!(run.stdout.contains("on 2/2"), "{}", run.stdout);
}
