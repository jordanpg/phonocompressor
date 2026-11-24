"""Experiment definitions (TOML files in tools/experiments/).

An experiment is a list of cases (text + predictions) crossed with models and seeds:

    name = "exp0"
    voice_id = "..."
    models = ["eleven_flash_v2_5", ...]
    seeds = [1]
    max_credits = 400          # synth refuses to run if the estimate exceeds this
    cases_from = "sample/messages.json"   # optional: community messages (target = "spoken")

    [[cases]]
    id = "dis_is"
    text = "딧잇"
    t = "dis is"               # reading model T: transliterate, read as English
    n = "[tisit̚]"              # reading model N: native-language reading
    note = "..."
"""

from __future__ import annotations

import json
import tomllib
from dataclasses import dataclass
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


def cases_from_messages(path: Path) -> list[Case]:
    """Community sample messages: [{"<n>": {"unicode": ..., "spoken": ..., "notes": ...}}, ...]."""
    data = json.loads(path.read_text(encoding="utf-8"))
    return [
        Case(id=f"msg{key}", text=m["unicode"], target=m["spoken"], note=m.get("notes") or "")
        for item in data
        for key, m in item.items()
    ]


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
            (case, TtsRequest(text=case.text, model_id=model, voice_id=self.voice_id, seed=seed))
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
    if "cases_from" in data:  # path relative to the repo root
        cases += cases_from_messages(REPO_ROOT / data["cases_from"])
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
