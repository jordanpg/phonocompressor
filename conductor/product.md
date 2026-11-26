# Product Definition

## Overview

**Name:** phonocompressor

**One line:** A phonetic compression engine (Rust: a native CLI and a browser binding, with the web tool planned) that searches for short exotic-character strings whose TTS reading matches a target phrase.

**How it works, briefly:** ElevenLabs' 2.5-generation models (Flash v2.5, Turbo v2.5) transliterate unfamiliar scripts into Latin letters and read the result as English spelling. The engine turns an English phrase into target phones, searches for codepoint strings whose transliteration spells those sounds within a codepoint budget (configurable, default 6, spaces count), predicts how the TTS will read each candidate, and returns the top N. TTS output is not deterministic, so it returns several guesses rather than one answer. Example: "hundred thousand apartments" → `҈㌀s` (3 codepoints).

Full design, experiments and decision history: [`docs/PLAN.md`](../docs/PLAN.md).

## Problem

Live-chat TTS messages (e.g. Twitch) have a tiny character budget, and no tool finds the shortest string a specific TTS model will read as a given phrase. People hand-craft these messages by trial and error instead, which is slow, costs synthesis credits, and gives inconsistent results.

## Users

### Primary: chat viewers

- **Who:** Viewers in live-stream chats who send TTS messages.
- **Goal:** Type a phrase, get a few short strings that the stream's TTS will read back as that phrase.
- **Pain points:** Character limits; no way to predict what the TTS will say without paying to hear it.
- **How they use it:** The planned in-browser web tool. No install, no account, no API key.

### Secondary: the developer/researcher

- **Who:** The maintainer, running the CLI and the Python harness.
- **Goal:** Calibrate the engine against real ElevenLabs output and improve it, one measured change at a time.
- **How they use it:** `phono compress/score/read/eval/calibrate/fit`, plus `//tools:synth/transcribe/report` within the ElevenLabs free tier.

## Goals

1. **Ship the WASM web tool (Phase 3).** Run the engine fully in the browser. The binding (`crates/wasm`) works and matches the CLI; the front end and its deploy are next.
2. **Add more TTS models.** Calibrate more ElevenLabs models as new `ModelProfile`s beyond `eleven_flash_v2_5`. Each needs an exp0-style probe, a community-message run and fitted weights.
3. **Beat hand-made messages.** Keep matching or beating hand-crafted reference messages on intelligibility of real TTS output.

## Current status (2026-11-25)

- Phase 0 (harness, reading-model experiment) and Phase 1 (native CLI search) are done. The CLI runs end to end in about 1.5–3 s per phrase (release build).
- Phase 2 is partly done: an order-5 Kneser-Ney G2P handles unknown words. Reading model N (native readings for v3/v4) is not started.
- Phase 3 (WASM): the binding is done (`wasm-binding_20261125`, #14): typed `compress()`, CLI parity, a Worker test page, measured in Chrome and Firefox. The Svelte front end and the GitHub Pages deploy are not started.
- Only `eleven_flash_v2_5` (also covering `eleven_turbo_v2_5`) has a profile. The CLI rejects other models.

## Success metrics

All measured on real ElevenLabs clips, transcribed locally, over the 34 reference messages.

| Metric | Latest (exp7, phono v6) | Community reference (exp1) | Target |
|---|---|---|---|
| Whisper similarity to the phrase | 0.900 | 0.847 | ≥ community, and rising |
| Phone fidelity (wav2vec2) | 0.844 | 0.791 | ≥ community, and rising |
| Credits per experiment | 14 (exp7) to ~152 (exp5) | — | Stay within the 10,000/month free tier |

**North star:** a viewer hears the intended phrase on the first try.

Secondary measures: prediction quality of the scorer on unseen clips (Pearson/Spearman), within-phrase ranking accuracy, native time per phrase, and (for the web tool) data file and bundle size. The data file is currently 12.5 MB.

## Scope

### In scope

- English input phrases.
- ElevenLabs TTS models that have a calibrated `ModelProfile`.
- A configurable codepoint budget (default 6).
- Top-N results with scores.

### Out of scope today

- Non-English input.
- TTS providers other than ElevenLabs. Nothing in the repo supports them yet, and each would need its own measurement work, not just a new profile. The design should not rule them out (see [product-guidelines.md](./product-guidelines.md)).
- A server or hosted API. Everything shipped runs client-side.

### Known failure modes

Voicing swaps, ambiguous spellings and rare scripts. See PLAN.md.
