"""Community message library: fetch it into gitignored `sample/library.json`.

The output has the shape of `sample/messages.json`, which `phono eval` and `cases_from` already
read: `[{"<key>": {"id": .., "unicode": .., "spoken": .., ...}}, ...]`, one entry per line.

Keys stay stable across re-fetches. The hand-picked entries keep their keys through the
committed UUID → key table (`tools/library-keys.tsv`, no message content). Every other entry
is keyed `u` + the first 8 hex digits of its UUID, so later inserts renumber nothing.

Submitter fields and the vote count are dropped. The library URL comes from the environment
(`PHONO_LIBRARY_URL`, set from fnox by the mise task) and is never printed: the repo is public.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from .cache import REPO_ROOT

URL_ENV = "PHONO_LIBRARY_URL"
KEYS_PATH = REPO_ROOT / "tools" / "library-keys.tsv"
OUT_PATH = REPO_ROOT / "sample" / "library.json"

# Fields kept from each entry, in this order. The CLI's minimal loader needs "unicode" before
# "spoken", and the key's object must open with a plain string field.
KEEP = ("id", "unicode", "spoken", "notes", "characters", "createdAt", "updatedAt")


def load_key_table(path: Path) -> dict[str, str]:
    """UUID → key, from a TSV of `uuid<TAB>key` lines (# comments and blank lines skipped)."""
    table: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        uuid, key = line.split("\t")
        table[uuid] = key
    return table


def entry_key(uuid: str, table: dict[str, str]) -> str:
    return table.get(uuid) or "u" + uuid.replace("-", "")[:8].lower()


def clean_entry(entry: dict) -> dict:
    """Keep only the KEEP fields, in order. Missing strings become "", missing lists []."""
    return {f: entry.get(f, [] if f == "characters" else "") for f in KEEP}


def build(entries: list[dict], table: dict[str, str]) -> list[dict[str, dict]]:
    """Cleaned, keyed entries, oldest first (ties broken by UUID)."""
    seen: dict[str, str] = {}
    out = []
    for e in sorted(entries, key=lambda e: (e.get("createdAt", ""), e["id"])):
        key = entry_key(e["id"], table)
        if key in seen:
            raise ValueError(f"key {key} is shared by {seen[key]} and {e['id']}")
        seen[key] = e["id"]
        out.append({key: clean_entry(e)})
    return out


def as_map(items: list[dict[str, dict]]) -> dict[str, dict]:
    return {k: v for item in items for k, v in item.items()}


def diff(previous: list[dict[str, dict]], current: list[dict[str, dict]]):
    """(new, changed, removed) keys between two builds."""
    old, new = as_map(previous), as_map(current)
    added = [k for k in new if k not in old]
    changed = [k for k in new if k in old and old[k] != new[k]]
    removed = [k for k in old if k not in new]
    return added, changed, removed


def content_hash(items: list[dict[str, dict]]) -> str:
    blob = json.dumps(items, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(blob.encode("utf-8")).hexdigest()[:16]


def dumps(items: list[dict[str, dict]]) -> str:
    lines = [json.dumps(item, ensure_ascii=False) for item in items]
    return "[\n" + ",\n".join(lines) + "\n]\n"


def read(path: Path) -> list[dict[str, dict]]:
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else []


def fetch(url: str) -> list[dict]:
    """The library's entries. Errors never include the URL."""
    import httpx

    try:
        r = httpx.get(url, timeout=60, follow_redirects=True)
    except httpx.HTTPError as e:
        raise SystemExit(f"library fetch failed: {type(e).__name__}") from None
    if r.status_code != 200:
        raise SystemExit(f"library fetch failed: HTTP {r.status_code}")
    return r.json()["entries"]
