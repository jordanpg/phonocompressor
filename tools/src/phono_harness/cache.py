"""On-disk cache for synthesized clips and their transcripts.

Layout under the cache root (default `<repo>/.cache/harness`, override with PHONO_CACHE_DIR):
    tts/<key>.wav          16 kHz mono PCM from ElevenLabs
    tts/<key>.json         request parameters + alignment + response metadata
    asr/<key>.<ver>.json   local ASR output for that clip
"""

from __future__ import annotations

import json
import os
import wave
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
SAMPLE_RATE = 16_000


def root() -> Path:
    return Path(os.environ.get("PHONO_CACHE_DIR", REPO_ROOT / ".cache" / "harness"))


def wav_path(key: str) -> Path:
    return root() / "tts" / f"{key}.wav"


def meta_path(key: str) -> Path:
    return root() / "tts" / f"{key}.json"


def asr_path(key: str, version: str) -> Path:
    return root() / "asr" / f"{key}.{version}.json"


def has_clip(key: str) -> bool:
    return wav_path(key).exists() and meta_path(key).exists()


def save_clip(key: str, pcm: bytes, meta: dict) -> None:
    path = wav_path(key)
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SAMPLE_RATE)
        w.writeframes(pcm)
    write_json(meta_path(key), meta)


def read_json(path: Path) -> dict | None:
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else None


def write_json(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    tmp.replace(path)
