# Tech Stack

## Overview

| Layer | Choice | Status |
|---|---|---|
| Engine | Rust (edition 2024, stable) | Working |
| Data build | Rust (`phono-builder`) | Working |
| CLI | Rust (`phono-cli`, clap) | Working |
| Browser binding | Rust (`phono-wasm`, wasm-bindgen + tsify) | Working: matches the CLI, runs in Chrome and Firefox |
| Test harness | Python ≥ 3.13 (uv) | Working |
| Web front end | TypeScript + Svelte + Vite, over the `phono-wasm` module | Planned (Phase 3) |
| Backend | None: the engine runs client-side | — |
| Database | None: one postcard-encoded data file | — |
| Hosting | GitHub Pages (static) | Planned |
| CI | GitHub Actions (`.github/workflows/ci.yml`), through mise | Working |
| Data releases | GitHub Releases tagged `phono-data/vX.Y.Z`, built by `.github/workflows/release-data.yml` (`docs/RELEASES.md`) | Working; first tag after merge |

## Rust workspace

Cargo workspace, `resolver = "3"`, version 0.1.0, license `Apache-2.0`.

| Crate | Binary | Purpose | Dependencies |
|---|---|---|---|
| `crates/core` (`phono-core`) | — | Phone features, G2P, reader, scorer, search. No I/O; builds for `wasm32`. | `serde` 1 (derive), `postcard` 1 (alloc) |
| `crates/builder` (`phono-builder`) | `phono-build` | Builds `data/generated/phono.bin` from Phoible, CMUdict, the G2P model and the transliteration table; `--format-version` prints the data format. Tasks `repro` (two builds must be byte-identical) and `release-data` (`release-data.mjs`: writes a `phono-data` release) | `phono-core`, `clap` 4, `deunicode` 1.6 |
| `crates/cli` (`phono-cli`) | `phono` | `compress` (`--json`), `score`, `read`, `phones`, `spell`, `eval`, `calibrate`, `fit` | `phono-core`, `clap` 4, `serde_json` 1 |
| `crates/wasm` (`phono-wasm`) | — | Browser binding: `Engine` from the data file's bytes and a typed `compress()`; option checks. Also a test page (`www/`), its dev server (`serve.mjs`), the CLI parity check (`parity/`) and the third-party notices generator (`notices.mjs`, `about.toml`) | `phono-core`, `wasm-bindgen` =0.2.130, `tsify` 0.5 (`js`), `serde-wasm-bindgen` 0.6, `js-sys` 0.3 |

Build profiles: dependencies build at `opt-level = 2` and the workspace at `opt-level = 1` in dev builds, so the builder's G2P training stays usable without `--release`.

### Data file

- `data/generated/phono.bin`, postcard-encoded, built by `mise run data` (about 15 s). Not committed.
- 12.5 MB since v3, mostly unpruned G2P tables (u64 keys + f32). Shrinking it (delta-coded keys, u16 log-probs, entropy pruning) matters for the WASM bundle.
- Byte-reproducible: `Translit.map` and `Lexicon.words` stay `HashMap`s but are written in key order (`data::sorted_map`), so the same commit always gives the same bytes. `mise run //crates/builder:repro` checks a full build.
- Format `FORMAT_VERSION` (2, in `crates/core/src/data.rs`); an engine rejects any other. Published as `phono-data/vX.Y.Z` releases with major = format (`docs/RELEASES.md`).

### Data sources

| Source | Where | Used for | License |
|---|---|---|---|
| Phoible | `phoible/` (submodule) | Phone feature vectors / distance | CC BY 4.0 |
| CMUdict | `cmudict/` (submodule) | English pronunciations, G2P training | BSD-style |
| `deunicode` crate | Cargo | Transliteration table, plus overrides from exp10 in `crates/builder/src/translit.rs` | BSD-3-Clause |
| Python `unidecode` | `tools/` | Dev-only comparison; never feeds `phono.bin` | GPL-2.0+ |

## Python harness (`tools/`)

Package `phono-harness` 0.1.0, built with `uv_build`, run through the mise tasks in `tools/mise.toml` (`//tools:…`). Its virtualenv lives in `~/.cache/phonocompressor/tools-venv` (`UV_PROJECT_ENVIRONMENT`, set in `tools/mise.toml`).

| Dependency | Purpose |
|---|---|
| `httpx` ≥ 0.28 | ElevenLabs API |
| `numpy` ≥ 2.0 | Audio and metrics (`asr` group) |
| `torch` ≥ 2.6 | Local ASR, GPU intended (`asr` group) |
| `transformers` ≥ 4.56 | Whisper (words) and wav2vec2 (IPA phones) (`asr` group) |
| `unidecode` ≥ 1.3 | Dev-only reading-model-T predictions (GPL; never shipped) |

The ASR packages are a dependency group, `asr`, on by default (`default-groups`); `//tools:test` leaves it out (`--no-group asr`), so the tests and CI don't download torch.

Modules: `cli`, `elevenlabs`, `asr`, `cache`, `experiments`. Experiments are defined in `tools/experiments/exp*.toml`. Clips and transcripts are cached in `.cache/harness/`, and reports go to `tools/results/` (both gitignored).

## Web front end (planned)

- **TypeScript + Svelte + Vite.** A single-page tool with a small amount of interactive state (phrase, budget, model, results). Svelte compiles away most of its runtime, which keeps the bundle small next to a multi-megabyte data file.
- **Engine binding:** `crates/wasm` (built). `new Engine(bytes)` with `phono.bin`'s bytes, then
  `engine.compress(phrase, { model, maxCodepoints, topN, allowedScripts }) → [{ text, latin, phones, score }]`.
  `mise run //crates/wasm:build` writes the `--target web` package and its `.d.ts` to `target/wasm-pkg`.
- Run the engine in a Web Worker. Measured (PLAN.md, Phase 3): ready in about 0.4 s after the
  download, 2–6 s for a first search (1.1–1.8× native), faster on repeats. The download is
  mostly `phono.bin` (7.5 MB gzipped).
- The package manager for the front end is still to be chosen.

## Infrastructure

- **Hosting:** GitHub Pages, serving the static web tool (HTML, JS, `.wasm`, data file) and its `third-party-notices.html` (`mise run //crates/wasm:notices`; see `THIRD_PARTY.md`). No server. The deploy mirrors the data file from a `phono-data` release, since GitHub Releases sends no CORS headers (`docs/RELEASES.md`).
- **Data releases:** `.github/workflows/release-data.yml`.
  - **On a pushed `phono-data/v*` tag:** it runs `//crates/builder:repro` and `//crates/builder:release-data <version>`, checks the files, then creates a GitHub Release with `phono.bin.gz`, `manifest.json`, the data-only `third-party-notices.html` and `SHA256SUMS`.
  - **Permissions:** only the `publish` job has `contents: write`, and it runs no repository code.
  - **Dry runs:** on pull requests that touch the data's inputs, and on `workflow_dispatch`. They upload the files as a workflow artifact without creating a release.
  - **Tag protection:** `phono-data/*` tags are protected by ruleset 28262654, so only the admin can create, move or delete them.
- **Code distribution:** GitHub (`jordanpg/phonocompressor`), with submodules.
- **Secrets:** `ELEVENLABS_API_KEY` via fnox, exposed by mise only to the tasks that call the API (`//tools:credits`, `//tools:models`, `//tools:voices`, `//tools:synth`). `fnox.toml` is gitignored.
- **CI:** GitHub Actions, `.github/workflows/ci.yml`, on every pull request and push to `main`. Job `checks` runs `mise run ci:affected origin/<base>` on a pull request, the same task as the pre-push hook, and `mise run ci` (fmt:check, lint, test, wasm32 check, everywhere) on `main`. Job `wasm` runs `mise run data` and `mise run //crates/wasm:parity`, which builds the binding and its notices (the license gate) first. No secrets: nothing in CI calls ElevenLabs or needs `sample/library.json`, so `phono eval` stays a local gate. Actions are pinned by commit SHA.
- **Git hooks:** hk (`hk.pkl`), run from the wrappers in `.githooks/`; `mise run hooks` enables them per clone (sets `core.hooksPath`, relative, so a worktree on a branch without `.githooks/` runs none). pre-commit fixes and checks only the staged files (rustfmt, ruff format, ruff), with unstaged work stashed under hk's own refs, not the shared `git stash` list. pre-push runs `mise run ci:affected`. `--no-verify` or `HK=0` skips them.
- **Issue tracking:** GitHub Issues on the public repo, planned on the private user project `phonocompressor` (#8), which is linked to the repo. Managed with the `gh` CLI (2.105; needs the `project` scope). Issue types: Feature, Bug, Task. Board fields: Status, Priority, Size, Iteration. See `workflow.md`.

## Development tools

| Tool | Purpose | Config |
|---|---|---|
| mise (≥ 2026.11.7) | Installs Rust (with the wasm32 target), uv, fnox, Node, hk, ruff, the wasm-bindgen CLI and cargo-about (`mise install --monorepo`); runs all tasks; picks the projects a change affects (`--affected`) | `mise.toml` (root), `tools/mise.toml`, `crates/*/mise.toml` |
| wasm-bindgen CLI 0.2.130 | Generates the JS glue and `.d.ts` for `crates/wasm`. Must equal the `wasm-bindgen` crate version | `crates/wasm/mise.toml` (`github:wasm-bindgen/wasm-bindgen`), `Cargo.toml` |
| cargo-about 0.9.3 | Finds the license texts of the crates in the `.wasm` for the third-party notices, and fails on a license `about.toml` doesn't accept | `crates/wasm/mise.toml` (`github:EmbarkStudios/cargo-about`), `crates/wasm/about.toml` |
| Node 26 (LTS) | Runs the parity check (`//crates/wasm:parity`), the test page server (`//crates/wasm:serve`), the notices generator (`//crates/wasm:notices`) and the data release script (`//crates/builder:release-data`); later `web/` | `mise.toml` (root) |
| cargo | Rust build and tests | `Cargo.toml` |
| uv | Python env and runs | `tools/pyproject.toml`, `tools/uv.lock` |
| fnox | Secret injection | `fnox.toml` (gitignored) |
| rustfmt | Rust formatting | `rustfmt.toml` (edition 2024) |
| clippy | Rust lint, warnings as errors | `mise run lint` |
| ruff | Python format, lint, import sort (installed by mise) | `[tool.ruff]` in `tools/pyproject.toml` |
| hk 2.14.0 | Git hooks: staged-file fixes on commit, `ci:affected` on push. Must equal the hk package `hk.pkl` imports | `hk.pkl`, `.githooks/` |

### Testing

| Kind | Tool | Command |
|---|---|---|
| Rust unit tests | `cargo test` (inline `#[cfg(test)] mod tests`) | `cargo test --workspace` (or `mise run test`, which adds the harness's pytest) |
| Engine regression | `phono eval` over the community library (`sample/library.json`, gitignored), with the hand-picked 34 as a subset | `mise run phono eval` |
| Prediction vs. real clips | `phono calibrate` | `mise run phono calibrate` |
| Real TTS experiments | harness `synth` → `transcribe` → `report` | `mise run //tools:…` (spends credits) |
| wasm32 build | `phono-core` and `phono-wasm` build for `wasm32-unknown-unknown` | `mise run //crates/wasm:check` |
| Reproducible data | Two full builds of `phono.bin` compared byte for byte (also the release workflow's first step) | `mise run //crates/builder:repro` |
| Binding vs. CLI | Node runs `crates/wasm/parity/cases.json` through both and compares | `mise run //crates/wasm:parity` |
| Binding in a browser | Test page with the engine in a Web Worker; `/?bench` times it | `mise run //crates/wasm:serve` |
| Python tests | none yet | — |
| Front end | to be chosen with the web tool | — |

## Decision log

### Calibrate on ElevenLabs Flash v2.5

**Date:** 2026-11-24 · **Status:** accepted

**Context:** exp0 showed the 2.5-generation models transliterate unfamiliar scripts and read the result as English (reading model T), while v3/v4 read some scripts natively.

**Decision:** Target `eleven_flash_v2_5` (which also covers `eleven_turbo_v2_5`), since it is predictable and bills at 0.5×.

**Consequences:** Model-specific settings live in a `ModelProfile` (v6.2). Other models need their own probe, community-message run and fitted weights.

### Add Multilingual v2 as a second profile

**Date:** 2026-11-26 · **Status:** accepted

**Context:** Goal 2 is more TTS models. exp11–exp14 (about 1,870 credits) found `eleven_multilingual_v2` reads most scripts by transliteration like Flash v2.5, but reads kana, Cyrillic and Devanagari natively, never reads `@` as "at", reads glued digits, and is noisier (about 0.07 lower truth on the same texts).

**Decision:** Ship `MULTILINGUAL_V2` in `PROFILES` with its own script table (kana native; risks for `҈`, syllabics, Ethiopic, conjoining jamo), symbol words and `glued_number_drop`. It keeps Flash's preference order, split costs, weights and prediction settings: once its script risks were set, `phono fit` gave no held-out gain over Flash's weights (78.3–80.8% vs 79.5%). Flash stays the default.

**Consequences:** `ReaderParams` carries the per-model symbol table (`symbol_words`), and `phono fit --prior profile` can start a new model from an existing profile. The capital-letter and plural rules stay shared, since Multilingual v2's clips agree with them. Its live answers beat the community's on that model but don't beat Flash's answers there.

### Transliteration table from `deunicode`, not `unidecode`

**Date:** 2026-11-25 · **Status:** accepted

**Context:** The table ships inside the WASM bundle and is published on GitHub (inside `phono.bin`, in the `phono-data` releases). A table derived from GPL-2.0+ `unidecode` would put both under the GPL.

**Decision:** Build the table from `deunicode` (BSD-3) in Rust, with ten overrides justified by exp10 clips. Keep `unidecode` for dev comparisons only.

**Consequences:** The repo is Apache-2.0. The 34 sample answers and `calibrate` output were unchanged. `mise run data` no longer needs Python.

### Tags `<name>/v<semver>`; data versioned by its format

**Date:** 2026-11-27 · **Status:** accepted (`data-release_20261127`, [#9](https://github.com/jordanpg/phonocompressor/issues/9))

**Context:** The web tool should download `phono.bin` once, keep it, and offer updates. The data changes without code (retrained G2P, overrides, input bumps) and code without data, so they need separate versions. CLI builds will be released too, probably with dist.

**Decision:**
- **Tags:** every artifact is released on a tag `<name>/v<semver>`, named after the Cargo package where there is one (`phono-cli/v…` later). dist reads a `PACKAGE/VERSION` tag as a release of one package; `@` is not one of its forms.
- **Data versions:** `phono-data/vX.Y.Z`. X is `FORMAT_VERSION`, Y a change to `phono.bin`, and Z a rebuild with the same `phono.bin`.
- **Release contents:** the release is `phono.bin.gz` (Node zlib, level 9, no name or timestamp), `manifest.json` (schema 1), the data-only notices and `SHA256SUMS`.
- **Where releases are built:** always in CI from the tag, never from a local file.

**Consequences:**
- `phono.bin` had to become byte-reproducible (sorted map serialization; same format, same reader).
- Clients check `unpacked_sha256`, which doesn't depend on the compressor.
- Browsers can't fetch release assets (no CORS), so the web deploy mirrors them.
- No readable transliteration table is published.
- When dist is added, its tag-triggered workflow must ignore `phono-data/*` tags.

### Svelte + Vite for the web tool

**Date:** 2026-11-25 · **Status:** accepted (not yet built)

**Context:** The web tool is one page with a little state, next to a large WASM module and data file.

**Decision:** TypeScript, Svelte and Vite.

**Consequences:** Small runtime overhead; TypeScript style guide applies.

### Static hosting on GitHub Pages

**Date:** 2026-11-25 · **Status:** accepted (not yet deployed)

**Context:** Everything runs client-side, and the code lives on GitHub.

**Decision:** Host the web tool on GitHub Pages.

**Consequences:** No server-side code, keys or storage. The data file is served as a static asset next to the `.wasm` (see "Fetch the data file at run time").

### Fetch the data file at run time, don't embed it

**Date:** 2026-11-25 · **Status:** accepted (`wasm-binding_20261125`)

**Context:** PLAN.md planned to embed `phono.bin` (12.6 MB) in the `.wasm`.

**Decision:** The page fetches `phono.bin` and passes its bytes to `new Engine(bytes)`.

**Consequences:** The `.wasm` stays at 150 KB gzipped. The browser caches code and data separately, so a code change doesn't download the data again, and the raw bytes can be freed after decoding (0.3–0.4 s). The front end has to fetch two files and handle a failed data download.

### `tsify` for the binding's TypeScript types

**Date:** 2026-11-25 · **Status:** accepted (`wasm-binding_20261125`)

**Context:** wasm-bindgen doesn't generate interfaces for plain option and result objects, and the types must be generated, not hand-written.

**Decision:** `tsify` 0.5 (MIT OR Apache-2.0; `tsify-next` is deprecated in its favour) with its `Ts<T>` wrapper and the `js` feature (serde-wasm-bindgen). Options are read field by field because serde-wasm-bindgen ignores unknown keys.

**Consequences:** `phono_wasm.d.ts` has typed `CompressOptions` and `Guess` with no `any`. The conversion layer is about 3% of the `.wasm`'s code.

### mise monorepo layout for tasks

**Date:** 2026-11-25 · **Status:** accepted ([#24](https://github.com/jordanpg/phonocompressor/issues/24))

**Context:** One flat `mise.toml` held the tasks of three projects (Rust workspace, `tools/` harness, `crates/wasm`), and the web front end would add a fourth.

**Decision:** Use mise's monorepo layout. The root keeps shared tools, fnox secrets and workspace tasks (`data`, `phono`, `fmt`, `fmt:check`, `lint`); `tools/` and `crates/wasm/` (later `web/`) have their own `mise.toml`, and their tasks are named by path (`//tools:synth`, `//crates/wasm:build`). No aliases for the old names. The harness tasks share a task template for `uv run phono-harness`.

**Consequences:** Root `fmt`, `fmt:check` and `lint` run cargo, then each project's task of the same name (`depends_post = ["//...:fmt"]`), so a new project only adds its own tasks. mise forbids `secrets` in templates, so each API task lists the key itself. The wasm32 target stays on the root `rust`, because a second `rust@stable` request with other options breaks `mise install --monorepo`. Plain `mise install` only installs the root's tools; use `--monorepo`. Inside a project directory, a bare task name only finds that project's tasks.

### Git hooks with hk, gates scoped with `mise run --affected`

**Date:** 2026-11-26 · **Status:** accepted

**Context:** Hooks that run the whole repo's gates on every commit or push get slower with each project, and the web front end will add the slowest (type checks, browser tests). Most changes touch one project.

**Decision:** Hooks are hk's (`hk.pkl`). pre-commit fixes and checks the staged files with per-file tools only (rustfmt, ruff format, ruff). pre-push, and CI on pull requests, run `mise run ci:affected`: `lint`, `test` and `check` in the projects changed since the base and in the projects that depend on them, using mise's project graph. Cargo crates and their path dependencies come from `Cargo.toml`; `tools/`, `docs/` and `conductor/` are declared in `[monorepo.projects]`, since a file outside every project counts as a change to all of them. Each crate has its own `lint` and `test` (root templates `cargo:lint`, `cargo:test`); root `lint` and `test` only gather every project's. CI on `main` runs every gate (`mise run ci`). `fmt:check` stays whole-repo; it takes under a second.

**Consequences:** A change to one crate lints and tests only that crate and its dependents; a docs or conductor change runs no lint or tests; root files (`mise.toml`, `Cargo.lock`, `README.md`, ...) and the submodules select everything. Clippy and tests now run once per crate, so dependencies can build twice with different features. Not `hk install`: it writes into `.git/hooks`, which worktrees share, and fails on branches without `hk.pkl`. The `.githooks/` wrappers resolve per worktree. When `web/` lands, mise finds it as a Node project; add `depends_add = ["cargo:phono-wasm"]` so a core or binding change runs its tests too.

## Version requirements

| Component | Minimum | Notes |
|---|---|---|
| Rust | stable with edition 2024 (1.85+) | Installed by mise |
| Python | 3.13 | `requires-python = ">=3.13"`, ruff `target-version = "py313"` |
| mise | 2026.11.7 | Older versions reject `secrets` on a task (before 2026.10.4) or can't find a project's own task templates from another project |
| wasm target | `wasm32-unknown-unknown` | For `phono-core` and `crates/wasm`; `targets` on `rust` in the root `mise.toml` |
| wasm-bindgen | 0.2.130, exactly | The crate and the CLI must match |
| Node | 26 | Parity check and test page server |
