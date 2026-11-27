"""Tests for building experiment cases from message files (synthetic data only)."""

import json

from phono_harness.experiments import cases_from_files, load_keys


def write(path, items):
    path.write_text(json.dumps(items, ensure_ascii=False), encoding="utf-8")
    return str(path)


def test_load_keys_takes_last_tab_field_and_skips_comments(tmp_path):
    p = tmp_path / "k.keys"
    p.write_text("# probe keys\n\n8\nabcd-ef\t10\n u1a2b3c4d \n", encoding="utf-8")
    assert load_keys(str(p)) == {"8", "10", "u1a2b3c4d"}


def test_cases_keys_filter_every_file_by_base_key(tmp_path):
    community = write(
        tmp_path / "library.json",
        [
            {"8": {"unicode": "ab", "spoken": "a b"}},
            {"u1": {"unicode": "cd", "spoken": "c d"}},
            {"u2": {"unicode": "ef", "spoken": "e f"}},
        ],
    )
    ours = write(
        tmp_path / "ours.json",
        [
            {"8": {"unicode": "AB", "spoken": "a b"}},
            {"u1": {"unicode": "cd", "spoken": "c d"}},  # same text as community: dropped
            {"u2": {"unicode": "EF", "spoken": "e f"}},
        ],
    )
    cases = cases_from_files([community, ours], keys={"8", "u1"})
    assert [(c.id, c.text) for c in cases] == [
        ("msg8", "ab"),
        ("msgu1", "cd"),
        ("msg8_ours", "AB"),
    ]


def test_without_keys_every_case_is_kept(tmp_path):
    f = write(tmp_path / "m.json", [{"1": {"unicode": "a", "spoken": "a"}}])
    assert [c.id for c in cases_from_files(f)] == ["msg1"]
