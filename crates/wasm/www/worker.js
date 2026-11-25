// Module Worker for the test page: loads the wasm binding and the data file once, then answers
// compress requests, so a multi-second search never blocks the page.
//
// Messages out: {type: "ready", version, timings, sizes}, {type: "failed", error} (start-up),
// {type: "result", id, guesses, ms} and {type: "error", id, error} (per request).

import { Engine, initSync, version } from "./pkg/phono_wasm.js";

const since = (t) => performance.now() - t;

async function fetchBytes(url, fix) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status} ${res.statusText}; ${fix}`);
  return new Uint8Array(await res.arrayBuffer());
}

const ready = (async () => {
  const t0 = performance.now();
  const [wasm, data] = await Promise.all([
    fetchBytes("./pkg/phono_wasm_bg.wasm", "run `mise run wasm:build`"),
    fetchBytes("./phono.bin", "run `mise run data`"),
  ]);
  const fetchMs = since(t0);
  let t = performance.now();
  initSync({ module: wasm });
  const compileMs = since(t);
  t = performance.now();
  const engine = new Engine(data); // copies the bytes in; `data` can be collected after this
  const decodeMs = since(t);
  postMessage({
    type: "ready",
    version: version(),
    timings: { fetchMs, compileMs, decodeMs, totalMs: since(t0) },
    sizes: { wasm: wasm.length, data: data.length },
  });
  return engine;
})();
ready.catch((e) => postMessage({ type: "failed", error: String(e?.message ?? e) }));

onmessage = async ({ data: { id, phrase, options } }) => {
  let engine;
  try {
    engine = await ready;
  } catch {
    return; // start-up failure already reported
  }
  const t = performance.now();
  try {
    const guesses = engine.compress(phrase, options);
    postMessage({ type: "result", id, guesses, ms: since(t) });
  } catch (e) {
    postMessage({ type: "error", id, error: String(e?.message ?? e) });
  }
};
