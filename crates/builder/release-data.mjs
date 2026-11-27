// Write a phono-data release: everything a `phono-data/vX.Y.Z` GitHub Release holds, built from
// the current tree. The release workflow runs this; so can anyone checking a release locally.
//
//   phono.bin.gz              phono.bin, gzipped (no file name, no timestamp)
//   manifest.json             version, data format, source commit, inputs, each file's size and SHA-256
//   third-party-notices.html  the data's notices (crates/wasm/notices.mjs --data-only)
//   SHA256SUMS                for `sha256sum -c`
//
// phono.bin is built fresh into a temp directory, never taken from data/generated/, so a stale or
// locally changed file can't be released. The version's major must equal the data format
// (`phono-build --format-version`), and the tree must be clean unless --allow-dirty is passed.
// docs/RELEASES.md has the versioning rule and the manifest schema.
//
// Usage: mise run //crates/builder:release-data <X.Y.Z> [--out <dir>] [--allow-dirty]
// (default out: target/phono-data/)

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { gunzipSync, gzipSync } from "node:zlib";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const REPO = "https://github.com/jordanpg/phonocompressor";
const NAME = "phono-data";
/** Version of the manifest's own layout; bumped when a field changes meaning or goes away. */
const SCHEMA = 1;

function fail(message) {
  console.error(`release-data: ${message}`);
  process.exit(1);
}

const { values: flags, positionals } = parseArgs({
  allowPositionals: true,
  options: { out: { type: "string" }, "allow-dirty": { type: "boolean" } },
});
if (positionals.length !== 1) fail("usage: release-data.mjs <X.Y.Z> [--out <dir>] [--allow-dirty]");
const VERSION = positionals[0];
const semver = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(VERSION);
if (!semver) fail(`version ${JSON.stringify(VERSION)} is not X.Y.Z (no leading "v")`);
const OUT = resolve(flags.out ?? join(ROOT, "target/phono-data"));

/** Run a command in the repo root and return its stdout. Its stderr goes to ours. */
function run(cmd, args) {
  try {
    return execFileSync(cmd, args, {
      cwd: ROOT,
      encoding: "utf8",
      maxBuffer: 256 << 20,
      stdio: ["ignore", "pipe", "inherit"],
    });
  } catch (e) {
    fail(`\`${cmd} ${args.slice(0, 3).join(" ")}\` failed (${e.status ?? e.code}); see above`);
  }
}

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

// --- Checks, before the build ---

const dirty = run("git", ["status", "--porcelain", "--untracked-files=no"]).trim();
if (dirty && !flags["allow-dirty"]) {
  fail(`the tree has uncommitted changes; commit them, or pass --allow-dirty for a local check:\n${dirty}`);
}

const builder = ["run", "--release", "--locked", "-q", "-p", "phono-builder", "--"];
const format = Number(run("cargo", [...builder, "--format-version"]).trim());
if (!Number.isInteger(format)) fail("`phono-build --format-version` printed no number");
if (Number(semver[1]) !== format) {
  fail(`version ${VERSION}: the major must equal the data format, ${format} (e.g. ${format}.0.0)`);
}

// --- Inputs ---

/** A submodule's web URL and the commit the superproject records for it. */
function submodule(name) {
  const url = run("git", ["config", "-f", ".gitmodules", "--get", `submodule.${name}.url`]);
  const commit = run("git", ["rev-parse", `HEAD:${name}`]).trim();
  return { repository: url.trim().replace(/\.git$/, ""), commit };
}

const metadata = JSON.parse(run("cargo", ["metadata", "--format-version", "1", "--locked"]));
const deunicode = metadata.packages.find((p) => p.name === "deunicode");
if (!deunicode) fail("deunicode is no longer a dependency; update the manifest's inputs");

// --- Build and package ---

const tmp = mkdtempSync(join(tmpdir(), "phono-data-"));
let bin;
try {
  const binPath = join(tmp, "phono.bin");
  try {
    // The build log is long (EM iterations, sample words); show it only if the build fails.
    execFileSync("cargo", [...builder, "--out", binPath], {
      cwd: ROOT,
      stdio: ["ignore", "pipe", "pipe"],
      maxBuffer: 256 << 20,
    });
  } catch (e) {
    console.error(`${e.stdout ?? ""}${e.stderr ?? ""}`);
    fail(`phono-build failed (${e.status ?? e.code}); see above`);
  }
  bin = readFileSync(binPath);
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

// Node's gzip header has no file name and a zero mtime, so the bytes depend only on the data and
// zlib's version. Clients check unpacked_sha256, which depends on neither.
const gz = gzipSync(bin, { level: 9 });
if (!gunzipSync(gz).equals(bin)) fail("gzip round trip changed phono.bin");

mkdirSync(OUT, { recursive: true });
writeFileSync(join(OUT, "phono.bin.gz"), gz);
run("node", [
  join(ROOT, "crates/wasm/notices.mjs"),
  "--data-only",
  "--out",
  join(OUT, "third-party-notices.html"),
]);
const notices = readFileSync(join(OUT, "third-party-notices.html"));

const manifest = {
  schema: SCHEMA,
  name: NAME,
  version: VERSION,
  format,
  source: {
    repository: REPO,
    commit: run("git", ["rev-parse", "HEAD"]).trim(),
    dirty: dirty !== "",
  },
  inputs: {
    deunicode: { version: deunicode.version },
    cmudict: submodule("cmudict"),
    phoible: submodule("phoible"),
  },
  files: [
    {
      name: "phono.bin.gz",
      size: gz.length,
      sha256: sha256(gz),
      unpacked_name: "phono.bin",
      unpacked_size: bin.length,
      unpacked_sha256: sha256(bin),
    },
    { name: "third-party-notices.html", size: notices.length, sha256: sha256(notices) },
  ],
};
const manifestBytes = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`);
writeFileSync(join(OUT, "manifest.json"), manifestBytes);

const sums = [
  ["phono.bin.gz", gz],
  ["manifest.json", manifestBytes],
  ["third-party-notices.html", notices],
]
  .map(([name, bytes]) => `${sha256(bytes)}  ${name}\n`)
  .join("");
writeFileSync(join(OUT, "SHA256SUMS"), sums);

console.log(
  `release-data: wrote ${NAME} ${VERSION} (format ${format}) to ${OUT}\n` +
    `  phono.bin ${(bin.length / 1e6).toFixed(1)} MB, gzipped ${(gz.length / 1e6).toFixed(1)} MB, ` +
    `sha256 ${manifest.files[0].unpacked_sha256}` +
    (manifest.source.dirty ? "\n  warning: built from a dirty tree (--allow-dirty)" : ""),
);
