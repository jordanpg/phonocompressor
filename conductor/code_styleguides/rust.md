# Rust Style Guide

Conventions for the `phono-core`, `phono-builder` and `phono-cli` crates, written from the code as it stands. Conductor ships no Rust template, so this guide is project-specific throughout. See `general.md` for the cross-language rules.

## Tooling (source of truth)

- **Edition 2024**, stable toolchain (installed by mise). `rustfmt.toml` sets only `edition = "2024"`, so everything else is rustfmt's defaults (100 columns, 4-space indent).
- **Format:** `cargo fmt --all` (part of `mise run fmt`). Check: `mise run fmt:check`.
- **Lint:** `cargo clippy --workspace --all-targets -- -D warnings` (part of `mise run lint`). Clippy warnings are errors. Prefer fixing over `#[allow]`; when an `allow` is needed, put it on the smallest item and say why.
- **Tests:** `cargo test --workspace`.
- Formatting-only commits go in `.git-blame-ignore-revs`.

## Crate boundaries

| Crate | May do | Must not do |
|---|---|---|
| `phono-core` | Pure computation over bytes and data passed in | File, network, env, clock or thread I/O. Dependencies that don't build for `wasm32-unknown-unknown`. |
| `phono-builder` | Read Phoible, CMUdict, `deunicode`; train G2P; write `phono.bin` | Pull in GPL data (e.g. `unidecode` output) except through the explicit `--translit` dev flag |
| `phono-cli` | Read files, parse args, print results | Hold engine logic that the web tool will also need. Move it into core. |
| `crates/wasm` (`phono-wasm`) | wasm-bindgen glue: decode the data file from bytes the caller fetched, check options, map results | Engine logic: anything beyond marshalling and option checks belongs in core |

`phono-core` dependencies stay minimal (today: `serde`, `postcard`). Add shared dependency versions to `[workspace.dependencies]` and refer to them with `.workspace = true`.

## Model-specific values

Anything that depends on which TTS reads the output belongs in a `ModelProfile` (`crates/core/src/model.rs`), not in a module constant:

- how each script is read and its risk, and whether the search uses it by default;
- the per-chunk script preference;
- reader constants (splitting, glued numbers);
- fitted scorer weights.

Target phones, phone distance and search stay model-independent. When a reader rule is still code because only one model has been measured, say so in the module docs (as `model.rs` does).

## Naming

- Standard Rust casing: `snake_case` functions and modules, `PascalCase` types, `SCREAMING_SNAKE_CASE` constants and statics.
- Domain terms are fine as names: `phones`, `chunk`, `tile`, `fidelity`, `translit`, `g2p`. Don't spell them out.
- Profile statics are named for the model: `FLASH_V2_5`.
- Test names describe behaviour: `deletion_costs_follow_position_and_stress`, `tiles_exact_spelling_first`.

## Documentation and comments

- Every module starts with a `//!` doc saying what it does and where it sits in the pipeline. Link related items with intra-doc links (`[`ModelProfile`]`, `[`crate::reading`]`).
- `///` on public items. Enum variants say what they mean for the engine, not just restate the name.
- Tuned constants and rules cite their evidence:

  ```rust
  // Voicing: raised from 1.0 after exp2 (g/k, d/t swaps misheard). Calibrated on exp1+exp2.
  ```

- Point to `docs/PLAN.md` for long rationale instead of copying it into comments.

## Errors

- Fallible functions return `Result<T, String>`. Messages say what failed and, where possible, how to fix it:

  ```rust
  .map_err(|e| format!("{}: {e} (run `mise run data`)", cli.data.display()))?;
  ```

- Use `?` to propagate; `main` in the binaries returns `Result<(), String>`.
- Don't `unwrap()` or `expect()` on data or user input in non-test code. `expect("…")` is acceptable for true invariants, with the invariant stated in the message.
- An unknown model id, missing data file or format mismatch is an error, never a silent fallback.
- Introduce a typed error enum only when a caller (e.g. the wasm binding) needs to branch on the kind of error.

## Data format

- `phono.bin` is postcard-encoded serde data. Bump `data::FORMAT_VERSION` on any change to the serialized layout. Loading already rejects a mismatched version (`data file version 1 != expected 2; rebuild it`) rather than misreading it; keep it that way.
- Size is a user-facing cost for the web tool. Prefer compact types (`u16`/`u32` ids, quantized weights) in new tables, and note the size change in the commit.

## Performance

- Native speed is about 1.5–3 s per phrase; a first search in the browser takes 1.1–1.8× as long (PLAN.md, Phase 3). Measure before optimizing (`--release`, same phrases), and report before/after numbers.
- Avoid allocation in the inner loops of search and scoring; reuse buffers.
- No `unsafe`. There is none today.

## Tests

- Unit tests live inline in each module under `#[cfg(test)] mod tests { use super::*; … }`.
- Prefer small hand-written fixtures (a few TSV lines, a few CMUdict entries) over loading the full data file, so tests stay fast and need no `mise run data`.
- Exploratory dumps that are useful by hand but not assertions go behind `#[ignore]` (e.g. `score::probe::print_distances`).
- Tests pin behaviour that real clips justified: a reader rule, an override, a score ordering. Name the experiment in a comment when that's the source.
- Unit tests don't replace `phono eval` / `phono calibrate` for output quality (see `workflow.md`).

## Imports and structure

- Imports go at the top of the module; rustfmt orders them. Don't hand-sort or add blank-line groups that rustfmt would undo.
- `lib.rs` declares the modules and holds the `Engine` that ties them together. Keep new public API on `Engine` or on clearly named module functions; the wasm binding will wrap it.
- One concept per module (`reading`, `score`, `search`, `tile`, `g2p`, …). Split a module when it gains a second concept, not by line count.
