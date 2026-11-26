# phonocompressor

Turn an English phrase into a **very short string of Unicode characters** that sounds like the phrase when read aloud by ElevenLabs text-to-speech. The codepoint budget is configurable (default 6, spaces count).

For example, "hundred thousand apartments" can be written as `҈㌀s` (3 codepoints): `҈` reads as "*100.000*", `㌀` reads as "apartment", and `s` pluralizes it.

This project searches for short strings of exotic characters whose TTS reading lands on the intended sound. (It was built for short messages read aloud by a TTS in a live chat, hence the small default budget.)

## How it works

The TTS models on ElevenLabs' 2.5 generation (Flash v2.5, Turbo v2.5) don't read unfamiliar scripts in their native language. They **transliterate** each character into Latin letters (a table equivalent to Python's [`unidecode`](https://pypi.org/project/Unidecode/); we build ours from Rust's [`deunicode`](https://crates.io/crates/deunicode)), then read the result as English spelling. `딧잇` becomes "dis·is" and is read as "this is"; `㌀` becomes "apartment". The engine exploits that:

```
English phrase
  │  CMUdict (+ order-5 Kneser-Ney G2P for unknown words)
  ▼
target phones
  │  ① beam search: spell phones with transliteration chunks, ≤ 6 codepoints
  │  ② re-rank: predict how the TTS reads each candidate, score against target
  ▼
top-N codepoint strings with scores
```

- **Phone distance** uses [Phoible](https://phoible.org/) feature vectors, with a weighted edit distance that cares about stress and position.
- **Reader** models how the TTS turns a Latin string into sounds: dictionary lookup, word splitting of run-together pseudo-words, initialisms, number words.
- **Scorer** weights are fitted offline against real ElevenLabs clips (`phono fit`), including per-script risk and a spelling-closeness term so results stay recognizable as words.
- **Model profiles** (`crates/core/src/model.rs`) hold everything that depends on the TTS model. Only `eleven_flash_v2_5` (which also covers `eleven_turbo_v2_5`) is calibrated; other models are rejected until they get a profile.

Output is non-deterministic on the TTS side, so the engine returns the top N guesses rather than one answer.

The full design, experiment write-ups and decision history are in [`docs/PLAN.md`](docs/PLAN.md).

## Status

The native CLI works end to end (about 1.5–3 s per phrase in release). The browser binding (`crates/wasm`) works too: it returns the same guesses as the CLI and runs in Chrome and Firefox, about 0.4 s to start and 2–6 s for a first search (see PLAN.md, Phase 3). The web front end and its deployment are not built yet.

**Scope today:** the engine targets one TTS, ElevenLabs `eleven_flash_v2_5` (and `eleven_turbo_v2_5`, which behaves the same). The reader, script risks and scorer weights are all calibrated against that model's actual output, and the CLI rejects models without a calibrated profile. Model-specific settings are kept in a `ModelProfile` so more ElevenLabs models can be added by probing and calibrating them. Other TTS providers might be supported someday, but nothing in the repo does that yet, and it would need its own measurement work, not just a new profile.

Against a set of hand-made reference messages (34 samples, Whisper transcripts of real ElevenLabs output), the current engine matches or beats them on sound and word intelligibility; see PLAN.md for the numbers and for known failure modes (voicing swaps, ambiguous spellings, rare scripts).

## Layout

| Path | Contents |
|---|---|
| `crates/core` | Phone features, G2P, reader, scorer, search. No I/O; builds for `wasm32`. |
| `crates/builder` | Builds `data/generated/phono.bin` from Phoible, CMUdict, the G2P model and the transliteration table. |
| `crates/cli` | The `phono` command-line tool. |
| `crates/wasm` | Browser binding (wasm-bindgen): `Engine` and a typed `compress()`, plus a test page (`www/`) and a parity check against the CLI (`parity/`). |
| `tools/` | Python harness (uv): ElevenLabs synthesis, local Whisper/wav2vec2 transcription, experiment reports. |
| `tools/experiments/` | Experiment definitions (`exp0`–`exp9`). |
| `docs/PLAN.md` | Design, results and decisions. |
| `cmudict/`, `phoible/` | Git submodules (data sources). |

## Getting started

Requirements: [mise](https://mise.jdx.dev/) (it installs Rust with the wasm32 target, uv, fnox, Node and the wasm-bindgen CLI), and a clone with submodules.

```sh
git clone --recurse-submodules https://github.com/jordanpg/phonocompressor.git
cd phonocompressor

mise run data      # export transliteration table + build data/generated/phono.bin (~15 s)
```

Tasks are split across three mise configs: the root (`data`, `phono`, `fmt`, `fmt:check`, `lint`), the harness in `tools/` and the binding in `crates/wasm/`. Project tasks are named by path, such as `mise run //tools:synth`; `mise tasks --all` lists them all. Each task installs the tools it needs on first run; `mise install --monorepo` installs them all up front.

### CLI

```sh
mise run phono compress 'hundred thousand apartments'   # find strings (-n top, -m codepoint budget, --json)
mise run phono score '҈㌀s' 'hundred thousand apartments' # fidelity + alignment
mise run phono read 'ꀠddꏒꀊm'                            # how the TTS should read a string
mise run phono phones 'bad dream'                        # target pronunciation
mise run phono spell 'K AE1 T'                           # spelling options for ARPAbet phones
```

Global flags: `--model <id>` (default `eleven_flash_v2_5`), `--all-scripts` (also search scripts the profile leaves off by default), `--scripts <names>` (search only these, e.g. `latin,hangul,yi`), `--data <path>`.

Evaluation commands:

```sh
mise run phono eval                 # score the sample messages and compress their phrases
mise run phono calibrate            # predicted vs. actual fidelity on harness clips
mise run phono fit                  # fit scorer weights with cross-validation
```

`eval` reads `sample/messages.json` (hand-made reference messages, gitignored). `calibrate` and `fit` need clips exported by the harness.

### WASM binding

`crates/wasm` runs the same engine in a browser. The caller fetches `phono.bin` and passes its bytes in; the engine decodes them once and is reused for every phrase. The test page does this in a Web Worker so a search doesn't block the page.

```js
import init, { Engine } from "./pkg/phono_wasm.js";

await init();
const engine = new Engine(new Uint8Array(await (await fetch("phono.bin")).arrayBuffer()));
engine.compress("bad dream", { maxCodepoints: 6, topN: 10 });
// → [{ text, latin, phones, score }, …]
```

Options: `model` (default `eleven_flash_v2_5`), `maxCodepoints` (1–30, default 6), `topN` (1–50, default 10) and `allowedScripts` (names such as `latin`, `hangul`, `yi`; default: the model's default scripts). Invalid input throws an `Error` that says what to change. The TypeScript types are generated into `phono_wasm.d.ts`.

```sh
mise run //crates/wasm:build     # target/wasm-pkg: .wasm, JS glue, .d.ts (wasm-bindgen --target web)
mise run //crates/wasm:check     # phono-core and phono-wasm build for wasm32
mise run //crates/wasm:parity    # same guesses as `phono compress --json` on crates/wasm/parity/cases.json (Node)
mise run //crates/wasm:serve     # test page at http://127.0.0.1:8000/; open /?bench to time it
```

### Test harness

The harness synthesizes candidates through ElevenLabs and transcribes them locally. It is built to fit the **free tier** (10,000 credits/month); synthesized clips are cached, so nothing is paid for twice.

Cost per clip is `ceil(codepoints × model multiplier)`, so it follows the codepoint budget directly. Flash v2.5 bills at 0.5× on the API, and each request rounds up to a whole credit:

| Codepoints in the clip | Credits (Flash v2.5) |
|---|---|
| 1–2 | 1 |
| 3–4 | 2 |
| 5–6 | 3 |

At the default budget of 6, a candidate costs at most 3 credits per seed, so two seeds is at most 6. The free tier's 10,000 credits would cover roughly 3,300 clips of that length, before counting reruns. `//tools:synth` prints the same estimate on a dry run and refuses to spend past an experiment's `max_credits`. These figures are the harness's estimate, and ElevenLabs' own usage counter has not always agreed with it, so treat the dashboard as authoritative.

```sh
mise run //tools:credits               # credit usage (free API call)
mise run //tools:synth exp9            # dry run: estimated cost of uncached clips
mise run //tools:synth exp9 --spend    # actually synthesize
mise run //tools:transcribe exp9       # local Whisper + wav2vec2 (no credits)
mise run //tools:report exp9           # → tools/results/exp9.md
mise run //tools:export                # clips → data/generated/clips.tsv for calibrate/fit
```

The API key is `ELEVENLABS_API_KEY`, provided through [fnox](https://fnox.jdx.dev/) and only exposed to the tasks that call the API. Transcription uses PyTorch and is intended to run on a GPU.

## Data and licenses

| Source | Used for | License |
|---|---|---|
| Phoible | phone distance | CC BY 4.0 |
| CMUdict | English pronunciations | BSD-style |
| [`deunicode`](https://crates.io/crates/deunicode) | transliteration table, built by `phono-build` | BSD-3-Clause |
| Python `unidecode` | dev comparison only (`mise run //tools:translit-unidecode`); never feeds the data file | GPL-2.0+ |

Generated data lives in `data/generated/` and is not committed. See [THIRD_PARTY.md](THIRD_PARTY.md) for attribution. The code is licensed MIT OR Apache-2.0.
