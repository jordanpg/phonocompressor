# Third-party data and software

The code in this repository is licensed MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`).
The data it is built from has its own terms.

| Component | Used for | License | Notes |
|---|---|---|---|
| [Phoible](https://phoible.org) segment features | phone distances | CC BY 4.0 | Attribution required. Submodule `phoible/`. |
| [CMUdict](https://github.com/cmusphinx/cmudict) | English pronunciations, G2P training | BSD-style | Submodule `cmudict/`; see its `LICENSE`. |
| [`deunicode`](https://crates.io/crates/deunicode) | the transliteration table (chunk inventory) | BSD-3-Clause | Copyright (c) 2015 Amit Chowdhury, 2018–2021 Kornel Lesinski, 2020–2021 Hunter WB. Its README lists its data sources: Text::Unidecode by Sean M. Burke, Unicodey by Cal Henderson, gh-emoji and any_ascii. |

`deunicode`'s table is a descendant of Text::Unidecode's, the same ancestor as Python's
`unidecode`. We rely on the crate's declared BSD-3-Clause license, and compare against Python
`unidecode` (GPL-2.0+) only in development (`mise run dev:translit-unidecode`). Its output never
goes into `data/generated/phono.bin`. The builder reads that table only if you pass `--translit`
by hand.

`crates/builder/src/translit.rs` holds our own overrides to the table (`OVERRIDES`). Each one has to
come from observed TTS behaviour or Unicode data.

The ElevenLabs harness clips, `sample/` messages and `tools/results/` are not distributed.
