# Implementation Plan: Use the full community sample library

**Track ID:** community-library_20261126
**Spec:** [spec.md](./spec.md)
**Issue:** [#39](https://github.com/jordanpg/phonocompressor/issues/39)
**Created:** 2026-11-26
**Status:** [~] In Progress

## Overview

Fetch the library into a gitignored file the existing loaders can read, with clean targets and stable keys. Record the offline baseline. Spend credits in two steps: first a probe of the scripts where the reader and the community disagree most, then the full set and a refit. Every task stops for the user to verify it (`workflow.md`), and every `--spend` needs a separate go-ahead.

## Issues

- Track: [#39 Use the full community sample library](https://github.com/jordanpg/phonocompressor/issues/39), a Feature issue. P1, L.
- Each task is a Task sub-issue of #39, titled `<phase>.<task> <description>`, all P1. Sizes are on the board.

## Phase 1: Fetch and format

Get the library onto disk in a shape `phono eval` and the harness already read, with nothing from it committed.

### Tasks

- [x] Task 1.1: Add the library URL to fnox and a `//tools:fetch-library` task that writes `sample/library.json` in the `[{"<key>": {…}}]` shape. Drop `submittedBy`, keep `id`, `unicode`, `spoken`, `notes`, scripts and timestamps, keep the 34's keys through a UUID → key table, key the rest from their UUID, and print the count, new and changed entries, and a hash. Tests run on synthetic fixtures, with no library content. ([#40](https://github.com/jordanpg/phonocompressor/issues/40))
- [~] Task 1.2: Add `target`, built from rules plus the gitignored `sample/targets.tsv` overrides, at fetch time. `load_messages` (CLI) and `cases_from_messages` (harness) prefer `target` over `spoken`. Tests cover each rule and the fallback. ([#41](https://github.com/jordanpg/phonocompressor/issues/41))

### Verification

- [ ] A re-fetch keeps every key. The 34 keep their keys and still match `sample/messages.json`.
- [ ] `phono eval sample/messages.json` gives unchanged output.
- [ ] `git status` shows nothing from `sample/`, and the URL appears in no tracked file.
- [ ] Gates pass.

## Phase 2: Offline baseline

### Tasks

- [ ] Task 2.1: Add a `--by-script` summary to `phono eval` (mean community and ours per script, using the core's script lookup) and a `--subset <keys file>` filter. Run it on the library and write the baseline into PLAN.md: overall, the 34 vs the rest, per script, and the lowest-scoring community cases by key. ([#42](https://github.com/jordanpg/phonocompressor/issues/42))

### Verification

- [ ] The numbers match the 2026-11-26 check (0.855 / 0.952 overall), or the difference is explained.

## Phase 3: Live probe of low-scoring scripts

### Tasks

- [ ] Task 3.1: Define the next experiment: community entries in Katakana, Cherokee, Ethiopic, Canadian syllabics, Hangul Jamo, Hiragana, Armenian and Devanagari, plus our top answer for each phrase, on Flash v2.5, seeds 1 and 2, same voice as exp1. Show the dry-run estimate and wait for approval. ([#43](https://github.com/jordanpg/phonocompressor/issues/43))
- [ ] Task 3.2: Synthesize, transcribe and report it, then add a PLAN.md write-up (setup, credits, results, decision). For each script, record whether the reader or the entries were wrong, and open an issue per confirmed reader blind spot. ([#44](https://github.com/jordanpg/phonocompressor/issues/44))

### Verification

- [ ] Report and write-up exist, with no phrases in committed files. Follow-up issues are filed.

## Phase 4: Full run and refit

### Tasks

- [ ] Task 4.1: Define an experiment for the remaining library entries not yet clipped (community and ours, Flash v2.5, 2 seeds). Show the estimate and wait for approval, then synthesize, transcribe and report, with a PLAN.md write-up. ([#45](https://github.com/jordanpg/phonocompressor/issues/45))
- [ ] Task 4.2: Make `phono fit`'s cross-validation group cases that share a chunk. Refit on all Flash clips, and compare held-out ranking, `phono eval` and `phono calibrate` against v6's weights. Adopt the new weights only if held-out ranking improves, and record the result either way. ([#46](https://github.com/jordanpg/phonocompressor/issues/46))

### Verification

- [ ] The fit's held-out numbers come from chunk-grouped folds, and the adopt-or-reject decision is recorded in PLAN.md.

## Phase 5: Switch the reference set

### Tasks

- [ ] Task 5.1: Point `phono eval`'s default and `product.md`'s success metrics at `sample/library.json`, with the 34 reported as a subset. Update `workflow.md` (quality gates, the fetch step), the Conductor index and the README where they mention the 34. ([#47](https://github.com/jordanpg/phonocompressor/issues/47))

### Verification

- [ ] Docs and defaults agree, and `mise run phono eval` works after a fresh fetch.

## Final Verification

- [ ] All acceptance criteria met
- [ ] Tests passing; gates pass
- [ ] No library content, submitter name or URL in any tracked file, issue or PR
- [ ] Documentation updated
- [ ] Ready for review

---

_Generated by Conductor. Tasks will be marked [~] in progress and [x] complete._
