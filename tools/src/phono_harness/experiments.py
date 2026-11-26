"""Experiment definitions (TOML files in tools/experiments/).

An experiment is a list of cases (text + predictions) crossed with models and seeds:

    name = "exp0"
    voice_id = "..."
    models = ["eleven_flash_v2_5", ...]
    seeds = [1]
    max_credits = 400          # synth refuses to run if the estimate exceeds this
    cases_from = "sample/messages.json"   # optional: community messages (target, else spoken)
    # or several files: a text already seen for the same phrase is dropped
    # cases_from = ["data/generated/ours-v1.json", "data/generated/ours-v2.json"]
    # cases_exclude = "sample/messages.json"  # optional: drop texts listed here too

    [[cases]]
    id = "dis_is"
    text = "딧잇"
    t = "dis is"               # reading model T: transliterate, read as English
    n = "[tisit̚]"              # reading model N: native-language reading
    note = "..."
    normalization = "on"       # optional: apply_text_normalization for this case (default: not sent)
"""

from __future__ import annotations

import json
import tomllib
from dataclasses import dataclass, replace
from pathlib import Path

from .cache import REPO_ROOT
from .elevenlabs import TtsRequest

EXPERIMENTS_DIR = REPO_ROOT / "tools" / "experiments"


@dataclass(frozen=True)
class Case:
    id: str
    text: str
    t: str = ""
    n: str = ""
    note: str = ""
    target: str = ""  # plain-English phrase the message is meant to sound like (scored in reports)
    normalization: str = ""  # apply_text_normalization to send ("" = don't send: the API default)


def cases_from_messages(path: Path) -> list[Case]:
    """Community sample messages: [{"<n>": {"unicode": ..., "spoken": ..., "notes": ...}}, ...].

    The case's target is the entry's `target` (library.json) when set, else its `spoken` label.
    """
    data = json.loads(path.read_text(encoding="utf-8"))
    return [
        Case(
            id=f"msg{key}",
            text=m["unicode"],
            target=m.get("target") or m["spoken"],
            note=m.get("notes") or "",
        )
        for item in data
        for key, m in item.items()
    ]


def _phrase_key(case: Case) -> tuple[str, str]:
    return (" ".join(case.target.lower().split()), case.text)


def cases_from_files(paths: str | list[str], exclude: str | list[str] = ()) -> list[Case]:
    """Cases from one or more message files (paths relative to the repo root), in order.

    With several files, a text already seen for the same phrase is dropped, and so is one
    listed in an `exclude` file (e.g. a replay leaving out texts another experiment already
    covers). A kept case whose id an earlier file used gets the file's stem as a suffix, e.g.
    `msg8_ours-v2`. A single file keeps every case and id as before."""
    if isinstance(paths, str):
        paths = [paths]
    if isinstance(exclude, str):
        exclude = [exclude]
    seen = {_phrase_key(c) for p in exclude for c in cases_from_messages(REPO_ROOT / p)}
    ids: set[str] = set()
    cases = []
    for p in paths:
        for c in cases_from_messages(REPO_ROOT / p):
            if len(paths) > 1 or exclude:
                if _phrase_key(c) in seen:
                    continue
                seen.add(_phrase_key(c))
            if c.id in ids:
                c = replace(c, id=f"{c.id}_{Path(p).stem}")
            ids.add(c.id)
            cases.append(c)
    return cases


@dataclass(frozen=True)
class Experiment:
    name: str
    description: str
    voice_id: str
    models: list[str]
    seeds: list[int | None]
    max_credits: int
    cases: list[Case]

    def requests(self) -> list[tuple[Case, TtsRequest]]:
        return [
            (
                case,
                TtsRequest(
                    text=case.text,
                    model_id=model,
                    voice_id=self.voice_id,
                    seed=seed,
                    apply_text_normalization=case.normalization or None,
                ),
            )
            for case in self.cases
            for model in self.models
            for seed in self.seeds
        ]


def load(name_or_path: str) -> Experiment:
    path = Path(name_or_path)
    if not path.suffix:
        path = EXPERIMENTS_DIR / f"{name_or_path}.toml"
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    cases = [Case(**c) for c in data.get("cases", [])]
    cases += cases_from_files(data.get("cases_from", []), data.get("cases_exclude", []))
    ids = [c.id for c in cases]
    if len(ids) != len(set(ids)):
        raise SystemExit(f"{path}: duplicate case ids")
    return Experiment(
        name=data.get("name", path.stem),
        description=data.get("description", ""),
        voice_id=data["voice_id"],
        models=list(data["models"]),
        seeds=list(data.get("seeds", [None])),
        max_credits=int(data.get("max_credits", 500)),
        cases=cases,
    )
