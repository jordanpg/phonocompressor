// Write the web bundle's third-party notices: one HTML page with the licenses of the Rust crates
// compiled into the .wasm and of the data in phono.bin. The deploy serves it with the bundle.
//
// Crates: cargo-about (config: about.toml) picks each crate's license from its license expression
// and finds the text in the crate's own files. The list is phono-wasm's dependency graph on wasm32
// without proc-macro crates, which only run while compiling (`cargo tree -e no-proc-macro`).
// Our own crates are left out: they are MIT OR Apache-2.0 like the rest of the repo. Fails if a
// crate's text didn't come from its own files: cargo-about then falls back to a generic text with
// no copyright line (for example when a clarification's checksum no longer matches).
//
// Data: the CMUdict and deunicode license texts are read from cmudict/ and the crate's source,
// and the submodule commits from git, so they match what `mise run data` builds from.
//
// Usage: mise run //crates/wasm:notices [--out <file>] (default: target/wasm-pkg/third-party-notices.html)

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const TARGET = "wasm32-unknown-unknown";
const REPO = "https://github.com/jordanpg/phonocompressor";

const { values: flags } = parseArgs({ options: { out: { type: "string" } } });
const OUT = flags.out
  ? resolve(flags.out)
  : join(ROOT, "target/wasm-pkg/third-party-notices.html");

function fail(message) {
  console.error(`notices: ${message}`);
  process.exit(1);
}

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
    fail(`\`${cmd} ${args.slice(0, 2).join(" ")}\` failed (${e.status ?? e.code}); see above`);
  }
}

const ESCAPES = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" };
const html = (s) => String(s).replace(/[&<>"]/g, (c) => ESCAPES[c]);
const key = (krate) => `${krate.name}@${krate.version}`;

// Online, so cargo can download crate sources that aren't cached yet; the rest runs offline.
const metadata = JSON.parse(run("cargo", ["metadata", "--format-version", "1", "--locked"]));
const members = new Set(metadata.workspace_members);
const ours = new Set(metadata.packages.filter((p) => members.has(p.id)).map((p) => p.name));

/** name@version of every third-party crate compiled into the .wasm. */
function shippedCrates() {
  const tree = run("cargo", [
    "tree", "--locked", "--offline", "-p", "phono-wasm", "--target", TARGET,
    "-e", "normal,no-proc-macro", "--prefix", "none", "--format", "{p}",
  ]);
  const crates = new Set();
  for (const line of tree.split("\n")) {
    const m = /^(\S+) v(\S+)/.exec(line);
    if (m && !ours.has(m[1])) crates.add(`${m[1]}@${m[2]}`);
  }
  if (crates.size === 0) fail("`cargo tree` listed no third-party crates for phono-wasm");
  return crates;
}

/** The license texts the shipped crates need, each with the crates it covers. */
function crateLicenses(shipped) {
  const about = JSON.parse(
    run("cargo", [
      "about", "generate", "--format", "json", "--locked", "--offline", "--fail",
      "-c", "crates/wasm/about.toml", "-m", "crates/wasm/Cargo.toml",
    ]),
  );
  const licenses = [];
  const covered = new Set();
  for (const license of about.licenses) {
    const used = license.used_by.map((u) => u.crate).filter((c) => shipped.has(key(c)));
    const crates = [...new Map(used.map((c) => [key(c), c])).values()];
    if (crates.length === 0) continue;
    if (!license.source_path) {
      fail(
        `the ${license.id} text for ${crates.map(key).join(", ")} is cargo-about's generic one, ` +
          "not the crate's own file; add or fix a clarification in crates/wasm/about.toml",
      );
    }
    for (const c of crates) covered.add(key(c));
    licenses.push({ id: license.id, name: license.name, text: license.text, crates });
  }
  const missing = [...shipped].filter((k) => !covered.has(k));
  if (missing.length > 0) fail(`cargo-about found no license for ${missing.join(", ")}`);
  return licenses;
}

/** A submodule's web URL and the commit the superproject records for it. */
function submodule(name) {
  const url = run("git", ["config", "-f", ".gitmodules", "--get", `submodule.${name}.url`]);
  const commit = run("git", ["rev-parse", `HEAD:${name}`]).trim();
  return { url: url.trim().replace(/\.git$/, ""), commit };
}

function readOrFail(path, fix) {
  if (!existsSync(path)) fail(`${path} not found; ${fix}`);
  return readFileSync(path, "utf8");
}

function dataSection() {
  const phoible = submodule("phoible");
  const cmudict = submodule("cmudict");
  const cmudictLicense = readOrFail(
    join(ROOT, "cmudict/LICENSE"),
    "run `git submodule update --init`",
  );
  const deunicode = metadata.packages.find((p) => p.name === "deunicode");
  if (!deunicode) fail("deunicode is no longer a dependency; update the data notices");
  const deunicodeLicense = readOrFail(
    join(dirname(deunicode.manifest_path), "LICENSE"),
    "the crate's license file moved; update notices.mjs",
  );
  const features =
    `${phoible.url}/blob/${phoible.commit}/raw-data/FEATURES/phoible-segments-features.tsv`;
  return `
<h2 id="data">Data in <code>phono.bin</code></h2>

<section id="phoible">
<h3>PHOIBLE segment features</h3>
<p>
  <code>phono.bin</code> contains PHOIBLE's segment feature table
  (<a href="${html(features)}"><code>phoible-segments-features.tsv</code></a>, commit
  <code>${html(phoible.commit.slice(0, 12))}</code>), which the engine uses to tell how alike two
  sounds are. Changes: the table is re-encoded, and a contour value (a sequence such as
  <code>-,+</code>) keeps only its first and last values.
</p>
<p>
  Moran, Steven &amp; McCloy, Daniel (eds.) 2019. PHOIBLE 2.0. Jena: Max Planck Institute for the
  Science of Human History. <a href="https://phoible.org">https://phoible.org</a>.
  DOI: <a href="https://doi.org/10.5281/zenodo.2626687">10.5281/zenodo.2626687</a>.
</p>
<p>
  Licensed under the
  <a href="https://creativecommons.org/licenses/by/4.0/">Creative Commons Attribution 4.0
  International License</a> (CC BY 4.0). Provided as-is, without warranties; see section 5 of the
  license.
</p>
</section>

<section id="cmudict">
<h3>CMU Pronouncing Dictionary</h3>
<p>
  <code>phono.bin</code> contains English pronunciations from the
  <a href="${html(cmudict.url)}">CMU Pronouncing Dictionary</a> (commit
  <code>${html(cmudict.commit.slice(0, 12))}</code>) and a spelling-to-sound model trained on
  them.
</p>
<pre>${html(cmudictLicense)}</pre>
</section>

<section id="deunicode">
<h3>deunicode transliteration table</h3>
<p>
  <code>phono.bin</code> contains a transliteration table built from the
  <a href="https://crates.io/crates/deunicode/${html(deunicode.version)}"><code>deunicode</code></a>
  crate (${html(deunicode.version)}), with our own changes to some entries. deunicode credits its
  data to Text::Unidecode by Sean M. Burke, Unicodey by Cal Henderson, gh-emoji and any_ascii.
</p>
<pre>${html(deunicodeLicense)}</pre>
</section>`;
}

function crateSection(licenses) {
  const rows = new Map();
  licenses.forEach((license, i) => {
    for (const krate of license.crates) {
      const row = rows.get(key(krate)) ?? { krate, notices: [] };
      row.notices.push(`<a href="#license-${i}">${html(license.id)}</a>`);
      rows.set(key(krate), row);
    }
  });
  const sorted = [...rows.values()].sort(
    (a, b) =>
      a.krate.name.localeCompare(b.krate.name) || a.krate.version.localeCompare(b.krate.version),
  );
  const table = sorted
    .map(({ krate, notices }) => {
      const page = `https://crates.io/crates/${krate.name}/${krate.version}`;
      return (
        `<tr><td><a href="${html(page)}">${html(krate.name)}</a></td>` +
        `<td>${html(krate.version)}</td><td>${html(krate.license ?? "")}</td>` +
        `<td>${notices.join(", ")}</td></tr>`
      );
    })
    .join("\n");
  const texts = licenses
    .map((license, i) => {
      const users = license.crates.map((c) => html(c.name)).join(", ");
      return (
        `<section id="license-${i}">\n<h3>${html(license.name)}: ${users}</h3>\n` +
        `<pre>${html(license.text)}</pre>\n</section>`
      );
    })
    .join("\n\n");
  return {
    count: sorted.length,
    body: `
<h2 id="crates">Rust crates in the WebAssembly module</h2>
<p>
  The crates compiled into the WebAssembly module and its JavaScript glue. Procedural macros,
  which only run while compiling, are not listed. The last column links the license texts that
  apply to each crate; where a crate offers a choice of licenses, it names the one we use.
</p>
<table>
<thead><tr><th>Crate</th><th>Version</th><th>Crate's license</th><th>Used under</th></tr></thead>
<tbody>
${table}
</tbody>
</table>

<h2 id="licenses">License texts</h2>

${texts}`,
  };
}

const licenses = crateLicenses(shippedCrates());
const crates = crateSection(licenses);
const page = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>phonocompressor: third-party notices</title>
<style>
  body {
    font: 16px/1.5 system-ui, sans-serif;
    margin: 2rem auto;
    max-width: 60rem;
    padding: 0 1rem;
  }
  table { border-collapse: collapse; }
  th, td { text-align: left; padding: 0.2rem 0.8rem 0.2rem 0; border-bottom: 1px solid #ccc; }
  pre { white-space: pre-wrap; font-size: 0.85rem; background: #f4f4f4; padding: 0.8rem; }
</style>
</head>
<body>
<main>
<h1>Third-party notices</h1>
<p>
  phonocompressor runs in your browser from a WebAssembly module and a data file,
  <code>phono.bin</code>. Both contain other people's work, under the licenses below.
</p>
<p>
  phonocompressor itself is licensed under the MIT License or the Apache License 2.0, at your
  option (<a href="${REPO}/blob/main/LICENSE-MIT">LICENSE-MIT</a>,
  <a href="${REPO}/blob/main/LICENSE-APACHE">LICENSE-APACHE</a>). Source:
  <a href="${REPO}">${REPO.replace("https://", "")}</a>.
</p>
<ul>
<li><a href="#data">Data in <code>phono.bin</code></a></li>
<li><a href="#crates">Rust crates in the WebAssembly module</a></li>
<li><a href="#licenses">License texts</a></li>
</ul>
${dataSection()}
${crates.body}
</main>
</body>
</html>
`;

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, page);
console.log(
  `notices: wrote ${OUT} ` +
    `(${crates.count} crates, ${licenses.length} license texts, 3 data sources)`,
);
