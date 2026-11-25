"""Minimal ElevenLabs REST client.

Only the calls the harness needs: subscription (credit usage), model list, and
text-to-speech with character timestamps. Audio is requested as pcm_16000
(raw s16le mono), which the free tier allows and the ASR models consume directly.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import time
from dataclasses import asdict, dataclass

import httpx

API_BASE = "https://api.elevenlabs.io"
OUTPUT_FORMAT = "pcm_16000"


class ApiError(RuntimeError):
    def __init__(self, status: int, body: str):
        super().__init__(f"ElevenLabs API error {status}: {body}")
        self.status = status
        self.body = body


@dataclass(frozen=True)
class TtsRequest:
    text: str
    model_id: str
    voice_id: str
    seed: int | None = None
    output_format: str = OUTPUT_FORMAT
    # "auto" | "on" | "off"; None sends nothing (the API default, "auto").
    apply_text_normalization: str | None = None

    @property
    def key(self) -> str:
        """Stable cache key: anything that changes the audio changes the key. Optional fields
        left at None are omitted, so adding one doesn't change the keys of cached clips."""
        fields = {k: v for k, v in asdict(self).items() if not (k in _OPTIONAL and v is None)}
        blob = json.dumps(fields, sort_keys=True, ensure_ascii=False).encode()
        return hashlib.sha256(blob).hexdigest()[:24]


# Fields added after clips were cached: omitted from the cache key when None.
_OPTIONAL = {"apply_text_normalization"}


class Client:
    def __init__(self, api_key: str | None = None):
        key = api_key or os.environ.get("ELEVENLABS_API_KEY")
        if not key:
            raise SystemExit(
                "ELEVENLABS_API_KEY is not set. Run this through its mise task "
                "(e.g. `mise run harness:credits`), which injects it from fnox."
            )
        self._http = httpx.Client(base_url=API_BASE, headers={"xi-api-key": key}, timeout=90)

    def _request(self, method: str, url: str, **kwargs) -> httpx.Response:
        for attempt in range(5):
            r = self._http.request(method, url, **kwargs)
            if r.status_code == 429 or r.status_code >= 500:
                time.sleep(2**attempt)
                continue
            if r.status_code >= 400:
                raise ApiError(r.status_code, r.text[:800])
            return r
        raise ApiError(r.status_code, r.text[:800])

    def subscription(self) -> dict:
        return self._request("GET", "/v1/user/subscription").json()

    def remaining_credits(self) -> int:
        s = self.subscription()
        return int(s["character_limit"]) - int(s["character_count"])

    def models(self) -> list[dict]:
        return self._request("GET", "/v1/models").json()

    def voices(self) -> list[dict]:
        """Voices in the account ("My Voices"), including premade ones."""
        return self._request("GET", "/v1/voices").json()["voices"]

    def synthesize(self, req: TtsRequest) -> tuple[bytes, dict]:
        """Returns (raw PCM bytes, metadata incl. character alignment)."""
        body: dict = {"text": req.text, "model_id": req.model_id}
        if req.seed is not None:
            body["seed"] = req.seed
        if req.apply_text_normalization is not None:
            body["apply_text_normalization"] = req.apply_text_normalization
        r = self._request(
            "POST",
            f"/v1/text-to-speech/{req.voice_id}/with-timestamps",
            params={"output_format": req.output_format},
            json=body,
        )
        data = r.json()
        pcm = base64.b64decode(data["audio_base64"])
        interesting = ("request-id", "history-item-id", "character-cost", "x-character-count")
        meta = {
            "request": asdict(req),
            "alignment": data.get("alignment"),
            "normalized_alignment": data.get("normalized_alignment"),
            "headers": {k: v for k, v in r.headers.items() if k.lower() in interesting},
            "created": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        }
        return pcm, meta
