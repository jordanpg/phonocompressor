# phonocompressor — plan

## Goal

Take an English phrase and produce **≤ 6 Unicode codepoints** that, when posted to
Twitch chat and read by ElevenLabs TTS, sound like the phrase. Spaces and
punctuation count toward the 6. TTS output is not deterministic, so we return
the **top N guesses**, not one answer. Ships as a WASM bundle; a native CLI
comes first.

Fixed constraints: input is English; any ElevenLabs model may be used
(we pick whichever is most predictable); messages reach the TTS
essentially unmodified; testing must fit the ElevenLabs **free tier**
(10,000 credits/month, 1 credit per character, Flash v2.5 at half price
via API).

## Key finding: the samples are spelled, not sounded out

Every message in `sample/messages.json` decodes cleanly if each character is
replaced by its [unidecode](https://pypi.org/project/Unidecode/)
transliteration and the result is read as **English spelling**:

| Message | unidecode chunks | Read as English |
|---|---|---|
| `ꀠddꏒꀊm` | ba·d·d·re·a·m | "baddream" → *bad dream* |
| `ꄫꀔ` | to·e | "toe" |
| `ꀂꀑꊂꌟ` | i·o·wa·sux | "iowasux" → *iowa sucks* |
| `딧잇 엪잋솪` | dis·is · ep·ic·swagg | *this is epic swag* |
| `ㅊ헻ㅋ힘PC` | c·hec·k·him·P·C | *check him PC* |
| `嬶ꍱ둥@ꏮ壗` | Kaka·chux·dung·@·jo·Mama | *kaka chucks dung at joe mama* |
| `҈㌖㌐ꂶ동` | \*100.000\*·kilometer·giga·max·dong | *100,000 kilometer giga max dong* |

It is unidecode specifically. anyascii disagrees on 嬶/壗 (Bi/Jin), 핓
(Pich) and ㄹ (l), and those versions don't fit the intended phrases. Several
messages only work as spelling. Read as Korean, 딧잇 is [디싣] and ㅊ is read
by its name, *chieut*.

So we have two candidate **reading models**:

- **T (transliterate):** each character becomes its Latin chunk. A run of
  characters without spaces becomes one pseudo-word, which the TTS reads with
  English spelling rules.
- **N (native):** the character is read in its own language (Korean for
  Hangul, Mandarin or Japanese for CJK, …).

The truth is probably T for scripts ElevenLabs doesn't support (Yi,
Cherokee, Canadian syllabics, Runic, CJK Compatibility) and possibly a mix
for supported ones (Hangul, kana, CJK, Cyrillic, Devanagari). Phase 0
settles this. The rest of the plan assumes T, with N as a second kind of
chunk.

Two caveats:

- The community may simply be choosing characters by the Unicode name shown
  on their site. For Hangul and Yi the Unicode name equals unidecode's output,
  so those messages don't prove the TTS transliterates. The CJK cases
  (kaka/mama) and ҈ → "100,000" are the strongest evidence that something
  unidecode-like sits in the real pipeline.
- Under T, the motivating example "hundred thousand apartments" becomes
  **`҈㌀s`** (3 codepoints): ҈ = `*100.000*`, ㌀ = `apartment`.

### Phase 0 result (exp0, 2026-11-24): T confirmed on the 2.5-generation models

Full table: `tools/results/exp0.md`. 20 cases × 7 models, voice Adam (premade,
standing in for "weeb", which free accounts can't use via the API), 1 seed,
205 credits. Billing rounds each request up to a whole credit, so very
short requests cost more than characters × multiplier.

- **`eleven_turbo_v2_5` and `eleven_flash_v2_5` read through transliteration
  (T).** Examples, as transcribed by Whisper:
  - `딧잇` → "This is"; `ㅊ헻ㅋ힘` → "Check him"; `솪` → "Swag"
  - `ꌟ` → "Sucks"; `ꀠddꏒꀊm` → "Bad dream"; `ᏃᎧ` → "NOCA"
  - `嬶` → "Kaka"; `萙` → "Kutabireru" (10 letters from one codepoint)
  - `㌀` → "apartment"; **`҈㌀s` → "100,000 apartments"** (3 codepoints)

  Whisper language ID says English (≥ 0.9) on all of these. The two models
  behave almost identically, and the English-only `eleven_flash_v2` mostly
  agrees.
- **Stage 2 of T is real English reading of the Latin string.** `hu·ri` →
  "hurry", `km` → "calm/come" (a pseudo-word, not "kilometers"), `(tm)` →
  "TM".
- **Exceptions on the 2.5 models.** Devanagari is read natively (`क` → "ka"),
  so it falls under model N, not T. Cyrillic `Ж` comes out /ʒi/.
- **`eleven_multilingual_v2`** is mostly T, but reads katakana natively
  (`フリ` → "free") and is noisier.
- **`eleven_v3` is unusable.** It produces silence for Yi, Cherokee, Runic,
  Japanese-only kanji and ҈.
- **`eleven_v4` and `eleven_v4_turbo` are unusable.** They read Hangul as
  Korean (`딧잇` → "did it" = [디싣]), garble Yi, and read `㌀` as "zero".

**Decision:** target `eleven_flash_v2_5`. Turbo v2.5 is deprecated but
behaves the same. Reading model T is the primary model. Model N is only
needed for Devanagari (and katakana if Multilingual v2 is ever targeted).

Not measured yet: the actual "weeb" voice.

### Phase 0 result (exp1, 2026-11-24): community messages on Flash v2.5

All 34 sample messages, 2 seeds, same stand-in voice. Full table:
`tools/results/exp1.md` (gitignored, like `sample/`, since it reproduces the messages).

- **Whisper language ID says English on 67 of 68 clips.** Model T holds
  across real community messages, not just the hand-picked cases.
- **Similarity to the intended phrase** (crude spelling similarity of Whisper's
  English transcript): mean 0.86 / 0.88 per seed, with 24 of 34 messages at
  ≥ 0.8 on each seed. 13 messages come out exactly right on both seeds:
  *where obama, very moldy frog, iowa sucks, funeral, toe cancer, this is epic
  swag, harpy, pickle pounder, bad dream, jesus, check him PC, time loop,
  calypso*.
- **Run-to-run variance is real but modest.** Mean cross-seed similarity is
  0.88, and 16 of 34 transcripts are identical. The differences cluster in
  long run-together strings and ambiguous ones.
- **Why the low scorers fail** (each is a design lesson):
  - *Words run together.* `빞빞엠t뤀` → "Bibi Pemtrak" (target "beep beep am
    truck") and `암빅ꍲn궀` → "Ombigchungus" (target "am big chungus"). With
    no spaces, the whole string gets one English stress pattern. The search
    must weigh a space (1 codepoint) against a mangled word boundary, and the
    reranker's letter-to-sound must handle long pseudo-words.
  - *Model-specific messages.* `フリダクイ` → "Huridakui" (target "free the
    kwee"). Its note says it was written for v3. On 2.5 models, katakana
    follows T using unidecode's romanization (`hu`, not `fu`).
  - *Missing English sounds.* `ꊀꄣㄹ되욋` → "What are Doos" (target "what are
    thoooose"): no chunk gives /ð/.
  - *Noise from the metric, not the audio.* Whisper turns rare words into real
    ones ("rizz" → "Rids", "shoomimi" → "Show me me"), and spelling similarity
    penalizes homophones. The Phase 1 scorer should compare **phones**
    (CMUdict target vs. wav2vec2 output, using Phoible distances), not
    Whisper text.
- Picking robust outputs: since the same text varies by seed, a candidate's
  harness score should average over 2+ seeds.

Credits: exp1 was estimated at 183 (each request rounded up). The account
counter has been inconsistent (it read 205 after exp0, and 163 after exp1);
the dashboard is the authority.

### Chunk inventory (unidecode over the listed blocks)

13,133 distinct alphabetic chunks. Most are 3–6 letters. Hangul syllables
supply 11,172 (up to 7 letters, e.g. `ggwaegs`) and Yi 1,165 (CV plus a tone
letter t/x/p that is read as a consonant: `sux`, `nex`, `wat`). A few long
outliers give the biggest gains: CJK Compatibility (`apartment`,
`kilometer`, `kilowatt`, `percent`, …) and Japanese-only kanji with long
readings (`kutabireru`, `sakenomoto`). Non-alphabetic entries matter too
(`҈` → `*100.000*`, `™` → `(tm)`, `㎞` → `km`).

## Architecture

```
English phrase
  │ CMUdict (with stress) → letter-to-sound rules for unknown words
  ▼
target phones  h ʌ n d ɹ ə d θ aʊ z ə n d …
  │
  │ ① candidate generation: phones → spelling options → tile with chunks (≤ 6 codepoints)
  │ ② re-ranking: read each candidate string as English → align phones → score
  ▼
top-N codepoint strings + scores
```

**Phone distance (Phoible).** Weighted Hamming over the 37 features in
`phoible/raw-data/FEATURES/phoible-segments-features.tsv`. Major-class
features weigh most, then place, with laryngeal features light. Contour
segments (diphthongs) are compared half by half.

**Sequence score.** A weighted edit distance between target and candidate
phones:
- Phone substitutions cost the feature distance.
- Deletions are weighted by stress (CMUdict 0/1/2) and position: stressed
  vowels and word-initial consonants are expensive; schwas and consonants in
  clusters are cheap.
- Extra vowels inserted between consonants are cheap.

The result is normalized to a 0–1 fidelity.

**① Candidate generation.** A dynamic program over the state
(target phone index, position in a trie of chunks, codepoints used ≤ 6):
- Each step picks a spelling for the next phone(s) from a
  phone-to-spelling table (/k/ → c, k, ck; /ks/ → x; /ʃ/ → sh, ch, ti; …),
  starting hand-written and later learned from CMUdict alignments.
- The chosen letters are walked through the chunk trie. Completing a chunk
  costs 1 codepoint, and so does a space at a word boundary.
- Approximate matches are allowed using the costs above.
- Keep the top K per state.

**② Re-ranking.** English spelling depends on context: `to`+`e` reads
"toe", not "to-ee". So each candidate's full Latin string goes through
English letter-to-sound and gets scored against the target phones. The
best N are returned.

**Reading model N** fits in as chunks that produce phones directly instead
of letters: Korean phonology for Hangul, pinyin and Japanese readings from
Unihan for CJK. It's enabled for a script only if Phase 0 shows the TTS
reads that script natively.

**Choosing a character for each chunk.** Many characters share a chunk
(`ka`: Cherokee, kana, Yi, …). Prefer the script whose reading Phase 0
showed to be most reliable.

## Repo layout

```
crates/core      phone features, scoring, search — no I/O, builds for wasm32
crates/builder   native: Phoible + CMUdict + transliteration table + Unihan → one binary data file
crates/wasm      wasm-bindgen bindings (embeds the data file)
crates/cli       native CLI for iterating
tools/           Python (uv, run via mise): ElevenLabs harness, ASR, evaluation
sample/          community sample messages (test set)
```

## Phases

### Phase 0 — test harness and reading-model experiment

Status: harness built (`tools/`). Usage:

```sh
mise run harness:credits              # credit usage this period (free API call)
mise run harness:models               # TTS models + cost multipliers (free API call)
mise run harness:synth exp0           # dry run: count uncached clips, estimate cost
mise run harness:synth exp0 --spend   # synthesize (refuses if over the experiment's max_credits)
mise run harness:transcribe exp0      # local Whisper + wav2vec2 on the GPU, no credits
mise run harness:report exp0          # → tools/results/exp0.md
```

Experiments live in `tools/experiments/*.toml`. Clips and transcripts are
cached in `.cache/harness/` (gitignored), keyed by every request parameter
that affects the audio. The API key comes from fnox via mise task `secrets`,
so only the three API tasks ever see it.

- `tools/synth`: ElevenLabs TTS via `/v1/text-to-speech/{voice}/with-timestamps`,
  API key from `ELEVENLABS_API_KEY`. Every result is cached by
  (model, voice, text, seed, normalization setting) in a gitignored
  `.cache/`, so nothing is paid for twice.
- `tools/asr`: local Whisper set to English ("what would a viewer hear?") and
  a local wav2vec2 phoneme recognizer that outputs IPA ("what phones came
  out?"). Both run locally, with no ElevenLabs credits spent on transcription.
- **Experiment 0**: about 20 strings where T and N predict different audio,
  run on 3–4 models (Flash v2.5, Multilingual v2, v3, v4 Turbo) with one
  English voice. Examples:
  - `딧잇` (T: "dis is" / N: [디싣])
  - `ㅊ헻ㅋ힘` (T: "check him" / N: "chieut het kieuk him")
  - `㌀` (T: "apartment" / N: "apāto")
  - `क` (T: "k" / N: "ka")
  - `ᚠ` (T: "f" / letter name "fehu")
  - `嬶`, `フリ` (T: "huri" / N: "furi"), `҈`, `҈㌀s`
  - Yi alone vs. Yi mixed with Hangul, to check whether neighbours switch the
    language.

  Budget: about 400 credits.
- Run the 34 sample messages through the harness. This shows what an
  "acceptable" fidelity looks like in practice and gives a reference point
  for tuning our scorer. Budget: about 200 credits.

### Phase 1 — core search (native CLI)

Phoible feature distance, CMUdict lookup, transliteration chunk table, the
candidate-generation search, and a simple letter-to-sound re-ranker. Goal:
reproduce or beat the sample messages under our own scorer, then check the
top candidates with the harness.

**Status (2026-11-24): working end to end; harness check (exp2) prepared, not run.**

```sh
mise run data                                   # unidecode export + builder → data/generated/phono.bin (~15 s)
mise run phono compress 'hundred thousand apartments'
mise run phono score '҈㌀s' 'hundred thousand apartments'   # fidelity + alignment
mise run phono read 'ꀠddꏒꀊm'                    # how the TTS should read it
mise run phono eval                             # all sample phrases vs. the community's messages
```

What's in `crates/core`:

- `features`: Phoible feature table. Distance is normalized over the features
  either segment specifies. Vowel pairs use vowel quality only (height,
  backness, rounding, tenseness, with contours), plus a rhotic term, because
  Phoible encodes ɚ as ə plus a diacritic.
- `g2p`: graphone joint-sequence model (1–2 letters ↔ 0–2 phones). The
  builder trains it with EM alignment on CMUdict. Since v3 it is an order-5
  modified Kneser-Ney n-gram (`--g2p-order`), precomputed in backoff form,
  with memoized `pronounce` (see "v3: Kneser-Ney G2P" below). Held out 2%:
  **66.0% word accuracy, 9.4% phone error rate** (v1–v2 Witten-Bell
  trigram: 54.1%, 12.8%). It reads pseudo-words sensibly: `sux` → S AH1 K S,
  `gigamax` → G IH1 G AH0 M AE0 K S.
- `reading`: how the TTS reads a Latin string:
  - dictionary first;
  - **word splitting** for unknown letter runs (`iowasux` → iowa + sux,
    matching exp1);
  - initialisms for vowel-less runs;
  - number words (`*100.000*` → one hundred thousand) and `@ & + %`.
- `score`: alignment with Phoible substitution costs. Deletion costs depend
  on stress and position (onset 0.9, stressed vowel 1.0, schwa 0.35). Vowel
  substitutions are scaled up, and a consonant/vowel mismatch adds a penalty.
  **Sanity check:** it scores the community messages much as the real TTS
  rendered them. Messages ElevenLabs read perfectly score 1.0; "free the kwee"
  scores 0.41 and "lip chips…" 0.46.
- `search`: beam over target phone positions. Each step spells 1–2 phones via
  graphone units walked through the chunk trie, deletes a phone, or matches a
  whole phonetic chunk (`҈`, `㌀`). Every finished candidate (8–15k per query)
  is kept, deduplicated by transliteration, and the cheapest 600 are re-read
  and scored. About 0.6–1.3 s per phrase in release.

**Result by our own scorer** (`phono eval`, 34 sample phrases): mean
fidelity **0.94 vs. 0.85** for the community's messages; ours is ≥ theirs on
25–26 of 34. This is the scorer grading its own search, so it's optimistic
until exp2 runs.

**exp2 result (real TTS, 2026-11-24): the search loses to the community.**
Same setup as exp1 (Flash v2.5, voice Adam, 2 seeds), but playing our top
answer per phrase. Mean Whisper spelling similarity is **0.78 for ours vs.
0.87 for the community's**. Ours wins clearly on 4 phrases ("lip chips…", "no
rizz", "free the kwee", "shoomimi") and loses clearly on 20. Our predicted
fidelity correlates only moderately with what Whisper heard (Pearson 0.47,
Spearman 0.49, n = 68). The search optimizes against the reading model and
finds its blind spots:

- **Voicing swaps are too cheap** (Phoible weights voicing lightly). Results:
  `게숫` → "geysas" (target "jesus"), `둑둑` → "doot doot", `gwon` for
  "quan".
- **Ambiguous spellings are read confidently.** `ge` is either "jee" or "gay";
  `togansér` (target "toe cancer") came out "tug answer".
- **Rare scripts are risky.** CJK chunks with a capital and trailing space
  (`傲` "Ao "), Canadian syllabics and Ethiopic (`ᗈ`, `ዐ`, `ᒎ`) produced
  misreadings like `옿傲밈잋한` → "Oh, I can mimic an". The community sticks
  to Yi, Hangul and Latin, which proved reliable.

**Calibration after exp2 (offline, no credits).**
`mise run harness:export` then `mise run phono calibrate` compares, for each of
the 136 exp1+exp2 clips:

- **Predicted:** our fidelity for the text.
- **Truth:** the mean of two independent measures:
  - phone-level fidelity of what wav2vec2 heard against the target, with the
    same Phoible costs (vowel differences ×2 and no rhotic penalty, because
    the recognizer's vowel labels are noisy: English ʌ comes out `a`,
    schwas `ɚ`; tuned to maximize agreement with Whisper);
  - Whisper text similarity.

  The two agree at r = 0.61.

| prediction | r vs truth |
|---|---|
| v1: single best reading | 0.47 |
| + expected fidelity over the G2P 3-best readings | 0.52 |
| + per-script risk (`Script::risk`) | 0.58 |
| + voicing weight 1 → 3 (`v2`, now default) | 0.59 |

The script-risk table was written after seeing exp2's failures, so part of
its gain is in-sample. exp3 tests v2's answers out of sample.

**exp3 result (real TTS, 2026-11-24): better, still behind on word
intelligibility.** v2's answers, same setup as exp1/exp2, about 118 credits:

| run | answers | phone fidelity | Whisper similarity |
|---|---|---|---|
| exp1 | community | 0.783 | 0.847 |
| exp2 | phono v1 | 0.750 | 0.727 |
| exp3 | phono v2 | **0.780** | 0.751 |

(Whisper similarity here is `calibrate`'s edit-distance measure; the
harness report's difflib measure gives 0.80/0.79 per seed vs. exp1's
0.86/0.88.)

- **Sound:** v2 now matches the community on phone-level closeness.
- **Words:** it still loses on whether Whisper hears the intended *words*.
- **Clear wins:** "Calypso" 1.0 (community 0.66), "harpy" 1.0 (0.66),
  "no rizz" 0.91 (0.64), "lip chips…" 0.75 (0.49).
- **Clear losses:**
  - "beep beep am truck" → "Bipipeng Drug";
  - "quantum" → "Quandam";
  - "check him PC" → "Shefim PC" (`chep` read "shef");
  - "time loop" → "time look";
  - "what are thoooose".
- **Voicing swaps are still being exploited** (`d룩` "drug" for "truck",
  `돔` for "tum"). Each is cheap at the phone level but breaks word
  recognition.

**Prediction quality on exp3 (out of sample):** Pearson 0.71 / Spearman 0.64
against truth. v1-style scoring gets 0.69 / 0.68 on the same clips, so v2's
in-sample gains mostly did not transfer. The scorer is decent within a set of
answers, but it doesn't yet know what makes *words* recognizable.

**Next idea:** judge fidelity per target word with a harsher aggregate (one
unrecognizable word should sink a candidate), and reward readings that land
on real dictionary words. Test offline with `phono calibrate` on all 204
clips before spending credits.

**v3: Kneser-Ney G2P (from the WikiPron review, 2026-11-24).**
[wikipron-modeling](https://github.com/CUNY-CL/wikipron-modeling) trains the
same kind of model as ours (Novak et al.'s pair n-gram), but with Kneser-Ney
smoothing and an order tuned on a dev set. With our aligner and units
unchanged, on its CMUdict train/test split (10,915 test words, word error
ignoring stress):

| model | trigram WB (v2) | KN 4 | **KN 5 (v3)** | KN 6 | KN 8 |
|---|---|---|---|---|---|
| word error | 34.0% | 27.8% | **26.6%** | 26.4% | 26.5% |
| phone error | 8.5% | 6.8% | 6.6% | 6.5% | 6.5% |

Order 5 is the default: 5 unit ids fit in a u64 key, and higher orders add
almost nothing. Modified KN (three discounts) adds +0.3 points of held-out
accuracy over a single discount. To stay as fast as v2, `pronounce` is
memoized: one search made 1,430 calls for 372 distinct words. `rerank` also
reads each candidate once instead of twice. Search time is now about the
same as v2's (13.0 s vs 13.9 s for six phrases). `phono.bin` grows from 3.3 to 12.5 MB
(unpruned f32 tables).

WikiPron's own English data wasn't adopted:
- It is noisy: 153 distinct phones in the paper's UK English sample, and 37.4%
  best word error on US English in the SIGMORPHON 2021 shared task.
- Its pre-scraped files carry no stress.
- Its Wiktionary data is CC BY-SA.

It does have about 43.7k plain words that CMUdict lacks, including Twitch
slang. That is useful at most as a small hand-checked slang list.

**exp4 result (real TTS, 2026-11-24, ≤ 102 credits): better prediction,
same answers.** v3's top answers; 19 of 34 changed from v2, and the 15
unchanged ones reused exp3's cached clips.

- **Prediction on unseen clips (exp4):** predicted vs truth went from Pearson
  0.648 / Spearman 0.712 with the v2 G2P to 0.745 / 0.762 with v3. On a paired
  bootstrap, Pearson gains +0.097 (95% CI +0.023 to +0.201); Spearman gains
  +0.050 (P(Δ ≤ 0) = 0.05).
- **Pooled over all 272 clips**, the gain is only +0.02. Each model
  over-predicts most on the answers it picked itself (v3 on exp4: predicted
  0.896, truth 0.759; v2 on exp3: 0.902 vs 0.766). The search exploits
  whichever model it uses.
- **The answers didn't improve:**

  | run | answers | phone fidelity | Whisper similarity |
  |---|---|---|---|
  | exp1 | community | 0.789 | 0.847 |
  | exp3 | phono v2 | 0.778 | 0.751 |
  | exp4 | phono v3 | 0.783 | 0.736 |

  The harness report's difflib similarity is 0.78/0.79 per seed. Of the 19
  changed answers, 5 got better, 6 worse and 8 stayed about the same.
  - Best gain: "doug doug did that", heard exactly.
  - Losses: mostly risky scripts the search still picks (Canadian syllabics
    `ᕠ` `ᕦ`, CJK `囸` `筽`, Armenian `Թ`), plus `싯엪` → "Sipix".
  - So the bottleneck is still word intelligibility and script choice, not
    letter-to-sound.

Clips: `data/generated/clips-exp4.tsv` and `clips-all4.tsv` (exp1–exp4;
`harness:export` still writes only exp1+exp2).

Known limitations / next steps (in priority order after exp2):

0. **Make re-ranking robust to reading uncertainty.**
   - Score the expected fidelity over the G2P n-best (ambiguous spellings
     score lower).
   - Raise the voicing weight.
   - Default to the scripts exp0–exp2 showed are reliable (Latin, Hangul, Yi,
     CJK Compatibility words, Cyrillic number signs); others opt-in or
     penalized.
   - Then measure against a **phone-level** harness metric (wav2vec2 IPA vs.
     target, via Phoible distances) instead of Whisper text, and fit the
     scorer weights to exp0–exp2 clips.
1. **Generation spells without context.** Spelling costs ignore neighbouring
   letters, so mid-search `to|c…` beats `toe|c…`. The better spelling then
   loses its state slot before re-ranking sees it ("toe cancer", "time
   loop", "iowa sucks" still miss the community's exact solution, though
   close alternatives score ≥ 0.95). Fix: score units with the joint trigram
   (state = last two graphone units) during generation.
2. Results vary slightly between runs (ties broken in hash-map order). Make
   ordering deterministic.
3. The scorer's weights are hand-set. Fit them against harness data
   (exp1 + exp2: real TTS clips with phone transcriptions).
4. Speed is fine natively; WASM (Phase 3) may need a smaller beam or pruning.
   The data file is 12.5 MB since v3: the G2P tables are unpruned u64 keys +
   f32. Delta-coded keys, u16 log-probs, or entropy pruning would shrink it.

### Phase 2 — better re-ranking and native readings

Replace rule-based letter-to-sound with a model trained on CMUdict, add
reading model N for whichever scripts Phase 0 says need it, and learn the
phone-to-spelling table from CMUdict alignments.

### Phase 3 — WASM

wasm-bindgen API:
`compress(phrase, { maxCodepoints: 6, topN, allowedScripts }) → [{ text, latin, phones, score }]`.

## Data and licenses

| Source | Used for | License |
|---|---|---|
| Phoible features | phone distance | CC-BY 4.0 (attribution required) |
| CMUdict | English pronunciations | BSD-style |
| Unihan | CJK readings (model N) | Unicode License |
| Transliteration table | chunk inventory | Python `unidecode` is **GPL-2.0+**; Rust `deunicode` is BSD-3. We can also rebuild most of the table from Unicode names + Unihan. Decide before shipping the table in the bundle. |

## Open questions

- Which ElevenLabs model and voice to standardize on (Phase 0 decides).
- Whether a space between pseudo-words is worth its codepoint
  (`ꆌ rizz` suggests yes for word boundaries).
- How far to trust the "close enough" behaviour the samples show. The
  harness scores should set the threshold, not intuition.
