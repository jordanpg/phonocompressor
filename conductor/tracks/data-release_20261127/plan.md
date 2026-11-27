# Implementation Plan: Publish the runtime data release

**Track ID:** data-release_20261127
**Spec:** [spec.md](./spec.md)
**Issue:** [#9](https://github.com/jordanpg/phonocompressor/issues/9)
**Created:** 2026-11-27
**Status:** [~] In Progress

## Overview

Make `phono.bin` byte-reproducible first, since checksums and update prompts mean nothing without it. Then build the release files locally (data-only notices, gzip, manifest, checksums) with one mise task, so CI only runs that task. Then add the release workflow and protect the tag namespace. Last, write the contract for #1 and update the docs. Every task stops for the user to verify it (`workflow.md`).

Setup in this worktree: `git submodule update --init` before the first build or test.

## Issues

- Track: [#9 Publish the runtime data release](https://github.com/jordanpg/phonocompressor/issues/9), a Feature issue. P2.
- Each task is a Task sub-issue of #9, titled `<phase>.<task> <description>`, all P2. Sizes are on the board.

## Phase 1: Reproducible data file

Same commit, same bytes, without changing the format.

### Tasks

- [x] Task 1.1: Serialize `Translit.map` (`crates/core/src/chunks.rs`) and `Lexicon.words` (`crates/core/src/lexicon.rs`) in sorted key order with serde `serialize_with` helpers, keeping the `HashMap` types. Tests first: two maps with the same entries built in different insertion orders serialize to identical bytes, and the bytes still deserialize into the same `Data`. `FORMAT_VERSION` stays 2. ([#59](https://github.com/jordanpg/phonocompressor/issues/59))
- [~] Task 1.2: Add `phono-build --format-version` (prints `FORMAT_VERSION` and exits) and a `//crates/builder:repro` task that builds `phono.bin` twice into a temp directory and fails unless they're byte-identical. If the G2P model's maps turn out unstable, sort them too. ([#60](https://github.com/jordanpg/phonocompressor/issues/60))

### Verification

- [ ] `mise run //crates/builder:repro` passes.
- [ ] `phono eval` output is identical before and after.
- [ ] A `phono.bin` built before the change still loads (same format).
- [ ] Gates pass (`mise run ci:affected`).

## Phase 2: Release files

One task produces everything a release contains, so CI and a local check run the same thing.

### Tasks

- [ ] Task 2.1: Add `--data-only` to `crates/wasm/notices.mjs`: write only the data notices (Phoible citation and CC BY 4.0, CMUdict and `deunicode` license texts), without running cargo-about. The existing output is unchanged without the flag. ([#61](https://github.com/jordanpg/phonocompressor/issues/61))
- [ ] Task 2.2: Add a `//crates/builder:release-data` task (Node script, as `notices.mjs` is) that writes `<out>/phono.bin.gz` (gzip, no name or timestamp), `manifest.json`, `third-party-notices.html` (from 2.1) and `SHA256SUMS`, given a version. The manifest records name, version, format (from `--format-version`), source commit, inputs (`deunicode` version from `cargo metadata`, CMUdict and Phoible submodule commits), and each file's size and SHA-256, plus `unpacked_size` and `unpacked_sha256` for the data file. It fails if the version's major isn't the format, or if the tree is dirty unless `--allow-dirty` is passed. ([#62](https://github.com/jordanpg/phonocompressor/issues/62))

### Verification

- [ ] Running the task twice on the same commit gives identical files.
- [ ] `sha256sum -c SHA256SUMS` passes, and gunzipping `phono.bin.gz` gives the `unpacked_sha256` in the manifest.
- [ ] A wrong major (e.g. `3.0.0`) is rejected.
- [ ] The data-only notices page opens and holds the three data notices and no crate list.

## Phase 3: Workflow and tag protection

### Tasks

- [ ] Task 3.1: Add `.github/workflows/release-data.yml`. On a pushed `phono-data/v*` tag: check out the tag with submodules, run `//crates/builder:repro`, then `//crates/builder:release-data` with the tag's version, then `gh release create <tag>` with the four files and notes listing the inputs. A dry run uploads the same files as a workflow artifact instead: on `workflow_dispatch`, and on pull requests that touch the workflow, `crates/builder/` or `notices.mjs` (`workflow_dispatch` only works once the file is on `main`). `permissions: contents: write` only on the publishing job, actions pinned by SHA as in `ci.yml`. ([#63](https://github.com/jordanpg/phonocompressor/issues/63))
- [ ] Task 3.2: Create a tag ruleset with `gh api`: target `tag`, include `refs/tags/phono-data/*`, rules creation, update and deletion, bypass for the repository admin role. Show the user the exact payload and apply it only after they confirm. Record the ruleset id in `docs/RELEASES.md`. ([#64](https://github.com/jordanpg/phonocompressor/issues/64))

### Verification

- [ ] The PR's dry run passes, and its artifact matches a local `release-data` run on the same commit (same `unpacked_sha256`).
- [ ] The ruleset is listed (`gh api repos/jordanpg/phonocompressor/rulesets`) with the intended target, rules and bypass.

## Phase 4: Contract and docs

### Tasks

- [ ] Task 4.1: Write `docs/RELEASES.md` ([#65](https://github.com/jordanpg/phonocompressor/issues/65)):
  - the tag convention for every artifact (`<name>/v<semver>`, Cargo package names where they exist, the dist compatibility reason);
  - the data versioning rule and how to cut a data release;
  - the manifest schema;
  - how a web client should use a release: the deploy mirrors the release to the site's origin (Releases sends no CORS headers), the client checks `unpacked_sha256`, a new format downloads without asking, and only a newer compatible version prompts.
- [ ] Task 4.2: Update the docs ([#66](https://github.com/jordanpg/phonocompressor/issues/66)):
  - README "Data and licenses" and THIRD_PARTY.md: where the data is published, that its notices ship with it, and that releases are built in CI from a tag;
  - `conductor/tech-stack.md`: Infrastructure, plus a decision-log entry for the tag convention and data versioning;
  - PLAN.md "Transliteration source": step 5 done as the data release, step 6 corrected to Apache-2.0 only, the stale branch note fixed;
  - `conductor/index.md` and `tracks.md`.

  Offer a follow-up issue for CLI releases (`phono-cli/v*`).

### Verification

- [ ] The user reviews `docs/RELEASES.md` as the contract [#1](https://github.com/jordanpg/phonocompressor/issues/1) builds on.
- [ ] No doc still describes publishing a readable table.

## Final Verification

- [ ] All acceptance criteria in `spec.md` met
- [ ] Gates pass and the PR's dry run is green
- [ ] Documentation updated
- [ ] Ready for review. After merge, the first tag `phono-data/v2.0.0` is pushed from `main` only with the user's go-ahead (out of scope for the track).

---

_Generated by Conductor. Tasks will be marked [~] in progress and [x] complete._
