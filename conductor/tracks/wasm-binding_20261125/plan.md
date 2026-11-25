# Implementation Plan: Build the WASM binding

**Track ID:** wasm-binding_20261125
**Spec:** [spec.md](./spec.md)
**Issue:** [#14](https://github.com/jordanpg/phonocompressor/issues/14) (sub-issue of [#1](https://github.com/jordanpg/phonocompressor/issues/1))
**Created:** 2026-11-25
**Status:** [~] In Progress

## Overview

Make `phono-core` build for wasm32 (#2). Add the two small core and CLI pieces that the binding and the parity check need. Build `crates/wasm`, check it against the CLI under Node, then add a plain test page that runs it in a Web Worker and measure it in browsers. Every task stops for the user to verify it (`workflow.md`).

## Issues

- Track: [#14 Build the WASM binding](https://github.com/jordanpg/phonocompressor/issues/14), a Feature issue under #1. P0, L.
- Task 1.1 is the existing [#2](https://github.com/jordanpg/phonocompressor/issues/2), moved under #14 with its title and fields kept.
- Every other task is a Task sub-issue of #14, titled `<phase>.<task> <description>`, all P0. Sizes are on the board.
- [#3](https://github.com/jordanpg/phonocompressor/issues/3) (shrink phono.bin) stays under #1, outside this track.

## Phase 1: Build for wasm32

Prove `phono-core` builds for `wasm32-unknown-unknown`, and make the check one command.

### Tasks

- [x] Task 1.1: Add `wasm32-unknown-unknown` to `targets` on `rust` in `mise.toml`; build `phono-core` for wasm32 and fix anything that doesn't compile; add a `wasm:check` mise task; drop the "not installed" note from the wasm32 gate in `workflow.md` ([#2](https://github.com/jordanpg/phonocompressor/issues/2))

### Verification

- [x] `mise run wasm:check` passes after `mise install`; `mise run fmt:check`, `mise run lint` and `cargo test --workspace` pass

## Phase 2: Core and CLI support

Give the core a way to switch model and scripts without decoding the data again, and give the CLI machine-readable output for the parity check. Test first where it fits.

### Tasks

- [x] Task 2.1: Add an `Engine` method that switches the model profile and allowed scripts by rebuilding the `ChunkSet` and weights, plus script lookup by name with an error listing valid names; reimplement `allow_all_scripts` on it. Tests: only allowed scripts in the chunk set, switching back restores the default, unknown name errors ([#15](https://github.com/jordanpg/phonocompressor/issues/15))
- [x] Task 2.2: Move `fmt_phones` from `phono-cli` into `phono-core`; add `--json` (`text`, `latin`, `phones`, `score`, `codepoints` per result) and `--scripts <names>` to `phono compress` ([#16](https://github.com/jordanpg/phonocompressor/issues/16))
  - Deviation: `fmt_phones` was already in `phono-core` (`phone.rs`). The shared piece is the per-word join the CLI's `target:` line did inline, now `phone::fmt_word_phones` (`B AE1 D | D R IY1 M`), which `--json`'s `phones` and the wasm binding use. `--scripts` is global, like `--all-scripts`, and conflicts with it.

### Verification

- [x] `phono eval` answers unchanged before vs. after (numbers in the task issues)
- [x] `phono compress 'bad dream' --json` parses as JSON; plain output unchanged
- [x] Gates pass; `mise run wasm:check` still passes

## Phase 3: The `crates/wasm` binding

Create the crate and export the engine with typed options and results.

### Tasks

- [x] Task 3.1: Create `crates/wasm` (`cdylib`, `wasm-bindgen`) in the workspace; install `wasm-bindgen-cli` with mise, pinned to the crate's version; add a `wasm:build` task that runs `wasm-bindgen --target web` into a gitignored directory; check new dependencies' licenses and update `THIRD_PARTY.md` if needed ([#17](https://github.com/jordanpg/phonocompressor/issues/17))
  - Output goes to `target/wasm-pkg` (already gitignored). Review: Task 1.1's `check:wasm` was renamed `wasm:check`, so every wasm task is `wasm:<name>`; it now builds `phono-wasm` too.
- [x] Task 3.2: Export a constructor from the data file's bytes (`Uint8Array`) and `compress(phrase, { model, maxCodepoints, topN, allowedScripts })` → `[{ text, latin, phones, score }]`, with the CLI's defaults; choose how to generate the TypeScript types (license and size checked); validate input with messages in the product-guidelines style; keep mapping and validation in plain Rust with native unit tests ([#18](https://github.com/jordanpg/phonocompressor/issues/18))
  - Types: `tsify` 0.5.8 (MIT OR Apache-2.0; `tsify-next` is deprecated in its favour), `Ts<T>` at the boundary. Options are read field by field, not with `Ts::to_rust`, because serde-wasm-bindgen ignores unknown keys (a typo would be silently dropped) and its type errors don't name the field. Ranges: `maxCodepoints` 1–30, `topN` 1–50, whole numbers. Also rejected: a phrase with nothing to pronounce (`!!!`), which the CLI answers with an empty list. `model::profile` now gives the CLI and the binding one unknown-model message.

### Verification

- [x] `mise run wasm:build` produces `.wasm`, `.js` and `.d.ts`; the `.d.ts` has typed `compress` options and results (no `any`)
- [x] Native tests for validation and mapping pass; clippy is clean

## Phase 4: Parity with the CLI

Show the wasm build gives the same guesses as `phono compress`.

### Tasks

- [x] Task 4.1: Commit public phrases and option sets (defaults, a smaller budget, a larger `topN`, all scripts; never `sample/messages.json`); pin Node in `mise.toml`; add a Node script (`initSync`, reads `phono.bin` from disk) and a `wasm:parity` task that compares it with `phono compress --json`: same text in the same order, scores within 1e-4; it fails loudly without `phono.bin` or the build ([#19](https://github.com/jordanpg/phonocompressor/issues/19))

### Verification

- [x] `mise run wasm:parity` passes
- [x] Changing one option on one side makes it fail

## Phase 5: Test page and measurements

Run the binding in real browsers, off the main thread, and record what it costs.

### Tasks

- [x] Task 5.1: Plain HTML/JS page with phrase, budget and top N inputs; a module Web Worker fetches the `.wasm` and `phono.bin`, initializes once and runs `compress()` per request; list text, Latin, phones and score; show errors and keep the input; add a `wasm:serve` task (`.wasm` served as `application/wasm`) ([#20](https://github.com/jordanpg/phonocompressor/issues/20))
- [~] Task 5.2: Measure in Chrome and Firefox: start-up time (fetch and decode), time per phrase for the parity phrases at defaults, `.wasm` and `phono.bin` sizes raw and gzip; record in `docs/PLAN.md` and a comment on #1; open a speed-tuning issue if time per phrase is well above native ([#21](https://github.com/jordanpg/phonocompressor/issues/21))
- [ ] Task 5.3: Document the binding: PLAN.md (Phase 3 status, fetch-not-embed decision, repo layout line), README (`wasm:build`, `wasm:parity`, `wasm:serve`), `tech-stack.md` (`crates/wasm` row, new tools, decision-log entry) and `workflow.md` (`wasm:check` and `wasm:parity` gates) ([#22](https://github.com/jordanpg/phonocompressor/issues/22))

### Verification

- [ ] `mise run wasm:serve`, then "hundred thousand apartments" shows the CLI's top results in Chrome and Firefox
- [ ] The measurements are in PLAN.md and on #1

## Final Verification

- [ ] All acceptance criteria in spec.md met
- [ ] `mise run fmt:check`, `mise run lint`, `cargo test --workspace`, `mise run wasm:check` and `mise run wasm:parity` pass; `phono eval` answers unchanged
- [ ] Documentation updated (PLAN.md, README, tech-stack.md, workflow.md)
- [ ] New dependencies' licenses checked; `THIRD_PARTY.md` updated if needed
- [ ] All task issues closed after verification; #14 closed; #1 stays open for the front-end track
- [ ] Ready for review

---

_Generated by Conductor. Tasks will be marked [~] in progress and [x] complete._
