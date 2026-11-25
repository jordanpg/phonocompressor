// Static dev server for the wasm test page. Serves crates/wasm/www at /, the wasm-bindgen
// package (target/wasm-pkg) at /pkg/ and data/generated/phono.bin at /phono.bin, with `.wasm`
// as application/wasm. No caching, so every load fetches again. Local use only.
//
// Usage: mise run wasm:serve [--port 8000] [--host 127.0.0.1]

import { createReadStream, existsSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const WWW = resolve(ROOT, "crates/wasm/www");
const PKG = resolve(ROOT, "target/wasm-pkg");
const DATA = resolve(ROOT, "data/generated/phono.bin");

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".bin": "application/octet-stream",
  ".json": "application/json",
  ".ts": "text/plain; charset=utf-8",
};

const { values: flags } = parseArgs({
  options: {
    port: { type: "string", default: "8000" },
    host: { type: "string", default: "127.0.0.1" },
  },
});

for (const [path, fix] of [
  [DATA, "run `mise run data`"],
  [resolve(PKG, "phono_wasm.js"), "run `mise run wasm:build`"],
]) {
  if (!existsSync(path)) {
    console.error(`wasm:serve: ${path} not found; ${fix}`);
    process.exit(2);
  }
}

/** The file a URL path maps to, or null. Never leaves the mapped directory. */
function fileFor(urlPath) {
  if (urlPath === "/phono.bin") return DATA;
  const [dir, rest] = urlPath.startsWith("/pkg/")
    ? [PKG, urlPath.slice("/pkg/".length)]
    : [WWW, urlPath === "/" ? "index.html" : urlPath.slice(1)];
  const file = resolve(dir, rest);
  return file.startsWith(dir + sep) ? file : null;
}

const server = createServer((req, res) => {
  const send = (status, text) => {
    res.writeHead(status, { "Content-Type": "text/plain; charset=utf-8" });
    res.end(text);
    console.log(`${status} ${req.method} ${req.url}`);
  };
  if (req.method !== "GET" && req.method !== "HEAD") return send(405, "GET or HEAD only\n");
  let path;
  try {
    path = decodeURIComponent(new URL(req.url, "http://localhost").pathname);
  } catch {
    return send(400, "bad URL\n");
  }
  const file = fileFor(path);
  if (!file || !existsSync(file) || !statSync(file).isFile()) return send(404, "not found\n");
  res.writeHead(200, {
    "Content-Type": MIME[extname(file)] ?? "application/octet-stream",
    "Content-Length": statSync(file).size,
    "Cache-Control": "no-store",
  });
  if (req.method === "HEAD") return res.end();
  createReadStream(file).pipe(res);
});

server.on("error", (e) => {
  console.error(`wasm:serve: ${e.message} (try --port <other>)`);
  process.exit(1);
});
server.listen(Number(flags.port), flags.host, () => {
  console.log(`wasm:serve: http://${flags.host}:${flags.port}/ (Ctrl-C to stop)`);
});
