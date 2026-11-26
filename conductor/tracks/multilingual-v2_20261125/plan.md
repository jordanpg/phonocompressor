# Implementation Plan: Calibrate a second ElevenLabs model (Multilingual v2)

**Track ID:** multilingual-v2_20261125
**Spec:** [spec.md](./spec.md)
**Issue:** [#6](https://github.com/jordanpg/phonocompressor/issues/6)
**Created:** 2026-11-25
**Status:** [~] In Progress

## Overview

Measure Multilingual v2 first: a probe, then the community messages, then earlier engine answers replayed on it. Build its profile from those clips, moving reader rules before fitting weights, because the fit scores what the reader predicts. Then check the profile's own answers on real audio and wire it into the parity check and docs. Every harness run gets its own dry run and approval, and every task stops for the user to verify it (`workflow.md`).

## Issues

- Track: [#6 Calibrate a second ElevenLabs model](https://github.com/jordanpg/phonocompressor/issues/6), the existing Feature issue. P2, XL.
- Each task is a Task sub-issue of #6, titled `<phase>.<task> <description>`, all P2. Sizes are on the board.

Credit estimates at 1 credit per character, to be confirmed by `harness:models` and each dry run:

| Run | Credits |
|---|---|
| exp11 | ~100 |
| exp12 | ~350 |
| exp13 | ~1,260 |
| exp14 | ≤ 340, less answers already recorded |

## Phase 1: Probe (exp11)

Find out how Multilingual v2 reads each script and where it departs from the reader's rules.

### Tasks

- [x] Task 1.1: exp11: probe Multilingual v2's scripts and reader rules. Link the main checkout's gitignored harness data into the worktree; confirm the model and its cost with `harness:models`; write `tools/experiments/exp11.toml` with exp0's 20 cases plus cases for hiragana, more katakana, the remaining scripts, numbers (`100.000`, glued digits, `2s`), capitals (`timE`, `checkhimPC`, `tHe`) and symbols, 2 seeds, a `max_credits` cap; dry run (exp0's seed-1 clips cached), approval, spend, transcribe, report; PLAN.md write-up with a draft script table and preference order ([#27](https://github.com/jordanpg/phonocompressor/issues/27))

### Verification

- [x] `tools/results/exp11.md` exists; PLAN.md's write-up records the credits spent and a draft script table covering every `Script`

## Phase 2: Community and replay data (exp12, exp13)

Collect the clips the baseline and the fit need.

### Tasks

- [x] Task 2.1: Let an experiment's `cases_from` take a list of files, dropping a text already seen for the same phrase; existing experiment files keep working (ruff clean; dry runs of exp1 and exp7 list the same cases as before) ([#28](https://github.com/jordanpg/phonocompressor/issues/28))
- [x] Task 2.2: exp12: the 34 community messages × 2 seeds on Multilingual v2; report; write-up next to exp1 (Whisper similarity, phone fidelity, language ID, silent clips). Stop point: if it isn't reading the messages through transliteration, ask before exp13 ([#29](https://github.com/jordanpg/phonocompressor/issues/29))
- [x] Task 2.3: exp13: the v1–v6 engine answers (126, community texts excluded) × 2 seeds on Multilingual v2; report; export one clips file of the model's default-setting clips from exp11–exp13; short PLAN.md entry ([#30](https://github.com/jordanpg/phonocompressor/issues/30))

### Verification

- [x] The clips file lists only `eleven_multilingual_v2`, and `phono fit` with Flash's weights reports about 200 or more within-phrase pairs

## Phase 3: The Multilingual v2 profile

Build the profile, move the reader rules its clips disagree with, then fit the weights.

### Tasks

- [~] Task 3.1: Add `MULTILINGUAL_V2` to `model.rs` and `PROFILES`: script table and preference order from exp11; Flash's reader constants and weights copied for now. Tests first (id resolves, katakana `Native`, default-on scripts). Baseline `phono calibrate --model eleven_multilingual_v2` ([#31](https://github.com/jordanpg/phonocompressor/issues/31))
- [ ] Task 3.2: Check the capital-letter rules, plurals, number grouping, glued-number drop and split costs against Multilingual v2's clips. Rules they disagree with become profile data, tests first. Flash's `phono eval` byte-identical; `calibrate` recorded for both models ([#32](https://github.com/jordanpg/phonocompressor/issues/32))
- [ ] Task 3.3: `phono fit --model eleven_multilingual_v2` with substitution scales held (as in v6); put the weights and `predict` in the profile with a comment on their source; report in-sample and held-out ranking next to Flash's weights ([#33](https://github.com/jordanpg/phonocompressor/issues/33))

### Verification

- [ ] Gates pass (fmt, clippy, tests, `wasm:check`); Flash's `phono eval` unchanged; `phono --model eleven_multilingual_v2 compress` works

## Phase 4: Live check and integration (exp14)

Check the answers the profile actually gives, then hook it into parity and the docs.

### Tasks

- [ ] Task 4.1: exp14: `phono eval --model eleven_multilingual_v2 --export` → the profile's top answers × 2 seeds; report; write-up against exp12's community numbers; a follow-up issue if it falls short ([#34](https://github.com/jordanpg/phonocompressor/issues/34))
- [ ] Task 4.2: Add a Multilingual v2 case to `crates/wasm/parity/cases.json` and run `wasm:parity`; update PLAN.md (model profiles, item 7, open questions), product.md (status, scope), tech-stack.md (decision log), the README and the conductor index ([#35](https://github.com/jordanpg/phonocompressor/issues/35))

### Verification

- [ ] `wasm:parity` passes; the docs name both calibrated models

## Final Verification

- [ ] All acceptance criteria met
- [ ] Tests passing
- [ ] Documentation updated
- [ ] Ready for review

---

_Generated by Conductor. Tasks will be marked [~] in progress and [x] complete._
