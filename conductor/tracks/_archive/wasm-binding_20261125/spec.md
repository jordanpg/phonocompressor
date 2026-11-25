# Specification: Build the WASM binding

**Track ID:** wasm-binding_20261125
**Type:** Feature
**Created:** 2026-11-25
**Status:** Complete
**Issue:** [#14 Build the WASM binding](https://github.com/jordanpg/phonocompressor/issues/14), a sub-issue of [#1 Ship the WASM web tool](https://github.com/jordanpg/phonocompressor/issues/1)

## Summary

Add `crates/wasm`, a wasm-bindgen binding that runs the phonocompressor search in a browser and returns the same guesses as `phono compress`, plus a minimal test page that runs it in a Web Worker. This is the first part of #1. The Svelte front end and the GitHub Pages deploy are a later track.

## Context

- Product goal 1 is the WASM web tool (Phase 3): the engine runs fully in the browser, with no server. Phase 3 has not started, and `crates/wasm` does not exist yet.
- `phono-core` is meant to build for `wasm32-unknown-unknown`, but that hasn't been checked: the target is not installed ([#2](https://github.com/jordanpg/phonocompressor/issues/2)).
- The binding wraps the same path the CLI uses: `Engine::from_bytes(bytes, profile)`, then `Searcher::new(&engine).search(phrase, &SearchConfig)`, which returns `Candidate { text, latin, words, fidelity, codepoints, … }`. Profiles come from `model::for_model(id)`. Only `eleven_flash_v2_5` has one.
- `Engine::new` derives only the `ChunkSet` (filtered by the profile's default scripts) and the weights from the profile. The core can only switch all scripts on (`allow_all_scripts`); it has no per-script list. The CLI prints plain text only.
- Native search takes about 1–2 s per phrase (release build). `phono.bin` is 12.5 MB and not committed; `mise run data` builds it.
- Planned API (PLAN.md, Phase 3): `compress(phrase, { model, maxCodepoints: 6, topN, allowedScripts }) → [{ text, latin, phones, score }]`.

## User Story

As the developer building the web tool, I want to call `compress()` from JavaScript in a browser and get the same guesses as the CLI, so that the Svelte front end can be built on a binding that is known to work.

## Acceptance Criteria

- [x] **Builds for wasm32.** `phono-core` and `crates/wasm` build for `wasm32-unknown-unknown`. The target is on `rust` in `mise.toml`, and a mise task runs the check (closes #2).
- [x] **Typed `compress()` API.** JavaScript creates an engine from the data file's bytes and calls `compress(phrase, { model, maxCodepoints, topN, allowedScripts })`, which returns `[{ text, latin, phones, score }]`. The TypeScript types are generated, not hand-written. An empty phrase, an out-of-range budget or `topN`, an unknown model or an unknown script returns an error message that says what failed and how to fix it, never a silent fallback.
- [x] **Same output as the CLI.** For a committed list of public phrases and option sets, the wasm build running under Node returns the same text in the same order as `phono compress --json` with the same settings, and scores within 1e-4. One mise task runs the check, and it fails loudly if `phono.bin` is missing.
- [x] **Test page in a Worker.** A minimal static page (plain HTML and JS, no framework) fetches `phono.bin`, runs `compress()` in a module Web Worker, and lists each result's text, Latin, phones and score. It shows errors and keeps the input. Browser time per phrase (Chrome and Firefox), start-up time (fetch and decode), and `.wasm` and `phono.bin` sizes (raw and gzip) are recorded in PLAN.md.

## Dependencies

- Existing code: `phono-core` (`Engine`, `Searcher`/`SearchConfig`, `ModelProfile`, `data::Data`), the `compress` command in `phono-cli`, and `phono.bin` from `mise run data`.
- [#2](https://github.com/jordanpg/phonocompressor/issues/2) (build for wasm32) is Phase 1 of this track.
- [#3](https://github.com/jordanpg/phonocompressor/issues/3) (shrink phono.bin) is a separate track and does not block this one. This track uses the current 12.5 MB file.

## Out of Scope

- The Svelte front end, its accessibility work, and choosing the front end's package manager, formatter and linter.
- GitHub Pages deployment and any CI, including running the wasm32 gate in CI.
- Shrinking `phono.bin` or changing the data format (#3).
- Browser speed tuning (smaller beam, pruning). This track measures speed only; follow-ups become issues.

## Technical Notes

- **Data loading: fetch at runtime, not embedded.** The Worker fetches `phono.bin` and passes the bytes to the engine constructor (`Engine::from_bytes`). The `.wasm` stays small, the data file is cached on its own, and the raw bytes can be freed after decoding. This changes PLAN.md's "embeds the data file"; record the decision in PLAN.md and tech-stack.md.
- **One decoded data file for every option set.** `model` and `allowedScripts` come with each call, but the core fixes the profile and script set when the `Engine` is built. Add a core method that switches profile and scripts by rebuilding the `ChunkSet` and weights, without decoding again. `allow_all_scripts` becomes one case of it, and `phono eval` answers must not change.
- **Toolchain.** `wasm-bindgen-cli` installed by mise and pinned to the same version as the `wasm-bindgen` crate (the CLI and crate versions must match). Output built with `--target web`, which a module Worker loads with `initSync`. Build output is not committed.
- **Generated types.** wasm-bindgen doesn't generate TypeScript interfaces for plain option and result objects on its own. Choose a way that does (for example a serde + TypeScript derive crate) in task 3.2, and check its license and size cost.
- **Parity check.** `phono compress` gains `--json` and `--scripts` so it can run the same settings as the binding. Both sides format phones with one shared function in `phono-core`. The phrase list holds public phrases only (for example from the README), never `sample/messages.json`. Native and wasm floating-point math (for example `exp` and `ln`) may differ in the last bits, so scores are compared within a tolerance. If the order differs, find out why rather than loosen the check.
- **Gates.** The usual gates apply (fmt, clippy, tests, `phono eval` before and after the core and CLI changes), plus a license check on every new dependency.

---

_Generated by Conductor. Review and edit as needed._
