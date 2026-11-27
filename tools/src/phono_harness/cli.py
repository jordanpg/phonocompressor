"""phono-harness CLI. Normally invoked through mise tasks (see tools/mise.toml):

mise run //tools:credits                 credit usage this period            (API, free)
mise run //tools:models                  TTS models and cost multipliers     (API, free)
mise run //tools:synth exp0              estimate cost of uncached clips     (dry run)
mise run //tools:synth exp0 --spend      ...and actually synthesize them     (API, costs credits)
mise run //tools:transcribe exp0         local ASR on cached clips           (GPU, free)
mise run //tools:report exp0             markdown report to tools/results/
mise run //tools:fetch-library           community library to sample/library.json (gitignored)
"""

from __future__ import annotations

import argparse
import math
import sys

from . import cache, library
from .experiments import REPO_ROOT, load


def cmd_credits(_args) -> None:
    from .elevenlabs import Client

    s = Client().subscription()
    used, limit = int(s["character_count"]), int(s["character_limit"])
    print(f"tier={s.get('tier')} status={s.get('status')}")
    print(f"credits used {used} / {limit}  (remaining {limit - used})")
    if s.get("next_character_count_reset_unix"):
        import datetime as dt

        reset = dt.datetime.fromtimestamp(s["next_character_count_reset_unix"], dt.UTC)
        print(f"resets {reset:%Y-%m-%d %H:%M} UTC")


def _tts_models(client) -> dict[str, dict]:
    return {m["model_id"]: m for m in client.models() if m.get("can_do_text_to_speech")}


def cmd_models(_args) -> None:
    from .elevenlabs import Client

    for mid, m in _tts_models(Client()).items():
        rates = m.get("model_rates") or {}
        print(
            f"{mid:28} cost×{rates.get('character_cost_multiplier', '?')}"
            f" discount×{rates.get('cost_discount_multiplier', '?')}"
            f" langs={len(m.get('languages') or [])}"
            f" free_max_chars={m.get('max_characters_request_free_user')}"
            f"{'  [alpha]' if m.get('requires_alpha_access') else ''}"
            f"  {m.get('name', '')}"
        )


def cmd_voices(_args) -> None:
    from .elevenlabs import Client

    for v in sorted(
        Client().voices(), key=lambda v: (v.get("category") or "", v.get("name") or "")
    ):
        labels = v.get("labels") or {}
        desc = ", ".join(
            f"{k}={labels[k]}" for k in ("language", "accent", "gender", "age") if labels.get(k)
        )
        print(f"{v['voice_id']}  {v.get('category', '?'):12} {v.get('name', '')[:28]:28} {desc}")


def cmd_synth(args) -> None:
    from .elevenlabs import ApiError, Client

    exp = load(args.experiment)
    todo = [(c, r) for c, r in exp.requests() if not cache.has_clip(r.key)]
    print(f"{exp.name}: {len(exp.requests())} clips, {len(todo)} not cached")
    if not todo:
        return

    client = Client()
    models = _tts_models(client)
    unknown = sorted({r.model_id for _, r in todo} - models.keys())
    if unknown:
        raise SystemExit(
            f"not TTS models for this account: {unknown} (see `mise run //tools:models`)"
        )

    def cost(r) -> int:
        # Billing rounds each request up to a whole credit (exp0: 1-char requests on 0.5× models cost 1).
        rates = models[r.model_id].get("model_rates") or {}
        return math.ceil(len(r.text) * float(rates.get("character_cost_multiplier", 1.0)))

    estimate = sum(cost(r) for _, r in todo)
    remaining = client.remaining_credits()
    print(
        f"estimated cost ≤ {estimate:.0f} credits (budget {exp.max_credits}, remaining {remaining})"
    )
    if estimate > exp.max_credits or estimate > remaining:
        raise SystemExit("refusing: estimate exceeds the experiment budget or remaining credits")
    if not args.spend:
        print("dry run: pass --spend to synthesize")
        return

    failures = 0
    for i, (case, req) in enumerate(todo, 1):
        try:
            pcm, meta = client.synthesize(req)
        except ApiError as e:
            failures += 1
            print(f"[{i}/{len(todo)}] {case.id} {req.model_id}: FAILED {e}", file=sys.stderr)
            if e.status in (401, 402, 403):  # account-level: every later request would fail too
                print("account-level error; stopping", file=sys.stderr)
                break
            continue
        meta["case"] = case.id
        cache.save_clip(req.key, pcm, meta)
        print(f"[{i}/{len(todo)}] {case.id:16} {req.model_id:24} {len(pcm) / 32000:.2f}s")

    after = client.remaining_credits()
    print(
        f"credits used so far: {remaining - after} (billing lags; re-check with `mise run //tools:credits`)"
    )
    if failures:
        raise SystemExit(f"{failures} request(s) failed")


def cmd_transcribe(args) -> None:
    from .asr import ASR_VERSION, Transcriber, load_wav

    exp = load(args.experiment)
    todo = [
        (c, r)
        for c, r in exp.requests()
        if cache.has_clip(r.key) and (args.force or not cache.asr_path(r.key, ASR_VERSION).exists())
    ]
    print(f"{exp.name}: {len(todo)} clip(s) to transcribe")
    if not todo:
        return
    t = Transcriber()
    for i, (case, req) in enumerate(todo, 1):
        out = t.run(load_wav(cache.wav_path(req.key)))
        cache.write_json(cache.asr_path(req.key, ASR_VERSION), out)
        summary = "(silent)" if out.get("silent") else f"{out['languages'][0]}  {out['en']!r}"
        print(f"[{i}/{len(todo)}] {case.id:16} {req.model_id:24} {summary}")


def _md(s: str) -> str:
    return s.replace("|", "\\|").replace("\n", " ")


def _letters(s: str) -> str:
    return "".join(ch for ch in s.lower() if ch.isalnum())


def _sim(a: str, b: str) -> float:
    """Crude spelling similarity (0–1) ignoring case, spaces and punctuation.
    A stand-in until the Rust phone-level scorer exists."""
    import difflib

    return difflib.SequenceMatcher(None, _letters(a), _letters(b)).ratio()


def cmd_report(args) -> None:
    from collections import defaultdict
    from statistics import mean

    from unidecode import unidecode

    from .asr import ASR_VERSION

    exp = load(args.experiment)
    asr_by = {}  # (case id, model, seed) -> asr dict | None
    for c, req in exp.requests():
        asr_by[(c.id, req.model_id, req.seed)] = (
            cache.read_json(cache.asr_path(req.key, ASR_VERSION))
            if cache.has_clip(req.key)
            else None
        )
    has_targets = any(c.target for c in exp.cases)

    lines = [f"# {exp.name}", "", exp.description.strip(), ""]
    lines += [f"Voice `{exp.voice_id}`, seeds {exp.seeds}, ASR {ASR_VERSION}.", ""]

    if has_targets:
        lines += [
            "## Summary",
            "",
            "`sim` = spelling similarity of the Whisper (English) transcript to the target phrase,",
            "ignoring case/spaces/punctuation (0–1). Crude: it penalizes homophones and digits-vs-words.",
            "",
            "| model | seed | clips | silent | mean sim | sim ≥ 0.8 |",
            "|---|---|---|---|---|---|",
        ]
        for model in exp.models:
            for seed in exp.seeds:
                rows = [(c, asr_by[(c.id, model, seed)]) for c in exp.cases if c.target]
                done = [(c, a) for c, a in rows if a is not None]
                silent = sum(1 for _, a in done if a.get("silent"))
                sims = [
                    _sim(a.get("en", ""), c.target) if not a.get("silent") else 0.0 for c, a in done
                ]
                good = sum(1 for s in sims if s >= 0.8)
                ms = f"{mean(sims):.2f}" if sims else "—"
                lines.append(
                    f"| {model} | {seed} | {len(done)} | {silent} | {ms} | {good}/{len(done)} |"
                )
        if len(exp.seeds) > 1:
            lines += ["", "Seed agreement (same model, transcripts across seeds):", ""]
            for model in exp.models:
                agree = defaultdict(list)
                for c in exp.cases:
                    texts = [asr_by[(c.id, model, s)] for s in exp.seeds]
                    if all(t is not None and not t.get("silent") for t in texts):
                        first = texts[0]["en"]
                        agree["sim"].append(min(_sim(first, t["en"]) for t in texts[1:]))
                        agree["exact"].append(
                            all(_letters(t["en"]) == _letters(first) for t in texts)
                        )
                if agree["sim"]:
                    lines.append(
                        f"- {model}: mean cross-seed sim {mean(agree['sim']):.2f}, "
                        f"identical transcripts {sum(agree['exact'])}/{len(agree['exact'])}"
                    )
        lines.append("")

    for case in exp.cases:
        latin = "·".join(unidecode(ch).strip() or "∅" for ch in case.text)
        lines += [f"## `{case.id}`: {case.text}", "", f"- unidecode: `{latin}`"]
        if case.target:
            lines.append(f"- target: **{case.target}**")
        if case.t or case.n:
            lines += [f"- T predicts: {case.t or '—'}", f"- N predicts: {case.n or '—'}"]
        if case.note:
            lines.append(f"- note: {case.note}")
        if case.normalization:
            lines.append(f"- apply_text_normalization: `{case.normalization}`")
        sim_h = " sim |" if case.target else ""
        lines += [
            "",
            f"| model | seed | sec | lang (top 3) | Whisper EN |{sim_h} Whisper native | phones |",
            "|---|---|---|---|---|---|---|" + ("---|" if case.target else ""),
        ]
        for model in exp.models:
            for seed in exp.seeds:
                asr = asr_by[(case.id, model, seed)]
                sim = ""
                if asr is None:
                    cells = ["", "*not synthesized/transcribed*", "", "", ""]
                elif asr.get("silent"):
                    cells = [f"{asr['duration_s']}", "*silent*", "", "", ""]
                    sim = "0.00"
                else:
                    langs = " ".join(f"{code} {p:.2f}" for code, p in asr["languages"])
                    native = asr.get("native")
                    native_s = f"{native['language']}: {native['text']}" if native else ""
                    cells = [
                        f"{asr['duration_s']}",
                        langs,
                        asr["en"],
                        native_s,
                        f"`{asr['phones']}`",
                    ]
                    sim = f"{_sim(asr['en'], case.target):.2f}" if case.target else ""
                if case.target:
                    cells.insert(3, sim)
                lines.append(f"| {model} | {seed} | " + " | ".join(_md(x) for x in cells) + " |")
        lines.append("")
    text = "\n".join(lines)
    out = REPO_ROOT / "tools" / "results" / f"{exp.name}.md"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8")
    print(text)
    print(f"\nwritten to {out.relative_to(REPO_ROOT)}", file=sys.stderr)


def cmd_export_clips(args) -> None:
    """TSV of transcribed clips for `phono calibrate`: experiment, case, seed, model, text,
    target phrase, Whisper English, recognized IPA phones. Only cases with a target phrase, and
    only clips made with the API's default settings (calibration models the default reading).
    Contains sample phrases when exported from exp1/exp2: write it somewhere gitignored."""
    from pathlib import Path

    from .asr import ASR_VERSION

    def clean(s: str) -> str:
        return s.replace("\t", " ").replace("\n", " ")

    rows = 0
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8") as f:
        f.write("exp\tcase\tseed\tmodel\ttext\ttarget\twhisper_en\tphones\n")
        for name in args.experiments:
            exp = load(name)
            for case, req in exp.requests():
                asr = cache.read_json(cache.asr_path(req.key, ASR_VERSION))
                if not case.target or asr is None or req.apply_text_normalization is not None:
                    continue
                cells = [
                    exp.name,
                    case.id,
                    str(req.seed),
                    req.model_id,
                    case.text,
                    case.target,
                    asr.get("en", ""),
                    asr.get("phones", ""),
                ]
                f.write("\t".join(clean(c) for c in cells) + "\n")
                rows += 1
    print(f"wrote {rows} clips to {out}")


def cmd_translit_table(args) -> None:
    """Dump unidecode's output for every BMP codepoint as TSV: hex codepoint, JSON-quoted output.

    Dev only, for comparing against the shipped (deunicode) table. Python unidecode is
    GPL-2.0+, so this output never feeds the data file (see docs/PLAN.md, Data and licenses).
    """
    import json
    from pathlib import Path

    from unidecode import unidecode

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    n = 0
    with out.open("w", encoding="utf-8") as f:
        for cp in range(0x20, 0x10000):
            if 0xD800 <= cp <= 0xDFFF:
                continue
            t = unidecode(chr(cp))
            if t and t != "[?]":
                f.write(f"{cp:04X}\t{json.dumps(t, ensure_ascii=True)}\n")
                n += 1
    print(f"wrote {n} entries to {out}")


def cmd_fetch_library(args) -> None:
    """Fetch the community library into gitignored sample/library.json (see library.py)."""
    import os
    from pathlib import Path

    url = os.environ.get(library.URL_ENV)
    if not url:
        raise SystemExit(f"{library.URL_ENV} is not set (it comes from fnox via the mise task)")
    out = Path(args.out)
    table = library.load_key_table(Path(args.keys))
    overrides = library.load_overrides(Path(args.targets))
    items = library.build(library.fetch(url), table, overrides)
    added, changed, removed = library.diff(library.read(out), items)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(library.dumps(items), encoding="utf-8")
    keys = library.as_map(items)
    picked = sum(1 for k in table.values() if k in keys)
    print(f"wrote {len(items)} entries to {out}")
    print(f"new {len(added)}, changed {len(changed)}, removed {len(removed)} since the last fetch")
    print(f"hand-picked keys present: {picked}/{len(table)}")
    by_rule, by_override, unused = library.target_stats(items, overrides)
    print(f"targets: {by_rule} cleaned by rule, {by_override} from {Path(args.targets).name}")
    print(f"content hash {library.content_hash(items)}")
    if removed:
        print("removed keys: " + " ".join(removed))
    if unused:
        print("overrides with no entry: " + " ".join(unused))


def main(argv: list[str] | None = None) -> None:
    p = argparse.ArgumentParser(
        prog="phono-harness",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("credits").set_defaults(func=cmd_credits)
    sub.add_parser("models").set_defaults(func=cmd_models)
    sub.add_parser("voices").set_defaults(func=cmd_voices)
    s = sub.add_parser("synth")
    s.add_argument("experiment")
    s.add_argument("--spend", action="store_true", help="actually call the API (default: dry run)")
    s.set_defaults(func=cmd_synth)
    t = sub.add_parser("transcribe")
    t.add_argument("experiment")
    t.add_argument("--force", action="store_true", help="re-transcribe even if cached")
    t.set_defaults(func=cmd_transcribe)
    ec = sub.add_parser("export-clips")
    ec.add_argument("experiments", nargs="+")
    ec.add_argument("--out", required=True)
    ec.set_defaults(func=cmd_export_clips)
    tt = sub.add_parser("translit-table")
    tt.add_argument("out")
    tt.set_defaults(func=cmd_translit_table)
    fl = sub.add_parser("fetch-library")
    fl.add_argument("--out", default=str(library.OUT_PATH))
    fl.add_argument("--keys", default=str(library.KEYS_PATH))
    fl.add_argument("--targets", default=str(library.OVERRIDES_PATH))
    fl.set_defaults(func=cmd_fetch_library)
    r = sub.add_parser("report")
    r.add_argument("experiment")
    r.set_defaults(func=cmd_report)
    args = p.parse_args(argv)
    args.func(args)


if __name__ == "__main__":
    main()
