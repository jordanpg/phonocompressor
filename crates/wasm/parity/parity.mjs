// Check that the wasm binding returns the same guesses as `phono compress --json`.
//
// For every case in cases.json, runs the release CLI and the `--target web` package (under
// Node, with initSync) on the same phrase and options, then compares the results: the same
// text, Latin and phones in the same order, and scores within TOLERANCE (native and wasm
// floating point may differ in the last bits). Exits 1 on any difference, 2 if an input is
// missing.
//
// Usage: mise run wasm:parity (builds both sides first), or
//   node crates/wasm/parity/parity.mjs [--skew-wasm '{"topN": 3}']
// --skew-wasm merges extra options into the wasm side only, to check that a difference fails.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const PKG = `${ROOT}target/wasm-pkg`;
const DATA = `${ROOT}data/generated/phono.bin`;
const CLI = `${ROOT}target/release/phono`;
const CASES = fileURLToPath(new URL("cases.json", import.meta.url));
const TOLERANCE = 1e-4;

const { values: flags } = parseArgs({ options: { "skew-wasm": { type: "string" } } });
const skew = flags["skew-wasm"] ? JSON.parse(flags["skew-wasm"]) : {};

function need(path, fix) {
  if (!existsSync(path)) {
    console.error(`wasm:parity: ${path} not found; ${fix}`);
    process.exit(2);
  }
}
need(DATA, "run `mise run data`");
need(`${PKG}/phono_wasm.js`, "run `mise run wasm:build`");
need(CLI, "run `cargo build --release -p phono-cli`");

/** `phono compress --json` arguments for a case's options. */
function cliArgs(phrase, options) {
  const args = ["--data", DATA, "compress", phrase, "--json"];
  for (const [key, value] of Object.entries(options)) {
    if (key === "model") args.push("--model", value);
    else if (key === "maxCodepoints") args.push("--max", String(value));
    else if (key === "topN") args.push("--top", String(value));
    else if (key === "allowedScripts") args.push("--scripts", value.join(","));
    else throw new Error(`cases.json: no CLI flag for option ${key}`);
  }
  return args;
}

function runNative(phrase, options) {
  const t = performance.now();
  const out = spawnSync(CLI, cliArgs(phrase, options), { encoding: "utf8" });
  if (out.status !== 0) {
    throw new Error(`phono compress failed (${out.status}): ${out.stderr.trim()}`);
  }
  return { results: JSON.parse(out.stdout), secs: (performance.now() - t) / 1000 };
}

function runWasm(engine, phrase, options) {
  const t = performance.now();
  const results = engine.compress(phrase, { ...options, ...skew });
  return { results, secs: (performance.now() - t) / 1000 };
}

/** Differences between the two result lists, and the largest score difference. */
function compare(native, wasm) {
  const problems = [];
  if (native.length === 0) {
    problems.push("no native results; pick a phrase the engine can compress");
  }
  if (native.length !== wasm.length) {
    problems.push(`${native.length} native results vs ${wasm.length} wasm`);
  }
  let maxDelta = 0;
  for (let i = 0; i < Math.min(native.length, wasm.length); i++) {
    const [n, w] = [native[i], wasm[i]];
    for (const key of ["text", "latin", "phones"]) {
      if (n[key] !== w[key]) {
        const [a, b] = [JSON.stringify(n[key]), JSON.stringify(w[key])];
        problems.push(`#${i + 1} ${key}: native ${a}, wasm ${b}`);
      }
    }
    const delta = Math.abs(n.score - w.score);
    maxDelta = Math.max(maxDelta, delta);
    if (!(delta <= TOLERANCE)) {
      problems.push(`#${i + 1} score: native ${n.score}, wasm ${w.score}`);
    }
  }
  return { problems, maxDelta };
}

const wasm = await import(`${PKG}/phono_wasm.js`);
wasm.initSync({ module: readFileSync(`${PKG}/phono_wasm_bg.wasm`) });
const engine = new wasm.Engine(readFileSync(DATA));
const { cases } = JSON.parse(readFileSync(CASES, "utf8"));
if (Object.keys(skew).length > 0) console.log(`skewing the wasm side with ${JSON.stringify(skew)}`);

let failed = 0;
for (const { phrase, options = {} } of cases) {
  const label = `${JSON.stringify(phrase)} ${JSON.stringify(options)}`;
  let problems;
  let summary = "";
  try {
    const native = runNative(phrase, options);
    const ours = runWasm(engine, phrase, options);
    const cmp = compare(native.results, ours.results);
    problems = cmp.problems;
    summary =
      `${native.results.length} results, max score diff ${cmp.maxDelta.toExponential(1)}, ` +
      `native ${native.secs.toFixed(1)} s, wasm ${ours.secs.toFixed(1)} s`;
  } catch (e) {
    problems = [String(e.message ?? e)];
  }
  if (problems.length === 0) {
    console.log(`ok    ${label}: ${summary}`);
  } else {
    failed++;
    console.log(`FAIL  ${label}${summary ? `: ${summary}` : ""}`);
    for (const p of problems.slice(0, 5)) console.log(`        ${p}`);
    if (problems.length > 5) console.log(`        … ${problems.length - 5} more`);
  }
}

console.log(
  failed === 0
    ? `wasm:parity: all ${cases.length} cases match`
    : `wasm:parity: ${failed} of ${cases.length} cases differ`,
);
process.exit(failed === 0 ? 0 : 1);
