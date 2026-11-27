# Product Guidelines

## Voice and tone

**Concise and direct.** Plain sentences. State results and limits outright, with the numbers and what they were measured on. No marketing language.

This applies to the README, `docs/PLAN.md`, CLI output and the web tool alike. The web tool's readers are chat viewers rather than developers, so it uses fewer terms, but the same plainness.

### By context

| Context | Tone | Example |
|---|---|---|
| Results | State what was measured | "Whisper similarity 0.900 vs. community 0.847 (exp7, 34 phrases, 2 seeds)." |
| Limits | Say it once, plainly | "Only `eleven_flash_v2_5` is calibrated. Other models are rejected." |
| CLI errors | What failed, then how to fix it | ``data/generated/phono.bin: No such file or directory (os error 2) (run `mise run data`)`` |
| Web tool, no good result | Honest, with a next step | "No string under 6 codepoints scores well. Try a higher budget or a shorter phrase." |
| Uncertainty | Name it, don't hide it | "TTS output varies between runs, so these are the top 5 guesses." |

### Words to use

- **codepoints** when counting the budget (not "characters" or "letters": one codepoint may read as a whole word).
- **read / heard** for what the TTS does and what a listener gets.
- **guesses / candidates** for results. The TTS is not deterministic.
- **calibrated / measured / not measured** for model and script support.

### Words to avoid

- "magic", "AI-powered", "smart", "perfect", "guaranteed".
- Claims about a model, script or provider that no experiment has measured.
- Describing the method as limited to 6 codepoints. The budget is configurable; 6 is the default.

## Design principles

### 1. Measure against real TTS

Engine changes are judged by experiments on real ElevenLabs clips (Whisper for words, wav2vec2 for phones), not by intuition. Results and decisions are written up in `docs/PLAN.md`.

- **Do:** Run `phono eval` and `phono calibrate` on existing clips before spending credits. Record each experiment's setup, cost and results.
- **Do:** Treat a disagreement between the reader model and real clips as a finding.
- **Don't:** Ship a reader rule, override or weight change justified only by "it should read like this".
- **Don't:** Read an improvement off a single seed or a single phrase.

### 2. Browser-first, no I/O in the core

`phono-core` does no I/O and builds for `wasm32`. Everything shipped must run client-side.

- **Do:** Pass data in as bytes (`Engine::from_bytes`). Keep file, network and environment access in `phono-cli`, `phono-builder` and `tools/`.
- **Do:** Watch data file and bundle size, and time per phrase, as user-facing costs.
- **Don't:** Add a dependency to `phono-core` that doesn't build for `wasm32-unknown-unknown`.
- **Don't:** Plan features that need a server.

### 3. Credit-frugal

Calibration fits the ElevenLabs free tier (10,000 credits/month).

- **Do:** Cache every clip by every parameter that affects the audio. Dry-run by default. Set `max_credits` on every experiment.
- **Do:** Reuse existing clips before synthesizing new ones.
- **Don't:** Run `//tools:synth --spend` without the user's explicit go-ahead.
- **Don't:** Synthesize clips that a cached clip already answers.

### 4. License-clean distribution

Only permissively licensed code and data ships (in the WASM bundle, on GitHub Pages, or in the `phono-data` releases). The repo is Apache-2.0.

- **Do:** Build the transliteration table from `deunicode` (BSD-3) plus overrides written from observed TTS behaviour. Attribute data sources in `THIRD_PARTY.md`.
- **Do:** Check a new dependency's or dataset's license before adding it.
- **Do:** Ship `third-party-notices.html` (`mise run //crates/wasm:notices`) with every deploy, and link it from the site. Add new data sources and bundled npm packages to it.
- **Don't:** Let GPL-licensed `unidecode` (or anything derived from its table) feed shipped data. It stays a dev-only comparison in `tools/`.
- **Don't:** Copy entries from `unidecode`'s table into overrides.

### 5. Support diverse models and providers

Only ElevenLabs Flash v2.5 is calibrated today, but nothing in the design should assume it is the only target.

- **Do:** Keep everything model-specific (script readings and risks, character preferences, reader constants, fitted weights) in a `ModelProfile`, not in module constants.
- **Do:** Keep target phones, phone distance and search model-independent.
- **Do:** Leave room for provider-level differences (billing, request shape, normalization) in the harness rather than hard-coding ElevenLabs everywhere.
- **Don't:** Accept an uncalibrated model or provider silently. Reject it until it has a measured profile.

## Accessibility (web tool)

Target WCAG 2.1 AA.

- Full keyboard use: type the phrase, run, and copy a result without a mouse.
- Visible focus; contrast of at least 4.5:1 for text.
- Results are copyable as plain text. Never convey score only through color.
- Exotic-script output may not render in every font. Show the Latin transliteration next to each result so it stays readable.

## Error handling

- **Prevent:** Validate input up front (empty phrase, non-English input, budget out of range, unknown model).
- **Report:** `[what failed] + [why, if useful] + [how to fix it]`. Core and CLI errors are `Result<_, String>` messages in this style.
- **Never fail silently:** an unknown model, a missing data file or a missing profile is an error, not a fallback.
- **In the web tool:** keep the user's input when something fails; offer a concrete next step (raise the budget, shorten the phrase).
