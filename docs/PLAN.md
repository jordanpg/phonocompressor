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
Only `eleven_flash_v2_5` (which also covers `eleven_turbo_v2_5`) has a
profile. A new model needs an exp0-style probe and a community-message run
to get one. Models that read scripts natively (v3, v4) also need reading
model N.

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
   data/generated/clips-allN.tsv`).
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
7. **Supporting another model.** Each one needs:
   - an exp0-style probe;
   - a community-message run (exp1-style);
   - a new `ModelProfile` with its script table, reader constants and
     `phono fit --model <id>` weights.

   Two more things come with it:
   - Move the capital-letter rules, plurals and number grouping from
     `reading.rs` into profile data once a second model's clips disagree
     with them.
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

wasm-bindgen API:
`compress(phrase, { model, maxCodepoints: 6, topN, allowedScripts }) → [{ text, latin, phones, score }]`,
where `model` is an ElevenLabs model id with a profile (default `eleven_flash_v2_5`).

## Data and licenses

| Source | Used for | License |
|---|---|---|
| Phoible features | phone distance | CC-BY 4.0 (attribution required) |
| CMUdict | English pronunciations | BSD-style |
| Unihan | CJK readings (model N) | Unicode License |
| Transliteration table | chunk inventory | Python `unidecode` is **GPL-2.0+**; Rust `deunicode` is BSD-3. We can also rebuild most of the table from Unicode names + Unihan. Decide before shipping the table in the bundle. |

## Open questions

- ~~Which ElevenLabs model to standardize on.~~ Phase 0 picked Flash
  v2.5, and since v6.2 other models can get their own profiles (item 7).
  The real voice is still unmeasured: the free-tier API can't use Voice
  Library voices, so the harness uses premade Adam.
- Whether a space between pseudo-words is worth its codepoint
  (`ꆌ rizz` suggests yes for word boundaries).
- How far to trust the "close enough" behaviour the samples show. The
  harness scores should set the threshold, not intuition.
