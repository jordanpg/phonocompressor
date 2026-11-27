//! `phono compress`: the JSON shape, the budget, ranking and close-enough flags, scripts, and
//! the text output agreeing with `--json`.

use crate::common::{Guess, compress, phono};
use phono_core::chunks::Script;
use phono_core::model::{ModelProfile, profile};
use phono_core::phone::parse_pron;
use phono_core::search::SearchConfig;
use std::collections::HashSet;

/// Checks what every result list must satisfy, whatever the weights: at most `top` results,
/// within the codepoint budget, ranked by score less the search's cost per codepoint, one string
/// per reading, the close-enough flag matching the profile's cut, readable phones, and only
/// scripts `allowed` accepts.
#[track_caller]
fn check(
    guesses: &[Guess],
    model: &ModelProfile,
    top: usize,
    max: usize,
    allowed: impl Fn(Script) -> bool,
) {
    assert!(!guesses.is_empty(), "no results");
    assert!(guesses.len() <= top, "{} results, top {top}", guesses.len());
    let mut readings = HashSet::new();
    for g in guesses {
        // Strings that transliterate alike read alike; the search keeps only the first.
        assert!(
            readings.insert(g.latin.to_lowercase()),
            "{g:?}: a second string reading {:?}",
            g.latin
        );
        assert_eq!(g.codepoints, g.text.chars().count(), "{g:?}");
        assert!(g.codepoints <= max, "{g:?}: over the budget of {max}");
        assert!((0.0..=1.0).contains(&g.score), "{g:?}");
        assert_eq!(g.close_enough, model.is_close_enough(g.score), "{g:?}");
        assert!(!g.latin.is_empty(), "{g:?}");
        for word in g.phones.split(" | ") {
            assert!(
                parse_pron(word).is_some_and(|p| !p.is_empty()),
                "{g:?}: bad phones {word:?}"
            );
        }
        for c in g.text.chars() {
            assert!(
                allowed(Script::of(c)),
                "{g:?}: {c:?} is {:?}",
                Script::of(c)
            );
        }
    }
    let per_codepoint = SearchConfig::default().codepoint_cost;
    let rank = |g: &Guess| g.score - per_codepoint * g.codepoints as f32;
    for w in guesses.windows(2) {
        assert!(
            rank(&w[0]) >= rank(&w[1]),
            "not ranked: {:?} then {:?}",
            w[0],
            w[1]
        );
    }
}

/// `check` at the defaults (top 10, 6 codepoints, the profile's default scripts).
#[track_caller]
fn check_defaults(phrase: &str, model_id: &str) -> Vec<Guess> {
    let model = profile(model_id).unwrap();
    let guesses = compress(phrase, &["--model", model_id]);
    check(&guesses, model, 10, 6, |s| model.script(s).default_on);
    guesses
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn flash_defaults() {
    for phrase in [
        "bad dream",
        "hundred thousand apartments",
        "zorblax is here",
    ] {
        check_defaults(phrase, "eleven_flash_v2_5");
    }
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn multilingual_defaults() {
    for phrase in ["you and me at home", "good morning everyone"] {
        check_defaults(phrase, "eleven_multilingual_v2");
    }
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn budget_and_top_options() {
    let flash = profile("eleven_flash_v2_5").unwrap();
    let default_on = |s: Script| flash.script(s).default_on;
    check(
        &compress("bad dream", &["--max", "4"]),
        flash,
        10,
        4,
        default_on,
    );
    let wide = compress("zorblax is here", &["--max", "8", "--top", "25"]);
    check(&wide, flash, 25, 8, default_on);
    assert!(wide.len() > 10, "--top 25 gave {} results", wide.len());
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn scripts_option_limits_the_characters() {
    let flash = profile("eleven_flash_v2_5").unwrap();
    let only = [Script::Hangul, Script::Yi];
    let guesses = compress("bad dream", &["--scripts", "hangul,yi"]);
    check(&guesses, flash, 10, 6, |s| only.contains(&s));
    // --all-scripts may use scripts the profile leaves off, and still keeps the other rules.
    check(
        &compress("bad dream", &["--all-scripts"]),
        flash,
        10,
        6,
        |_| true,
    );
}

/// Quality floors: phrases the engine has answered well since v5. If a deliberate change drops
/// one, look at why before lowering the floor.
#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn familiar_phrases_get_a_close_enough_answer() {
    for phrase in [
        "bad dream",
        "good morning",
        "free kilo",
        "hundred thousand apartments",
    ] {
        let guesses = compress(phrase, &[]);
        assert!(
            guesses[0].close_enough,
            "{phrase}: best is {:?}",
            guesses[0]
        );
    }
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn text_output_lists_the_json_results() {
    let guesses = compress("bad dream", &[]);
    let run = phono(&["compress", "bad dream"]).ok();
    run.has_line("target: B AE1 D | D R IY1 M");
    let ranked: Vec<&str> = run
        .lines()
        .into_iter()
        .filter(|l| l.as_bytes().get(2) == Some(&b'.'))
        .collect();
    assert_eq!(ranked.len(), guesses.len(), "{}", run.stdout);
    for (i, (line, g)) in ranked.iter().zip(&guesses).enumerate() {
        let start = format!(
            "{:2}. {:.3}  {}cp  {}",
            i + 1,
            g.score,
            g.codepoints,
            g.text
        );
        assert!(line.starts_with(&start), "{line:?} vs {start:?}");
    }
    assert!(
        run.line("No string of").is_none(),
        "unexpected note:\n{}",
        run.stdout
    );
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn a_tight_budget_says_nothing_scores_well() {
    let guesses = compress("good morning everyone", &["--max", "2"]);
    assert!(guesses.iter().all(|g| !g.close_enough), "{guesses:?}");
    let cut = profile("eleven_flash_v2_5").unwrap().close_enough;
    phono(&["compress", "good morning everyone", "--max", "2"])
        .ok()
        .has_line(&format!(
            "No string of 2 codepoints or fewer scores well (close enough on eleven_flash_v2_5 \
             is {cut:.2} or more). Try a higher budget or a shorter phrase."
        ));
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn want_reports_where_a_string_was_generated() {
    let best = compress("bad dream", &[]).remove(0);
    phono(&["compress", "bad dream", "--want", &best.text])
        .ok()
        .has_line(&format!("want {}: generated at rank …", best.text));
    phono(&["compress", "bad dream", "--want", "zzzz"])
        .ok()
        .has_line("want zzzz: not generated …");
}

#[test]
#[ignore = "needs phono.bin: mise run //crates/cli:e2e"]
fn the_search_is_deterministic() {
    let args = ["compress", "zorblax is here", "--json"];
    assert_eq!(phono(&args).ok().stdout, phono(&args).ok().stdout);
}
