//! Argument handling that needs no data file: these run under plain `cargo test`.

use crate::common::{out_dir, phono_raw};

#[test]
fn help_lists_every_command() {
    let run = phono_raw(&["--help"]).ok();
    for cmd in [
        "phones",
        "read",
        "spell",
        "score",
        "compress",
        "calibrate",
        "fit",
        "eval",
    ] {
        run.has_line(&format!("  {cmd} …"));
    }
}

#[test]
fn conflicting_flags_are_rejected_before_loading() {
    // clap exits 2 on a usage error, before the data file is read (there is none here).
    for args in [
        &["compress", "bad dream", "--json", "--want", "x"][..],
        &["--scripts", "latin", "--all-scripts", "phones", "x"],
        &["fit", "clips.tsv", "--refit-risks", "--free-risks", "yi"],
        &["calibrate", "clips.tsv", "--word-dump", "w.tsv"],
    ] {
        phono_raw(args).fails(2, &["error:"]);
    }
}

#[test]
fn a_missing_data_file_says_how_to_build_it() {
    let missing = out_dir("missing_data").join("phono.bin");
    let missing = missing.display().to_string();
    phono_raw(&["--data", &missing, "phones", "x"]).fails(1, &[&missing, "mise run data"]);
}

#[test]
fn the_default_data_file_is_relative_to_the_working_directory() {
    phono_raw(&["phones", "x"]).fails(1, &["data/generated/phono.bin", "mise run data"]);
}
