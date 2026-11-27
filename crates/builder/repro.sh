#!/usr/bin/env bash
# The data release's reproducibility gate: two builds of the same commit must be byte-identical,
# or a release's checksum would change on every rebuild. Two separate processes, so each HashMap
# gets a different random seed. Run by `mise run //crates/builder:repro` and the release workflow.
set -euo pipefail
cd "$(dirname "$0")/../.."

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

for n in 1 2; do
  echo "build $n"
  # The build log is long; show it only if the build fails.
  if ! cargo run --release -q -p phono-builder -- --out "$tmp/phono-$n.bin" >"$tmp/build-$n.log" 2>&1; then
    cat "$tmp/build-$n.log" >&2
    exit 1
  fi
done

if ! cmp "$tmp/phono-1.bin" "$tmp/phono-2.bin"; then
  echo "phono.bin is not reproducible: two builds of the same tree differ" >&2
  exit 1
fi

sha=$(sha256sum "$tmp/phono-1.bin" | cut -d' ' -f1)
format=$(cargo run --release -q -p phono-builder -- --format-version)
echo "reproducible: $sha  phono.bin (format $format)"
