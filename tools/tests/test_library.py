"""Tests for the library fetch's pure parts, on synthetic entries (no library content)."""

import json

import pytest

from phono_harness import library
from phono_harness.experiments import cases_from_messages

U1 = "11111111-aaaa-4bbb-8ccc-000000000001"
U2 = "22222222-aaaa-4bbb-8ccc-000000000002"
U3 = "3333ABCD-aaaa-4bbb-8ccc-000000000003"


def entry(uuid, unicode="ab", spoken="a b", created="2026-01-01 00:00:00", **extra):
    e = {
        "id": uuid,
        "unicode": unicode,
        "spoken": spoken,
        "notes": "",
        "submittedBy": {"displayName": "someone", "provider": "x"},
        "characters": [{"position": 0, "glyph": "a", "type": {"name": "Latin"}}],
        "score": 3,
        "createdAt": created,
        "updatedAt": created,
    }
    e.update(extra)
    return e


def test_key_table_parses_tsv_and_skips_comments(tmp_path):
    p = tmp_path / "keys.tsv"
    p.write_text(f"# header\n\n{U1}\t8\n{U2}\t10\n", encoding="utf-8")
    assert library.load_key_table(p) == {U1: "8", U2: "10"}


def test_hand_picked_keep_their_key_others_keyed_from_uuid():
    table = {U1: "8"}
    assert library.entry_key(U1, table) == "8"
    assert library.entry_key(U3, table) == "u3333abcd"


def test_clean_entry_drops_submitter_and_score_and_keeps_field_order():
    cleaned = library.clean_entry(entry(U1))
    assert list(cleaned) == [
        "id",
        "unicode",
        "spoken",
        "target",
        "notes",
        "characters",
        "createdAt",
        "updatedAt",
    ]
    assert "submittedBy" not in cleaned and "score" not in cleaned


def test_clean_entry_fills_missing_fields():
    cleaned = library.clean_entry({"id": U1, "unicode": "x", "spoken": "x"})
    assert cleaned["notes"] == "" and cleaned["characters"] == []


def test_build_orders_by_creation_then_uuid():
    items = library.build(
        [
            entry(U2, created="2026-01-02 00:00:00"),
            entry(U3, created="2026-01-01 00:00:00"),
            entry(U1, created="2026-01-01 00:00:00"),
        ],
        {},
    )
    assert [next(iter(i)) for i in items] == ["u11111111", "u3333abcd", "u22222222"]


def test_build_rejects_key_collisions():
    with pytest.raises(ValueError):
        library.build([entry(U1), entry(U2)], {U1: "8", U2: "8"})


def test_refetch_keeps_every_key_when_entries_are_inserted():
    table = {U1: "8"}
    first = library.build([entry(U1), entry(U2, created="2026-01-03 00:00:00")], table)
    second = library.build(
        [
            entry(U1),
            entry(U3, created="2026-01-02 00:00:00"),
            entry(U2, created="2026-01-03 00:00:00"),
        ],
        table,
    )
    assert set(library.as_map(first)) <= set(library.as_map(second))
    assert library.diff(first, second) == (["u3333abcd"], [], [])


def test_diff_reports_changed_and_removed():
    first = library.build([entry(U1), entry(U2)], {})
    second = library.build([entry(U1, spoken="a bee")], {})
    assert library.diff(first, second) == ([], ["u11111111"], ["u22222222"])


def test_score_changes_do_not_count_as_changes():
    first = library.build([entry(U1, score=1)], {})
    second = library.build([entry(U1, score=9)], {})
    assert library.diff(first, second) == ([], [], [])
    assert library.content_hash(first) == library.content_hash(second)


def test_content_hash_tracks_content():
    a = library.build([entry(U1)], {})
    b = library.build([entry(U1, notes="flash v2.5")], {})
    assert library.content_hash(a) != library.content_hash(b)


def test_output_round_trips_and_feeds_cases_from(tmp_path):
    items = library.build([entry(U1, unicode="ꀠb", spoken="hi there")], {U1: "8"})
    p = tmp_path / "library.json"
    p.write_text(library.dumps(items), encoding="utf-8")
    assert json.loads(p.read_text(encoding="utf-8")) == items
    assert library.read(p) == items
    [case] = cases_from_messages(p)
    assert (case.id, case.text, case.target) == ("msg8", "ꀠb", "hi there")


def test_output_has_one_entry_per_line_with_key_first():
    # The CLI's loader finds each key as the string right before `": {` ahead of "unicode".
    text = library.dumps(library.build([entry(U1), entry(U2)], {U1: "8"}))
    lines = text.splitlines()
    assert lines[0] == "[" and lines[-1] == "]"
    assert lines[1].startswith('{"8": {"id": ')
    assert lines[2].startswith('{"u22222222": {"id": ')


def test_read_missing_file_is_empty(tmp_path):
    assert library.read(tmp_path / "nope.json") == []


# --- targets ---


@pytest.mark.parametrize(
    ("spoken", "target"),
    [
        ("plain words", "plain words"),
        ("(aside) the blue kettle", "the blue kettle"),
        ("ends with an aside (lol)", "ends with an aside"),
        ("koko('s) stiff", "koko's stiff"),  # attached to a word: optional letters, kept
        ("cat/dog house", "cat"),  # a/b: the first alternative
        ("cat / dog", "cat"),
        ("100k cats", "100,000 cats"),
        ("5K steps", "5,000 steps"),
        ("100,000 cats", "100,000 cats"),
        ("1/2 cup", "1/2 cup"),  # a slash between digits is not an alternative
        ("ok kk 3kg", "ok kk 3kg"),  # only a bare <digits>k token
        ("  spaced   out  ", "spaced out"),
        ("(only an aside)", "(only an aside)"),  # nothing left: keep the label
    ],
)
def test_clean_target_rules(spoken, target):
    assert library.clean_target(spoken) == target


def test_overrides_parse_tsv(tmp_path):
    p = tmp_path / "targets.tsv"
    p.write_text("# key<TAB>target\n\nu1\tgreen lamp\n8\tblue door\n", encoding="utf-8")
    assert library.load_overrides(p) == {"u1": "green lamp", "8": "blue door"}
    assert library.load_overrides(tmp_path / "missing.tsv") == {}


def test_build_adds_target_after_spoken():
    [item] = library.build([entry(U1, spoken="100k cats")], {U1: "8"})
    fields = item["8"]
    assert fields["target"] == "100,000 cats"
    assert list(fields)[:4] == ["id", "unicode", "spoken", "target"]


def test_override_beats_rule():
    items = library.build([entry(U1, spoken="cat/dog house")], {U1: "8"}, {"8": "dog house"})
    assert items[0]["8"]["target"] == "dog house"


def test_target_stats_count_rules_overrides_and_unused():
    items = library.build(
        [entry(U1, spoken="100k"), entry(U2, spoken="plain"), entry(U3, spoken="x/y")],
        {U1: "8"},
        {"u3333abcd": "y", "gone": "z"},
    )
    assert library.target_stats(items, {"u3333abcd": "y", "gone": "z"}) == (1, 1, ["gone"])


def test_cases_prefer_target_and_fall_back_to_spoken(tmp_path):
    p = tmp_path / "m.json"
    p.write_text(
        json.dumps(
            [
                {"1": {"unicode": "a", "spoken": "a b", "target": "ay bee"}},
                {"2": {"unicode": "c", "spoken": "c d"}},
                {"3": {"unicode": "e", "spoken": "e f", "target": ""}},
            ]
        ),
        encoding="utf-8",
    )
    assert [c.target for c in cases_from_messages(p)] == ["ay bee", "c d", "e f"]
