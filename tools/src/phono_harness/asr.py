"""Local speech recognition for synthesized clips (no ElevenLabs credits).

Three views of each clip:
  - Whisper language ID: which language does the audio sound like? (separates
    reading model T, which comes out as English, from N, which comes out as Korean, Japanese, …)
  - Whisper forced to English: roughly "what would an English-speaking viewer hear?"
  - wav2vec2 XLSR-53 espeak phoneme recognizer: language-neutral IPA phones,
    the closest thing to "what sounds came out".
"""

from __future__ import annotations

import wave

import numpy as np
import torch
from transformers import (
    Wav2Vec2FeatureExtractor,
    Wav2Vec2ForCTC,
    Wav2Vec2PhonemeCTCTokenizer,
    WhisperForConditionalGeneration,
    WhisperProcessor,
)

from .cache import SAMPLE_RATE

WHISPER_MODEL = "openai/whisper-large-v3-turbo"
PHONE_MODEL = "facebook/wav2vec2-xlsr-53-espeak-cv-ft"
# Bump when the models or the output shape change, so cached transcripts are recomputed.
ASR_VERSION = "v1"


def load_wav(path) -> np.ndarray:
    with wave.open(str(path), "rb") as w:
        if w.getframerate() != SAMPLE_RATE or w.getnchannels() != 1 or w.getsampwidth() != 2:
            raise ValueError(f"{path}: expected 16 kHz mono s16 WAV")
        frames = w.readframes(w.getnframes())
    return np.frombuffer(frames, dtype=np.int16).astype(np.float32) / 32768.0


class Transcriber:
    def __init__(self, device: str | None = None):
        self.device = device or ("cuda" if torch.cuda.is_available() else "cpu")
        self.dtype = torch.float16 if self.device == "cuda" else torch.float32

        self.wproc = WhisperProcessor.from_pretrained(WHISPER_MODEL)
        self.whisper = (
            WhisperForConditionalGeneration.from_pretrained(WHISPER_MODEL, dtype=self.dtype)
            .to(self.device)
            .eval()
        )
        lang_to_id = self.whisper.generation_config.lang_to_id  # {"<|en|>": id, ...}
        self._lang_codes = [tok.strip("<|>") for tok in lang_to_id]
        self._lang_ids = torch.tensor(list(lang_to_id.values()), device=self.device)

        self.fe = Wav2Vec2FeatureExtractor.from_pretrained(PHONE_MODEL)
        # do_phonemize=False: we only decode model output, so no espeak/phonemizer needed.
        self.tok = Wav2Vec2PhonemeCTCTokenizer.from_pretrained(PHONE_MODEL, do_phonemize=False)
        self.w2v = Wav2Vec2ForCTC.from_pretrained(PHONE_MODEL).to(self.device).eval()

    @torch.inference_mode()
    def _languages(self, feats: torch.Tensor, top: int = 3) -> list[tuple[str, float]]:
        start = torch.tensor(
            [[self.whisper.generation_config.decoder_start_token_id]], device=self.device
        )
        logits = self.whisper(input_features=feats, decoder_input_ids=start).logits[0, -1].float()
        probs = logits[self._lang_ids].softmax(-1)
        p, i = probs.topk(top)
        return [
            (self._lang_codes[j], round(float(q), 3))
            for q, j in zip(p.tolist(), i.tolist(), strict=True)
        ]

    @torch.inference_mode()
    def _whisper_text(self, feats: torch.Tensor, language: str) -> str:
        ids = self.whisper.generate(feats, language=language, task="transcribe")
        return self.wproc.batch_decode(ids, skip_special_tokens=True)[0].strip()

    @torch.inference_mode()
    def _phones(self, audio: np.ndarray) -> str:
        values = self.fe(audio, sampling_rate=SAMPLE_RATE, return_tensors="pt").input_values
        logits = self.w2v(values.to(self.device)).logits
        return self.tok.batch_decode(logits.argmax(-1))[0].strip()

    def run(self, audio: np.ndarray) -> dict:
        duration = len(audio) / SAMPLE_RATE
        rms = float(np.sqrt(np.mean(audio**2))) if len(audio) else 0.0
        out: dict = {
            "asr_version": ASR_VERSION,
            "duration_s": round(duration, 2),
            "rms_dbfs": round(20 * np.log10(rms + 1e-9), 1),
        }
        # Whisper hallucinates text ("you", "Thank you.") on silence; don't ask it.
        if duration < 0.15 or out["rms_dbfs"] < -45:
            out["silent"] = True
            return out
        feats = self.wproc(audio, sampling_rate=SAMPLE_RATE, return_tensors="pt").input_features
        feats = feats.to(self.device, self.dtype)
        langs = self._languages(feats)
        out |= {
            "languages": langs,
            "en": self._whisper_text(feats, "en"),
            "phones": self._phones(audio),
        }
        top = langs[0][0]
        if top != "en":
            out["native"] = {"language": top, "text": self._whisper_text(feats, top)}
        return out
