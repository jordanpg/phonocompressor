# Releases

How phonocompressor's artifacts are tagged and released, and the contract a client relies on when it
downloads the data. Today only the data is released; CLI builds will follow under their own tags.

## Tags

Every release is a GitHub Release on a tag `<name>/v<semver>`:

| Artifact | Tag | Released by |
|---|---|---|
| Runtime data (`phono.bin`) | `phono-data/vX.Y.Z` | `.github/workflows/release-data.yml` |
| CLI (later) | `phono-cli/vX.Y.Z` | not set up yet |

- `<name>` is the Cargo package name where there is one. That is the form [dist](https://github.com/axodotdev/cargo-dist) reads as a release of one package in a workspace (`PACKAGE/VERSION`). `@` is not one of its forms. `phono-data` is not a package: it names the data.
- The artifacts release out of step. Retraining the G2P model or bumping `deunicode`, CMUdict or Phoible changes the data without new code, and most code changes need no new data. The data format number ties them: each engine build reads exactly one format (see [Versioning the data](#versioning-the-data)).
- The `.wasm` package is code. It ships with the web tool's deploy, not as a release here.

Tags matching `refs/tags/phono-data/*` are protected by repository ruleset **28262654** ("phono-data tags"): only the repository admin can create, move or delete them. A published tag is never moved. A mistake is fixed by releasing the next version.

## Versioning the data

`phono-data/vX.Y.Z`:

- **X (major)** is the data format, `FORMAT_VERSION` in `crates/core/src/data.rs` (`phono-build --format-version` prints it). It changes when the layout of `phono.bin` changes. An engine rejects a file of any other format, so a new major needs a new engine build. The release workflow fails if the tag's major isn't the format the tagged commit builds.
- **Y (minor)** is a content change: `phono.bin` differs from the previous release (a different `unpacked_sha256`). Examples: new overrides, a retrained model, a newer input.
- **Z (patch)** is a rebuild with the same `phono.bin`: only the notices or the manifest changed.

A new major starts at `X.0.0`. The first release is `phono-data/v2.0.0`.

`phono.bin` is byte-reproducible: two builds from the same commit are identical (`mise run //crates/builder:repro` checks this), so its checksum identifies the data.

## Cutting a data release

1. Make sure `main` holds what you mean to release and CI is green. The release workflow builds from the tag alone and doesn't use data from any earlier build.
2. Build the release locally and compare its data with the latest release to pick the version:

   ```sh
   mise run //crates/builder:release-data 2.1.0 --out target/phono-data-next
   gh release download phono-data/v2.0.0 -p manifest.json -O - | jq -r '.files[0].unpacked_sha256'
   jq -r '.files[0].unpacked_sha256' target/phono-data-next/manifest.json
   ```

   - **Same checksum:** bump the patch, or don't release.
   - **Different checksum:** bump the minor.
   - **A new format:** start the next major at `.0.0`.

   `release-data` refuses a dirty tree and a major that isn't the current format.
3. Tag the commit on `main` and push the tag on its own. GitHub starts no workflow when more than three tags are pushed at once.

   ```sh
   git tag -a phono-data/v2.1.0 -m "phono-data 2.1.0"
   git push origin phono-data/v2.1.0
   ```

4. Watch the run (`gh run watch`). The `build` job:
   - checks out the tag with its submodules;
   - runs `//crates/builder:repro`, which builds twice and fails if the builds differ;
   - runs `//crates/builder:release-data <version>`;
   - checks `SHA256SUMS`.

   The `publish` job runs no repository code. It downloads the build's files and checks:
   - the checksums;
   - that the manifest's version matches the tag;
   - that `source.commit` is the tagged commit and the tree was clean.

   Then it creates the release, with release notes listing the inputs.
5. If the run fails before publishing, fix `main`, delete the tag (`git push origin :refs/tags/phono-data/v2.1.0`; only the admin can) and tag again. Once a release is published, leave it: release the next version instead.

**Dry runs** build the same files and upload them as a workflow artifact, without creating a release:
- automatically on pull requests that touch the workflow, `crates/builder/`, `crates/core/`, `crates/wasm/notices.mjs`, `Cargo.lock` or the submodules;
- by hand with `gh workflow run release-data.yml --ref <branch>`, which needs the workflow on `main`.

A pull request's run builds GitHub's merge commit, so its `source.commit` is that commit.

## Release contents

| File | What it is |
|---|---|
| `phono.bin.gz` | `phono.bin` gzipped at level 9, with no file name and a zero timestamp |
| `manifest.json` | the version, the format, where the data came from, and every file's size and SHA-256 |
| `third-party-notices.html` | the data's notices: the Phoible citation and CC BY 4.0, and the CMUdict and `deunicode` licenses |
| `SHA256SUMS` | checksums of the other three files, for `sha256sum -c` |

`phono.bin` holds:
- the Phoible feature table;
- the CMUdict lexicon;
- the transliteration table (`deunicode` plus the exp10 overrides);
- the G2P model trained on CMUdict.

Anyone redistributing the data must ship `third-party-notices.html` with it.

To check a release by hand:

```sh
gh release download phono-data/v2.0.0 --repo jordanpg/phonocompressor --dir phono-data
cd phono-data && sha256sum -c SHA256SUMS
gunzip -k phono.bin.gz && sha256sum phono.bin   # equals files[0].unpacked_sha256
```

To reproduce one, check out the tag (with submodules) and run `mise run //crates/builder:release-data X.Y.Z`. Its `phono.bin` has the same SHA-256. The `.gz` bytes match too unless zlib's output changed.

## Manifest schema

`manifest.json`, schema 1. This one is from the dry run on PR #68, so `source.commit` is that PR's merge commit:

```json
{
  "schema": 1,
  "name": "phono-data",
  "version": "2.0.0",
  "format": 2,
  "source": {
    "repository": "https://github.com/jordanpg/phonocompressor",
    "commit": "959a30f511ca8b68c7305148fbf5429b0fbe9187",
    "dirty": false
  },
  "inputs": {
    "deunicode": {
      "version": "1.6.2"
    },
    "cmudict": {
      "repository": "https://github.com/cmusphinx/cmudict",
      "commit": "74790861f652b15e4ac49015a90074ad62a27690"
    },
    "phoible": {
      "repository": "https://github.com/phoible/dev",
      "commit": "5f82b9c3fbb0b5c630de20e254c5fdad645e3e56"
    }
  },
  "files": [
    {
      "name": "phono.bin.gz",
      "size": 7238096,
      "sha256": "b16cc688c346c467b9f5a66c782148598f3c60f7a07d817fb89eca4a2147a907",
      "unpacked_name": "phono.bin",
      "unpacked_size": 12642247,
      "unpacked_sha256": "292d99dccfd89d220d2c15576bac0f18eed57cea9452e4810c2bdaecbf536d69"
    },
    {
      "name": "third-party-notices.html",
      "size": 6265,
      "sha256": "297091210f41b501407dd82041b334af41c6113b7a3f7fbf56c2b432f5dc7efe"
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `schema` | Version of this layout. It is bumped only when a field changes meaning or is removed. Added fields don't bump it, so readers ignore fields they don't know. |
| `name` | Always `phono-data`. |
| `version` | `X.Y.Z`, the tag without `phono-data/v`. |
| `format` | The data format, equal to `X`. An engine loads the file only if this equals its `FORMAT_VERSION`. |
| `source.repository`, `source.commit` | Where the files were built from. For a release, the commit is the tagged commit. |
| `source.dirty` | Whether the tree had uncommitted changes. Always `false` in a release; `true` only in a local `--allow-dirty` build. |
| `inputs` | What the data was built from: the `deunicode` crate version, and the CMUdict and Phoible submodule commits. |
| `files[]` | One entry per file except `manifest.json` and `SHA256SUMS`: `name`, `size` in bytes and `sha256` (lowercase hex). |
| `files[].unpacked_*` | For a compressed file: the name, size and SHA-256 of its contents once unpacked. |

Clients find the data by name (`unpacked_name == "phono.bin"`), not by position in `files`.

## Using a release from the web

This is the contract for the web tool ([#1](https://github.com/jordanpg/phonocompressor/issues/1)) and any other browser client.

**Mirror the files; don't fetch them from GitHub.** Browsers can't read GitHub Release assets:
- neither the `github.com/.../releases/download/...` URL nor the `release-assets.githubusercontent.com` host it redirects to sends `Access-Control-Allow-Origin`;
- assets come as `application/octet-stream`;
- the unauthenticated API allows 60 requests an hour per IP.

So the site's deploy does the fetching:
1. Pick the newest `phono-data` release whose major equals the format of the `.wasm` it deploys. The binding's `version()` reports it, e.g. `0.1.0 (data v2)`.
2. Download that release and check it with `sha256sum -c SHA256SUMS`.
3. Serve it from the site's own origin, for example:

   ```
   data/manifest.json                 the mirrored release's manifest (fetched by clients, not cached)
   data/2.0.0/phono.bin.gz            the release files, under their version (cacheable forever)
   data/2.0.0/third-party-notices.html
   ```

   Serve `phono.bin.gz` as a plain file. If the server adds `Content-Encoding: gzip`, the browser unpacks it before the client's own `DecompressionStream` does. Link the notices page from the tool.

Since the deploy only mirrors data that its `.wasm` reads, `data/manifest.json` always has the format the deployed engine expects.

**The client:**

1. **Fetch the manifest.** On start, fetch `data/manifest.json` without the HTTP cache. Load the copy kept in browser storage (`phono.bin` and its manifest), if there is one. If the manifest can't be fetched, use the stored copy, provided its format is the engine's.
2. **Decide whether to download:**

   | Stored copy | Action |
   |---|---|
   | None, or a different `format` from the served manifest | Download now, without asking. The stored file can't be loaded anyway. |
   | Same format, same `unpacked_sha256` | Keep it. Store the new manifest if the version changed (a patch release). |
   | Same format, newer `version`, different `unpacked_sha256` | Keep using the stored copy and offer the update. Download only when the user accepts. |
   | Same format, version not newer | Keep it. |

   "Newer" compares `X.Y.Z` numerically.
3. **Download and verify.**
   - Fetch `data/<version>/phono.bin.gz` and unpack it with `DecompressionStream('gzip')`.
   - Check the size against `unpacked_size` and the SHA-256 (`crypto.subtle.digest`) against `unpacked_sha256`. Reject the file on any mismatch.
   - Only then replace the stored copy with the new bytes and their manifest.
   - `new Engine(bytes)` also rejects a file whose format isn't its own, which guards against a bad mirror.

The client checks the unpacked file rather than the `.gz`. Its checksum depends only on the data, not on the compressor.
