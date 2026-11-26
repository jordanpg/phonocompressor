# Specification: Use the full community sample library

**Track ID:** community-library_20261126
**Type:** Feature
**Created:** 2026-11-26
**Status:** Draft
**Issue:** [#39 Use the full community sample library](https://github.com/jordanpg/phonocompressor/issues/39)

## Summary

Replace the 34 hand-picked reference messages with the full community library (298 entries on 2026-11-26) as the engine's reference set. That means fetching it locally, scoring it offline, and running live ElevenLabs experiments on it. The 34 stay as a named subset, so results stay comparable with exp1–exp10.

## Context

- Product goal 3 is to beat hand-made messages, and it is measured over `sample/messages.json`. That file holds 34 hand-picked, safe-for-work messages, gitignored and keyed by their index in the library at the time they were picked (`"8"`, `"10"`, …).
- The same community runs a moderated library of hand-crafted messages behind a JSON API (`{entries, types}`). Each entry has `id` (UUID), `unicode`, `spoken`, `notes`, `submittedBy`, per-character script `characters`, `createdAt` and `updatedAt`. The library owner has given permission to use it, and the user has approved sending every entry to ElevenLabs.
- **What the full set adds** (offline check, 2026-11-26):
  - Predicted fidelity over all 298: community 0.855, ours 0.952 (ours ≥ community on 284). The 34 alone: 0.843 vs 0.961, so they are representative and the gain is breadth, not difficulty.
  - New scripts: Canadian syllabics (21 entries), Ethiopic (10), Hiragana (8), Armenian, Devanagari, Runic. Community entries score lowest offline in Katakana (0.60), Cherokee (0.66), Ethiopic (0.70), Canadian syllabics (0.71) and Jamo (0.72), against Yi at 0.88. Each low score is either a reader blind spot or a bad entry; only clips can tell which.
  - Heavy chunk reuse: the most common two-glyph chunk is in 38 entries, the next in 18 and 17. Held-out checks that split by phrase alone will overstate generalisation.
  - About 13 `notes` name a voice and model, mostly Turbo v2.5.
  - About 30 `spoken` labels are written for people rather than sound: digits for a spelled number, parenthetical asides, `a/b` alternatives, `100k`.
- `phono fit` learns from clips, not phrases. New phrases only help the weights after a live run.
- Credits: the 264 new entries cost about 751 credits per seed on Flash v2.5 (0.5×, rounded up per request). Community plus our answers at 2 seeds comes to about 3,000 credits, roughly 30% of the 10,000/month free tier.
- **Public repo.** About 205 entries are crude or sexual, and some name real people. Library phrases, submitter names and the library's URL (it names a channel) must stay out of commits, committed experiment TOMLs, docs and issues, per `workflow.md`. `sample/`, `data/generated/` and `tools/results/exp*.md` are already gitignored.

## User Story

As the developer calibrating the engine, I want the full community library available locally as a reference set, so that I can measure and fit the engine against roughly nine times as many real, hand-crafted messages and the scripts the 34 never covered.

## Acceptance Criteria

- [ ] **Fetch.** One mise task downloads the library into gitignored `sample/library.json`, in the shape the existing loaders read. The URL comes from fnox or the environment, not a committed file. Submitter fields are dropped. The 34 hand-picked entries keep their current keys, and every other entry gets a key that stays stable across re-fetches. The task prints the entry count, how many are new or changed since the last fetch, and a content hash.
- [ ] **Targets.** Each entry carries a `target` used for scoring. Rules clean the common label forms, and a gitignored override file handles the rest. `phono eval` and the harness's `cases_from` use `target` when present and fall back to `spoken`, so `sample/messages.json` still works unchanged.
- [ ] **Offline baseline.** `phono eval` over the library is recorded in PLAN.md: overall, the 34 vs the rest, and per script. Cases are cited by key only.
- [ ] **Live probe.** An experiment on the entries in the low-scoring scripts (community and our answers, Flash v2.5, 2 seeds) is synthesized after the user approves the dry-run estimate, then transcribed, reported and written up. Each reader blind spot it confirms becomes its own issue.
- [ ] **Full run and refit.** Once the user approves the spend, the rest of the library is run live. `phono fit` is refitted on all clips, with cross-validation folds that keep entries sharing a chunk together. New weights ship only if held-out ranking improves, and `phono eval` and `phono calibrate` are compared before and after.
- [ ] **Metrics moved.** `product.md`'s success metrics and the `phono eval` default point at the library set, with the 34 still reported as a subset for continuity.

## Dependencies

- Existing code: `phono eval` and `load_messages` (`crates/cli/src/main.rs`), `cases_from_messages` (`tools/src/phono_harness/experiments.py`), `phono fit` and `phono calibrate`, and the harness's synth, transcribe and report commands.
- fnox (already set up for `ELEVENLABS_API_KEY`) for the library URL.
- No other track.

## Out of Scope

- Reader or scorer fixes for blind spots found in the live probe. Each becomes its own issue or track.
- A Turbo v2.5 or Multilingual v2 `ModelProfile` built from the voice and model `notes`. The notes are recorded, not acted on.
- Committing any library content, mirroring the library, or publishing it.
- The web tool and its deploy.
- Submitting messages to the library.

## Technical Notes

- Keys: the 34 keep theirs through a committed UUID → key table, which holds no message content. New entries are keyed from their UUID (e.g. `u` + 8 hex digits), so later inserts don't renumber anything.
- Harness case ids stay `msg<key>`, so the existing exp1–exp10 reports keep their ids.
- Clean `target` by rule where it's safe (strip parentheticals, take the first of `a/b`, `100k` → `100,000`). Everything else goes in the gitignored override file.
- Group the cross-validation folds by shared chunk: connect entries that share a non-Latin chunk of two or more glyphs, then split by component.
- Committed experiment TOMLs build cases from gitignored files (`cases_from`), as exp7 does. Descriptions cite keys and counts, never phrases.
- `workflow.md` rule 4 still applies: every `--spend` needs the user's go-ahead, with the dry-run estimate shown first.

---

_Generated by Conductor. Review and edit as needed._
