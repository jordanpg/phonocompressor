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

**Model profiles (v6.2).** Whatever depends on the TTS model lives in a
`ModelProfile` (`crates/core/src/model.rs`):
- how each script is read (transliterated, native, unmeasured), its risk,
  and whether the search uses it by default;
- the order for picking a character per chunk;
- the reader's splitting and glued-number constants;
- the fitted scorer weights.

Target phones, phone distances and the search are the same for every model.
Two models have profiles: `eleven_flash_v2_5` (the default, also covering
`eleven_turbo_v2_5`) and, since exp11–exp14, `eleven_multilingual_v2`. Its
profile differs in the script table (kana native; risks for `҈`, syllabics,
Ethiopic, conjoining jamo), the symbol words (`@` silent) and
`glued_number_drop`; the rest is Flash's. A new model needs an exp0-style
probe, a community-message run and a replay of earlier answers to get one.
Models that read scripts natively (v3, v4) also need reading model N.

## Repo layout

```
crates/core      phone features, scoring, search — no I/O, builds for wasm32
crates/builder   native: Phoible + CMUdict + transliteration table + Unihan → one binary data file
crates/wasm      wasm-bindgen binding (the caller fetches the data file), test page, CLI parity check
crates/cli       native CLI for iterating
tools/           Python (uv, run via mise): ElevenLabs harness, ASR, evaluation
sample/          community sample messages (test set)
```

## Phases

### Phase 0 — test harness and reading-model experiment

Status: harness built (`tools/`). Usage:

```sh
mise run //tools:credits              # credit usage this period (free API call)
mise run //tools:models               # TTS models + cost multipliers (free API call)
mise run //tools:synth exp0           # dry run: count uncached clips, estimate cost
mise run //tools:synth exp0 --spend   # synthesize (refuses if over the experiment's max_credits)
mise run //tools:transcribe exp0      # local Whisper + wav2vec2 on the GPU, no credits
mise run //tools:report exp0          # → tools/results/exp0.md
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
`mise run //tools:export` then `mise run phono calibrate` compares, for each of
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
`//tools:export` still writes only exp1+exp2).

**v4: real spellings and script choice (2026-11-24; exp5 below).**

The metric that matters is **within-phrase ranking**: of two recorded answers
for the same phrase whose outcomes differ by ≥ 0.05, does the scorer prefer
the better one? `phono calibrate` now prints it. v3 got only 57 of 94 pairs
right (61%). The pooled r of 0.63 mostly reflected which phrases are hard.

What didn't help (`calibrate --dump` + fitting on exp1–exp3, testing on
exp4):
- Judging each word separately (mean, min, or sigmoid of per-word fidelity).
- Penalizing pseudo-words that run across word boundaries. Such clips do
  fare worse, but v3 already predicts them lower.

What did help: **spelling**. An English TTS says a word the way it says that
spelling.
- `ꍰ힘피시` ("chephimpisi") was heard as "Shefim PC";
  `ㅊ헻ㅋ힘PC` ("checkhimPC") was heard right.
- On its own, the candidate reading's spelling similarity to the target
  predicted the outcome better than phone fidelity (r 0.69 vs 0.64).

Changes:
- **Real words vs names** (`Engine::target_words`). A word counts as real if
  it is a whole-token dictionary word, a number or a symbol. Usernames and
  coinages (`xqc`, `pokimane`, `chungus`) are judged by sound only: no
  spelling bonus, no penalty.
- **Spelling term** (`words::spelling`, `Predict::spelling` = 0.4). Each real
  word is looked for in the reading's letters by best-substring edit
  distance. The result is blended in by the share of the phrase that is real
  words.

  | | v3 | v4 |
  |---|---|---|
  | pooled r vs truth (272 clips) | 0.631 | 0.707 |
  | pooled r vs wav2vec2 phones only | 0.519 | 0.538 |
  | within-phrase ranking | 62.8% | 68.1% |
  | exp4 Pearson / Spearman | 0.741 / 0.756 | 0.729 / 0.779 |

  The v3 column already includes v4's grouping of split names into one word
  (pooled r 0.629 → 0.631).
- **Spelled candidates** (`tile.rs`). Generation spelled phone by phone and
  never produced real spellings. `왜r 오바마` (fidelity 0.909) was not among
  11k generated candidates, so the search returned "erobamas" (0.718). v4
  also tiles the phrase's actual letters with chunks:
  - a space between words is optional;
  - up to 2 letter edits (drop a letter, or swap similar letters);
  - number tokens are matched by number chunks;
  - up to 400 tilings join the rerank.

  Results: "where obama" → `w헤레옵암a` ("whereobama"); "check him pc" →
  `ꍯck힘 ㍶`; usernames get spelled exactly when it's cheap
  ("sodapoppin" → `소닾옾핀`). Search time is still about 2–3 s.
- **Scripts off by default** (`Script::allowed_by_default`; `--all-scripts`
  turns them back on):
  - Ethiopic (mean outcome 0.59);
  - Canadian syllabics (0.69);
  - conjoining Hangul jamo (0.64);
  - Runic (silent on Flash v2.5 in exp0).
- **Reader:** apostrophes are kept for dictionary lookups (`you're`).
  `phono phones` lists each target word as `word`/`name`, which also fixes
  its display of split words.

By our own scorer (optimistic as always), v4 beats the community on 32 of 34
phrases (0.950 vs 0.812). 28 of 34 answers changed and now mostly spell the
real words: "iowasucks", "toecancer", "picklepounder", "nextplease",
"timeloop", "calypso". Doubtful ones: "joemama@aha", "ot are2s" (for
"what are thoooose").

**exp5 result (real TTS, 2026-11-24, ~152 credits; 665/10000 used after):
v4 beats the community for the first time, on both measures.**

| run | answers | phone fidelity | Whisper similarity | harness sim (per seed) | sim ≥ 0.8 |
|---|---|---|---|---|---|
| exp1 | community | 0.789 | 0.847 | 0.86 / 0.88 | 24 / 24 of 34 |
| exp4 | phono v3 | 0.783 | 0.736 | 0.78 / 0.79 | 17 / 18 |
| exp5 | **phono v4** | **0.830** | **0.872** | **0.89 / 0.90** | **28 / 30** |

- Phone fidelity (wav2vec2, independent of Whisper) rose too. So the gain
  is in the audio, not an artifact of a spelling-based metric.
- **Per phrase**, using the mean of both truths and a 0.05 margin:
  - vs the community: better on 10, worse on 3, about the same on 21;
  - vs v3: better on 20, worse on 2;
  - heard exactly on both seeds: 12 of 34.
- **Prediction on the unseen clips:** Pearson 0.794 / Spearman 0.765 (v3 on
  exp4: 0.745). Within-phrase ranking over all 157 pairs in exp1–exp5: 75.2%.
  v4 still over-predicts its own picks (0.950 predicted vs a truth of about
  0.85).
- **The 3 losses:**
  - `ot ㌃2s` → "Out R2s": a digit inside a word is read as a digit.
  - `墸` (CJK "Sho ") → "show, ma'am": CJK chunks with a capital and a
    trailing space still mislead.
  - "thisesepicswag" → "The Decepic swag": the word "is" was lost.

Clips: `data/generated/clips-exp5.tsv`, `clips-all5.tsv` (exp1–exp5).

Known gaps after exp5 (addressed in v5 below):
- **Digits inside a word** (`2s`) are read as digits, and so are uppercase
  CJK readings (`Sho `). Penalize or skip both.
- **Ties in reader segmentation.** The community's "varymoldifrog" splits
  as mold|ifrog, while the TTS said "moldy frog".
- **Uppercase runs are read as one word.** The TTS reads "PC" as letters.
- The phonetic generator still misses natural spellings. The tiler covers
  real words, not near-homophones.

**v5: reader fixes and a wider tiler (2026-11-24; exp6 below).**

Reader (`reading.rs`). Each rule comes from what the TTS did in exp1–exp5:
- **Capitals.** A run of two or more capitals inside a mixed-case word is
  its own word: `checkhimPC` → check · him · P C, heard that way in exp1.
  A single capital stays put: exp5 heard `tHe` as "the".
- **Numbers glued to letters.**
  - `2s` is a plural ("twos"); exp5 heard "t uː z" on both seeds.
  - A digit run followed directly by other letters (`3the`, `2eut`) gets a
    second reading in which it is dropped, weighted 50/50. It was dropped
    in 5 of the 10 such clips from exp2–exp5. The n-best expectation then
    prices the risk, so no new penalty term is needed.
- **Maximal onset.** Splitting a letter run between a consonant and a vowel
  costs as much as an extra word (`SPLIT_ONSET` = 1.0), because readers
  carry the consonant over to the vowel. Evidence:
  - "varymoldifrog" → "very moldy frog" (now vary|moldi|frog, was mold|ifrog);
  - "thisisepicswang" → "this is Sepik swang";
  - "biepbiepamtruck" → "beep beep'em truck".

  Any value from 0.3 to 1.0 helps offline. 1.0 also stops `gigamaxdong`
  from splitting as gig|amax|dong (AMAX is in CMUdict).
- **Curly apostrophes** count as apostrophes, so `you’re` is one word. Only
  `Engine::target` was affected: `phono score` and calibrate's truth. Search
  targets already normalized them. The v4 numbers below are recomputed with
  this fix so the comparison is fair.
- **`Sho `:** the reader already predicted what the TTS said ("show,
  ma'am"), and v4 scored that answer only 0.719. The real miss was
  generation, fixed next.

Tiler (`tile.rs`):
- A chunk that ends in a word boundary (CJK `嬶` "Kaka ", `墹` "Mama ")
  can end a word and supply the space. Before, CJK word chunks never
  appeared in spelled candidates.
- `x` spells `ks`/`cks`/`cs` as one edit (`chucks` → `ꍱ` chux), and the
  words "at", "and", "plus", "percent" may be spelled by their symbol.
- **Bug fix:** a punctuation token (`-` in "Joe mama - HA HA") was matched
  by every symbol chunk. That forced v4's `죄맘a@a하`, which exp5 heard as
  "Joe Mama at Aha".
- Tilings are enumerated into a pool 25× the budget. They are deduplicated
  by the letters they spell, then ranked by edits, script risk and
  codepoints. Depth-first order had filled the 400 slots with re-chunkings
  of whatever prefix came first: `푼Ճ…` (Armenian) crowded out v4's
  `푼ch간ym`.
- Chunk representatives prefer lowercase forms (Armenian `ճ` "ch" over `Ճ`
  "Ch").

Ranking and determinism:
- **Shorter answers win near-ties** (`SearchConfig::codepoint_cost` =
  0.005 per codepoint, used for ranking only, not part of `fidelity`).
  v4 chose `҈㌖㌐ꂶe동` over the community's 5-codepoint `҈㌖㌐ꂶ동` by about
  0.0005. Its filler `e` was heard as "gigamaxidong".
- **Search is deterministic.** Ties in G2P spelling costs, beam layers,
  goals and the rerank were broken in hash-map order. Three runs now give
  byte-identical output.

`phono score` now also prints `predict`, the value the search ranks by.

Offline (`phono calibrate`, exp1–exp5, 340 clips):

| | v4 | v5 |
|---|---|---|
| pooled r vs truth | 0.718 | 0.764 |
| pooled Spearman | 0.726 | 0.767 |
| within-phrase ranking (157 pairs) | 75.2% | 77.1% |
| same, 27 phrases that motivated no change (114 pairs) | 80.7% | 81.6% |
| pooled r on those 27 phrases | 0.726 | 0.735 |

Ablations (v5 minus one rule, pooled r / ranking): no onset 0.753 / 75.2%;
no glued-number drop 0.734 / 77.1%; no capitals split 0.756 / 77.1%. So the
onset rule carries the ranking gain, and the other two carry pooled r. All
three rules came from the clips they are tested on, so most of the gain is
in-sample. On the untouched phrases it is small but not negative.

By our own scorer, v5 beats the community on 33 of 34 phrases (0.960 vs
0.827). 16 of 34 answers changed from v4. Search takes about 2.4 s per
phrase.

| phrase | v4 (exp5) | v5 |
|---|---|---|
| kaka chucks dung at joe mama | `찿ꍱ둥@墸맘` | `嬶ꍱ둥@죄墹` (community-style) |
| Joe mama - HA HA | `죄맘a@a하` | `죄墹하 하` |
| Free the kwee | `3t何퀴` | `f레et何쿼` |
| very moldy frog | `ヹꏜ몰df록` | `ヹꏜ몰디f록` (community's spelling) |
| 100,000 Kilometer Giga max dong | `҈㌖㌐ꂶe동` | `҈㌖㌐ꂶ동` (= community) |
| What are thoooose | `ot ㌃2s` | `옽 ar숫e` |
| you’re not funny | `요레놑ꃚㅥy` (heard right) | `ꒆ俄놑ꃚㅥy` (a CJK `E ` gamble) |

**exp6 result (real TTS, 2026-11-24, 84 credits; 749/10000 used after):
v5 improves on v4 and leads the community further.** Same setup as exp1–exp5.
Only the 15 changed answers needed new clips; `҈㌖㌐ꂶ동` reused exp1's.

| run | answers | phone fidelity | Whisper similarity |
|---|---|---|---|
| exp1 | community | 0.791 | 0.847 |
| exp5 | phono v4 | 0.832 | 0.872 |
| exp6 | **phono v5** | **0.845** | **0.894** |

(Phone fidelity uses v5's truth, with `you’re` read as one word, so exp1
and exp5 differ from the earlier tables by ≤ 0.002.)

- **Per phrase** (mean of both truths, 0.05 margin):
  - vs v4: better on 4, worse on 0, about the same on 30;
  - vs the community: better on 11, worse on 2, about the same on 21;
  - heard exactly on both seeds: 14 of 34 (v4: 12).
- **Gains:**
  - "this is epic swag": `t힛잇엪잋솩` heard exactly, "This is Epic Swag",
    on both seeds. So the run-together string was fine; v4's `es` was the
    problem.
  - "kaka chucks dung at joe mama": `嬶ꍱ둥@죄墹` heard "Cock-a-chucks-dung
    at Joe Mama", like the community's message (0.58 → 0.80).
  - "Joe mama - HA HA": no more stray `@` ("Joe Mama, haha", 0.73 → 0.81).
  - "Free the kwee": "Free the cold" / "Frita Cueo" (0.38 → 0.62).
- **The CJK `E ` gamble failed.** `ꒆ俄놑ꃚㅥy` ("yurE notfunny") was heard
  "You're **E** not funny" on both seeds (0.92 → 0.87).
- **Still losing to the community:**
  - "very moldy frog": `ヹꏜ몰디f록` was heard "Very mold-a-frog" (0.89 vs
    0.96). The community's identical-sounding `ꃬꏜ몰ꄂf록` ("varymoldifrog")
    was heard "moldy".
  - "What are thoooose": "Adarsus" (0.39 vs 0.54).
- **Prediction on the unseen clips:** Pearson 0.798 / Spearman 0.736.
  Over all 408 clips (exp1–exp6): pooled r 0.772, within-phrase ranking
  80.0% of 200 pairs. Predicted 0.960 for its own picks vs about 0.87
  truth, so it still over-predicts its own picks.

Clips: `data/generated/clips-exp6.tsv`, `clips-all6.tsv` (exp1–exp6).

**v5.1: a trailing capital is a letter (after exp6, offline only).**
`split_caps` also splits off a single capital that ends a letter run after
two or more lowercase letters, and reads it as its letter name:
- `timE` → "Tim A" (exp2), `nexT` → "next to" (exp4), `yurE` → "you're E"
  (exp6): 6 of 6 clips;
- after one letter it stays in the word: `nO` → "no", `aM` → "um", 6 clips.

Offline effect on 408 clips: ranking unchanged (80.0%), pooled r 0.772 →
0.766. The three affected answers each had an audible extra letter, but
the scorer charges a full stressed vowel for a letter the TTS says
lightly, so they are now under-predicted. The rule matters for the
search: msg91 goes back to v4's `요레놑ꃚㅥy` (heard exactly in exp5).
msg171 now spells "u" with `筽` "O " read as the letter O (`xdx筽노퇴`,
0.830 vs 0.819), which is untested. Answers in
`data/generated/ours-v5b.json`.

Still open after exp6:
- **"very moldy frog".** Two spellings the reader treats the same were
  heard differently: "verymoldifrog" → "mold-a-frog", "varymoldifrog" →
  "moldy frog". Nothing here models that yet.
- **"what are thoooose"** is still weak: nothing spells /ð/ + long /uː/
  cheaply.
- **Capitalized CJK one-letter chunks** (`E `, `O `, `He `) are now read
  as letters at the end of a run. Inside a run (`tHe`), the only evidence
  is one exp5 clip.
- **Over-prediction of our own picks** (0.96 predicted vs about 0.87
  heard) persists. Fitting the scorer weights (item 3 below) on the 408
  clips is the next offline lever.
- The phonetic generator still misses natural spellings. The tiler covers
  real words, not near-homophones.

**v6: fitted scorer weights (after exp6, offline only).**
`mise run phono fit` tunes the scorer's hand-set weights on the 408 clips
of exp1–exp6. Truth is the same as `phono calibrate`'s and stays measured
with the hand-set weights (`Weights::HAND`). That keeps truth comparable
across versions.
- **Objective:** the decision the search makes. For each pair of recorded
  answers to the same phrase (156 distinct answers, 243 pairs ≥ 0.02 apart),
  a logistic loss on the predicted difference, weighted by the truth
  difference.
- **Regularization:** an L2 pull towards the hand-set values in log space.
  `del_primary` stays at 1.0 to anchor the scale.
- **Optimizer:** coordinate descent in log space, about 2 s per fit.
- **Validation:**
  - 5-fold cross-validation by phrase, repeated with 4 different splits, so
    held-out phrases are never seen in fitting;
  - fitting without the answers new in exp6, then testing on pairs that
    involve them.

Held-out within-phrase ranking (pairs ≥ 0.05 apart):

| fit | mean truth | Whisper-only truth | phones-only truth |
|---|---|---|---|
| hand-set (v1–v5) | 80.0% | 74.9% | 73.7% |
| all 15 parameters free | 84.6–86.4% | 79.5–80.2% | 76.1–77.4% |
| **substitution scales fixed (v6)** | **83.1–83.4%** | **78.8%** | – |

Whisper text similarity doesn't depend on any scorer weight, so the gain
against it isn't circular.

All three truths agree that losing a sound costs more than v1–v5 assumed:

| weight | hand | v6 |
|---|---|---|
| `del_unstressed` (full vowel) | 0.45 | 0.94 |
| `del_consonant` (singleton) | 0.70 | 1.23 |
| `del_onset` | 0.90 | 1.22 |
| `ins_unstressed` | 0.35 | 0.26 |
| `ins_consonant` | 0.60 | 0.75 |
| `Predict::spelling` | 0.40 | 0.52 |
| `Predict::risk_scale` | 1.0 | 1.3 |

The free fit also made vowel-for-vowel swaps much cheaper
(`vowel_sub_scale` 3.0 → 1.3, `sub_scale` 2.0 → 3.7). Against Whisper-only
truth that adds only about 1 point. The search has avoided vowel swaps since
v1, so the clips can't show whether it would exploit cheap ones. v6 keeps
both substitution scales at their hand-set values.

Offline, all 408 clips (in-sample): within-phrase ranking 80.0% → 86.0%,
pooled r 0.766 → 0.776. `phono calibrate --hand` reproduces the v5 numbers.
The predictions for our own picks did not come down (exp6 answers: 0.961
predicted vs about 0.87 heard). A ranking objective doesn't move the
offset, and the winner's curse remains.

**The search's answers barely change: 3 of 34 differ from v5.1.**
- `doug doug did that`: `도욱됙딛t핱` → `도욱듁딛t핱`.
- `xdx u no toe`: `xdx筽노퇴` → `ꀓꄄu놑회`.
- `What are thoooose`: `옽 ar숫e` → `옽㌃수읏`.

Each is a near-tie flip: for "xdx u no toe", the top 8 candidates fall within
0.016 of each other. The fitted weights mostly re-order answers the search
would not pick anyway. Answers are in `data/generated/ours-v6.json`; exp7
below tests them.

What the worst exp6 answers show instead is reading-model blind spots on
short pseudo-words:
- `쿼` "kweo" was heard "cold" / "Cueo";
- `퀴` "kwi" was heard "kawaii" (exp5), and `취e` "Kauai" (exp3), where
  our G2P says K W IY;
- `됙` "doeg" was heard "Dag" / "Deg".

There is also a generation gap: `f레et何퀴` (fidelity 0.974) is not among
17k generated candidates for "Free the kwee". But the clips above suggest
it would be misread as "kawaii".

**exp7 result (real TTS, 2026-11-24, 14 credits; 763/10000 used after):
level with exp6, as expected from near-tie flips.** Same setup as exp1–exp6.
Only 6 clips were new: msg91's answer reused exp5's clip.

| run | answers | phone fidelity | Whisper similarity | harness sim (per seed) |
|---|---|---|---|---|
| exp1 | community | 0.791 | 0.847 | 0.86 / 0.88 |
| exp6 | phono v5 | 0.845 | 0.894 | 0.90 / 0.91 |
| exp7 | phono v6 (+ v5.1 reader) | 0.844 | 0.900 | 0.91 / 0.92 |

| phrase | exp6 → exp7 | truth | heard in exp7 |
|---|---|---|---|
| doug doug did that | `도욱됙딛t핱` → `도욱듁딛t핱` | 0.80 → **0.89** | "Doug Doug did that." / "Doug Jug did that." |
| you're not funny (v5.1 rule) | `ꒆ俄놑ꃚㅥy` → `요레놑ꃚㅥy` | 0.87 → **0.92** | "You're not funny." (both seeds; exp5 clip) |
| xdx u no toe | `xdx 노퇴` → `ꀓꄄu놑회` | 0.76 → **0.70** | "XIX Sunoto." / "XDX Unotho." |
| What are thoooose | `옽 ar숫e` → `옽㌃수읏` | 0.39 → 0.40 | "Otaris Wuse." / "Otaresus." |

- **The flips came out as coin flips:** one better, one worse, one the same.
  The v5.1 trailing-capital rule paid off on msg91.
- **v6 ordered the new answers better:** fit without exp7, then tested on the
  11 new within-phrase pairs exp7 adds, v6 got 10 right and the hand-set
  weights 9. Refit on exp1–exp7 (476 clips), every weight stays within about
  10% of v6's, and held-out ranking is 84.2–84.6% vs 81.0% hand-set. So
  v6's weights stay.
- **Readings:**
  - `ꀓꄄ` "exdiex" was heard as the letters "XDX" (and "XIX").
  - `놑회` "nothoe" came out with the /θ/ the reader predicted ("Unotho").
  - `듁` "dyug" was heard "Doug" once and "Jug" once. The reader says
    D AY1 AH1 G, but the TTS made /dj/ into /dʒ/. This adds to item 5's
    short pseudo-word cases.
  - "thoooose" is still unsolved.

**v6.1: the spelling term matches each word once, in order.** All 24 pairs
the scorer still mis-orders were listed. "doug doug did that" was
mis-ordered 4 times, because `words::spelling` looked for each real word
anywhere in the letters. So "doug doug" got full credit from the single
"doug" in "dougdoegdidthat" (heard "DougDag did that" in exp5–exp7).
Real words are now matched in order and without overlap by one DP. The
letter-weighted mean is unchanged for phrases without repeated or
reordered words.
- Effect: ranking 182 → 183 of 211 pairs, pooled r 0.781 → 0.782.
- One search answer changes: "doug doug did that" → `도욱도욱딛핱`
  ("dougdougdidhat"; predicted 0.943, untested). It spells both "doug"s
  exactly but reads "that" as "hat".

Tried and rejected: accepting dictionary homophones as spellings ("dug" for
"doug", since exp4's `둑둑 딛t핱`, "dugdug didthat", was heard exactly and is
still the best answer for that phrase). Over exp1–exp7 it fixed 4 pairs and
broke 7:
- "nite", "yu" and the letter "r" (for "are") inside run-together strings
  got full spelling credit but weren't heard as the word;
- `둑둑딛ᒢ@` got full credit while failing for other reasons (UCAS, `@`).

Clips: `data/generated/clips-exp7.tsv`, `clips-all7.tsv` (exp1–exp7);
answers `ours-v6b.json` (v6.1).

**exp8 result (real TTS, 2026-11-24, about 6 credits): v6.1's one changed
answer is worse.** Same setup as exp1–exp7; only msg14's 2 clips were new.
`도욱도욱딛핱` ("dougdougdidhat") was heard "Dog dog die that." and "Doug
Doug did hat.":

| answer | run | truth | predicted (v6.1) | heard |
|---|---|---|---|---|
| `둑둑 딛t핱` | exp4 | **0.93** | 0.928 | "Doug Doug did that." ×2 |
| `도욱듁딛t핱` | exp7 | 0.89 | 0.937 | "Doug Doug did that." / "Doug Jug did that." |
| `도욱됙딛t핱` | exp5, exp6 | 0.80 | 0.935 | "DougDag did that!" / "Doug Deg died that." |
| `도욱도욱딛핱` | exp8 | 0.78 | **0.943** | "Dog dog die that." / "Doug Doug did hat." |

The scorer still ranks it first of the 7 answers, for two reasons the
order fix doesn't touch:
- **A lost word onset is cheap.** "didhat" drops the /ð/ of "that". The
  scorer charges one onset deletion (about 9% of the phrase), but the
  listener hears a different word. v4's per-word aggregates didn't help
  offline. This case is evidence for retrying a "word lost or replaced"
  penalty.
- **An exact spelling inside a run is not a guarantee.** "dougdoug" was
  heard "Dog dog" on one seed, while exp4's "dugdug" was heard "Doug Doug".

Over exp1–exp8 (544 clips, 215 pairs) the ordered and unordered spelling
matches now tie at 185 pairs. The unordered one picked the better answer
here only because it credited `도욱듁`'s second "doug" with the first. That
would also let an answer drop a repeated word for free ("beep am truck"),
so v6.1 stays.

Clips: `data/generated/clips-exp8.tsv`, `clips-all8.tsv` (exp1–exp8).

**v6.2: model profiles (refactor, no change in answers).** The user wants to
support other ElevenLabs models later, so everything calibrated on Flash
v2.5 moved out of the code paths into a `ModelProfile`:
- `Script::risk`/`usable`/`allowed_by_default` and the script preference
  order became the profile's script table;
- the reader's `SPLIT_*` and `GLUED_NUMBER_DROP` constants became
  `ReaderParams`;
- `Weights::FIT` and `Predict::default()` became the profile's `weights` and
  `predict`.

The CLI takes `--model` (default `eleven_flash_v2_5`). `phono calibrate`
and `phono fit` now read the clips file's model column and use only that
model's clips; before, they ignored it. All 544 clips are Flash v2.5.

Checked against v6.1's binary: `phono eval` (with its export), `calibrate`
(fitted and `--hand`), `fit`, `--all-scripts compress`, and `read`/`score`
on mixed scripts give byte-identical output.

Not moved, because only Flash v2.5 has been measured: the capital-letter
rules, plurals and number grouping in `reading.rs`.

Per the ElevenLabs docs (reviewed 2026-11-24), Flash v2.5 doesn't normalize
text by default. The user believes their TTS setup sends
`apply_text_normalization: "auto"` and no `language_code`, which matches the
harness.

**exp9 result (real TTS, 2026-11-24, 100 credits; 869/10000 used after):
no new lever. The reader was already right about camelCase and most
symbols, and Flash v2.5 doesn't normalize text by default.** This was item
6's probe: 19 cases × 2 seeds on Flash v2.5, same voice. `tools/experiments/exp9.toml` has the
cases and the reader's predictions; the report is `tools/results/exp9.md`.
The harness can now send `apply_text_normalization` per case (`normalization
= "on"`). The cache key leaves it out when unset, so cached clips keep their
keys.

| question | heard (seed 1 / seed 2) | conclusion |
|---|---|---|
| camelCase as a free word boundary | `ambigchungus` "I'm Big Chungus" ×2; `amBigChungus` "Ambig Chungus" / "I'm Big Chungus"; `빞빞엠T뤀` "Bipipem Truck" / "Bit by Pemtruck" vs lowercase "Bibi Pemtrak" / "Bip by Pemtruck" | No boundary: phones are unchanged. The reader's rule (an interior capital stays in the word) holds. exp1's "Ombigchungus" for `암빅ꍲn궀` has the same phones as "I'm Big Chungus": Whisper split identical audio differently. |
| interior capitals in a pseudo-word | `doeg` /aɡɑː/, /dəka/; `doEg` /duːik/, /duːiŋ/; `dOeg` /doːɡ/, /lɑu/ | Isolated 4-letter pseudo-words are too unstable to measure. `doEg` gave /duːi…/ on both seeds, so a capital vowel may be read as its letter name (a capital consonant isn't: `tHe`, exp5). The search never emits capitals, so this only matters when scoring other people's texts. |
| `kwi` (Latin) | "quay" /xwei/, "Quays" /kwai/ | Not /kwiː/ (G2P: K W IY0). A final `i` in a one-syllable pseudo-word reads /aɪ/-like, as in "hi" and "pi". Same direction as `퀴` → "kawaii" (item 5). |
| `you&me` | "You and me" ×2 | `&` → "and" confirmed. |
| `#blessed`, `a=b`, `and/or` | "Blessed" ×2; "A." / "A, B."; "And or" ×2 | `#`, `=` and `/` are silent, as the reader assumes. |
| `$5` | "Fire me" / "Five dollars" | `$` before a digit may be read "dollars" (1 of 2). The reader drops it. |
| `⅔ done`, `½ off` | "2-3 done" ×2 (/tuː θɹiː/); "Half off" / "One how off" | `⅔` → "two three" (the reader agrees). `½` → "(one) half", not unidecode's "1/2" (the reader says "one two"). |
| `3×3`, `90°` | "Three of three" / "3-3"; "90D" ×2 | `×` isn't "ex" (the reader says "three ex three"). `°` isn't "degrees". |
| `gr8 m8`, default vs normalization "on" | default /dʒiː ɑːɹ eɪt ɛm eɪt/ ("GR8M8"), /ɹeɪt ɛm eɪt/; "on": "Great Mate" ×2 | Default = no normalization, as the models page says. The reader's prediction (G R eight M eight) matches seed 1 exactly. With "on", glued digits become rebus sounds; that would need a different reader. |

- `½ × ° ⅔` are outside the searchable scripts (`Script::Other`), so their
  readings only affect scoring of texts that contain them. No new symbol
  words to add.
- Calibration: `clips-all9.tsv` (580 clips; the 2 normalization-"on" clips
  are left out). v6 weights rank 190/227 within-phrase pairs (83.7%).
  - A refit with substitution scales held gets 188/227 in-sample and
    80–81% held out.
  - A free refit again makes vowel swaps cheaper (3.0 → 2.0), which v6
    rejected.
  - The new exp9 pairs are mostly capital variants the reader reads
    identically, so no weights can order them.
  - So v6's weights stay.

**exp11 result (real TTS, 2026-11-25, 249 credits; 1218/10000 used after):
Multilingual v2 reads most scripts by transliteration, like Flash v2.5, but
reads Cyrillic, Devanagari and kana natively, and doesn't drop glued digits.**
This was the probe for item 7 (issue #6, track `multilingual-v2_20261125`):
exp0's 20 cases plus 27 new ones × 2 seeds on `eleven_multilingual_v2`, same
voice. exp0's seed-1 clips were cached, so 74 clips were paid for. The
dry-run estimate (1 credit per character at cost ×1.0) was exact. Cases and
predictions: `tools/experiments/exp11.toml`; report: `tools/results/exp11.md`.
Seeds agree less than on Flash: 19/47 identical transcripts.

| script | heard (seed 1 / seed 2) | draft reading |
|---|---|---|
| Latin | `checkhimPC` "Check Him PC" ×2 | T |
| Hangul | `딧잇` "This is" ×2; `솪` "Swag" ×2; `빞빞` "Bip bip" ×2; but `거` /ke/ ×2, `벗` /pat/, /pot/ | T, noisy on lone syllables (`벗` is the Korean reading) |
| Hangul compat jamo | `ㅊ헻ㅋ힘` "Check him" ×2 | T |
| Hangul jamo (conjoining) | `가` (U+1100 U+1161) "God" / "Yeah", /ɡaː/ ×2 | T, unstable; off by default, as on Flash |
| Yi | `ꌟ` "Suck" ×2; `ꀠddꏒꀊm` "Bad dream" ×2 | T |
| CJK compat | `㌀` "Apartment" ×2; `㎞` "KM" / "Game is" | T |
| Cyrillic number signs | `҈` "Hander a thousand" / "100,000"; `҈㌀s` "100,000 apartments" ×2 | T |
| CJK unified | `嬶` "Caca"/"Kaka"; `萙` "Kutabireru" ×2; `嬶萙` "Kaka Kutabireru" ×2 | T (Whisper's language ID is often not English, but the phones are unidecode's) |
| Katakana | `フリ` "Free" /fuːɾiː/ / "Hootie" /hri/; `アパート`, `キロ` /apato/, /kiro/ ×2 | N on seed 1 of `フリ`; the others fit both readings. Native, as exp0 said |
| Hiragana | `ふり`, `あぱーと`: same phones as their katakana twins | Native, like katakana |
| Cherokee | `ᏃᎧ` /noka/ ×2; `ᎠᎢ` "I" / "Bye" | T |
| Canadian syllabics | `ᐊᐃ` "I" / "Bye" | T |
| Ethiopic | `ሀለ` "Hala" ×2 | T |
| Armenian | `ԱԲ` "A. B." ×2 | T (unidecode gives capitals, read as letters) |
| Runic | `ᚠ` "F" ×2; `ᚠᚱ` "Far" / "Ha" | T, unstable |
| Devanagari | `क` /ka/, /k/; `कमल` "Kamal" ×2 (unidecode: `kml`) | N |
| Cyrillic | `Привет` "Привет" (Russian) ×2 | N |
| Other | `™` "Me" / "May" (unidecode `(tm)`) | unmeasured, as on Flash |

Draft preference order: Flash's, unchanged. The order only breaks ties
between scripts that transliterate to the same chunk, and exp11 found no
transliterated script that reads worse than its neighbours in Flash's order
except Runic (already near the end). Draft defaults: on for every T script
except Runic, Hangul jamo, Canadian syllabics and Ethiopic (as on Flash);
risk values wait for exp12–exp13, which give per-script outcomes over many
more clips.

Reader rules (Flash's constants, which the reader uses today):

| rule | heard (seed 1 / seed 2) | agrees? |
|---|---|---|
| glued number dropped (`glued_number_drop` 1.0) | `3the` "Three the" ×2; `2eut` "to a" / "To it" | No: the digit was read in 4 of 4 clips (Flash: 5 of 10) |
| plural `2s` → "twos" | /tuːs/, /tuːz/ | Yes |
| a run of capitals is its own word | `checkhimPC` "Check Him PC" ×2 | Yes |
| interior capital consonant stays in the word | `tHe` "The" / "Ta-" (clipped) | Yes, weakly |
| trailing capital is its own letter (v5.1) | `nexT` "Next" ×2; `timE` "Tammy" /taɪmiː/, "Timmy" /tɪmiː/ | Mixed: `nexT` drops the letter, `timE` reads E as /iː/ |
| `100.000` grouped as "one hundred thousand" | "Thank you" / "100 Julian" (German-like) | No, but the search only reaches it through `҈`, which is read right |
| `&`, `%` read as words | "You and me" ×2; "50%" /fɪfti pɚsɛnt/ ×2 | Yes |
| `+`, `@` read as words | `you+me` "Oops, me" / "You, me"; `joemama@aha` "Jai Mama Nawa" / "Gemma Mazawa" | No: neither was read "plus" or "at" |

So task 3.2 has candidates: `glued_number_drop` (probably near 0 for this
model), the trailing-capital rule, and the `+`/`@` symbol words. Each needs
the exp12–exp13 clips before it moves into the profile.

**exp12 result (real TTS, 2026-11-25, 334 credits; 1552/10000 used after):
the community messages on Multilingual v2 are read by transliteration, but
less well and less steadily than on Flash v2.5.** exp1's 34 messages × 2
seeds on `eleven_multilingual_v2`, same voice (`tools/experiments/exp12.toml`;
`tools/results/exp12.md`, gitignored like exp1's). Measured with
`phono calibrate`'s truth, so the numbers sit next to exp1's:

| run | model | phone fidelity | Whisper similarity | Whisper says English | silent | identical across seeds |
|---|---|---|---|---|---|---|
| exp1 | Flash v2.5 | 0.791 | 0.847 | 67/68 | 0 | 16/34 |
| exp12 | Multilingual v2 | 0.739 | 0.734 | 41/68 | 0 | 14/34 |

(exp1's phone fidelity is 0.791 here and 0.783 in the exp3 table; `calibrate`
has changed since. Both rows above come from the current build.) The harness
report's difflib similarity is 0.77 / 0.78 per seed, against exp1's 0.86 /
0.88, with 17 and 19 of 34 messages at 0.8 or more (exp1: 24 and 24).

- **No silent clips**, and the phones are unidecode's chunks read as English
  (`ꀂꀑꊂꌟ` "Iowa sucks" /aː j ɔ w a s a k s/, `ꀠddꏒꀊm` "Bad dream" in exp11).
  So the model still reads these texts by transliteration.
- **Language ID is not English on 27 of 68 clips** (Japanese 6, Swedish 4,
  then Polish, Welsh and others, one each or two), against 1 of 68 on Flash.
  These are mostly pseudo-word messages and the audio is noisier (accents
  drift between words). The Whisper English transcript still matches the
  phones, so this doesn't mean the model reads the text as another language.
- **18 messages are 0.05 or more worse than on Flash, 4 better** (mean of
  phone and Whisper truth per message). The biggest drops are `҈ꃑ쿰이s`
  ("100,000 fat kumis" → "What I would have thought, Miss?" / "100% of the fat
  comes"), `ꄫꀔ` ("toe" → "two"), `ꀂꀑꊂꌟ` (seed 1 "ayahuasks") and `҈㌖㌐ꂶ동`.
  Messages with `҈` are the worst: its reading varies a lot here (exp11:
  "Hander a thousand" / "100,000").

Decision: the track's stop point (non-English language ID) was met on its
face, so exp13 waited for the user's go-ahead, and got it.

**exp13 result (real TTS, 2026-11-25, 1244 credits; 2796/10000 used after):
the fit data for Multilingual v2.** Every distinct answer phono v1–v6 gave
for the community phrases (126 texts, 628 characters; the community's own
texts left out, since exp12 has them) × 2 seeds on `eleven_multilingual_v2`:
252 clips, none silent, 0 not cached afterwards (`tools/experiments/exp13.toml`,
`tools/results/exp13.md`; the harness's `cases_from` now takes a list of files
and `cases_exclude`). Mean similarity per seed (harness difflib measure)
0.73 / 0.73, with 60/126 and 62/126 at 0.8 or more; transcripts identical
across seeds on 38/127. Whisper says English on 157/256 (61%; exp12:
60%, exp1: 99%).

`mise run //tools:export`-style clips file for the model, from exp11–exp13
(`data/generated/clips-mv2.tsv`, gitignored, default-setting clips only):
338 clips, all `eleven_multilingual_v2` (exp11 18, exp12 68, exp13 252). With
Flash's reader constants and weights (no profile yet; the file's model column
was relabelled for this check), `phono calibrate` ranks **161 of 210
within-phrase pairs (76.7%)** like truth; Flash's own clips give 83.7%
(exp9). Prediction correlates 0.50 with truth (mean of phone and Whisper
measures). Per source, predicted / actual phones / Whisper:

| exp | clips | predicted | phones | Whisper |
|---|---|---|---|---|
| exp11 | 18 | 0.900 | 0.644 | 0.537 |
| exp12 (community) | 68 | 0.843 | 0.739 | 0.734 |
| exp13 (engine answers) | 252 | 0.854 | 0.705 | 0.676 |

Flash's weights over-predict on this model by about 0.15 to 0.2. That is the
gap the profile's fit and reader checks (tasks 3.1–3.3) work on.

**Multilingual v2 vs Flash v2.5 on the same texts (2026-11-25, no credits).**
The 160 texts both models have clips for (34 community messages and the 126
replayed answers; Flash's from `clips-all9.tsv`), each scored with
`calibrate`'s truth (mean of phone and Whisper measures, over 2 seeds):

| | Flash v2.5 | Multilingual v2 |
|---|---|---|
| mean truth, all 160 texts | 0.782 | 0.702 |
| community messages (34) | 0.819 | 0.736 |
| engine answers (126) | 0.772 | 0.693 |
| seed 1 vs seed 2 truth (Pearson) | 0.82 | 0.79 |
| phrases where some engine answer ≥ the community's | 25/34 | 28/34 |

- **Same behaviour, lower level.** Both kinds of text lose about 0.08, so the
  model is uniformly harder to hear, not worse on one kind of text.
- **The models agree on what works.** Per-text truth correlates 0.73 between
  them (Spearman 0.73), close to each model's own seed-to-seed ceiling.
  Within a phrase, of the 164 answer pairs both models separate by 0.05 or
  more, 134 (82%) are in the same order.
- **What differs is specific:** kana read natively (the one script change the
  search notices; Cyrillic and Devanagari were already native on Flash),
  glued digits read rather than dropped, `+` and `@` not read as words, `҈`
  unstable, and more noise overall (Whisper hears English on about 60% of
  clips vs 99%).
- **Prediction:** Flash's profile ranks 76.7% of Multilingual v2's
  within-phrase pairs right vs 83.7% of Flash's own, and correlates 0.50 vs
  0.77 with truth. Flash's figures are in-sample (its weights were fitted on
  those clips), so the true gap is smaller.

Expectation for the profile: the script table, the reader rules above and a
recalibrated level can fix specific losses and stop the CLI overstating its
confidence. They can't fix the model's noise floor. The v6 fit gained about
3 points of held-out ranking on Flash's 227 pairs; with 210 pairs here,
expect about as much. The profile should beat reusing Flash's and the
community's messages on this model, but not reach Flash's level. If the fit
barely beats Flash's weights, ship Flash's weights with the script and rule
changes, and treat exp14 as a sanity check.

**Multilingual v2 profile, first cut (2026-11-25).** `MULTILINGUAL_V2` in
`model.rs`, in `PROFILES`, so `--model eleven_multilingual_v2` works; Flash
stays the default. Its script table is Flash's except:
- kana is `Native` (exp11), so the search no longer uses it;
- `҈` risk 0.0 → 0.18. With Flash's profile, a least-squares fit of the
  over-prediction (predicted − truth) on per-script codepoint counts gives
  +0.29 per `҈` on Multilingual v2's clips and +0.06 on Flash's own. Every
  other script's excess was within the ±0.03 that Flash's in-sample check
  shows, so the rest keep Flash's risks.

The preference order, reader constants, weights and prediction settings are
Flash's (now shared constants in `model.rs`). Flash's `phono eval` is
byte-identical. Baseline `phono --model eleven_multilingual_v2 calibrate
data/generated/clips-mv2.tsv`: 161/210 within-phrase pairs (76.7%, as with
Flash's profile), Pearson 0.539 / Spearman 0.598 with truth (Flash's profile:
0.500 / 0.532). `eval` under the new profile changes one answer (`very moldy
frog` drops a kana) and lowers the predicted score of the `҈` answers
(e.g. 0.998 → 0.873), which still win: nothing else says "100,000" in 6
codepoints.

**Reader rules on Multilingual v2 (2026-11-25, no credits).** Each of
`reading.rs`'s Flash-derived rules, checked against every clip whose Latin
(`phono read`) exercises it, on both models' clips (`clips-mv2.tsv`,
`clips-all9.tsv`):

| rule | Multilingual v2 heard | verdict |
|---|---|---|
| a digit glued to letters is dropped about half the time (`glued_number_drop` 1.0) | read in 11 of 12 clips, the 12th unclear: `3the` "Three the" ×2, `3ᕞ ጔ` "Three the gwee", `3咜취e` /s ɹ iː …/ ×2. Flash dropped it in 4 of the 8 exp13-text clips | **disagrees**: 0.1 for this model |
| `@` → "at" | 0 of 14 clips over 7 texts (`chuxdung@joMama` "Dunga Joe Mama", `joemama@aha` "Jai Mama Nawa"). Flash: 11 of 14 between words | **disagrees**: `@` is silent on this model |
| `&` → "and", `%` → "percent" | 5 of 6; 2 of 2 | agrees |
| `+` → "plus" | `you+me` /j p s m e/, "You, me": 1 of 2 at most | too few clips; stays |
| a trailing capital after two or more letters is its own letter | 10 of 12: `timE lupe` /t i m i/ ×2, `yurE notfunny` "Yuri" ×2, `nexT pleas` "Nexty, please" ×2; only bare `nexT` ("Next" ×2) disagrees | agrees |
| a run of capitals in a word is its own word | `checkhimPC` "Check Him PC" ×4 | agrees |
| a single interior capital stays in the word | `joeMama` "Joe Mama", `tHe` /t/ (clipped); the Han-derived cases (`oHao`, `veRi`) are too noisy to read either way | agrees, weakly |
| plural `2s` | `2s` /tuːs/, /tuːz/; `ot are2s` /t uː z/ (1 of 2) | agrees |
| `100.000` grouped | only reachable through `҈`, whose instability is in its risk (0.18) | stays |

So two rules move into profile data:
- `glued_number_drop` was already a profile constant; Multilingual v2's is
  0.1 (about 1 drop in 12).
- The symbol table moved from `chunks.rs` into `ReaderParams::symbol_words`.
  `chunks::SYMBOL_WORDS` remains the list of symbols any model voices, so
  chunks containing one are still read through the reader. The tiler's
  symbol words follow the profile too.

The capital and plural rules stay in `reading.rs` for both models.

Split costs (`split_onset`, `split_short_word`, `split_lone_letter`,
`split_g2p_letter`) were checked by moving each one down and up (e.g. onset
0.5 and 1.5) under Multilingual v2's profile. `calibrate` stayed within 158–162
of 210 pairs and Pearson 0.524–0.552 (base 161, 0.542), so they keep Flash's
values. `glued_number_drop` 0.0–0.3 moves Pearson by 0.01 (few clips have a
glued digit), so the counts above set it.

After the change: Flash's `phono eval` byte-identical, Flash `calibrate`
unchanged (190/227, 0.773). Multilingual v2 `calibrate`: 161/210, Pearson
0.542. Its `eval` changes two answers: `kaka chucks dung at joe mama` swaps
`@` for `앝` ("at"), and `what are thoooose` now uses a glued `2` that this
model reads.

**Fitting Multilingual v2's weights (2026-11-25, no credits): Flash's
weights stay, once four script risks are set for this model.** `phono fit
--model eleven_multilingual_v2` on `clips-mv2.tsv`: 169 distinct answers to
43 phrases, 210 within-phrase pairs at |Δ| ≥ 0.05. Substitution scales held,
as in v6; 5 folds by phrase × 4 repeats. `fit` gained `--prior profile`,
which starts from and regularizes towards the profile's weights (here
Flash's) instead of v1–v5's hand-set ones. The default is unchanged, and
Flash's `fit` output is identical.

| weights | script risks | in-sample | held-out (λ 0–0.03) | Pearson over answers |
|---|---|---|---|---|
| hand-set (v1–v5) | Flash's | 73.8% | – | 0.564 |
| Flash's (v6) | Flash's | 76.7% | – (never fitted on these clips) | 0.572 |
| fitted, prior hand | Flash's | 81.4% | 76.7–79.3% | 0.533 |
| fitted, prior Flash | Flash's | 80.0% | 77.3–79.2% | 0.530 |
| fitted, prior Flash, `risk_scale` held | Flash's | 76.7% | 76.2–76.5% | 0.517 |
| **Flash's (v6)** | **Multilingual v2's** | **79.5%** | – | **0.574** |
| fitted, prior Flash, `risk_scale` held | Multilingual v2's | 79.0% | 78.8–79.9% | 0.536 |
| fitted, prior Flash | Multilingual v2's | 79.0% | 78.3–80.8% | 0.539 |

For comparison, Flash's clips: v6 weights 83.7% in-sample, a fresh fit
80.1–81.4% held out.

- **The first fits' gain was all script risk.** Fitting with Flash's script
  table cut `risk_scale` 1.3 → 0.14. With `risk_scale` held, the held-out
  gain vanished (76.5%). The pairs that flipped: answers using Canadian
  syllabics, Ethiopic or conjoining jamo beat their siblings more often than
  Flash's risks allow (8 pairs fixed, 1 broken). The per-script residuals
  from the first profile say the same (−0.04, −0.08, −0.09 per codepoint,
  against +0.03, +0.03, −0.01 on Flash). One global scale can't lower those
  three and keep `҈`'s penalty, so the fit lowered it for every script.
- **So the profile sets those three risks** (syllabics 0.05 → 0.02,
  Ethiopic 0.05 → 0, conjoining jamo 0.03 → 0) and keeps `risk_scale`. All
  three scripts are off by default on both models, so the default search's
  answers are unchanged. Only the predicted scores of the community messages
  that use them move (e.g. `푽ᆾꇤꆌm` 0.523 → 0.552). These risks were chosen
  from the same clips they are scored on, so 79.5% is in-sample. exp14 is
  the out-of-sample check.
- **Then no fit beats Flash's weights.** Held-out ranking stays between 78.3%
  and 80.8% (spread across repeats about ±1 point, i.e. 2 pairs), pooled
  correlation drops from 0.574 to about 0.54, and the fits push `del_onset`
  to 2.9 (Flash: 1.22) and `del_unstressed` to 1.1–1.7. That's more change
  than the data supports, so the profile keeps Flash's weights and
  prediction settings.
- `fit`'s "new in the last experiment" check is empty here: every
  non-community answer first appears in exp13, so there is nothing to train
  on. Cross-validation is the only held-out measure.

Flash's `phono eval` is byte-identical. Multilingual v2 `calibrate`: 167/210
(79.5%), Pearson 0.544 / Spearman 0.600.

**exp14 result (real TTS, 2026-11-25, 46 credits; 2842/10000 used after):
the Multilingual v2 profile's answers beat the community's on that model,
by about as much as Flash's answers do, and no more.** The profile's top
answer for each of the 34 phrases × 2 seeds on `eleven_multilingual_v2`
(`phono --model eleven_multilingual_v2 eval --export
data/generated/ours-mv2.json`; `tools/experiments/exp14.toml`,
`tools/results/exp14.md`). 60 of the 68 clips were cached from exp13, since
only 4 answers were new. Truth as in `calibrate`, all on Multilingual v2:

| answers | phone fidelity | Whisper similarity | ≥ community (of 34) | harness sim (per seed) |
|---|---|---|---|---|
| community (exp12) | 0.739 | 0.734 | – | 0.77 / 0.78 |
| Flash profile, v6 answers (exp13) | 0.779 | 0.803 | 28 | – |
| Flash profile, v6.1 answers (exp13/exp14) | 0.775 | 0.798 | 28 | – |
| **Multilingual v2 profile (exp14)** | **0.774** | **0.803** | **27** | **0.84 / 0.84** |

On Flash, for scale: community 0.791 / 0.847, v6 answers 0.844 / 0.900
(exp7).

- **Against the community on this model:** +0.035 phone fidelity and +0.07
  Whisper similarity, the same margins as Flash's answers have here. No
  silent clips; 17/34 transcripts identical across seeds (community: 14/34).
- **Against Flash's answers on this model: level.** Three answers differ
  from v6.1's:
  - `very moldy frog` drops its kana: truth 0.90 either way, but Whisper now
    hears "Very moldy frog." on both seeds ("Very Moldafrog." once before).
  - `what are thoooose` uses a glued `2` that this model reads: 0.34 → 0.42
    ("Otare2 OS").
  - `kaka chucks dung at joe mama` swaps `@` for `앝`: 0.72 → 0.70 ("Kakachuk
    Dungajui Mama"); the "at" is still lost.
- **Below the community on 7 phrases.** The biggest gaps are `what are
  thoooose` (0.62 vs 0.42, no /ð/ chunk, as on Flash), `no rizz` (`노 리z`
  "Norris", 0.75 vs 0.63) and `check him pc` (`c헻k힘 ㍶` "Check in PC" on one
  seed). The rest are within 0.03 (`toe`: `퇴` → "two").
- `calibrate` on exp11–exp14 (`clips-mv2b.tsv`, 406 clips; exp14 repeats 60
  exp13 clips): 180/227 within-phrase pairs (79.3%), Pearson 0.561. The
  profile still over-predicts its own answers (0.952 predicted, 0.774 / 0.803
  heard): this model's noise floor isn't in the scorer.

So the profile ships. It is a correct description of the model (kana not
searched, `@` silent, glued digits read, `҈` risky) and doesn't cost any
quality against Flash's answers, but on these 34 phrases it doesn't gain
any either. The model reads everything about 0.07 worse than Flash, whoever
wrote the text.

**Offline baseline on the full community library (2026-11-26, no credits;
`community-library_20261126`, #42).** `mise run //tools:fetch-library` puts
the whole library (298 entries) in gitignored `sample/library.json`; the 34
hand-picked messages keep their keys, the rest are keyed `u` + 8 hex digits
of their UUID. Phrases stay out of this file, so cases are cited by key.
Each entry is scored against its cleaned `target` (6 of 298 differ from the
label: 3 by rule, 3 by hand override). Commands:
`phono [--model …] eval sample/library.json --by-script`, plus
`--subset tools/library-keys.tsv` for the 34. Predicted fidelity, top-1:

| Set | n | Flash: community | Flash: ours | ours ≥ | Multilingual v2: community | ours | ours ≥ |
|---|---|---|---|---|---|---|---|
| All | 298 | 0.858 | 0.954 | 284 | 0.849 | 0.944 | 282 |
| The 34 | 34 | 0.843 | 0.961 | 33 | 0.829 | 0.952 | 33 |
| The rest | 264 | 0.859 | 0.954 | 251 | 0.852 | 0.943 | 249 |

The earlier check (0.855 / 0.952) scored the raw `spoken` labels. Cleaned
targets account for the +0.003 / +0.002. The 34 score slightly below the
rest for the community and slightly above for us, so the full set doesn't
make the job harder on average. What it adds is breadth.

By script, where each entry counts once under every script in its community
text (spaces and ASCII punctuation don't count). "Ours" is our answer for the
same phrases, written in whatever scripts the search picked:

| Script | n | Flash: community | ours | MLv2: community | ours |
|---|---|---|---|---|---|
| yi | 220 | 0.879 | 0.953 | 0.869 | 0.943 |
| hangul | 186 | 0.852 | 0.947 | 0.840 | 0.932 |
| latin | 169 | 0.871 | 0.962 | 0.870 | 0.956 |
| cyrillic-number | 25 | 0.951 | 0.982 | 0.834 | 0.865 |
| ucas | 22 | 0.692 | 0.920 | 0.685 | 0.921 |
| cjk-unified | 18 | 0.757 | 0.948 | 0.738 | 0.941 |
| hangul-compat-jamo | 17 | 0.880 | 0.938 | 0.876 | 0.934 |
| cjk-compat | 16 | 0.930 | 0.963 | 0.889 | 0.913 |
| ethiopic | 10 | 0.703 | 0.925 | 0.786 | 0.919 |
| hangul-jamo | 10 | 0.723 | 0.906 | 0.749 | 0.906 |
| hiragana | 7 | 0.753 | 0.907 | 0.647 | 0.907 |
| katakana | 7 | 0.599 | 0.920 | 0.531 | 0.920 |
| cherokee | 6 | 0.746 | 0.954 | 0.746 | 0.935 |
| armenian | 2 | 0.753 | 0.964 | 0.697 | 0.908 |
| other | 2 | 0.839 | 0.935 | 0.782 | 0.879 |
| runic | 1 | 0.936 | 0.977 | 0.936 | 0.977 |
| devanagari | 1 | 0.388 | 0.712 | 0.328 | 0.712 |
| cyrillic | 1 | 0.853 | 0.672 | 0.853 | 0.695 |

- **Lowest community entries.** On Flash these are `ua626e6e4` (0.13,
  Ethiopic), `247` (0.26, Katakana), `81` (0.35, Yi + syllabics),
  `u0e0888f4` (0.39, Hangul + Katakana + Devanagari), `u6f19f605` (0.43),
  `ueb344e87`, `u9f99a25a`, `u091361af`, `10` and `u14b6de9d` (0.51–0.53).
  On Multilingual v2 `247` drops to 0.05 and `ua626e6e4` rises to 0.52,
  because that profile scores Katakana as read natively and Ethiopic by
  transliteration.
- **The low scripts are where the reader is least tested.** Katakana,
  syllabics, Ethiopic, conjoining jamo, CJK unified, Hiragana and Cherokee
  all average 0.60–0.76 for the community, against 0.85–0.88 for Yi and
  Hangul. Our answers for those phrases score 0.91–0.95, partly because
  Flash's profile leaves Ethiopic, syllabics, conjoining jamo and Runic off
  by default, so the search writes them in other scripts. Only clips can say whether the community entries are
  bad or the reader misreads these scripts. That is the Phase 3 probe.
- **Script labels differ from the library's.** The earlier check grouped by
  the library's own character types and found Cherokee at 0.66. Entry `81`'s
  U+14B0 lies in the syllabics block, but the library labels it Cherokee.
  Here it counts as `ucas`.
- **Where ours scores below the community.** That's 14 entries on Flash
  and 16 on Multilingual v2, nearly all within 0.05. The largest gap is
  `uf4b4957b` (0.85 vs 0.67). Its community text uses ҂ (U+0482, Cyrillic
  thousands sign), which transliterates to `*1000*` and reads as "one
  thousand" in one codepoint. The search never offers it: `cyrillic-number`
  starts at U+0483, so U+0482 counts as plain Cyrillic. Filed as #49, to
  check live in the probe.
- **Multilingual v2** lowers community and ours alike by about 0.01 overall.
  It costs most where its profile is wary: `҈` (cyrillic-number, community
  0.951 → 0.834, ours 0.982 → 0.865) and CJK compatibility. Community
  entries in Ethiopic and conjoining jamo score higher there, because its
  profile gives those scripts lower risks.

**exp15 result (real TTS, 2026-11-26, 500 credits; 3342/10000 used after):
in the low-scoring scripts the community entries are better than predicted,
and our answers worse. Offline we led by 0.23; heard, we lead by 0.03.**
Setup: the 52 library entries whose community text has Katakana, Cherokee,
Ethiopic, Canadian syllabics, conjoining jamo, Hiragana, Armenian or
Devanagari (`tools/experiments/exp15.keys`, 6 of them among the 34), plus our
top answer for each phrase. Flash v2.5, seeds 1 and 2, exp1's voice. 212
clips, 24 of them cached. There were also two ҂ checks (#49). Truth is as in
`calibrate`: the mean of heard-phone and Whisper fidelity. Numbers come from
`phono calibrate data/generated/clips-exp15.tsv --dump`, grouped by the
scripts of each phrase's community text. For the community rows,
`phono calibrate --by-script` on the community clips gives the same numbers.
"Gap" is predicted − truth. The risk penalty is what the predictor
subtracts (`risk_scale` 1.3 × the per-codepoint risk). "Gap without risk"
adds it back. The reference is the scripts the risk barely touches: on exp1
(the 34 community messages) Latin, Hangul and Yi clips have gap +0.03 to
+0.04, and +0.04 to +0.05 without risk. In exp15 the same scripts in these
52 entries are at +0.05 to +0.06 without risk.

| script | n | community: predicted | risk penalty | truth | gap | gap without risk | ours: predicted | truth | ours > community |
|---|---|---|---|---|---|---|---|---|---|
| ucas | 22 | 0.692 | 0.123 | 0.776 | −0.08 | +0.04 | 0.920 | 0.807 | 11 |
| ethiopic | 10 | 0.703 | 0.111 | 0.725 | −0.02 | +0.09 | 0.925 | 0.666 | 3 |
| hangul-jamo | 10 | 0.723 | 0.060 | 0.683 | +0.04 | +0.10 | 0.906 | 0.762 | 9 |
| katakana | 7 | 0.599 | 0.124 | 0.706 | −0.11 | +0.02 | 0.920 | 0.780 | 4 |
| hiragana | 7 | 0.753 | 0.136 | 0.829 | −0.08 | +0.06 | 0.907 | 0.808 | 3 |
| cherokee | 6 | 0.746 | 0.071 | 0.750 | −0.00 | +0.07 | 0.954 | 0.778 | 5 |
| armenian | 2 | 0.753 | 0.071 | 0.859 | −0.11 | −0.04 | 0.964 | 0.814 | 1 |
| devanagari | 1 | 0.388 | 0.169 | 0.607 | −0.22 | −0.05 | 0.712 | 0.511 | 0 |
| **all 52** | 52 | **0.696** | | **0.745** | −0.05 | | **0.921** | **0.771** | **30** |

Harness Whisper similarity is 0.80 / 0.81 per seed. Within-phrase ranking is
24 of 36 pairs (66.7%), against about 80% on exp1–exp14.

Whether the reader or the entries were wrong, per script:

- **Katakana, Canadian syllabics, Hiragana: the reader's readings are right,
  but Flash's script risks are too high.** Without the risk, these entries
  over-predict by +0.02 to +0.06, at or below the +0.05 reference. With it,
  they are under-predicted by 0.08–0.11. The community entries are fine
  (truth 0.71–0.83). Hiragana's beat ours (0.829 vs 0.808). Filed as #50.
  A caveat: the community's characters are hand-picked ones that work, and
  the search would use any chunk in the script.
- **Ethiopic, conjoining jamo: the entries are weak, and the risk is
  earned.** Even without the risk the readings over-predict by +0.09 and
  +0.10, about 0.05 above the reference, and the entries are heard at 0.73
  and 0.68. Keeping syllabics, Ethiopic and jamo off by default on Flash
  stands. Our answers beat jamo 9 of 10 times.
- **Cherokee: about right either way.** The gap is −0.00 with risk and +0.07
  without, and the entries are heard at 0.75. Ours win 5 of 6.
- **Armenian, Devanagari:** 3 entries in all, too few to say.

Our answers were over-predicted by +0.15 here. Some of that is expected,
since the search picks the top of many predictions, but it is far above the
community's +0.02 on exp1. The worst cases aren't about scripts:

- **Pseudo-word targets:** names and coinages are misheard (`u531d33fc`,
  `uea38b3b8`, `u585b9961`, `247`). That's item 5 below, short pseudo-words.
- **A dropped first word** (`ua60036ce`).
- **A runaway loop:** `ua626e6e4`'s answer, one Hangul syllable repeated, made
  Flash loop for about half a minute on one seed (truth 0.27). The predictor
  can't see that (#51).
- **Recognizer noise:** some answers heard word-perfect (`uf9fa9b27`,
  `uc5786e8c`) still score about 0.73, because the phone recognizer is noisy.
  That caps truth below 1 for short phrases.

Our answers for Ethiopic phrases lose 7 of 10. Three of the cases above
(`u585b9961`, `ua60036ce`, `ua626e6e4`) are among them, and our answers aren't written in Ethiopic
(it's off by default).

**҂ (#49).** It was heard as a thousand on all 4 clips, but "one" was dropped
on 2 of them, like `҈`'s "hundred thousand". It's worth adding as a chunk for
"thousand", with a risk for the dropped "one".

So the low offline community scores in these scripts are half right. For
Katakana, syllabics and Hiragana the risk table is too harsh. For Ethiopic
and jamo the entries really are weak, and Cherokee is in between. The bigger lesson is that our
lead shrinks a lot on hard phrases (0.92 vs 0.70 predicted, 0.77 vs 0.75
heard). The full run (#45) and the refit (#46) should show whether that holds
across the library.

**exp16 result (real TTS, 2026-11-26, 1971 credits; 5313/10000 used after):
across the whole library, our answers and the community's sound about the
same. Heard, ours lead by 0.01; predicted, by 0.10.**
Setup: the 246 library entries not in exp15 (`tools/experiments/exp16.keys`,
the 34 among them), plus our top answer for each phrase. Six of our answers
were identical to the community text, so they were merged. That gives 486
cases and 972 clips, 862 of them new. Flash v2.5, seeds 1 and 2, exp1's
voice. The cost was 81% of the dry-run estimate (≤ 2426).
- Harness Whisper similarity: 0.91 and 0.92 per seed.
- Identical transcripts across seeds: 317 of 486.

With exp15, every one of the 298 entries now has clips. Numbers are
`phono calibrate` on `clips-exp15-16.tsv` (`--dump`), truth as in exp15,
paired by key:

| entries | n | community: predicted | truth | ours: predicted | truth | ours better / tied / worse |
|---|---|---|---|---|---|---|
| **all 298** | 298 (ours 292) | 0.858 | **0.844** | 0.954 | **0.854** | 118 / 67 / 107 |
| the 34 | 34 (ours 32) | 0.843 | 0.819 | 0.958 | 0.867 | 16 / 7 / 9 |
| the other 264 | 264 (ours 260) | 0.859 | 0.847 | 0.953 | 0.853 | 102 / 60 / 98 |

Within-phrase ranking over both experiments is 86 of 132 pairs (65.2%). On
exp1–exp16 together (1764 Flash clips) v6's weights rank 264 of 346
(76.3%), and the hand-set weights 251 (72.5%).

- **The 34 flattered us.** On them we lead by 0.05 heard, as in exp5–exp8.
  On the other 264 we lead by 0.006, winning 102 and losing 98.
- **Our answers are over-predicted by +0.10; the community's by +0.01.**
  Without the risk, community entries in Latin, Hangul and Yi over-predict
  by +0.03 to +0.04 (`calibrate --by-script` on the community clips alone).
  That is the cleanest reference so far. In the same scripts our answers are
  at +0.08 to +0.10. Part of that is the winner's curse. The rest is what
  the predictor doesn't see in the cases where we lose most:
  - **Words run together.** Our answer drops a space or comma, or puts two
    words in one run of glyphs, and the TTS blurs them into one:
    `uca070d66`, `uffdeb2e3`, `uda8c79e2`. The community entries keep the
    boundary.
  - **Chunk misreads:** `u07765a28` (b heard as d), `u3454bfcc`.
  - **Mixed-script mush:** `uf4b4957b`'s answer mixes Katakana, Yi, Hangul
    and a CJK character, and two of its words were not heard at all.
  - The exp15 cases (`u585b9961`, `ua626e6e4`, `uea38b3b8`) are still among
    the ten worst.
- **Script risks (#50), community clips over all 298.** The reference is
  +0.03 without risk. The scripts exp15 covered are unchanged, since exp15
  already had every entry in them. Two scripts are new:
  - CJK unified, 36 clips: −0.01 with risk, +0.06 without, so the risk is
    about earned.
  - `҈` (Cyrillic number signs), 50 clips: +0.08 with risk and +0.09
    without, so it is under-penalized. Our answers using it are at +0.13.

**Refit on exp1–exp16 (#46): not adopted; v6's weights stay.**
`phono fit` now keeps phrases that share a non-Latin chunk of two or more
glyphs in the same cross-validation fold (`--group chunk`, the default).
`--group phrase` is v6's per-phrase split.
- **The groups:** on the 1764 clips, 311 phrases fall into 133 groups. One
  group holds 142 phrases. Folds are balanced by phrase count, so that group
  fills one fold.
- **Effect on the estimate:** held-out ranking comes out 0.3–1.0 points lower
  than with per-phrase folds, as expected.
- **Two `fit` fixes:**
  - It now picks λ by held-out ranking. Loss chose λ 0, whose weights
    (`del_schwa` 30, free insertions, `risk_scale` 0) rank held-out pairs
    below v6.
  - The "new in the last experiment" check now orders experiments by number.
    It had taken exp9 for the last.

Held-out within-phrase ranking (5 chunk-grouped folds × 4, pairs ≥ 0.05
apart, substitution scales fixed). v6's weights weren't fitted on these
folds, so v6's row is its ranking of every pair. That includes the exp1–exp6
pairs v6 was fitted on, which favours v6.

| weights | held-out ranking | new in exp16 (fit without, 88 pairs) |
|---|---|---|
| v6 (current) | 76.3% | 55/88 |
| refit from v6, λ 0.01 | **78.5%** (78.3–78.9) | 55/88 |
| refit from hand-set, λ 0.003 | 77.7% | 55/88 |
| refit from v6, `risk_scale` held at 1.3, λ 0.003 | 77.2% | 55/88 |

The best refit raises deletion costs, as v6 did (`del_onset` 1.22 → 2.44,
`del_consonant` 1.23 → 2.68). It also cuts `risk_scale` from 1.3 to 0.48.
`phono eval` and `phono calibrate` with those weights compared with v6:
- **Answers:** 30 of the 298 change. They lean to CJK unified characters
  (32 → 40 codepoints across all answers).
- **Offline fidelity** rises (community 0.858 → 0.900, ours 0.954 → 0.967),
  but so does over-prediction. Community Latin, Hangul and Yi go from +0.01
  to +0.05, so the absolute numbers get worse.
- **Pooled correlation:** 0.605 → 0.609 on all clips, but 0.534 → 0.519 on
  the community clips.
- **Per script, a single lower `risk_scale` helps some and hurts others.**
  Katakana, syllabics and Hiragana move to the reference. Conjoining jamo
  (+0.14), Ethiopic (+0.08) and CJK unified (+0.08) end up under-penalized.
  CJK is where the new answers go, and our CJK answers are already the most
  over-predicted in exp15–exp16 (+0.135).

The decision: the cross-validated gain is real but small (+2.2 points).
Most of it is the `risk_scale` cut, which is the wrong fix: the scripts
disagree, so they need risks set per script (#50). Holding `risk_scale`
leaves +0.9 points, and the test closest to a new harness run (fit without
exp16's answers, test on them) shows no gain for any refit. Refit again
once #50 has set per-script risks.

**Close-enough cut from the harness (#10, 2026-11-26, no credits): 0.90 on Flash v2.5, 0.88
on Multilingual v2.** The web tool has to say when no result is good enough, so each profile
now has a cut on predicted fidelity (`ModelProfile::close_enough`). `phono compress` prints
"No string of N codepoints or fewer scores well…" when no guess reaches it, and `--json` and
the wasm binding mark each guess with `closeEnough`.

How the cut was set: `phono calibrate` prints a close-enough table. Each distinct answer
(phrase and text, its clips averaged over seeds and replays) counts as heard well when its
Whisper similarity is ≥ 0.8 (`--heard-well`). The table gives each cut's share heard well
above and below it, and the cut with the highest Youden's J (share of the heard-well answers
above the cut minus share of the rest above it). Clips: exp1–exp16 re-exported from the clip
cache (`clips-all16.tsv`). That gives 1758 Flash clips, against 1764 in the refit above: six
exp16 clips didn't come back, because the answers were regenerated and three hand-overridden
targets weren't available. There are 406 Multilingual v2 clips.

Flash v2.5, 699 answers, 473 heard well (68%):

| cut | above | heard well | below | heard well | J |
|---|---|---|---|---|---|
| 0.86 | 475 | 83% | 224 | 35% | 0.48 |
| 0.88 | 444 | 86% | 255 | 36% | 0.53 |
| **0.90** | **416** | **88%** | **283** | **38%** | **0.55** |
| 0.92 | 388 | 88% | 311 | 43% | 0.51 |
| 0.94 | 343 | 90% | 356 | 46% | 0.50 |
| 0.96 | 293 | 91% | 406 | 50% | 0.46 |

- **It holds up.** 0.90 is also the best cut at heard-well level 0.7. It is the best cut on
  exp15–exp16 alone too (588 answers, 88% above and 46% below, J 0.49), and no weights were
  fitted on those clips. A strict level of 0.9 moves it to 0.96.
- **It is sharper for the search's own answers.** On just our answers (exp2–exp8 and the
  exp15–exp16 replays, from `--dump`), the cut leaves 89% of 390 answers heard well above
  it and 21% of 137 below.
- **Multilingual v2**, 173 answers, 71 heard well (41%): the best cut is 0.88 at heard-well
  levels 0.7, 0.8 and 0.9. At 0.8, 72% of 79 answers are heard well above it and 15% of 94
  below (J 0.59). The sample is a quarter of Flash's.

What the cut means, plainly: above it, about 9 in 10 guesses were heard as the phrase. Below
it, fewer than half were. It is not a guarantee either way, since the TTS varies between runs
and the predictor over-predicts our answers by about 0.10 (exp16). Re-run the table after a
refit or a new harness run, and move the cut if the best split moves.

**Per-script risks on exp1–exp16 (#50, 2026-11-26): Katakana and Hiragana
0.03 → 0, Canadian syllabics 0.05 → 0.01; weights and `risk_scale` stay.**
New tools:
- `phono calibrate --fit-risks` fits the over-prediction without risk by
  weighted least squares on codepoints per script, with a baseline per
  source: community entries, our answers, probes. It reports each script's
  risk with a 90% interval from resampling phrases.
- `phono fit --refit-risks` does the same inside each cross-validation fold.
- `phono fit --free-risks` makes risks free parameters of the ranking fit.
- `--risks script=value` tries values on both commands.

Our answers get their own baseline because the search picks the top of
many predictions, and that bias shouldn't be charged to the scripts it
likes. The risk is subtracted from the sound score only, so each
codepoint's term is scaled by the sound score's share of the prediction.
The exp15 table's "gap without risk" added the whole penalty back, which
overstated it wherever spelling counts.

Least squares on the 1764 Flash clips, Latin, Hangul and Yi held at 0
(per codepoint, at `risk_scale` 1.3):

| script | current | community clips only | all clips | our answers only |
|---|---|---|---|---|
| baseline | | +0.019 | community +0.015, ours +0.085 | +0.078 |
| katakana | 0.03 | −0.009 | −0.003 | +0.016 |
| hiragana | 0.03 | +0.003 | −0.004 | – (no clips) |
| ucas | 0.05 | −0.003 | +0.010 | +0.076 |
| ethiopic | 0.05 | −0.006 | +0.003 | +0.083 |
| hangul-jamo | 0.03 | +0.062 | +0.051 | – |
| cjk-unified | 0.03 | +0.007 | +0.042 | +0.086 |
| cjk-compat | 0 | +0.029 | +0.024 | +0.026 |
| cyrillic-number (`҈`) | 0 | +0.069 | +0.073 | +0.066 |

- **Kana and syllabics:** the readings are right and the risks too high,
  as #50 said.
- **Ethiopic, CJK unified and syllabics: our answers are worse than the
  community's.** The search's picks in these scripts read about 0.08 worse
  per codepoint than the hand-picked ones, which is the caveat #50 raised.
  Katakana shows no such split.
- **Ethiopic's community entries are over-penalized too.** exp15's "the
  risk is earned" came from the overstated gap.
- **`҈` and CJK compatibility are under-penalized** by every measure.

Held-out within-phrase ranking (5 chunk-grouped folds × 4, 347 pairs):

| risks | weights | held-out | new in exp16 (89 pairs) |
|---|---|---|---|
| v6 (current) | v6 | 76.4% (in-sample) | 56 |
| **kana + syllabics, least squares per fold** | v6 | **77.2%** | 56 |
| + `҈` and CJK compat | v6 | 76.7% | 56 |
| every script, least squares per fold | v6 | 74.5% | 56 |
| every script, least squares per fold | refit | 75.8% | – |
| every script, ranking fit | v6 | 76.5% | 56 |
| every script, ranking fit | refit (`risk_scale` free → 0.84) | 78.0% | 56 |
| kana + syllabics | refit, `risk_scale` held | 76.9% | 56 |
| kana + syllabics | refit, `risk_scale` free → 0.52 | 78.9% | 56 |
| v6 (#46) | refit, `risk_scale` free → 0.48 | 78.7% | 56 |

- **Calibration and ranking pull apart.** The least-squares risks fit the
  absolute scores best (in-sample loss 0.527 → 0.474), but ranking falls.
- **A ranking fit hardly moves the risks.** 347 pairs, few of them per
  script, can't tell per-script risks apart.
- **The weight refit gains only by cutting `risk_scale` to about 0.5,**
  as #46's did. With the scale held it adds nothing, and no refit gains
  on the answers new in exp16.
- So only the three risks the issue named are adopted.

Effect on `phono eval`:
- **Answers:** 6 of 298 change. Answers use 11 kana codepoints instead of
  5, among them `ヹ`, which no clip had tested.
- **Offline fidelity:** community 0.857 → 0.862, ours 0.954 → 0.955.
- **Multilingual v2:** unchanged. It already sets its own kana and
  syllabics risks.
- **Close-enough cut (#10):** re-run with these risks on exp1–exp18
  (738 answers), the best split is still 0.90 (J 0.53).

**exp17 result (real TTS, 2026-11-26, ≤ 36 credits): the 6 changed answers
are heard better, 0.856 → 0.878.** Setup: the new answers for the 6 entries
(`tools/experiments/exp17.keys`), Flash v2.5, seeds 1 and 2, exp1's voice.
The old answers and the community entries come from exp15–exp16's clips.
- **2 improve by more than 0.05** (0.83 → 0.96, 0.71 → 0.87).
- **3 tie,** all three with `ヹ` for "ve", read correctly.
- **1 gets worse** (`81`, 0.79 → 0.64): its last word is heard split.
- **All 1776 Flash clips:** within-phrase ranking 270/353 (76.5%).

**exp18 result (real TTS, 2026-11-26, ≤ 168 credits): the refit's answers
are heard worse, 0.782 → 0.739; v6's weights stay.** Setup: the refit's
answers for the 31 entries where they differ from the adopted profile's
(`tools/experiments/exp18.keys`). The refit is v6's weights refitted with
these risks in place, λ 0.01, `risk_scale` 1.3 → 0.52, deletion costs about
doubled. Same model, voice and seeds as exp17. Of the 62 cases, 60 clips
were new; the adopted answers came from exp15–exp17's clips, plus one new.
- **Refit better / worse / tied (±0.05):** 6 / 12 / 13.
- **The 9 answers that gain a CJK unified character:** 0.751 → 0.689. The
  lower scale makes CJK cheap, and our CJK answers were already the most
  over-predicted (step 1).
- **All 1900 Flash clips:** the refit's weights still rank within-phrase
  pairs a little better (298/395 against 295/395), since they were fitted
  on most of them.
- **So held-out ranking on old answers doesn't predict the answers a refit
  produces.** Each refit since v6 has cut `risk_scale`. That makes the
  search reach for risky scripts it was never tested in, and the clips
  can't see that.
- **A future refit should hold `risk_scale`,** or be checked live on the
  answers it changes, as here.

Known limitations / next steps (in priority order after exp2; status as of exp9):

0. ~~**Make re-ranking robust to reading uncertainty.**~~ Done in v2–v4:
   n-best expectation, voicing weight 3, script risk and default scripts,
   phone-level truth in `phono calibrate`.
1. **Generation spells without context.** Spelling costs ignore neighbouring
   letters, so mid-search `to|c…` beats `toe|c…`. The better spelling then
   loses its state slot before re-ranking sees it. Since v4 the tiler
   covers real words, so this now matters mainly for names and
   near-homophones. Fix: score units with the joint trigram (state = last
   two graphone units) during generation.
2. ~~Results vary slightly between runs.~~ Deterministic since v5.
3. ~~The scorer's weights are hand-set.~~ Fitted in v6 (`phono fit`):
   ranking on held-out phrases improves by about 3 points, but the search's
   answers barely change. Refit after each new harness run (`phono fit
   data/generated/clips-allN.tsv`). The refit on exp1–exp16 was not
   adopted (see "Refit on exp1–exp16"), nor was a refit with #50's
   per-script risks. Both gain only by cutting `risk_scale`, and exp18
   heard the refit's answers worse (0.782 → 0.739). Hold `risk_scale` in
   future refits, or test the changed answers live.
5. **Readings of short pseudo-words.** The TTS reads `kwi` like "kawaii",
   `doeg` like "dag" and `dyug` like "jug", while our G2P is confident they
   read K W IY, D OW G and D AY AH G. Possible fixes: widen the n-best for short unknown words, or
   learn letter-to-sound corrections from the heard phones in the clips.
   This now looks like a bigger lever than item 1. Learned corrections are
   Flash observations, so store them in the model profile (`model.rs`), not
   in module constants. exp9 adds Latin `kwi` → /kweɪ/, /kwaɪ/ (a final `i`
   in a one-syllable pseudo-word reads /aɪ/-like). It also showed isolated
   4-letter pseudo-words are too unstable to measure, so collect these
   readings inside phrases.
6. ~~**exp9 probe: respelling tricks and symbols on Flash v2.5.**~~ Run
   2026-11-24 (see "exp9 result"): no new lever. CamelCase gives no
   boundary, the reader was right about `& # = / ⅔`, and Flash v2.5 doesn't
   normalize by default. Small reader fixes it suggests, none searchable
   today: `½` → "half", `×` not "ex", `$`+digits → "dollars".
7. **Supporting another model.** Multilingual v2 is done (exp11–exp14,
   track `multilingual-v2_20261125`): its profile ships, level with Flash's
   answers on that model and ahead of the community's. Each further model
   needs:
   - an exp0-style probe;
   - a community-message run (exp1-style);
   - a new `ModelProfile` with its script table, reader constants and
     `phono fit --model <id>` weights.

   Two more things come with it:
   - ~~Move the capital-letter rules, plurals and number grouping from
     `reading.rs` into profile data once a second model's clips disagree
     with them.~~ Checked on Multilingual v2: its clips disagree with the
     `@` symbol word and the glued-digit drop, which are now profile data
     (`ReaderParams::symbol_words`, `glued_number_drop`). The capital,
     plural and number-grouping rules agree, so they stay shared. The cheap
     order for the next model: probe, community run, replay, then
     `phono fit --prior profile`.
   - v3 and v4 need reading model N (Phase 2). exp0 found v4 reads Hangul as
     Korean, so that needs a Korean reader for Hangul runs: sounds change
     across syllables (`딧잇` → [디싣], heard "did it"). Per-character
     readings aren't enough. On v3/v4, `[brackets]` are audio tags and ALL
     CAPS add emphasis. v4 also reads IPA between slashes, but at about one
     codepoint per phone.
8. **Watch for a v4 migration.** ElevenLabs "strongly recommend[s]" v4. If
   the TTS this targets moves to v4, re-run exp0 on it first: the current
   chunk table is wrong there.
4. Speed is fine natively (about 2.4 s per phrase); WASM (Phase 3) may need
   a smaller beam or pruning. The data file is 12.5 MB since v3: the G2P
   tables are unpruned u64 keys + f32. Delta-coded keys, u16 log-probs, or
   entropy pruning would shrink it.

### Phase 2 — better re-ranking and native readings

Replace rule-based letter-to-sound with a model trained on CMUdict, add
reading model N for whichever scripts Phase 0 says need it, and learn the
phone-to-spelling table from CMUdict alignments.

### Phase 3 — WASM

**Status (2026-11-25):** the binding is done (#14). `crates/wasm` builds for wasm32, gives the
same guesses as `phono compress`, and runs in Chrome and Firefox from a plain test page. The
Svelte front end and the GitHub Pages deploy are the next track (#1).

wasm-bindgen API (`crates/wasm`; TypeScript types generated with `tsify`):

```ts
const engine = new Engine(bytes);  // phono.bin's bytes, fetched by the caller; decoded once
engine.compress(phrase, { model, maxCodepoints, topN, allowedScripts });
// → [{ text, latin, phones, score }], best first
```

`model` is an ElevenLabs model id with a profile (default `eleven_flash_v2_5`), `maxCodepoints`
a whole number from 1 to 30 (default 6), `topN` from 1 to 50 (default 10), and `allowedScripts`
a list of script names (`latin`, `hangul`, …; default: the profile's default scripts). The
engine switches model and scripts per call without decoding the data again
(`Engine::configure`). Invalid input throws an `Error` that says what to change: an empty
phrase or one with nothing to pronounce, a number out of range, an unknown model, script or
option name. Options are read field by field because serde-wasm-bindgen ignores unknown keys,
which would let a typo like `maxCodepoint` pass silently.

**Decision: fetch the data file at run time instead of embedding it.** The plan was to embed
`phono.bin` in the `.wasm`. The page fetches it instead and passes the bytes to `new Engine`.
The `.wasm` stays small (150 KB gzipped, against 7.5 MB for the data), the browser caches the
two separately so a code change doesn't download the data again, and the raw bytes can be
freed once decoded.

Checks: `mise run //crates/wasm:parity` runs the parity cases (`crates/wasm/parity/cases.json`, public
phrases only) through the CLI and through the wasm build under Node. Text, Latin and phones
must match in the same order, and scores within 1e-4; the largest difference seen is 2.9e-8.
`mise run //crates/wasm:serve` serves the test page (`crates/wasm/www`), which runs the engine in a
module Web Worker.

#### Browser measurements (2026-11-25)

The `crates/wasm` binding on the four parity phrases at the defaults (6 codepoints, top 10),
timed by the test page's `?bench` mode (`mise run //crates/wasm:serve`, then open `/?bench`): three
runs per phrase, in a Web Worker. Headless Chrome 158 and Firefox 156 on Windows, with the page
served from WSL2 on the same machine (Intel i5-12600K). Two passes per browser; the tables give
the range. Native is `phono compress` (release build), search time only, a fresh process per run.

Start-up, measured in the Worker. The page is served from localhost, so the fetch time says
nothing about a real network.

| Browser | Fetch `.wasm` + `phono.bin` | Compile | Decode `phono.bin` | Engine ready | Page usable |
|---|---|---|---|---|---|
| Chrome 158 | 0.08–0.09 s | 2 ms | 0.28–0.44 s | 0.36–0.53 s | 0.68–1.02 s |
| Firefox 156 | 0.05–0.08 s | 4 ms | 0.32 s | 0.37–0.40 s | 0.51–0.52 s |

Time per phrase. "First" is a phrase's first search on a fresh page; "repeat" is runs 2 and 3.

| Phrase | Native | Chrome first | Chrome repeat | Firefox first | Firefox repeat |
|---|---|---|---|---|---|
| bad dream | 1.5 s | 1.9–2.2 s | 0.8–0.9 s | 2.0 s | 0.8–0.9 s |
| hundred thousand apartments | 3.0–3.1 s | 3.7–3.9 s | 2.2–3.0 s | 3.8–4.1 s | 1.9–2.7 s |
| good morning everyone | 3.1–3.2 s | 4.2–5.8 s | 1.7–2.3 s | 4.5–4.6 s | 1.5–1.8 s |
| zorblax is here | 2.9–3.5 s | 3.7–4.9 s | 1.5–2.0 s | 3.5–3.7 s | 1.5–2.2 s |

The phrases ran in this order, so a later phrase's first search already shares the engine's
G2P cache with the phrases before it. Repeats beat a fresh native process because that cache
survives between calls (it is cleared at 100,000 entries). Native repeats within one process
weren't measured.

Sizes:

| File | Raw | gzip -9 |
|---|---|---|
| `phono_wasm_bg.wasm` | 498 KB | 150 KB |
| the same without its name section (`wasm-bindgen --remove-name-section`) | 374 KB | 137 KB |
| `phono_wasm.js` | 19 KB | 5 KB |
| `phono.bin` | 12.6 MB | 7.47 MB |

What this says:

- A first search takes 1.1–1.8× as long as native, so the browser is not far behind and no
  speed-tuning issue was opened. In absolute terms a first search still takes 2–6 s, and the
  Worker keeps the page responsive meanwhile.
- The download is the data file: 7.5 MB gzipped against 0.15 MB of code. At 20 Mbit/s that is
  about 3 s. Shrinking it is #3.
- Decoding `phono.bin` costs 0.3–0.4 s once per page load; compiling the `.wasm` is negligible.
- Native search here took 1.5–3.2 s per phrase, slower than the "about 2.4 s" Phase 1 noted.

## Data and licenses

| Source | Used for | License |
|---|---|---|
| Phoible features | phone distance | CC-BY 4.0 (attribution required) |
| CMUdict | English pronunciations | BSD-style |
| Unihan | CJK readings (model N) | Unicode License |
| Transliteration table | chunk inventory | Built by `phono-build` from Rust `deunicode` (BSD-3) plus our own overrides. Python `unidecode` (**GPL-2.0+**) is a dev-only comparison; see "Transliteration source" below. |

### Transliteration source: move from `unidecode` to `deunicode`

**Decision (2026-11-25):** switch the chunk table's source to the `deunicode`
crate (BSD-3). The goal is a browser-only web tool, so we will distribute the
table inside the WASM bundle, and we also want to publish the tables on GitHub.
Shipping tables derived from GPL-2.0+ `unidecode` would pull the bundle and the
published tables under the GPL, which blocks a permissive repo license
(MIT OR Apache-2.0 is the plan).

**Status (2026-11-25): steps 1–4 and 6 done on branch `worktree-deunicode`; step 5 only as
attribution (`THIRD_PARTY.md`), the table itself is not published yet.**

Step 1 table diff (BMP; `phono-build --dump-translit` vs `mise run //tools:translit-unidecode`):
- 40,557 codepoints in unidecode, 54,893 in deunicode. 40,172 are identical (99.1%); 383
  differ; unidecode has 2 that deunicode lacks (U+007F, U+30FC); deunicode adds 14,338
  (7,153 CJK, 954 Arabic, Vai, Tai, Kangxi, …), almost all in scripts the Flash profile
  doesn't use.
- Of the 383 differences, 198 are Braille and about 35 Hebrew; the rest are symbols, chess
  pieces, currency and a few letters. Only these fall in scripts the search uses:
  - **Kana:** し/シ `shi`→`si`, ち/チ `chi`→`ti`, つ/ツ `tsu`→`tu` (deunicode is Kunrei-style;
    unidecode is Hepburn for these three but `hu` for フ, so neither is consistent).
  - **Han:** 么, 什, 价, 旅, 术 (readings below).
  - **Latin:** `ŋ`/`Ŋ` have their cases swapped; U+0404 `Є` is `E` rather than `Ie`.

Steps 3–4: the builder builds the table from `deunicode` (`crates/builder/src/translit.rs`), so
`mise run data` no longer needs Python. `--translit FILE` loads a TSV instead, for
comparisons. With only the table swapped (no overrides):
- `phono eval`: all 34 answers byte-identical, mean fidelity 0.961 vs community 0.843, ours ≥
  community on 33/34, both as before.
- `phono calibrate` on `clips-all9.tsv` (580 clips): output identical.

**exp10** (2026-11-25, Flash v2.5, same voice, 2 seeds, 22 clips, about 22 credits;
`tools/results/exp10.md`) probed the disputed kana and Han. No sample message or clip used
them. The TTS followed unidecode in 8 of 9 codepoints, as read from the recognized phones:

| char | deunicode | unidecode | heard (seed 1 / 2) | override |
|---|---|---|---|---|
| し シ | `si` | `shi` | `ʃ i` / `ʃ i` | `shi` |
| ち チ | `ti` | `chi` | `tʃ i` / `tʃ i` | `chi` |
| つ ツ | `tu` | `tsu` | `s k iː` / `ts k iː` | `tsu` (weaker: vowel swallowed, never a bare `t`) |
| 么 | `Me ` | `Yao ` | `j aː` / `j ɑ` | `Yao ` |
| 什 | `Shen ` | `Shi ` | `s. i` / `s. i` | `Shi ` |
| 旅 | `Lv ` | `Lu ` | `l uː` / `l uː` | `Lu ` |
| 术 | `Shu ` | `Zhu ` | `dʒ uː` / `dʒ i` | `Zhu ` |
| 价 | `Jia ` | `Jie ` | `dʒ aː` / `dʒ i` | none (seeds split) |

The ten overrides are in `OVERRIDES` in `translit.rs`, each justified by these clips. The
trailing space and capital on the Han entries follow `deunicode`'s CJK entries (and exp1's
"Kaka "); exp10 can't test them, since each character was played alone. With the overrides:
- the 34 sample answers are still byte-identical, and `calibrate` is still identical (no clip
  uses these characters);
- the table matches unidecode on 40,182 codepoints (was 40,172) and the chunk inventory is
  13,187 (was 13,189 without overrides, 13,095 under unidecode);
- the kana and Han chunks rank low in the profile's script preference, so this mostly protects
  scoring of other people's texts and future phrases.

The other 373 differences are in scripts the Flash profile doesn't search (Braille, Hebrew,
symbols) or are Latin oddities (`ŋ`/`Ŋ` swapped, `Є`); none are measured.

To do:

1. **Diff the tables.** Export `deunicode`'s output over the same blocks as
   `data:translit` and compare it chunk by chunk with `unidecode`. The
   whole project rests on the TTS matching `unidecode`'s romanization
   (`hu` not `fu`, 嬶/壗 as Kaka/Mama, Hangul and Yi via Unicode names), so any
   disagreement has to be checked against real TTS output, not assumed
   harmless. A small exp0-style probe on the disagreeing chunks should
   settle it, and it is cheap on the free tier.
2. **Patch where they differ.** Where the TTS follows `unidecode` and
   `deunicode` doesn't, keep a small override table of our own (or rebuild
   those blocks from Unicode names plus Unihan, which the original plan
   already allowed for). The overrides must be written from observed TTS
   behaviour and Unicode data, not copied from `unidecode`'s table.
3. **Move the export into Rust.** Replace `data:translit` (the uv task running
   `phono-harness translit-table`) with a step in `crates/builder`, so the data
   build no longer needs Python. `unidecode` stays in `tools/` only for
   comparisons and the "what should the TTS read?" dev view, and never feeds
   shipped data.
4. **Re-run calibration.** The fitted weights (`phono fit`) and risk tables
   were fitted on clips chosen under the old table. Check that `calibrate`
   output is unchanged on the existing clips, and spend credits on a fresh
   experiment only if chunks changed.
5. **Publish the tables.** Release the chunk table (and ideally the
   per-script reading notes) as files on GitHub, with attribution for
   `deunicode`, Phoible (CC BY 4.0) and CMUdict in a `THIRD_PARTY.md`.
6. **Then license the repo.** Add `license = "MIT OR Apache-2.0"` to
   `[workspace.package]`, plus `LICENSE-MIT` and `LICENSE-APACHE`.

## Open questions

- ~~Which ElevenLabs model to standardize on.~~ Phase 0 picked Flash
  v2.5, and since v6.2 other models can get their own profiles (item 7).
  Multilingual v2 has one since exp14; Flash stays the default.
  The real voice is still unmeasured: the free-tier API can't use Voice
  Library voices, so the harness uses premade Adam.
- Whether a space between pseudo-words is worth its codepoint
  (`ꆌ rizz` suggests yes for word boundaries).
- ~~How far to trust the "close enough" behaviour the samples show. The
  harness scores should set the threshold, not intuition.~~ Set from the
  clips (#10): 0.90 on Flash v2.5 and 0.88 on Multilingual v2, re-checked
  by `phono calibrate`'s close-enough table (see "Close-enough cut from the
  harness").
