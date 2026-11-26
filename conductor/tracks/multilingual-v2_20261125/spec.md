# Specification: Calibrate a second ElevenLabs model (Multilingual v2)

**Track ID:** multilingual-v2_20261125
**Type:** Feature
**Created:** 2026-11-25
**Status:** Draft
**Issue:** [#6 Calibrate a second ElevenLabs model](https://github.com/jordanpg/phonocompressor/issues/6)

## Summary

Calibrate `eleven_multilingual_v2` as the second model with a `ModelProfile`, so the CLI and the WASM binding can search for it. The calibration comes from four harness runs on that model: a probe, the community messages, a replay of earlier engine answers, and a check of the new profile's own answers. Reader rules move from `reading.rs` into profile data only where Multilingual v2's clips disagree with them.

## Context

- Product goal 2 is more TTS models. Only `eleven_flash_v2_5` (which also covers `eleven_turbo_v2_5`) has a profile; the CLI and the binding reject any other model id.
- Since v6.2, a profile (`crates/core/src/model.rs`) holds the script table (reading, risk, default on), the script preference order, `ReaderParams`, and the scorer's `weights` and `predict`. `model::profile()` resolves the id for the CLI's `--model`, for `calibrate`/`fit` (which keep only that model's clips) and for the binding's `model` option.
- exp0 (2026-11-24, seed 1, clips cached) on Multilingual v2:
  - mostly reading model T: `딧잇` → "This is", `ꀠddꏒꀊm` → "Bad dream", `㌀` → "Apartment";
  - katakana read natively: `フリ` → "Free";
  - noisier than Flash v2.5: `거` → "Keo", `벗` → "Bye", `딧잇ꌟ` → "This is it", `҈` → "Hander a thousand", `™` → "Me", `Ж` → "J".
- Still Flash-only code in `reading.rs`: the capital-letter rules (`split_caps`), the plural rule and number grouping (`number_token_words`).
- `phono fit` learns from pairs of different answers to the same phrase. The local clips (`clips-all9.tsv`, Flash v2.5 only) hold 160 distinct answers to the 34 community phrases: the 34 community messages (173 characters) and 126 engine answers from v1–v6 (628 characters).
- Credits: about 891 of 10,000 used this month (after exp10).

## User Story

As a chat viewer whose stream's TTS uses Multilingual v2, I want to choose that model and get guesses calibrated on it, so that the strings I send are read as my phrase on that model, not only on Flash v2.5.

## Acceptance Criteria

- [ ] **Probe (exp11).** exp0's cases plus new cases for what exp0 left open (hiragana, kana, the other scripts, numbers, capitals, plurals, symbols) run on Multilingual v2 at 2 seeds. They are transcribed, reported, and written up in PLAN.md with a draft script table.
- [ ] **Community baseline (exp12).** The 34 community messages × 2 seeds on Multilingual v2, reported, with Whisper similarity and phone fidelity written up next to Flash v2.5's exp1.
- [ ] **Fit data (exp13).** The 126 earlier engine answers × 2 seeds on Multilingual v2, exported with exp12 as one clips file for that model.
- [ ] **Profile.** A `MULTILINGUAL_V2` profile for `eleven_multilingual_v2`:
  - script table and preference order from exp11;
  - reader constants checked against its clips;
  - weights from `phono fit --model eleven_multilingual_v2`, with held-out ranking reported next to Flash's weights.

  It is in `PROFILES`, so `--model eleven_multilingual_v2` and the binding's `model` option work. Flash v2.5 stays the default.
- [ ] **Reader rules.** The capital-letter rules, plurals and number grouping are each checked against Multilingual v2's clips. A rule they disagree with becomes profile data, with tests. A rule they agree with stays, and PLAN.md says so. Flash v2.5's `phono eval` output stays byte-identical.
- [ ] **Live check (exp14).** The new profile's top answer for each of the 34 phrases × 2 seeds on Multilingual v2. Whisper similarity and phone fidelity are recorded against exp12's community numbers. The profile ships either way; if it falls short, a follow-up issue says where.
- [ ] **Parity and docs.** `wasm:parity` has a Multilingual v2 case and passes. PLAN.md, product.md, tech-stack.md and the README name the new model.

## Dependencies

- Existing code: `ModelProfile` and `PROFILES` (`model.rs`), `Reader` (`reading.rs`), `phono calibrate`/`fit`/`eval --export`, the harness (`synth`, `transcribe`, `report`, `export-clips`), `crates/wasm/parity`.
- Local data in the main checkout, all gitignored: `fnox.toml` (API key), `.cache/harness/` (holds exp0's Multilingual v2 clips), `sample/messages.json`, `data/generated/ours-v*.json`.
- Credits: about 2,100 at the assumed 1 credit per character, spent in four separately approved runs.
- Not blocked by [#7](https://github.com/jordanpg/phonocompressor/issues/7) (reading model N): Multilingual v2 reads by transliteration except katakana, which is marked `Native` and never searched.

## Out of Scope

- v3, v4, v4 Turbo and reading model N (#7).
- Flash v2 or any third model.
- A model picker in the web tool (the front end isn't built).
- Changing the default model, or Flash v2.5's weights and reader rules.
- Pseudo-word reading corrections (PLAN.md item 5) and context-aware generation (#5).
- The real "weeb" voice (the free tier can't use it), `language_code`, text normalization "on", phoneme tags.

## Technical Notes

- **Cost check first.** `harness:models` (free) confirms Multilingual v2 is available and gives its cost multiplier before any estimate is shown. Each run gets its own dry run and approval (workflow rule 4).
- **Don't pay twice.** The harness cache, API key, samples and exports live in the main checkout. Symlink the gitignored paths (`.cache/harness`, `sample`, the needed `data/generated` files, `fnox.toml`) into the worktree. The first dry run must show exp0's seed-1 clips as cached.
- **The replay needs several case files.** `cases_from` takes one file. exp13 needs the v1–v6 exports, deduplicated and without the community texts already in exp12. Task 2.1 lets `cases_from` take a list. Keep sample text out of committed files and issues.
- **Stop point.** If exp12 shows Multilingual v2 doesn't read the community messages through transliteration (non-English language ID, many silent clips), stop and ask before exp13.
- **Fitting.** Start the profile from Flash's reader constants and weights. Fit as in v6: deletion and insertion costs move, substitution scales stay put, and held-out ranking is 5-fold by phrase. `phono fit` doesn't fit `ReaderParams`; check those by counting what the clips show (word splits, glued digits), as Flash's were.
- **Moving rules.** A moved rule becomes profile data (new `ReaderParams` fields or a small rules struct), not a module constant. Flash's values reproduce today's behaviour exactly.
- **Gates.** The usual gates (fmt, clippy, tests, `phono eval` on Flash before and after), plus `phono calibrate` for both models, `wasm:check` and `wasm:parity`.

---

_Generated by Conductor. Review and edit as needed._
