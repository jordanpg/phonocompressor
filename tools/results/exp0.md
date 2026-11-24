# exp0

Reading-model experiment. Each case is chosen so that reading model T (transliterate with a
unidecode-like table, then read the Latin string as English) and reading model N (read the
character natively in its own language) predict different audio. See docs/PLAN.md, Phase 0.

Voice `pNInz6obpgDQGcFmaJgB`, seeds [1], ASR v1.

## `dis_is`: 딧잇

- unidecode: `dis·is`
- T predicts: dis is /dɪs ɪz/
- N predicts: Korean [디싣] /tisit̚/
- note: Sample 'this is epic swag' relies on ㅅ coda = s.

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.84 | en 0.94 ru 0.01 es 0.01 | Disseus. |  | `d ɪ s ɪ` |
| eleven_flash_v2_5 | 1 | 0.65 | en 0.95 es 0.01 de 0.01 | Disse us. |  | `d ɪ s ɪ s` |
| eleven_flash_v2 | 1 | 0.6 | en 0.80 es 0.12 pt 0.01 | This is |  | `d ɪ s ɪ s` |
| eleven_multilingual_v2 | 1 | 0.88 | en 0.85 es 0.08 pt 0.01 | This is... |  | `d ɛ s ɪ s` |
| eleven_v3 | 1 | 0.56 | en 0.90 tl 0.04 es 0.01 | Did it. |  | `v e r e t` |
| eleven_v4_turbo | 1 | 0.96 | en 0.99 es 0.00 ja 0.00 | Did it. |  | `d ɪ ɾ ɪ t` |
| eleven_v4 | 1 | 0.96 | en 1.00 es 0.00 ja 0.00 | Did it. |  | `d ɪ ɾ ɪ t` |

## `check_him`: ㅊ헻ㅋ힘

- unidecode: `c·hec·k·him`
- T predicts: c·hec·k·him → 'check him'
- N predicts: chieut het kieuk him (compatibility jamo read by name)

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.88 | en 1.00 es 0.00 de 0.00 | Check him. |  | `tʃ ɛ k ɪ m` |
| eleven_flash_v2_5 | 1 | 0.65 | en 1.00 es 0.00 de 0.00 | Check him. |  | `tʃ ɛ k h ɪ m` |
| eleven_flash_v2 | 1 | 0.65 | en 0.99 es 0.00 ru 0.00 | Check him. |  | `tʃ ɛ k ɪ m` |
| eleven_multilingual_v2 | 1 | 0.84 | en 0.91 es 0.02 de 0.01 | Check him. |  | `tʃ ɛ k h ɪ m` |
| eleven_v3 | 1 | 1.04 | en 0.85 es 0.02 de 0.01 | Joim Hep Him. |  | `dʒ oː m h ɛ p h ɪ m` |
| eleven_v4_turbo | 1 | 1.44 | zh 0.27 de 0.12 ja 0.09 | Him. | zh: 哼哼 心 | `x ʌ x ç iː m` |
| eleven_v4 | 1 | 1.44 | en 0.23 de 0.14 zh 0.11 | Him. |  | `x ʌ x ʌ ç m` |

## `geo`: 거

- unidecode: `geo`
- T predicts: geo /dʒioʊ/
- N predicts: Korean /kʌ/
- note: Strongest single-syllable discriminator: English 'eo' vs Korean ㅓ.

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.65 | pt 0.65 en 0.17 es 0.04 | Geo. | pt: Gio | `tɕ i5 ou5` |
| eleven_flash_v2_5 | 1 | 0.65 | pt 0.66 en 0.17 es 0.04 | Geo. | pt: Gio | `tɕ i5 ou5` |
| eleven_flash_v2 | 1 | 0.28 | en 0.89 ru 0.03 es 0.02 | uh, |  | `dʒ aː` |
| eleven_multilingual_v2 | 1 | 0.79 | en 0.34 pt 0.20 ja 0.13 | Keo. |  | `k e` |
| eleven_v3 | 1 | 0.72 | en 0.49 ru 0.14 pt 0.06 | call. |  | `ɡ ɔ` |
| eleven_v4_turbo | 1 | 0.8 | en 0.84 ja 0.03 ko 0.02 | Call. |  | `k oː` |
| eleven_v4 | 1 | 0.8 | en 0.90 de 0.02 ru 0.01 | Call. |  | `k oː` |

## `beos`: 벗

- unidecode: `beos`
- T predicts: beos /bioʊs/ or /boʊs/
- N predicts: Korean [벋] /pʌt̚/

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.79 | ru 0.42 es 0.07 en 0.07 | Passoit? | ru: Посует? | `p a s uː i t` |
| eleven_flash_v2_5 | 1 | 0.56 | en 0.73 ko 0.04 ru 0.03 | But... |  | `b ʌ s aɪ` |
| eleven_flash_v2 | 1 | 0.6 | en 0.78 es 0.07 pt 0.04 | Pios. |  | `b iː oː s` |
| eleven_multilingual_v2 | 1 | 0.79 | en 0.64 ru 0.05 vi 0.05 | Bye. |  | `p a t` |
| eleven_v3 | 1 | 0.72 | en 0.44 de 0.06 ar 0.06 | but |  | `b ɔ t` |
| eleven_v4_turbo | 1 | 0.88 | en 0.57 sv 0.23 de 0.04 | Pot. |  | `p ɔ t` |
| eleven_v4 | 1 | 0.88 | sv 0.53 en 0.21 de 0.04 | pot. | sv: Pott. | `p ɔ t` |

## `swagg`: 솪

- unidecode: `swagg`
- T predicts: swagg /swæɡ/
- N predicts: Korean [솩] /swak̚/

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.98 | en 0.98 es 0.00 ru 0.00 | swag |  | `s w æ ɡ` |
| eleven_flash_v2_5 | 1 | 0.84 | en 0.99 es 0.00 ru 0.00 | Swag. |  | `s w æ ɡ` |
| eleven_flash_v2 | 1 | 0.65 | en 0.99 es 0.00 ru 0.00 | Swag. |  | `s w æ ɡ` |
| eleven_multilingual_v2 | 1 | 0.79 | en 1.00 es 0.00 pt 0.00 | Swag. |  | `s w ɛ ɡ` |
| eleven_v3 | 1 | 0.0 | *silent* |  |  |  |
| eleven_v4_turbo | 1 | 0.96 | en 0.97 ru 0.01 de 0.01 | Stux. |  | `s t a k s` |
| eleven_v4 | 1 | 0.96 | en 0.99 de 0.00 es 0.00 | Stux. |  | `s t a k s` |

## `yi_sux`: ꌟ

- unidecode: `sux`
- T predicts: sux /sʌks/
- N predicts: — (no Yi support; silence or garbage)

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.79 | en 0.99 es 0.00 ru 0.00 | Sucks. |  | `s a k s` |
| eleven_flash_v2_5 | 1 | 0.56 | en 0.99 es 0.00 ru 0.00 | Sucks. |  | `s a k s` |
| eleven_flash_v2 | 1 | 0.56 | en 0.99 es 0.00 ru 0.00 | Sucks. |  | `s a k s` |
| eleven_multilingual_v2 | 1 | 0.6 | en 0.97 ru 0.01 ja 0.00 | Suck. |  | `s a k` |
| eleven_v3 | 1 | 0.0 | *silent* |  |  |  |
| eleven_v4_turbo | 1 | 1.12 | en 0.98 ru 0.00 es 0.00 | Start. |  | `s t ɑː t` |
| eleven_v4 | 1 | 1.12 | en 0.98 es 0.00 de 0.00 | Start. |  | `s t ɑː t` |

## `yi_baddream`: ꀠddꏒꀊm

- unidecode: `ba·d·d·re·a·m`
- T predicts: ba·d·d·re·a·m → 'bad dream'
- N predicts: —
- note: Sample message, verbatim.

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 1.11 | en 0.99 es 0.00 de 0.00 | Bad dream. |  | `b æ d d ɹ iː m` |
| eleven_flash_v2_5 | 1 | 0.84 | en 1.00 es 0.00 de 0.00 | Bad dream. |  | `b æ d d ɹ iː m m` |
| eleven_flash_v2 | 1 | 0.93 | en 0.99 es 0.00 de 0.00 | Bad dream. |  | `b æ d d ɹ iː m` |
| eleven_multilingual_v2 | 1 | 1.02 | en 0.98 es 0.00 ru 0.00 | Bad dream. |  | `b æ d d ɹ iː m` |
| eleven_v3 | 1 | 0.24 | *silent* |  |  |  |
| eleven_v4_turbo | 1 | 1.04 | tr 0.39 en 0.07 ar 0.06 | Taddudum. | tr: Taddu'dum. | `t a d d u d ə m` |
| eleven_v4 | 1 | 1.04 | tr 0.30 ko 0.17 en 0.11 | Tat dudum. | tr: Tat dudum. | `t a d u d ə m` |

## `mix_hangul_yi`: 딧잇ꌟ

- unidecode: `dis·is·sux`
- T predicts: disissux → 'this is sucks'
- N predicts: Korean 디싣 + ?
- note: Compare with dis_is: does a Yi neighbour flip Hangul to T, or the reverse?

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.88 | en 0.99 es 0.00 ru 0.00 | This is sucks. |  | `d ɪ s ɪ s a k s` |
| eleven_flash_v2_5 | 1 | 0.84 | en 0.95 es 0.01 de 0.01 | This is sucks. |  | `d ɪ s ə s a k s` |
| eleven_flash_v2 | 1 | 0.79 | en 0.83 id 0.06 es 0.01 | This is Sue. |  | `d eɪ s i s uː` |
| eleven_multilingual_v2 | 1 | 0.88 | en 0.45 fr 0.26 pt 0.04 | This is it. |  | `d ɛ s ɛ s ø` |
| eleven_v3 | 1 | 0.64 | en 0.98 es 0.01 de 0.00 | Did it. |  | `d ɪ ɾ ɪ t` |
| eleven_v4_turbo | 1 | 0.88 | en 1.00 de 0.00 es 0.00 | Did it. |  | `d ɪ ɾ ɪ t` |
| eleven_v4 | 1 | 0.88 | en 0.98 es 0.00 de 0.00 | Did it. |  | `d ɪ ɾ ɪ t` |

## `hundred_k`: ҈

- unidecode: `*100.000*`
- T predicts: *100.000* → 'one hundred thousand'
- N predicts: — (combining mark, probably silent)

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 1.21 | en 0.96 es 0.01 pt 0.00 | 100,000. |  | `w ʌ n h ʌ n d ɹ d t aʊ z ə n d` |
| eleven_flash_v2_5 | 1 | 1.21 | en 0.96 es 0.01 pt 0.00 | 100,000. |  | `w ʌ n h ʌ n d ɹ d θ aʊ z ə n d` |
| eleven_flash_v2 | 1 | 1.07 | en 0.98 es 0.01 de 0.00 | 100,000. |  | `h ʌ n d ɹ ɚ d t ɑː z ə n` |
| eleven_multilingual_v2 | 1 | 1.07 | en 0.92 es 0.01 pt 0.01 | Hander a thousand. |  | `p ʌ n d ɚ j ɪ d aʊ z ə n` |
| eleven_v3 | 1 | 0.0 | *silent* |  |  |  |
| eleven_v4_turbo | 1 | 0.48 | en 0.79 es 0.05 ru 0.02 | Huh. |  | `` |
| eleven_v4 | 1 | 0.48 | en 0.91 es 0.03 pt 0.01 | Ha! |  | `` |

## `apartments`: ҈㌀s

- unidecode: `*100.000*·apartment·s`
- T predicts: 'hundred thousand apartments'
- N predicts: ?

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 1.63 | en 0.96 es 0.01 pt 0.00 | 100,000 Apartments. |  | `ɐ h ʌ n d ɹ ə d θ ɑː z ə n ɐ p ɑː t m ə n t s` |
| eleven_flash_v2_5 | 1 | 1.44 | en 0.98 es 0.01 ru 0.00 | 100,000 apartments. |  | `w ʌ n h ʌ n d ɹ ɚ d θ ɑː z ə n d ɐ p ɑː t t m ə n t s` |
| eleven_flash_v2 | 1 | 1.44 | en 0.96 es 0.01 de 0.00 | 100 housing apartments. |  | `h ʌ n d ɹ ɚ d h ɑː z ɪ n d ɐ p ɑː t t m ə n t s` |
| eleven_multilingual_v2 | 1 | 1.72 | en 0.97 es 0.01 de 0.00 | 100,000 Apartments. |  | `h ʌ n d ɹ ə d θ ɑː z ə n d ə p ɑː t m ə n t s` |
| eleven_v3 | 1 | 1.12 | en 0.96 ru 0.01 pt 0.00 | Kim's |  | `k ɪ m z` |
| eleven_v4_turbo | 1 | 0.8 | en 0.48 ko 0.34 ja 0.03 | Chess! |  | `tʃ ɛ s` |
| eleven_v4 | 1 | 0.8 | ko 0.43 en 0.43 ja 0.04 | Chess. | ko: 체스 | `tɕ iɛ5 s` |

## `apaato`: ㌀

- unidecode: `apartment`
- T predicts: apartment
- N predicts: Japanese アパート /apaːto/

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.98 | en 0.96 es 0.01 fr 0.00 | apartment. |  | `ɐ p ɑː t m ə n t` |
| eleven_flash_v2_5 | 1 | 0.74 | en 0.97 es 0.01 ru 0.00 | apartment. |  | `ɐ p ɑː t m ə n t` |
| eleven_flash_v2 | 1 | 0.79 | en 0.96 es 0.01 fr 0.00 | apartment. |  | `ɐ p ɑː t m ə n t` |
| eleven_multilingual_v2 | 1 | 1.11 | en 0.98 es 0.00 ru 0.00 | Apartment. |  | `ɐ p ɑː t m ə n t` |
| eleven_v3 | 1 | 1.36 | en 0.86 es 0.04 pt 0.02 | M-Team. |  | `ɪ m t iː m` |
| eleven_v4_turbo | 1 | 0.8 | en 0.97 es 0.01 de 0.00 | Zero. |  | `z i ɹ oʊ` |
| eleven_v4 | 1 | 0.8 | en 0.97 es 0.00 ru 0.00 | 0. |  | `z i ɹ oʊ` |

## `kaka`: 嬶

- unidecode: `Kaka`
- T predicts: Kaka
- N predicts: Mandarin bì? / Japanese kakā

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.79 | en 0.91 es 0.02 pt 0.02 | Caca. |  | `k aː k a` |
| eleven_flash_v2_5 | 1 | 0.65 | en 0.94 es 0.01 ru 0.01 | Kaka. |  | `k aː k a` |
| eleven_flash_v2 | 1 | 0.65 | en 0.90 es 0.03 pt 0.02 | Caca. |  | `k aː k a` |
| eleven_multilingual_v2 | 1 | 0.84 | en 0.83 es 0.04 ru 0.03 | Caca. |  | `k a k` |
| eleven_v3 | 1 | 0.0 | *silent* |  |  |  |
| eleven_v4_turbo | 1 | 0.88 | en 0.64 zh 0.11 de 0.10 | none |  | `n a n` |
| eleven_v4 | 1 | 0.88 | en 0.67 zh 0.13 de 0.09 | none |  | `n a n` |

## `kutabireru`: 萙

- unidecode: `Kutabireru`
- T predicts: Kutabireru (10 letters, 1 codepoint)
- N predicts: Mandarin ?

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 1.11 | en 0.61 id 0.18 tl 0.04 | Kuta bireru. |  | `k uː t ʌ b i r eː r ʊ` |
| eleven_flash_v2_5 | 1 | 0.93 | en 0.71 id 0.11 pt 0.03 | Kutabireru. |  | `k uː t ʌ b i r eː r uː` |
| eleven_flash_v2 | 1 | 0.93 | en 0.76 id 0.09 ms 0.04 | Kota Bireru. |  | `k uː t ʌ b i ɹ eː ɹ uː` |
| eleven_multilingual_v2 | 1 | 0.98 | id 0.62 en 0.06 es 0.06 | Kutabireru. | id: Kutabireru. | `k u t a b i r e r u` |
| eleven_v3 | 1 | 0.0 | *silent* |  |  |  |
| eleven_v4_turbo | 1 | 0.64 | en 0.20 de 0.19 ko 0.19 | NUNG |  | `n əɜ` |
| eleven_v4 | 1 | 0.64 | ko 0.38 en 0.15 zh 0.14 | NUNG | ko: 능 | `n əɜ ŋ` |

## `kana_huri`: フリ

- unidecode: `hu·ri`
- T predicts: huri /hʊɹi/
- N predicts: Japanese furi /ɸɯɾi/

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.74 | en 0.98 es 0.00 pt 0.00 | Hurry. |  | `h o ɹ i` |
| eleven_flash_v2_5 | 1 | 0.51 | en 0.97 es 0.01 pt 0.00 | Hooray. |  | `h oː ɹ i` |
| eleven_flash_v2 | 1 | 0.46 | en 0.81 es 0.03 id 0.03 | Hurry. |  | `h o r i` |
| eleven_multilingual_v2 | 1 | 0.74 | ko 0.50 en 0.22 ja 0.10 | Free. | ko: 프리 | `f uː ɾ iː` |
| eleven_v3 | 1 | 0.4 | en 0.43 pt 0.12 fr 0.05 | free, |  | `f r iː` |
| eleven_v4_turbo | 1 | 0.64 | ja 0.54 ko 0.21 en 0.16 | Furi. | ja: 不利 | `f u5 l i5` |
| eleven_v4 | 1 | 0.64 | ko 0.61 ja 0.17 en 0.12 | Furi. | ko: 흐리 | `f u5 l i5` |

## `deva_ka`: क

- unidecode: `k`
- T predicts: k
- N predicts: Hindi /kə/

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.56 | ur 0.48 id 0.11 hi 0.08 | God. | ur: کا | `k ɑ5` |
| eleven_flash_v2_5 | 1 | 0.28 | id 0.14 pt 0.12 es 0.11 | Good. | id: Terima kasih. | `k` |
| eleven_flash_v2 | 1 | 0.46 | en 0.91 es 0.02 de 0.02 | Okay. |  | `k aː` |
| eleven_multilingual_v2 | 1 | 0.6 | es 0.46 en 0.30 ru 0.05 | K | es: K | `k a` |
| eleven_v3 | 1 | 0.72 | ur 0.51 hi 0.31 en 0.09 | Kaha? | ur: کہاں | `k a h a` |
| eleven_v4_turbo | 1 | 0.72 | en 0.34 ru 0.15 vi 0.06 | K. |  | `k əɜ` |
| eleven_v4 | 1 | 0.72 | en 0.37 ru 0.18 de 0.08 | Cough. |  | `k o5` |

## `runic_f`: ᚠ

- unidecode: `f`
- T predicts: f
- N predicts: rune name 'fehu' / 'feoh'?

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.56 | en 0.87 ru 0.03 id 0.02 | F. |  | `ɛ f` |
| eleven_flash_v2_5 | 1 | 0.33 | en 0.69 es 0.07 pt 0.07 | Yeah. |  | `` |
| eleven_flash_v2 | 1 | 0.33 | en 0.66 ru 0.06 pt 0.04 | you |  | `ɛ f` |
| eleven_multilingual_v2 | 1 | 0.46 | en 0.77 ru 0.04 es 0.03 | F. |  | `ɛ f f` |
| eleven_v3 | 1 | 0.0 | *silent* |  |  |  |
| eleven_v4_turbo | 1 | 0.48 | en 0.46 de 0.11 fr 0.09 | Ah. |  | `` |
| eleven_v4 | 1 | 0.48 | en 0.50 de 0.22 fr 0.09 | Ah. |  | `` |

## `cherokee`: ᏃᎧ

- unidecode: `no·ka`
- T predicts: noka
- N predicts: — (no Cherokee support)

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.79 | en 0.97 ru 0.00 ja 0.00 | NOCA |  | `n oː k a` |
| eleven_flash_v2_5 | 1 | 0.7 | en 0.92 es 0.02 ru 0.02 | NOCA |  | `n o k a` |
| eleven_flash_v2 | 1 | 0.6 | en 0.91 es 0.02 ru 0.01 | NOCA |  | `n aʊ k a` |
| eleven_multilingual_v2 | 1 | 0.79 | ja 0.66 es 0.10 en 0.07 | No, car | ja: のか | `n o k a` |
| eleven_v3 | 1 | 0.0 | *silent* |  |  |  |
| eleven_v4_turbo | 1 | 0.8 | en 0.79 de 0.04 fr 0.04 | You |  | `` |
| eleven_v4 | 1 | 0.8 | en 0.73 de 0.07 fr 0.06 | You |  | `` |

## `cyr_zh`: Ж

- unidecode: `Zh`
- T predicts: Zh
- N predicts: Russian letter name /ʐɛ/

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.6 | pt 0.60 en 0.32 ru 0.01 | g. | pt: G. | `ʒ i` |
| eleven_flash_v2_5 | 1 | 0.37 | fr 0.57 pt 0.21 en 0.06 | G. | fr: J'ai... | `ʒ i` |
| eleven_flash_v2 | 1 | 0.46 | en 0.77 pt 0.05 es 0.04 | Yee. |  | `dʒ i` |
| eleven_multilingual_v2 | 1 | 0.79 | fr 0.83 en 0.08 ru 0.02 | J. | fr: Je. | `s. i.5` |
| eleven_v3 | 1 | 0.72 | en 0.90 ru 0.01 id 0.01 | G. |  | `dʒ i` |
| eleven_v4_turbo | 1 | 0.8 | pl 0.90 ru 0.04 en 0.04 | G. | pl: że | `ʒ ɛ` |
| eleven_v4 | 1 | 0.8 | pl 0.99 en 0.01 ru 0.00 | Że... | pl: że | `z` |

## `tm`: ™

- unidecode: `(tm)`
- T predicts: (tm) → 'TM' or 'trademark'
- N predicts: —

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.79 | en 0.82 es 0.03 ru 0.03 | TM. |  | `t iː ɛ m` |
| eleven_flash_v2_5 | 1 | 0.74 | en 0.89 ru 0.02 es 0.01 | TM. |  | `t iː ɛ m` |
| eleven_flash_v2 | 1 | 0.74 | en 0.97 es 0.01 de 0.00 | Trademark. |  | `t ɹ eɪ d m ɑː k` |
| eleven_multilingual_v2 | 1 | 0.51 | en 0.72 ru 0.05 pt 0.04 | Me. |  | `n eɪ` |
| eleven_v3 | 1 | 0.88 | en 0.96 es 0.01 ru 0.00 | TM. |  | `t iː ɛ m` |
| eleven_v4_turbo | 1 | 0.72 | en 0.96 es 0.01 pt 0.00 | Time. |  | `t aɪ m` |
| eleven_v4 | 1 | 0.72 | en 0.94 es 0.01 pt 0.01 | Time. |  | `t aɪ m` |

## `km`: ㎞

- unidecode: `km`
- T predicts: km → 'kilometer(s)' or 'K M'
- N predicts: Japanese キロメートル?

| model | seed | sec | lang (top 3) | Whisper EN | Whisper native | phones |
|---|---|---|---|---|---|---|
| eleven_turbo_v2_5 | 1 | 0.74 | en 0.47 ko 0.21 de 0.05 | Come. |  | `k eː l m` |
| eleven_flash_v2_5 | 1 | 0.56 | en 0.84 ru 0.03 pt 0.03 | Calm. |  | `k o m` |
| eleven_flash_v2 | 1 | 0.6 | en 0.69 pt 0.18 es 0.04 | KMN. |  | `k aɪ a m a` |
| eleven_multilingual_v2 | 1 | 0.7 | en 0.93 es 0.02 pt 0.01 | KM. |  | `k eː ɛ m` |
| eleven_v3 | 1 | 1.12 | en 0.98 es 0.00 ru 0.00 | Kalimoroders. |  | `k ʌ l ɪ m ɚ ɹ oʊ ɾ ɚ z` |
| eleven_v4_turbo | 1 | 1.04 | en 0.95 es 0.01 pt 0.01 | Kilometers. |  | `k ɪ l ɑː m ɪ ɾ ɚ z` |
| eleven_v4 | 1 | 1.04 | en 0.98 es 0.01 de 0.00 | Kilometers. |  | `k ɪ l ɑː m ɪ ɾ ɚ z` |
