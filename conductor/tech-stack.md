# Tech Stack

## Overview

| Layer | Choice | Status |
|---|---|---|
| Engine | Rust (edition 2024, stable) | Working |
| Data build | Rust (`phono-builder`) | Working |
| CLI | Rust (`phono-cli`, clap) | Working |
| Test harness | Python ≥ 3.13 (uv) | Working |
| Web front end | TypeScript + Svelte + Vite, over a wasm-bindgen module | Planned (Phase 3) |
| Backend | None: the engine runs client-side | — |
| Database | None: one postcard-encoded data file | — |
| Hosting | GitHub Pages (static) | Planned |
| CI | None yet | — |

## Rust workspace

Cargo workspace, `resolver = "3"`, version 0.1.0, license `MIT OR Apache-2.0`.

| Crate | Binary | Purpose | Dependencies |
|---|---|---|---|
| `crates/core` (`phono-core`) | — | Phone features, G2P, reader, scorer, search. No I/O; builds for `wasm32`. | `serde` 1 (derive), `postcard` 1 (alloc) |
| `crates/builder` (`phono-builder`) | `phono-build` | Builds `data/generated/phono.bin` from Phoible, CMUdict, the G2P model and the transliteration table | `phono-core`, `clap` 4, `deunicode` 1.6 |
| `crates/cli` (`phono-cli`) | `phono` | `compress`, `score`, `read`, `phones`, `spell`, `eval`, `calibrate`, `fit` | `phono-core`, `clap` 4 |
| `crates/wasm` | — | wasm-bindgen bindings that embed the data file | Planned, not created |

Build profiles: dependencies build at `opt-level = 2` and the workspace at `opt-level = 1` in dev builds, so the builder's G2P training stays usable without `--release`.

### Data file

- `data/generated/phono.bin`, postcard-encoded, built by `mise run data` (about 15 s). Not committed.
- 12.5 MB since v3, mostly unpruned G2P tables (u64 keys + f32). Shrinking it (delta-coded keys, u16 log-probs, entropy pruning) matters for the WASM bundle.

### Data sources

| Source | Where | Used for | License |
|---|---|---|---|
| Phoible | `phoible/` (submodule) | Phone feature vectors / distance | CC BY 4.0 |
| CMUdict | `cmudict/` (submodule) | English pronunciations, G2P training | BSD-style |
| `deunicode` crate | Cargo | Transliteration table, plus overrides from exp10 in `crates/builder/src/translit.rs` | BSD-3-Clause |
| Python `unidecode` | `tools/` | Dev-only comparison; never feeds `phono.bin` | GPL-2.0+ |

## Python harness (`tools/`)

Package `phono-harness` 0.1.0, built with `uv_build`, run through mise tasks. Its virtualenv lives in `~/.cache/phonocompressor/tools-venv` (`UV_PROJECT_ENVIRONMENT`).

| Dependency | Purpose |
|---|---|
| `httpx` ≥ 0.28 | ElevenLabs API |
| `numpy` ≥ 2.0 | Audio and metrics |
| `torch` ≥ 2.6 | Local ASR (GPU intended) |
| `transformers` ≥ 4.56 | Whisper (words) and wav2vec2 (IPA phones) |
| `unidecode` ≥ 1.3 | Dev-only reading-model-T predictions (GPL; never shipped) |

Modules: `cli`, `elevenlabs`, `asr`, `cache`, `experiments`. Experiments are defined in `tools/experiments/exp*.toml`. Clips and transcripts are cached in `.cache/harness/`, and reports go to `tools/results/` (both gitignored).

## Web front end (planned)

- **TypeScript + Svelte + Vite.** A single-page tool with a small amount of interactive state (phrase, budget, model, results). Svelte compiles away most of its runtime, which keeps the bundle small next to a multi-megabyte data file.
- **Engine binding:** `crates/wasm` with wasm-bindgen. Planned API (PLAN.md, Phase 3):
  `compress(phrase, { model, maxCodepoints: 6, topN, allowedScripts }) → [{ text, latin, phones, score }]`.
- Likely needs a smaller beam or pruning in the browser (native is about 1–2 s per phrase). Run the engine off the main thread (Web Worker).
- The package manager for the front end is still to be chosen.

## Infrastructure

- **Hosting:** GitHub Pages, serving the static web tool (HTML, JS, `.wasm`, data file). No server.
- **Code distribution:** GitHub (`jordanpg/phonocompressor`), with submodules.
- **Secrets:** `ELEVENLABS_API_KEY` via fnox, exposed by mise only to the tasks that call the API (`harness:credits`, `harness:models`, `harness:voices`, `harness:synth`). `fnox.toml` is gitignored.
- **CI:** none yet. `mise run fmt:check` and `mise run lint` are written to be CI-ready.
- **Issue tracking:** GitHub Issues on the public repo, planned on the private user project `phonocompressor` (#8), which is linked to the repo. Managed with the `gh` CLI (2.105; needs the `project` scope). Issue types: Feature, Bug, Task. Board fields: Status, Priority, Size, Iteration. See `workflow.md`.

## Development tools

| Tool | Purpose | Config |
|---|---|---|
| mise (≥ 2026.10.4) | Installs Rust, uv, fnox; runs all tasks | `mise.toml` |
| cargo | Rust build and tests | `Cargo.toml` |
| uv | Python env and runs | `tools/pyproject.toml`, `tools/uv.lock` |
| fnox | Secret injection | `fnox.toml` (gitignored) |
| rustfmt | Rust formatting | `rustfmt.toml` (edition 2024) |
| clippy | Rust lint, warnings as errors | `mise run lint` |
| ruff | Python format, lint, import sort | `[tool.ruff]` in `tools/pyproject.toml` |

### Testing

| Kind | Tool | Command |
|---|---|---|
| Rust unit tests | `cargo test` (inline `#[cfg(test)] mod tests`) | `cargo test --workspace` |
| Engine regression | `phono eval` over the 34 sample messages (`sample/`, gitignored) | `mise run phono eval` |
| Prediction vs. real clips | `phono calibrate` | `mise run phono calibrate` |
| Real TTS experiments | harness `synth` → `transcribe` → `report` | `mise run harness:…` (spends credits) |
| Python tests | none yet | — |
| Front end | to be chosen with the web tool | — |

## Decision log

### Calibrate on ElevenLabs Flash v2.5

**Date:** 2026-11-24 · **Status:** accepted

**Context:** exp0 showed the 2.5-generation models transliterate unfamiliar scripts and read the result as English (reading model T), while v3/v4 read some scripts natively.

**Decision:** Target `eleven_flash_v2_5` (which also covers `eleven_turbo_v2_5`), since it is predictable and bills at 0.5×.

**Consequences:** Model-specific settings live in a `ModelProfile` (v6.2). Other models need their own probe, community-message run and fitted weights.

### Transliteration table from `deunicode`, not `unidecode`

**Date:** 2026-11-25 · **Status:** accepted

**Context:** The table ships inside the WASM bundle and will be published on GitHub. A table derived from GPL-2.0+ `unidecode` would put both under the GPL.

**Decision:** Build the table from `deunicode` (BSD-3) in Rust, with ten overrides justified by exp10 clips. Keep `unidecode` for dev comparisons only.

**Consequences:** The repo can be MIT OR Apache-2.0. The 34 sample answers and `calibrate` output were unchanged. `mise run data` no longer needs Python.

### Svelte + Vite for the web tool

**Date:** 2026-11-25 · **Status:** accepted (not yet built)

**Context:** The web tool is one page with a little state, next to a large WASM module and data file.

**Decision:** TypeScript, Svelte and Vite.

**Consequences:** Small runtime overhead; TypeScript style guide applies.

### Static hosting on GitHub Pages

**Date:** 2026-11-25 · **Status:** accepted (not yet deployed)

**Context:** Everything runs client-side, and the code lives on GitHub.

**Decision:** Host the web tool on GitHub Pages.

**Consequences:** No server-side code, keys or storage. The data file must be served as a static asset (or embedded in the `.wasm`).

## Version requirements

| Component | Minimum | Notes |
|---|---|---|
| Rust | stable with edition 2024 (1.85+) | Installed by mise |
| Python | 3.13 | `requires-python = ">=3.13"`, ruff `target-version = "py313"` |
| mise | 2026.10.4 | Older versions reject `secrets` on a task |
| wasm target | `wasm32-unknown-unknown` | For `phono-core` and the planned `crates/wasm` |
