// Test page for the wasm binding: sends each request to the Worker and shows the guesses or the
// error. The form is never reset, so a failed request keeps its input.
//
// With `?bench` in the URL, the page times the parity phrases at the defaults instead (three
// runs each; the first is cold, the later ones reuse the engine's G2P cache) and shows the
// numbers. `window.benchResult` holds them for automated runs.

const $ = (id) => document.getElementById(id);
const secs = (ms) => `${(ms / 1000).toFixed(2)} s`;
const size = (bytes) =>
  bytes >= 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.round(bytes / 1e3)} KB`;

/** The distinct phrases in crates/wasm/parity/cases.json. */
const BENCH_PHRASES = [
  "bad dream",
  "hundred thousand apartments",
  "good morning everyone",
  "zorblax is here",
];
const BENCH_RUNS = 3;

const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
const pending = new Map();
let nextId = 1;
let onReady;
let onFailed;
const ready = new Promise((resolve, reject) => {
  onReady = resolve;
  onFailed = reject;
});

/** Run compress in the Worker; resolves to {guesses, ms} or rejects with the binding's error. */
function request(phrase, options) {
  const id = nextId++;
  worker.postMessage({ id, phrase, options });
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
}

worker.onmessage = ({ data: msg }) => {
  if (msg.type === "ready") {
    onReady({ ...msg, pageReadyMs: performance.now() });
  } else if (msg.type === "failed") {
    onFailed(new Error(msg.error));
  } else {
    const p = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.type === "result") p.resolve(msg);
    else p.reject(new Error(msg.error));
  }
};

worker.onerror = (e) => {
  onFailed(new Error(`${e.message || "worker error"} (did \`mise run //crates/wasm:build\` run?)`));
};

function showError(message) {
  $("error").textContent = message;
  $("error").hidden = false;
  $("results").hidden = true; // the previous request's guesses don't answer this one
}

function showResults(guesses, ms) {
  const body = $("results").tBodies[0];
  body.replaceChildren();
  guesses.forEach((g, i) => {
    const row = body.insertRow();
    const cells = [
      ["n", String(i + 1)],
      ["text", g.text],
      ["latin", JSON.stringify(g.latin)],
      ["phones", g.phones],
      ["score", g.score.toFixed(3)],
    ];
    for (const [cls, value] of cells) {
      const cell = row.insertCell();
      cell.className = cls;
      cell.textContent = value;
    }
  });
  $("caption").textContent = `${guesses.length} guesses in ${secs(ms)}`;
  $("results").hidden = false;
}

/** A number input as an option: left out when empty, so the binding's default applies. */
function numberOption(id) {
  const value = $(id).value.trim();
  return value === "" ? undefined : Number(value);
}

async function runForm() {
  $("go").disabled = true;
  $("error").hidden = true;
  $("status").textContent = "Searching…";
  const options = { maxCodepoints: numberOption("max"), topN: numberOption("top") };
  try {
    const { guesses, ms } = await request($("phrase").value, options);
    $("status").textContent = `Searched in ${secs(ms)}.`;
    showResults(guesses, ms);
  } catch (e) {
    $("status").textContent = "The search failed.";
    showError(e.message);
  } finally {
    $("go").disabled = false;
  }
}

async function runBench(start) {
  const result = {
    userAgent: navigator.userAgent,
    startup: { ...start.timings, pageReadyMs: start.pageReadyMs },
    sizes: start.sizes,
    phrases: [],
  };
  const body = $("bench-table").tBodies[0];
  for (const phrase of BENCH_PHRASES) {
    const entry = { phrase, ms: [], top: null };
    for (let run = 1; run <= BENCH_RUNS; run++) {
      $("status").textContent = `Benchmark: "${phrase}", run ${run} of ${BENCH_RUNS}…`;
      const { guesses, ms } = await request(phrase, {});
      entry.ms.push(Math.round(ms));
      entry.top = guesses[0]?.text ?? null;
    }
    result.phrases.push(entry);
    const row = body.insertRow();
    for (const value of [phrase, ...entry.ms.map(secs), entry.top ?? "(none)"]) {
      row.insertCell().textContent = value;
    }
  }
  $("bench-json").textContent = JSON.stringify(result, null, 2);
  $("status").textContent = "Benchmark done.";
  window.benchResult = result;
}

$("form").addEventListener("submit", (e) => {
  e.preventDefault();
  if (!$("go").disabled) runForm();
});

ready.then(
  (start) => {
    const t = start.timings;
    $("startup").textContent =
      `Engine ready (${start.version}) in ${secs(t.totalMs)}: fetched in ${secs(t.fetchMs)} ` +
      `(.wasm ${size(start.sizes.wasm)}, data ${size(start.sizes.data)}), compiled in ` +
      `${secs(t.compileMs)}, decoded in ${secs(t.decodeMs)}.`;
    $("status").textContent = "Ready.";
    if (new URLSearchParams(location.search).has("bench")) {
      $("bench").hidden = false;
      runBench(start).catch((e) => {
        $("status").textContent = "The benchmark failed.";
        showError(e.message);
        window.benchResult = { error: e.message };
      });
    } else {
      $("go").disabled = false;
    }
  },
  (e) => {
    $("status").textContent = "The engine failed to load.";
    showError(e.message);
    window.benchResult = { error: e.message };
  },
);
