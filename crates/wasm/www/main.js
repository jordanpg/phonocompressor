// Test page for the wasm binding: sends each request to the Worker and shows the guesses or the
// error. The form is never reset, so a failed request keeps its input.

const $ = (id) => document.getElementById(id);
const secs = (ms) => `${(ms / 1000).toFixed(2)} s`;
const size = (bytes) =>
  bytes >= 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.round(bytes / 1e3)} KB`;

const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
let busy = false;

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

worker.onmessage = ({ data: msg }) => {
  if (msg.type === "ready") {
    const t = msg.timings;
    $("status").textContent =
      `Engine ready (${msg.version}) in ${secs(t.totalMs)}: fetched in ${secs(t.fetchMs)} ` +
      `(.wasm ${size(msg.sizes.wasm)}, data ${size(msg.sizes.data)}), compiled in ` +
      `${secs(t.compileMs)}, decoded in ${secs(t.decodeMs)}.`;
    $("go").disabled = false;
  } else if (msg.type === "failed") {
    $("status").textContent = "The engine failed to load.";
    showError(msg.error);
  } else {
    busy = false;
    $("go").disabled = false;
    if (msg.type === "result") {
      $("status").textContent = `Searched in ${secs(msg.ms)}.`;
      showResults(msg.guesses, msg.ms);
    } else {
      $("status").textContent = "The search failed.";
      showError(msg.error);
    }
  }
};

worker.onerror = (e) => {
  $("status").textContent = "The worker failed to start.";
  showError(`${e.message || "worker error"} (did \`mise run wasm:build\` run?)`);
};

/** A number input as an option: left out when empty, so the binding's default applies. */
function numberOption(id) {
  const value = $(id).value.trim();
  return value === "" ? undefined : Number(value);
}

$("form").addEventListener("submit", (e) => {
  e.preventDefault();
  if (busy) return;
  busy = true;
  $("go").disabled = true;
  $("error").hidden = true;
  $("status").textContent = "Searching…";
  const options = { maxCodepoints: numberOption("max"), topN: numberOption("top") };
  worker.postMessage({ id: Date.now(), phrase: $("phrase").value, options });
});
