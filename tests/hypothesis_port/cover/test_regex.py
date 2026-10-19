# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original:
# test_internals_can_disable_newline_from_dollar_for_jsonschema (the private
# _temp_jsonschema_hack_no_end_newline flag); test_alphabet_codec_warning
# (HypothesisWarning); test_fullmatch_must_be_bool (argument type validation;
# hegel coerces fullmatch to bool); test_can_pass_union_for_alphabet (a
# generator as the from_regex alphabet; hegel takes a string of characters or
# characters()).
#
# Hegel's from_regex defaults to fullmatch=True where Hypothesis's defaults
# to False, so tests that relied on Hypothesis's default pass fullmatch=False.

import re
import string
import sys
import unicodedata

import pytest

import hegel
from hegel import InvalidArgument
from hegel import generators as gs
from tests.helpers import UNICODE_NEWER_THAN_PYTHON, skip_if_unicode_newer_than_python
from tests.hypothesis_port.common.compat import PYPY
from tests.hypothesis_port.common.debug import (
    assert_all_examples,
    assert_no_examples,
    check_can_generate_examples,
    find_any,
)

# Stand-ins for hypothesis.strategies._internal.regex: the character classes
# the regex module recognises.
SPACE_CHARS = set(" \t\n\r\f\v")
UNICODE_SPACE_CHARS = SPACE_CHARS | set("\x1c\x1d\x1e\x1f\x85")
UNICODE_DIGIT_CATEGORIES = {"Nd"}
UNICODE_SPACE_CATEGORIES = {"Zs", "Zl", "Zp"}
UNICODE_WORD_CATEGORIES = {"Lu", "Ll", "Lt", "Lm", "Lo", "Nd", "Nl", "No"}


def base_regex_strategy(regex, *, alphabet=None):
    return gs.from_regex(regex, fullmatch=True, alphabet=alphabet)


def is_ascii(s):
    return all(ord(c) < 128 for c in s)


def is_digit(s):
    return all(unicodedata.category(c) in UNICODE_DIGIT_CATEGORIES for c in s)


def is_ascii_digit(s):
    return all(c in string.digits for c in s)


def is_ascii_word(s):
    return all(c in string.ascii_letters + string.digits + "_" for c in s)


def is_space(s):
    return all(c in SPACE_CHARS for c in s)


def is_unicode_space(s):
    return all(
        unicodedata.category(c) in UNICODE_SPACE_CATEGORIES or c in UNICODE_SPACE_CHARS for c in s
    )


def is_word(s):
    return all(c == "_" or unicodedata.category(c) in UNICODE_WORD_CATEGORIES for c in s)


def ascii_regex(pattern):
    return re.compile(pattern, re.ASCII)


def unicode_regex(pattern):
    return re.compile(pattern, re.UNICODE)


def _test_matching_pattern(pattern, *, isvalidchar, is_unicode=False):
    r = unicode_regex(pattern) if is_unicode else ascii_regex(pattern)

    codepoints = range(sys.maxunicode + 1) if is_unicode else range(1, 128)
    for c in [chr(x) for x in codepoints]:
        if isvalidchar(c):
            msg = "%r supposed to match %r (%r, category %r), but it doesn't"
            assert r.search(c), msg % (pattern, c, c, unicodedata.category(c))
        else:
            assert not r.search(c), (
                f'"{pattern}" supposed not to match {c!r} (category '
                f"{unicodedata.category(c)!r}), but it does"
            )


@pytest.mark.parametrize("category,predicate", [(r"\w", is_word), (r"\d", is_digit), (r"\s", None)])
@pytest.mark.parametrize("invert", [False, True])
@pytest.mark.parametrize("is_unicode", [False, True])
def test_matching(category, predicate, invert, is_unicode):
    if predicate is None:
        # Special behaviour due to \x1c, INFORMATION SEPARATOR FOUR
        predicate = is_unicode_space if is_unicode else is_space
    if invert:
        category = category.swapcase()

        def pred(s):
            return not predicate(s)

    else:
        pred = predicate

    _test_matching_pattern(category, isvalidchar=pred, is_unicode=is_unicode)


@pytest.mark.parametrize(
    "pattern",
    [
        ".",  # anything
        "a",
        "abc",
        "[a][b][c]",
        "[^a][^b][^c]",  # literals
        "[a-z0-9_]",
        "[^a-z0-9_]",  # range and negative range
        "ab?",
        "ab*",
        "ab+",  # quantifiers
        "ab{5}",
        "ab{5,10}",
        "ab{,10}",
        "ab{5,}",  # repeaters
        "ab|cd|ef",  # branch
        "(foo)+",
        "(['\"])[a-z]+\\1",
        "(?:[a-z])(['\"])[a-z]+\\1",
        "(?P<foo>['\"])[a-z]+(?P=foo)",  # groups
        "^abc",  # beginning
        "\\d",
        "[\\d]",
        "[^\\D]",
        "\\w",
        "[\\w]",
        "[^\\W]",
        "\\s",
        "[\\s]",
        "[^\\S]",  # categories
    ],
)
@pytest.mark.parametrize("encode", [None, False, True])
def test_can_generate(pattern, encode):
    alphabet = gs.characters(max_codepoint=1000) if encode is None else None
    if encode:
        pattern = pattern.encode("ascii")
    assert_all_examples(
        gs.from_regex(pattern, fullmatch=False, alphabet=alphabet),
        re.compile(pattern).search,
    )


@pytest.mark.parametrize(
    "pattern",
    [
        re.compile("\\Aa\\Z", re.IGNORECASE),
        "(?i)\\Aa\\Z",
        re.compile("\\A[ab]\\Z", re.IGNORECASE),
        "(?i)\\A[ab]\\Z",
    ],
)
def test_literals_with_ignorecase(pattern):
    strategy = gs.from_regex(pattern, fullmatch=False)

    find_any(strategy, lambda s: s == "a")
    find_any(strategy, lambda s: s == "A")


@pytest.mark.parametrize(
    "pattern", [re.compile("\\A[^a][^b]\\Z", re.IGNORECASE), "(?i)\\A[^a][^b]\\Z"]
)
def test_not_literal_with_ignorecase(pattern):
    assert_all_examples(
        gs.from_regex(pattern, fullmatch=False),
        lambda s: s[0] not in ("a", "A") and s[1] not in ("b", "B"),
    )


def test_any_doesnt_generate_newline():
    assert_all_examples(gs.from_regex("\\A.\\Z", fullmatch=False), lambda s: s != "\n")


@pytest.mark.parametrize("pattern", [re.compile("\\A.\\Z", re.DOTALL), "(?s)\\A.\\Z"])
def test_any_with_dotall_generate_newline(pattern):
    find_any(gs.from_regex(pattern, fullmatch=False), lambda s: s == "\n", test_cases=10**6)


@pytest.mark.parametrize("pattern", [re.compile(b"\\A.\\Z", re.DOTALL), b"(?s)\\A.\\Z"])
def test_any_with_dotall_generate_newline_binary(pattern):
    find_any(gs.from_regex(pattern, fullmatch=False), lambda s: s == b"\n", test_cases=10**6)


@pytest.mark.parametrize(
    "pattern",
    ["\\d", "[\\d]", "[^\\D]", "\\w", "[\\w]", "[^\\W]", "\\s", "[\\s]", "[^\\S]"],
)
@pytest.mark.parametrize("is_unicode", [False, True])
@pytest.mark.parametrize("invert", [False, True])
def test_groups(pattern, is_unicode, invert):
    if is_unicode and "s" not in pattern.lower():
        # Unicode \w and \d draw letters and digits this Python may not know.
        skip_if_unicode_newer_than_python()
    # Under re.ASCII, the positive categories \d \s \w match only ascii
    # characters, and the negative categories \D \S \W match everything else.
    if "d" in pattern.lower():
        group_pred = is_digit if is_unicode else is_ascii_digit
    elif "w" in pattern.lower():
        group_pred = is_word if is_unicode else is_ascii_word
    else:
        # Special behaviour due to \x1c, INFORMATION SEPARATOR FOUR
        group_pred = is_unicode_space if is_unicode else is_space

    if invert:
        pattern = pattern.swapcase()
        _p = group_pred

        def group_pred(s):
            return not _p(s)

    pattern = f"^{pattern}\\Z"

    compiler = unicode_regex if is_unicode else ascii_regex
    strategy = gs.from_regex(compiler(pattern), fullmatch=False)

    find_any(strategy.filter(group_pred), is_ascii)
    if is_unicode:
        find_any(strategy, lambda s: group_pred(s) and not is_ascii(s))

    assert_all_examples(strategy, group_pred)


def test_caret_in_the_middle_does_not_generate_anything():
    r = re.compile("a^b")

    assert_no_examples(gs.from_regex(r, fullmatch=False))


def test_end_with_terminator_does_not_pad():
    assert_all_examples(gs.from_regex("abc\\Z", fullmatch=False), lambda x: x[-3:] == "abc")


def test_end():
    strategy = gs.from_regex("\\Aabc$", fullmatch=False)

    find_any(strategy, lambda s: s == "abc")
    find_any(strategy, lambda s: s == "abc\n")


def test_groupref_exists():
    assert_all_examples(
        gs.from_regex("^(<)?a(?(1)>)$", fullmatch=False),
        lambda s: s in ("a", "a\n", "<a>", "<a>\n"),
    )
    assert_all_examples(
        gs.from_regex("^(a)?(?(1)b|c)$", fullmatch=False),
        lambda s: s in ("ab", "ab\n", "c", "c\n"),
    )


def test_impossible_negative_lookahead():
    assert_no_examples(gs.from_regex("(?!foo)foo", fullmatch=False))


@hegel.test
def test_can_handle_boundaries_nested(tc):
    s = tc.draw(gs.from_regex("(\\Afoo\\Z)", fullmatch=False))
    assert s == "foo"


def test_groupref_not_shared_between_regex():
    # If group references are (incorrectly!) shared between regex, this would
    # fail as the would only be one reference.
    check_can_generate_examples(
        gs.tuples(
            gs.from_regex("(a)\\1", fullmatch=False), gs.from_regex("(b)\\1", fullmatch=False)
        )
    )


@pytest.mark.skipif(
    PYPY,  # Skip for now so we can test the rest
    reason=r"Triggers bugs in poor handling of unicode in re for these implementations",
)
@hegel.test
def test_group_ref_is_not_shared_between_identical_regex(tc):
    pattern = re.compile("^(.+)\\1\\Z", re.UNICODE)
    x = tc.draw(base_regex_strategy(pattern, alphabet=gs.characters()))
    y = tc.draw(base_regex_strategy(pattern, alphabet=gs.characters()))
    tc.assume(x != y)
    assert pattern.match(x).end() == len(x)
    assert pattern.match(y).end() == len(y)


@hegel.test
def test_does_not_leak_groups(tc):
    a = tc.draw(base_regex_strategy(re.compile("^(a)\\Z"), alphabet=gs.characters()))
    assert a == "a"
    b = tc.draw(base_regex_strategy(re.compile("^(?(1)a|b)(.)\\Z"), alphabet=gs.characters()))
    assert b[0] == "b"


def test_positive_lookbehind():
    find_any(gs.from_regex(".*(?<=ab)c", fullmatch=False), lambda s: s.endswith("abc"))


def test_positive_lookahead():
    find_any(gs.from_regex("a(?=bc).*", fullmatch=False), lambda s: s.startswith("abc"))


def test_negative_lookbehind():
    # no efficient support
    pattern = "[abc]*(?<!abc)d"
    strategy = gs.from_regex(pattern, fullmatch=False)

    # Without fullmatch the string is padded around the match, and Hegel's
    # padding can end in "abcd" ('dabcd' matches at its first character), so
    # the lookbehind is checked on the match itself.
    def match(s):
        return re.search(pattern, s).group()

    assert_all_examples(strategy, lambda s: not match(s).endswith("abcd"))
    assert_no_examples(strategy, lambda s: match(s).endswith("abcd"))


def test_negative_lookahead():
    # no efficient support
    strategy = gs.from_regex("^ab(?!cd)[abcd]*", fullmatch=False)

    assert_all_examples(strategy, lambda s: not s.startswith("abcd"))
    assert_no_examples(strategy, lambda s: s.startswith("abcd"))


@hegel.test
def test_generates_only_the_provided_characters_given_boundaries(tc):
    xs = tc.draw(gs.from_regex("^a+\\Z", fullmatch=False))
    assert set(xs) == {"a"}


@hegel.test
def test_group_backref_may_not_be_present(tc):
    s = tc.draw(gs.from_regex("^(.)?\\1\\Z", fullmatch=False))
    assert len(s) == 2
    assert s[0] == s[1]


def test_subpattern_flags():
    strategy = gs.from_regex("(?i)\\Aa(?-i:b)\\Z", fullmatch=False)

    # "a" is case insensitive
    find_any(strategy, lambda s: s[0] == "a")
    find_any(strategy, lambda s: s[0] == "A")
    # "b" is case sensitive
    find_any(strategy, lambda s: s[1] == "b")

    assert_no_examples(strategy, lambda s: s[1] == "B")


def test_can_handle_binary_regex_which_is_not_ascii():
    bad = b"\xad"
    assert_all_examples(gs.from_regex(bad, fullmatch=False), lambda x: bad in x)


@pytest.mark.parametrize("pattern", [b".", "."])
def test_regex_have_same_type_as_pattern(pattern):
    @hegel.test
    def test_result_type(tc):
        s = tc.draw(gs.from_regex(pattern, fullmatch=False))
        assert type(s) is type(pattern)

    test_result_type()


def test_can_pad_strings_arbitrarily():
    find_any(gs.from_regex("a", fullmatch=False), lambda x: x[0] != "a")
    find_any(gs.from_regex("a", fullmatch=False), lambda x: x[-1] != "a")


def test_can_pad_empty_strings():
    find_any(gs.from_regex("", fullmatch=False), bool)
    find_any(gs.from_regex(b"", fullmatch=False), bool)


def test_can_pad_strings_with_newlines():
    find_any(gs.from_regex("^$", fullmatch=False), bool)
    find_any(gs.from_regex(b"^$", fullmatch=False), bool)


def test_given_multiline_regex_can_insert_after_dollar():
    find_any(
        gs.from_regex(re.compile("\\Ahi$", re.MULTILINE), fullmatch=False),
        lambda x: "\n" in x and x.split("\n")[1],
    )


def test_given_multiline_regex_can_insert_before_caret():
    find_any(
        gs.from_regex(re.compile("^hi\\Z", re.MULTILINE), fullmatch=False),
        lambda x: "\n" in x and x.split("\n")[0],
    )


def test_does_not_left_pad_beginning_of_string_marker():
    assert_all_examples(gs.from_regex("\\Afoo", fullmatch=False), lambda x: x.startswith("foo"))


def test_bare_caret_can_produce():
    find_any(gs.from_regex("^", fullmatch=False), bool)


def test_bare_dollar_can_produce():
    find_any(gs.from_regex("$", fullmatch=False), bool)


def test_shared_union():
    # This gets parsed as [(ANY, None), (BRANCH, (None, [[], []]))], the
    # interesting feature of which is that it contains empty sub-expressions
    # in the branch.
    find_any(gs.from_regex(".|.", fullmatch=False))


@hegel.test
def test_issue_992_regression(tc):
    strat = gs.from_regex(
        re.compile(
            r"""\d +  # the integral part
            \.    # the decimal point
            \d *  # some fractional digits""",
            re.VERBOSE,
        ),
        fullmatch=False,
    )
    tc.draw(strat)


@pytest.mark.parametrize(
    "pattern,matching_str",
    [
        ("a", "a"),
        ("[Aa]", "A"),
        ("[ab]*", "abb"),
        (b"[Aa]", b"A"),
        (b"[ab]*", b"abb"),
        (re.compile("[ab]*", re.IGNORECASE), "aBb"),
        (re.compile(b"[ab]", re.IGNORECASE), b"A"),
    ],
)
def test_fullmatch_generates_example(pattern, matching_str):
    # A particular three-character string with a particular case pattern
    # is about one draw in six hundred, so the seeded search needs a
    # budget to match (Hypothesis's own minimal() allows fifty thousand).
    find_any(
        gs.from_regex(pattern, fullmatch=True),
        lambda s: s == matching_str,
        test_cases=5000,
    )


@pytest.mark.parametrize(
    "pattern,eqiv_pattern",
    [
        (r"", r"\A\Z"),
        (b"", rb"\A\Z"),
        (r"(?#comment)", r"\A\Z"),
        (rb"(?#comment)", rb"\A\Z"),
        ("a", "\\Aa\\Z"),
        ("[Aa]", "\\A[Aa]\\Z"),
        ("[ab]*", "\\A[ab]*\\Z"),
        (b"[Aa]", rb"\A[Aa]\Z"),
        (b"[ab]*", rb"\A[ab]*\Z"),
        (re.compile("[ab]*", re.IGNORECASE), re.compile("\\A[ab]*\\Z", re.IGNORECASE)),
        (re.compile(rb"[ab]", re.IGNORECASE), re.compile(rb"\A[ab]\Z", re.IGNORECASE)),
    ],
)
def test_fullmatch_matches(pattern, eqiv_pattern):
    assert_all_examples(gs.from_regex(pattern, fullmatch=True), lambda s: re.match(eqiv_pattern, s))


def test_issue_1786_regression():
    check_can_generate_examples(
        gs.from_regex(re.compile("\\\\", flags=re.IGNORECASE), fullmatch=False)
    )


def test_sets_allow_multichar_output_in_ignorecase_mode():
    # CharacterBuilder.strategy includes logic to add multi-character strings
    # via sampled_from(), if any are whitelisted as matching.  See issue #2657.
    find_any(
        gs.from_regex(re.compile("[\u0130_]", re.IGNORECASE), fullmatch=False),
        lambda s: len(s) > 1,
    )


@pytest.mark.parametrize("alphabet", ["ab", ("a", "b"), ["a", "b"], {"a", "b"}])
def test_can_pass_collections_for_alphabet(alphabet):
    assert_all_examples(
        gs.from_regex(r".+", fullmatch=False, alphabet=alphabet), lambda s: set(s) <= {"a", "b"}
    )


@pytest.mark.parametrize(
    "pattern, alphabet",
    [
        (r"[^\d\D]", gs.characters()),
        ("[^abc]", "abc"),
        ("[\\d]", "abc"),
        ("\\d", "abc"),
        ("[^a]", "a"),
        (".", "\n"),
        ("[^a]+", "a"),
        ("[a-z]", "0"),
    ],
)
def test_charsets_which_match_nothing_in_the_alphabet(pattern, alphabet):
    # Hypothesis's message says "specified alphabet"; Hegel's names the alphabet.
    with pytest.raises(InvalidArgument, match="matches no string over the alphabet"):
        check_can_generate_examples(gs.from_regex(pattern, fullmatch=False, alphabet=alphabet))


def test_charset_matching_nothing_only_rules_out_its_own_branch():
    assert_all_examples(
        gs.from_regex("[^a]|a", alphabet="a", fullmatch=True),
        lambda s: s == "a",
    )


def test_zero_minimum_repeats_incompatible_with_alphabet():
    # a subpattern which is incompatible with the alphabet can still be
    # repeated zero times - see issue #4857
    strategy = gs.from_regex("a*b*", alphabet="a", fullmatch=True)
    find_any(strategy, lambda s: s == "")
    find_any(strategy, lambda s: s == "aa")
    assert_all_examples(strategy, lambda s: set(s) <= {"a"})
    assert_all_examples(gs.from_regex("ab?", alphabet="a", fullmatch=True), lambda s: s == "a")


def test_inline_flags_do_not_leak_out_of_an_incompatible_subpattern():
    # `(?i:Ā)` is incompatible with the alphabet, but the group's flags must not leak
    # onto the following `k`.
    alphabet = gs.characters(max_codepoint=127)
    regex = re.compile("(?i:\u0100)*k")
    assert_all_examples(base_regex_strategy(regex, alphabet=alphabet), regex.fullmatch)
    # and likewise for a group which turns a flag *off*
    regex = re.compile("(?-i:\u0100)*k", re.IGNORECASE)
    find_any(base_regex_strategy(regex, alphabet=alphabet), lambda s: s == "K")


def test_ascii_inverted_categories_match_all_of_unicode():
    # \D under re.ASCII matches everything except [0-9], rather than being
    # restricted to ascii characters - see issue #4858
    regex = re.compile(r"\D", re.ASCII)
    assert regex.fullmatch("٣")
    assert_all_examples(gs.from_regex(regex, alphabet="٣", fullmatch=True), lambda s: s == "٣")


@pytest.mark.parametrize(
    "pattern",
    [
        re.compile("[Ï-İ]", re.IGNORECASE | re.ASCII),
        re.compile("[^\\u0000-\\u00ff]", re.ASCII),
        re.compile("ï", re.IGNORECASE | re.ASCII),
    ],
)
def test_ascii_flag_does_not_restrict_characters_or_ranges(pattern):
    # re.ASCII changes what \d \s \w match and restricts casefolding to ascii
    # characters, but explicit characters and ranges are unaffected - see
    # issue #4858
    assert_all_examples(base_regex_strategy(pattern, alphabet=gs.characters()), pattern.fullmatch)


def test_ascii_characters_still_fold_under_ascii_flag():
    strategy = gs.from_regex(re.compile("i", re.IGNORECASE | re.ASCII), fullmatch=True)
    find_any(strategy, lambda s: s == "I")


def test_class_with_negative_category_and_positive_members():
    # [\W_] matches the union of \W and {"_"} - see issue #4859
    regex = re.compile(r"[\W_]")
    assert_all_examples(gs.from_regex(regex, alphabet="_", fullmatch=True), lambda s: s == "_")
    find_any(gs.from_regex(r"\A[\W_]\Z", fullmatch=False), lambda s: s == "_")


@UNICODE_NEWER_THAN_PYTHON
def test_negated_class_excludes_all_members():
    # [^\W_] matches the complement of the union: word characters except "_"
    regex = re.compile(r"[^\W_]")
    assert_all_examples(base_regex_strategy(regex, alphabet=gs.characters()), regex.fullmatch)


def test_negated_class_with_members_outside_alphabet():
    assert_all_examples(gs.from_regex("[^bc]", alphabet="a", fullmatch=True), lambda s: s == "a")


@pytest.mark.parametrize(
    "alphabet",
    [
        ("a", 1),
        ("ab",),
        gs.sampled_from(["a", 1]),
        gs.sampled_from(["ab"]),
        gs.sampled_from("ab").map(str.upper),
        gs.integers(),
        gs.just("a") | gs.integers(),
    ],
)
def test_invalid_alphabets(alphabet):
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.from_regex(r".", fullmatch=False, alphabet=alphabet))


def test_regex_output_should_print_as_string():
    @hegel.test(phases=[hegel.Phase.generate, hegel.Phase.shrink])
    def test(tc):
        s = tc.draw(gs.from_regex(r"..", fullmatch=True))  # noqa: F841 (names the draw)
        raise AssertionError

    with pytest.raises(AssertionError) as err:
        test()

    assert "s = '00'" in "\n".join(err.value.__notes__)


@pytest.mark.skipif(sys.version_info < (3, 14), reason="re gained the \\z anchor in Python 3.14")
@pytest.mark.xfail(
    strict=True,
    raises=InvalidArgument,
    reason="Engine: \\z, the end-of-string anchor re accepts from Python 3.14, is rejected "
    "as a bad escape (see docs/engine-issues.md)",
)
@hegel.test
def test_end_of_string_anchor_z(tc):
    s = tc.draw(gs.from_regex(r"a\z", fullmatch=False))
    assert re.search(r"a\z", s)
