"""Community message library: fetch it into gitignored `sample/library.json`.

The output has the shape of `sample/messages.json`, which `phono eval` and `cases_from` already
read: `[{"<key>": {"id": .., "unicode": .., "spoken": .., ...}}, ...]`, one entry per line.

Keys stay stable across re-fetches. The hand-picked entries keep their keys through the
committed UUID → key table (`tools/library-keys.tsv`, no message content). Every other entry
is keyed `u` + the first 8 hex digits of its UUID, so later inserts renumber nothing.

Each entry gets a `target`, the phrase it is scored against: `spoken` cleaned by the rules in
`clean_target`, or a hand-written override from gitignored `sample/targets.tsv`
(`key<TAB>target`). Overrides win. Submitter fields and the vote count are dropped. The library URL comes from the environment
(`PHONO_LIBRARY_URL`, set from fnox by the mise task) and is never printed: the repo is public.
"""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

from .cache import REPO_ROOT

URL_ENV = "PHONO_LIBRARY_URL"
KEYS_PATH = REPO_ROOT / "tools" / "library-keys.tsv"
OUT_PATH = REPO_ROOT / "sample" / "library.json"
OVERRIDES_PATH = REPO_ROOT / "sample" / "targets.tsv"

# Fields kept from each entry, in this order; `target` is added after "spoken".
KEEP = ("id", "unicode", "spoken", "notes", "characters", "createdAt", "updatedAt")

_ALTERNATIVE = re.compile(r"\s*(?<=[^\W\d_]|\s)/(?=\s*[^\W\d_])")  # a/b, not 1/2
_ATTACHED_PAREN = re.compile(r"(?<=\w)\(([^()]*)\)")  # koko('s): optional letters
_ASIDE = re.compile(r"\([^()]*\)")  # (aside) on its own
_THOUSANDS = re.compile(r"\b(\d+)[kK]\b")  # 100k


def clean_target(spoken: str) -> str:
    """The label as a phrase to score against: written-for-people forms made into sound.

    - `a/b` alternatives: keep the first (`/` between digits is left alone);
    - a parenthetical attached to a word keeps its letters (`koko('s)` → `koko's`);
      one standing alone is an aside and is dropped;
    - `100k` → `100,000`, which the reader says as a number.
    If nothing is left, the label is kept as is.
    """
    s = _ALTERNATIVE.split(spoken, maxsplit=1)[0]
    s = _ATTACHED_PAREN.sub(r"\1", s)
    s = _ASIDE.sub(" ", s)
    s = _THOUSANDS.sub(r"\1,000", s)
    s = " ".join(s.split())
    return s or " ".join(spoken.split())


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


def load_overrides(path: Path) -> dict[str, str]:
    """Key → target, from a TSV of `key<TAB>target` lines. A missing file means no overrides."""
    if not path.exists():
        return {}
    out: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        key, target = line.split("\t", 1)
        out[key.strip()] = target.strip()
    return out


def entry_key(uuid: str, table: dict[str, str]) -> str:
    return table.get(uuid) or "u" + uuid.replace("-", "")[:8].lower()


def clean_entry(entry: dict, target: str | None = None) -> dict:
    """Keep only the KEEP fields, in order, with `target` after "spoken".

    Missing strings become "", missing lists []. The target defaults to the rule-cleaned label.
    """
    out = {}
    for f in KEEP:
        out[f] = entry.get(f, [] if f == "characters" else "")
        if f == "spoken":
            out["target"] = target or clean_target(out["spoken"])
    return out


def build(
    entries: list[dict], table: dict[str, str], overrides: dict[str, str] | None = None
) -> list[dict[str, dict]]:
    """Cleaned, keyed entries, oldest first (ties broken by UUID)."""
    overrides = overrides or {}
    seen: dict[str, str] = {}
    out = []
    for e in sorted(entries, key=lambda e: (e.get("createdAt", ""), e["id"])):
        key = entry_key(e["id"], table)
        if key in seen:
            raise ValueError(f"key {key} is shared by {seen[key]} and {e['id']}")
        seen[key] = e["id"]
        out.append({key: clean_entry(e, overrides.get(key))})
    return out


def target_stats(items: list[dict[str, dict]], overrides: dict[str, str]):
    """(targets changed by rule, targets from an override, override keys with no entry)."""
    entries = as_map(items)
    by_override = sum(1 for k in entries if k in overrides)
    by_rule = sum(
        1 for k, v in entries.items() if k not in overrides and v["target"] != v["spoken"]
    )
    unused = [k for k in overrides if k not in entries]
    return by_rule, by_override, unused


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
