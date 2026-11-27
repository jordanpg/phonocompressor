# Specification: Publish the runtime data release

**Track ID:** data-release_20261127
**Type:** Feature
**Created:** 2026-11-27
**Status:** Complete (2026-11-27; archived after the first release is validated)
**Issue:** [#9 Publish the transliteration tables](https://github.com/jordanpg/phonocompressor/issues/9)

## Summary

Publish the files the engine needs at run time, which a fresh machine would otherwise have to fetch and build, as versioned GitHub Releases. Each release holds a byte-reproducible `phono.bin` (gzipped), a manifest, the data's third-party notices and checksums.

## Context

- PLAN.md's "Transliteration source", step 5, asked to publish "the tables". Discussed on 2026-11-27, the intent is the runtime data: the future web tool (#1) downloads the heavy assets once, keeps them in browser storage, and offers an update when a newer compatible version is out. The readable transliteration TSV is dropped.
- `phono.bin` (postcard, 12.5 MB, about 7.5 MB gzipped) is the only runtime file that takes a real build. It holds the Phoible feature table, the CMUdict lexicon, the transliteration table (`deunicode` plus the exp10 overrides) and the G2P model (EM alignment and an order-5 Kneser-Ney n-gram trained on CMUdict). Building it needs the submodules and `mise run data` (about 15 s).
- The `.wasm` package is code, released with the web tool rather than here. CLI builds will be released later, under their own tags.
- **Builds are not byte-reproducible today.** `Translit.map` and `Lexicon.words` are `std::HashMap`s with random seeds, and postcard writes them in iteration order. Two builds from the same commit hold the same data but differ in bytes and checksums, so a client would see a new version on every rebuild.
- **The local `data/generated/` in the main checkout is stale.** Its `phono.bin` and `translit.tsv` (2026-11-24) predate the switch to `deunicode` and are derived from GPL-2.0+ `unidecode`. They must never be published, so releases are built in CI from a tag.
- Redistributing `phono.bin` needs the Phoible citation and CC BY 4.0 notice and the CMUdict and `deunicode` license texts. `crates/wasm/notices.mjs` already writes them (`dataSection()`), along with the `.wasm`'s crate licenses, which don't belong in a data release.
- **GitHub Releases can't serve browsers directly.** Checked on 2026-11-27: neither the `github.com/.../releases/download/...` redirect nor `release-assets.githubusercontent.com` sends `Access-Control-Allow-Origin`, and release assets come as plain `application/octet-stream` with no gzip encoding. The unauthenticated API allows 60 requests an hour per IP.
- **Slash tags work** (checked against kustomize's `kustomize/v5.8.3`): the release page, the asset download URL, `gh release view` and `gh release download` all accept them. `gh release create` with a slash tag hasn't been run here.
- The artifacts will release out of step: data changes (G2P retrains, overrides, `deunicode`/CMUdict/Phoible bumps) don't need new code, and code releases don't need new data. `FORMAT_VERSION` in `crates/core/src/data.rs` (2) is what ties them: each code build reads exactly one format.

## User Story

As the maintainer of the future web tool, and anyone else running the engine, I want a versioned, checksummed `phono.bin` release I can mirror and verify, so that clients can cache the data and offer updates without building it themselves.

## Acceptance Criteria

- [x] **Reproducible.** Two `mise run data` builds from the same commit produce byte-identical `phono.bin`, checked by a unit test on the serialized maps and a `//crates/builder:repro` task over the full build. The layout is unchanged: `FORMAT_VERSION` stays 2 and existing files still load.
- [x] **Tag convention.** Releases are tagged `<name>/v<semver>`, with the Cargo package name where there is one (`phono-cli/v0.1.0` later). Data uses `phono-data/vX.Y.Z`: major = `FORMAT_VERSION`, minor = a content change, patch = a notices- or metadata-only rebuild.
- [x] **Release workflow.** Pushing a `phono-data/v*` tag builds from that tag in CI and creates a GitHub Release with `phono.bin.gz`, `manifest.json`, a data-only `third-party-notices.html` and `SHA256SUMS`. The workflow fails if the tag's major isn't `FORMAT_VERSION` or if two builds differ. A `workflow_dispatch` dry run uploads the same files as a workflow artifact without creating a release. (Verified by pull request dry runs; `workflow_dispatch` works once the workflow is on `main`, and the publish job first runs on the first real tag.)
- [x] **Tag protection.** A repository ruleset restricts creating, updating and deleting `phono-data/*` tags to the repository admin.
- [x] **Contract.** `docs/RELEASES.md` sets out the tag convention, the versioning rule, the manifest schema, and how a web client should use a release: the deploy mirrors it to the site's origin, the client checks the SHA-256 of the unpacked file, a new format downloads without asking, and only a newer compatible version prompts.
- [x] **Docs.** README "Data and licenses", THIRD_PARTY.md, `conductor/tech-stack.md` and PLAN.md (step 5 marked done, the stale branch note fixed) describe the release. #9 is closed when the track is verified.

## Dependencies

- Existing code: `phono-build` (`crates/builder`), `Data` and `FORMAT_VERSION` (`crates/core/src/data.rs`), `Translit` (`crates/core/src/chunks.rs`), `Lexicon` (`crates/core/src/lexicon.rs`), `crates/wasm/notices.mjs`, `.github/workflows/ci.yml` (mise setup, SHA-pinned actions).
- No other track. #1 builds on the manifest contract but isn't needed here.

## Out of Scope

- The readable transliteration TSV and per-script tables.
- CLI builds and releases (`phono-cli/v*`, probably with dist), including giving `phono-cli` its own version and deciding how a released CLI gets its data. Offered as a new issue.
- The web tool's download, storage and update logic, and its deploy (#1). Releasing the `.wasm` package.
- Shrinking `phono.bin` (#3).
- Licensing the repo. It is already Apache-2.0 only (`Cargo.toml`, `LICENSE-APACHE`), which replaced the plan's `MIT OR Apache-2.0`. The docs task only corrects PLAN.md's step 6 wording to match.
- Pushing the first real `phono-data/v2.0.0` tag. That happens after merge, from `main`, only with the user's go-ahead.

## Technical Notes

- **Reproducibility:** keep the `HashMap` types for lookups and add serde `serialize_with` helpers that write entries in sorted order. Postcard encodes a map as length + entries either way, so deserialization and the format are unchanged. The G2P model's maps use a fixed hasher and training is fed sorted words; the repro task confirms they are stable rather than assuming it.
- **Compression:** `gzip -n -9` (no name or timestamp), so the gzip is reproducible on one gzip version. The manifest records the SHA-256 of both the gzip and the unpacked `phono.bin`. Clients check the unpacked one, which doesn't depend on the gzip tool, and unpack with `DecompressionStream('gzip')`.
- **Manifest (draft):** `name`, `version`, `format`, the source commit, the inputs (`deunicode` version, CMUdict and Phoible submodule commits), and per file `name`, `size`, `sha256`, plus `unpacked_size` and `unpacked_sha256` for `phono.bin.gz`.
- **Format check:** a `phono-build --format-version` flag prints `FORMAT_VERSION`, so the workflow compares the tag's major without grepping the source.
- **Notices:** a `--data-only` flag on `notices.mjs` writes only `dataSection()`. It shouldn't need cargo-about.
- **Workflow:** `permissions: contents: write`, actions pinned by commit SHA like `ci.yml`, `mise run data` twice and `cmp`, then `gh release create`. Release notes list the inputs and link `docs/RELEASES.md`.
- **Tag ruleset:** created with `gh api` (repository rulesets, target `tag`, include `refs/tags/phono-data/*`, rules creation/update/deletion, bypass for the admin role). This changes repo settings, so it's applied only after the user confirms the exact payload.

---

_Generated by Conductor. Review and edit as needed._
