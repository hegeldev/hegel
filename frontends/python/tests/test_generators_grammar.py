from __future__ import annotations

import re
from pathlib import Path

import pytest

import hegel
from hegel import generators as gs
from tests.helpers import failing, find_example, notes, run_test

INT_LIST = 'start: INT ("," INT)*\n%import common.INT\n'


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases, database="")


def is_int_list(s):
    return all(part.isdigit() for part in s.split(","))


def test_from_lark_generates_strings_of_the_grammar():
    check_all(gs.from_lark(INT_LIST), is_int_list)


def test_from_lark_shrinks_to_the_smallest_string():
    assert find_example(gs.from_lark(INT_LIST), lambda s: "," in s) == "0,0"


def test_from_lark_start_selects_the_rule():
    check_all(gs.from_lark('a: "a"+\nb: "b"+\n', start="b"), lambda s: set(s) == {"b"})


def test_from_lark_explicit_supplies_declared_terminals():
    check_all(
        gs.from_lark(
            'start: WORD "!" NUM\n%declare WORD NUM\n',
            explicit={
                "WORD": gs.from_regex("[a-z]{3}"),
                "NUM": gs.text(alphabet="0123456789", min_size=1),
            },
        ),
        lambda s: re.fullmatch(r"[a-z]{3}![0-9]+", s) is not None,
    )


def test_from_lark_explicit_overrides_a_pattern_terminal():
    check_all(
        gs.from_lark('start: "a" NUM\nNUM: /[0-9]+/\n', explicit={"NUM": gs.just("xyz")}),
        lambda s: s == "axyz",
    )


def test_from_lark_explicit_values_shrink_with_the_grammar():
    found = find_example(
        gs.from_lark(
            'start: NUM ("," NUM)*\n%declare NUM\n',
            explicit={"NUM": gs.integers(0, 1000).map(str)},
            max_size=40,
        ),
        lambda s: "," in s,
    )
    assert found == "0,0"


def test_from_lark_max_size_counts_explicit_values():
    check_all(
        gs.from_lark(
            'start: NUM ("," NUM)*\n%declare NUM\n',
            explicit={"NUM": gs.text(alphabet="0123456789", min_size=1, max_size=5)},
            max_size=12,
        ),
        lambda s: len(s) <= 12,
    )


def test_from_lark_rejects_a_string_whose_explicit_values_cannot_fit():
    generator = gs.from_lark(
        "start: NUM\n%declare NUM\n",
        explicit={"NUM": gs.text(min_size=20)},
        max_size=10,
    )
    failing(lambda tc: tc.draw(generator), hegel.FailedHealthCheck, database="")


def test_from_lark_explicit_values_must_be_strings():
    generator = gs.from_lark("start: NUM\n%declare NUM\n", explicit={"NUM": gs.integers()})
    error = failing(lambda tc: tc.draw(generator), hegel.InvalidArgument, database="")
    assert "explicit terminal 'NUM' must produce strings" in str(error)


def test_from_lark_alphabet_constrains_pattern_terminals():
    check_all(gs.from_lark("start: /[a-z]+/", alphabet="xyz"), lambda s: set(s) <= set("xyz"))
    check_all(
        gs.from_lark("start: /[a-z]+/", alphabet=gs.characters(min_codepoint=ord("q"))),
        lambda s: min(s) >= "q",
    )


def test_from_lark_max_size_caps_the_length():
    check_all(gs.from_lark(INT_LIST, max_size=10), lambda s: len(s) <= 10)


def test_from_lark_max_depth_caps_the_nesting():
    check_all(
        gs.from_lark('start: "(" start ")" | "x"', max_depth=3),
        lambda s: s.count("(") <= 2,
    )


def test_from_lark_inserts_ignored_terminals():
    find_example(
        gs.from_lark('start: "a" "b"\n%import common.WS\n%ignore WS\n'),
        lambda s: s not in ("ab",),
    )


def test_from_lark_draws_inside_collections():
    check_all(
        gs.lists(gs.from_lark(INT_LIST, max_size=20), max_size=3),
        lambda xs: all(is_int_list(x) for x in xs),
    )


def test_from_lark_import_path_resolves_imports(tmp_path: Path):
    (tmp_path / "lib.lark").write_text("LETTERS: /[a-c]+/\n")
    (tmp_path / "pkg").mkdir()
    (tmp_path / "pkg" / "mod.lark").write_text("%import .lib2.DIGIT\nTOKEN: DIGIT DIGIT\n")
    (tmp_path / "pkg" / "lib2.lark").write_text("DIGIT: /[0-9]/\n")
    check_all(
        gs.from_lark("%import lib.LETTERS\nstart: LETTERS\n", import_path=tmp_path),
        lambda s: set(s) <= set("abc"),
    )
    check_all(
        gs.from_lark("%import pkg.mod.TOKEN\nstart: TOKEN\n", import_path=str(tmp_path)),
        lambda s: re.fullmatch("[0-9]{2}", s) is not None,
    )


def test_from_lark_falls_back_to_the_bundled_grammars(tmp_path: Path):
    check_all(
        gs.from_lark("%import common.DIGIT\nstart: DIGIT\n", import_path=tmp_path),
        lambda s: s.isdigit(),
    )


def test_from_lark_missing_import_is_an_error():
    with pytest.raises(hegel.InvalidArgument):
        gs.from_lark("%import nowhere.X\nstart: X\n")


def test_from_lark_unreadable_import_is_an_error(tmp_path: Path):
    (tmp_path / "lib.lark").mkdir()
    with pytest.raises(hegel.InvalidArgument, match="Could not read grammar"):
        gs.from_lark("%import lib.X\nstart: X\n", import_path=tmp_path)
    (tmp_path / "bad.lark").write_bytes(b"\xff\xfe")
    with pytest.raises(hegel.InvalidArgument, match="Could not read grammar"):
        gs.from_lark("%import bad.X\nstart: X\n", import_path=tmp_path)


def test_from_lark_import_under_a_file_is_missing(tmp_path: Path):
    (tmp_path / "pkg").write_text("not a directory")
    with pytest.raises(hegel.InvalidArgument):
        gs.from_lark("%import pkg.mod.X\nstart: X\n", import_path=tmp_path)


@pytest.mark.parametrize(
    ("make", "message"),
    [
        (lambda: gs.from_lark(42), "must be a string in Lark's grammar syntax"),
        (lambda: gs.from_lark('start: "a"', start=1), "must be a rule name"),
        (lambda: gs.from_lark('start: "a"', explicit={1: gs.text()}), "must be a string"),
        (
            lambda: gs.from_lark('start: "a"', explicit={"A": "a"}),
            "must be a generator of strings",
        ),
        (lambda: gs.from_lark('start: "a"', import_path=3), "must be a directory path"),
        (lambda: gs.from_lark('start: "a"', max_size=0), "must be a positive integer"),
        (lambda: gs.from_lark('start: "a"', max_depth=True), "must be a positive integer"),
        (lambda: gs.from_lark('start: "a"', max_depth="2"), "must be a positive integer"),
        (lambda: gs.from_lark('start: "a"', max_depth=1 << 40), "must be less than 2**32"),
        (lambda: gs.from_lark('start: "a"', max_size=1 << 70), "must be less than 2**64"),
        (lambda: gs.from_lark('start: "a"', alphabet=""), "must not be empty"),
        (lambda: gs.from_lark('start: "a"', alphabet=gs.just("a")), "string of characters"),
        (lambda: gs.from_lark("start: ("), None),
        (lambda: gs.from_lark('start: "a"', start="nope"), None),
        (lambda: gs.from_lark("start: NUM\n%declare NUM\n"), None),
        (lambda: gs.from_lark('start: "abc"', max_size=2), None),
    ],
)
def test_from_lark_invalid_arguments(make, message):
    with pytest.raises(
        hegel.InvalidArgument, match=None if message is None else re.escape(message)
    ):
        make()


def test_from_lark_labels_distinguish_grammars_start_rules_and_terminals():
    grammar = "start: NUM\n%declare NUM\nother: NUM\n"
    a = gs.from_lark(grammar, explicit={"NUM": gs.text()})
    assert a.label == gs.from_lark(grammar, explicit={"NUM": gs.text()}).label
    assert a.label != gs.from_lark(grammar, explicit={"NUM": gs.from_regex("[0-9]+")}).label
    assert a.label != gs.from_lark(grammar, start="other", explicit={"NUM": gs.text()}).label
    assert gs.from_lark('start: "a"').label != gs.from_lark('start: "b"').label


def test_from_lark_repr():
    assert repr(gs.from_lark('start: "a"')) == "from_lark('start: \"a\"')"
    assert (
        repr(
            gs.from_lark(
                'a: "a"\nb: NUM\n%declare NUM\n', start="b", explicit={"NUM": gs.just("1")}
            )
        )
        == "from_lark('a: \"a\"\\nb: NUM\\n%declare NUM\\n', start='b', "
        "explicit={'NUM': just('1')})"
    )


def test_from_lark_prints_the_drawn_string():
    def body(tc):
        tc.draw(gs.from_lark('start: "ab"'), name="s")
        raise AssertionError

    error = failing(body, database="")
    assert notes(error)[0].endswith(" = 'ab'")
