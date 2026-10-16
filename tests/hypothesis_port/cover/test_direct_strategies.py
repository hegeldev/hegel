# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_fraction_is_in_bounds,
# test_fraction_gt_positive, test_fraction_lt_negative, test_fractions
# (fractions()); test_decimal_is_in_bounds, test_decimals, test_non_float_decimal
# (decimals()); test_incompatible_shared_strategies_warns,
# test_compatible_shared_strategies_do_not_warn,
# test_compatible_nested_shared_strategies_do_not_warn (shared());
# test_builds_error_messages (Hypothesis's builds(Enum) hint and
# InvalidArgument-is-a-TypeError). Parametrised rows for complex_numbers(),
# decimals(), fractions(), slices(), randoms(), the whitelist_*/blacklist_*
# aliases of characters(), and fixed_dictionaries(optional=OrderedDict())
# (mapping/optional type matching) are dropped from the tables, as are
# lists/text with min_size=100_000 (the 8KB buffer limit), text(alphabet="")
# (an empty alphabet), characters(codec="latin1") (codec aliases) and
# from_regex(alphabet=sampled_from("abc")) (generator alphabets for from_regex).

import collections
import enum
import math
from datetime import date, datetime, time, timedelta
from ipaddress import IPv4Network, IPv6Network

import pytest

import hegel
from hegel import generators as gs
from hegel.errors import InvalidArgument
from hegel.pretty import pretty_repr
from tests.hypothesis_port.common.debug import check_can_generate_examples, minimal

# Use `pretty` instead of `repr` for building test names, so that set and dict
# parameters print consistently across multiple worker processes with different
# PYTHONHASHSEED values.


def pretty(value):
    # Functions print by name, so test ids are stable across workers.
    if callable(value) and not isinstance(value, type):
        return getattr(value, "__name__", None) or repr(value)
    return pretty_repr(value)


def _values(entry):
    # Entries may be pytest.param(...) objects carrying xfail marks.
    return getattr(entry, "values", entry)


def fn_test(*fnkwargs):
    fnkwargs = list(fnkwargs)
    return pytest.mark.parametrize(
        ("fn", "args"),
        fnkwargs,
        ids=[
            "{}({})".format(fn.__name__, ", ".join(map(pretty, args)))
            for fn, args in map(_values, fnkwargs)
        ],
    )


def fn_ktest(*fnkwargs):
    fnkwargs = list(fnkwargs)
    return pytest.mark.parametrize(
        ("fn", "kwargs"),
        fnkwargs,
        ids=[f"{fn.__name__}(**{pretty(kwargs)})" for fn, kwargs in map(_values, fnkwargs)],
    )


def accepted(reason):
    """Arguments Hypothesis rejects but Hegel accepts."""
    return pytest.mark.xfail(strict=True, reason=f"Hegel accepts this: {reason}")


@fn_ktest(
    (gs.integers, {"min_value": math.nan}),
    (gs.integers, {"min_value": 2, "max_value": 1}),
    (gs.integers, {"min_value": math.nan}),
    (gs.integers, {"max_value": math.nan}),
    (gs.integers, {"min_value": -1.5, "max_value": -0.5}),
    (gs.integers, {"min_value": 0.1, "max_value": 0.2}),
    (gs.dates, {"min_value": "fish"}),
    (gs.dates, {"max_value": "fish"}),
    (gs.dates, {"min_value": date(2017, 8, 22), "max_value": date(2017, 8, 21)}),
    (gs.dates, {"min_value": datetime(2017, 8, 21)}),
    (gs.dates, {"max_value": datetime(2017, 8, 21)}),
    (gs.datetimes, {"min_value": "fish"}),
    (gs.datetimes, {"max_value": "fish"}),
    (gs.datetimes, {"allow_imaginary": 0}),
    (
        gs.datetimes,
        {"min_value": datetime(2017, 8, 22), "max_value": datetime(2017, 8, 21)},
    ),
    (
        gs.dicts,
        {"keys": gs.booleans(), "values": gs.booleans(), "min_size": 10, "max_size": 1},
    ),
    (gs.floats, {"min_value": math.nan}),
    (gs.floats, {"max_value": math.nan}),
    (gs.floats, {"min_value": complex(1, 2)}),
    (gs.floats, {"max_value": complex(1, 2)}),
    (gs.floats, {"exclude_min": None}),
    (gs.floats, {"exclude_max": None}),
    (gs.floats, {"exclude_min": True}),  # because min_value=None
    (gs.floats, {"exclude_max": True}),  # because max_value=None
    (gs.floats, {"min_value": 1.8, "width": 32}),
    (gs.floats, {"max_value": 1.8, "width": 32}),
    (gs.lists, {"elements": gs.integers(), "min_size": 10, "max_size": 9}),
    (gs.lists, {"elements": gs.integers(), "min_size": -10, "max_size": -9}),
    (gs.lists, {"elements": gs.integers(), "max_size": -9}),
    (gs.lists, {"elements": gs.integers(), "min_size": -10}),
    (gs.lists, {"elements": gs.integers(), "min_size": math.nan}),
    pytest.param(
        gs.lists,
        {"elements": gs.nothing(), "max_size": 1},
        marks=accepted("lists(nothing(), max_size=1) generates empty lists"),
    ),
    (gs.lists, {"elements": "hi"}),
    (gs.lists, {"elements": gs.integers(), "unique_by": 1}),
    (gs.lists, {"elements": gs.integers(), "unique_by": ()}),
    (gs.lists, {"elements": gs.integers(), "unique_by": (1,)}),
    (gs.lists, {"elements": gs.sampled_from([0, 1]), "min_size": 3, "unique": True}),
    (gs.lists, {"elements": gs.sampled_from([1, 2]), "min_size": 100_000, "unique": True}),
    (gs.text, {"min_size": 10, "max_size": 9}),
    (gs.text, {"alphabet": [1]}),
    (gs.text, {"alphabet": ["abc"]}),
    (gs.text, {"alphabet": gs.just("abc")}),
    (gs.text, {"alphabet": gs.sampled_from(["abc", "def"])}),
    (gs.text, {"alphabet": gs.just(123)}),
    (gs.text, {"alphabet": gs.sampled_from([123, 456])}),
    (gs.text, {"alphabet": gs.builds(lambda: "abc")}),
    (gs.text, {"alphabet": gs.builds(lambda: 123)}),
    (gs.from_regex, {"regex": 123}),
    (gs.from_regex, {"regex": b"abc", "alphabet": "abc"}),
    (gs.from_regex, {"regex": b"abc", "alphabet": b"def"}),
    (gs.from_regex, {"regex": "abc", "alphabet": "def"}),
    (gs.from_regex, {"regex": "aa|bb", "alphabet": "c"}),
    (gs.from_regex, {"regex": "[abc]", "alphabet": "def"}),
    (gs.from_regex, {"regex": "[ab]x[de]", "alphabet": "abcdef"}),
    (gs.from_regex, {"regex": "...", "alphabet": gs.builds(lambda: "a")}),
    (gs.from_regex, {"regex": "abc", "alphabet": gs.sampled_from("def")}),
    (gs.from_regex, {"regex": "abc", "alphabet": gs.characters(min_codepoint=128)}),
    (gs.from_regex, {"regex": "abc", "alphabet": 123}),
    (gs.from_regex, {"regex": "abc", "alphabet": [1]}),
    (gs.from_regex, {"regex": "abc", "alphabet": ["abc"]}),
    (gs.binary, {"min_size": 10, "max_size": 9}),
    (gs.floats, {"min_value": math.nan}),
    (gs.floats, {"min_value": "0"}),
    (gs.floats, {"max_value": "0"}),
    (gs.floats, {"min_value": 0.0, "max_value": -0.0}),
    (gs.floats, {"min_value": 0.0, "max_value": 1.0, "allow_infinity": True}),
    (gs.floats, {"max_value": 0.0, "min_value": 1.0}),
    (gs.floats, {"min_value": 0.0, "allow_nan": True}),
    (gs.floats, {"max_value": 0.0, "allow_nan": True}),
    (gs.floats, {"min_value": 0.0, "max_value": 1.0, "allow_infinity": True}),
    (gs.floats, {"min_value": math.inf, "allow_infinity": False}),
    (gs.floats, {"max_value": -math.inf, "allow_infinity": False}),
    (gs.fixed_dicts, {"mapping": "fish"}),
    (gs.fixed_dicts, {"mapping": {1: "fish"}}),
    (gs.fixed_dicts, {"mapping": {}, "optional": "fish"}),
    (gs.fixed_dicts, {"mapping": {}, "optional": {1: "fish"}}),
    (gs.fixed_dicts, {"mapping": {1: gs.none()}, "optional": {1: gs.none()}}),
    (gs.dicts, {"keys": gs.integers(), "values": 1}),
    (gs.dicts, {"keys": 1, "values": gs.integers()}),
    (gs.text, {"alphabet": "", "min_size": 1}),
    (gs.timedeltas, {"min_value": "fish"}),
    (gs.timedeltas, {"max_value": "fish"}),
    (
        gs.timedeltas,
        {"min_value": timedelta(hours=1), "max_value": timedelta(minutes=1)},
    ),
    (gs.times, {"min_value": "fish"}),
    (gs.times, {"max_value": "fish"}),
    (gs.times, {"min_value": time(2, 0), "max_value": time(1, 0)}),
    (gs.uuids, {"version": 6}),
    (gs.characters, {"min_codepoint": -1}),
    (gs.characters, {"min_codepoint": "1"}),
    (gs.characters, {"max_codepoint": -1}),
    (gs.characters, {"max_codepoint": "1"}),
    (gs.characters, {"categories": []}),
    (gs.characters, {"categories": ["Nd"], "exclude_categories": ["Nd"]}),
    (gs.characters, {"codec": 100}),
    (gs.characters, {"codec": "this is not a valid codec name"}),
    (gs.characters, {"codec": "ascii", "include_characters": "é"}),
    (gs.characters, {"codec": "utf-8", "categories": "Cs"}),
    (gs.sampled_from, {"elements": ()}),
    (gs.ip_addresses, {"v": "4"}),
    (gs.ip_addresses, {"v": 4.0}),
    (gs.ip_addresses, {"v": 5}),
    (gs.ip_addresses, {"v": 4, "network": "::/64"}),
    (gs.ip_addresses, {"v": 6, "network": "127.0.0.0/8"}),
    # only unicode strings are valid
    (gs.ip_addresses, {"network": b"127.0.0.0/8"}),
    (gs.ip_addresses, {"network": b"::/64"}),
)
def test_validates_keyword_arguments(fn, kwargs):
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(fn(**kwargs))


@fn_ktest(
    (gs.integers, {"min_value": 0}),
    (gs.integers, {"min_value": 11}),
    (gs.integers, {"min_value": 11, "max_value": 100}),
    (gs.integers, {"max_value": 0}),
    (gs.integers, {"min_value": -2, "max_value": -1}),
    (gs.lists, {"elements": gs.nothing(), "max_size": 0}),
    (gs.lists, {"elements": gs.integers()}),
    (gs.lists, {"elements": gs.integers(), "max_size": 5}),
    (gs.lists, {"elements": gs.booleans(), "min_size": 5}),
    (gs.lists, {"elements": gs.booleans(), "min_size": 5, "max_size": 10}),
    (gs.sets, {"min_size": 10, "max_size": 10, "elements": gs.integers()}),
    (gs.booleans, {}),
    (gs.just, {"value": "hi"}),
    (gs.integers, {"min_value": 12, "max_value": 12}),
    (gs.floats, {}),
    (gs.floats, {"min_value": 1.0}),
    (gs.floats, {"max_value": 1.0}),
    (gs.floats, {"min_value": math.inf}),
    (gs.floats, {"max_value": -math.inf}),
    (gs.floats, {"max_value": 1.0, "min_value": -1.0}),
    (gs.floats, {"max_value": 1.0, "min_value": -1.0, "allow_infinity": False}),
    (gs.floats, {"min_value": 1.0, "allow_nan": False}),
    (gs.floats, {"max_value": 1.0, "allow_nan": False}),
    (gs.floats, {"max_value": 1.0, "min_value": -1.0, "allow_nan": False}),
    (gs.sampled_from, {"elements": [1]}),
    (gs.sampled_from, {"elements": [1, 2, 3]}),
    (gs.fixed_dicts, {"mapping": {1: gs.integers()}}),
    (gs.dicts, {"keys": gs.booleans(), "values": gs.integers()}),
    (gs.text, {"alphabet": "abc"}),
    (gs.text, {"alphabet": set("abc")}),
    (gs.text, {"alphabet": gs.just("a")}),
    (gs.text, {"alphabet": gs.sampled_from("abc")}),
    (gs.text, {"alphabet": gs.builds(lambda: "a")}),
    (gs.characters, {"codec": "ascii"}),
    (gs.characters, {"categories": ["N"]}),
    (gs.characters, {"exclude_categories": []}),
    (gs.characters, {"include_characters": "a", "codec": "ascii"}),
    (gs.characters, {"exclude_characters": "a"}),
    (gs.characters, {"categories": ["Nd"]}),
    (gs.characters, {"exclude_categories": ["Nd"]}),
    (gs.from_regex, {"regex": "abc", "alphabet": "abc"}),
    (gs.from_regex, {"regex": "abc", "alphabet": "abcdef"}),
    (gs.from_regex, {"regex": "[abc]", "alphabet": "abcdef"}),
    (gs.from_regex, {"regex": "[a-f]", "alphabet": "abef"}),
    (gs.from_regex, {"regex": "[a-d]", "alphabet": "def"}),
    (gs.from_regex, {"regex": "[f-z]", "alphabet": "def"}),
    (gs.from_regex, {"regex": "abc", "alphabet": gs.characters(codec="ascii")}),
    (gs.from_regex, {"regex": "abc", "alphabet": ("a", "b", "c")}),
    (gs.from_regex, {"regex": "abc", "alphabet": ["a", "b", "c"]}),
    (gs.from_regex, {"regex": "abc", "alphabet": set("abc")}),
    (gs.ip_addresses, {}),
    (gs.ip_addresses, {"v": 4}),
    (gs.ip_addresses, {"v": 6}),
    (gs.ip_addresses, {"network": "127.0.0.0/8"}),
    (gs.ip_addresses, {"network": "::/64"}),
    (gs.ip_addresses, {"v": 4, "network": "127.0.0.0/8"}),
    (gs.ip_addresses, {"v": 6, "network": "::/64"}),
    (gs.ip_addresses, {"network": IPv4Network("127.0.0.0/8")}),
    (gs.ip_addresses, {"network": IPv6Network("::/64")}),
    (gs.ip_addresses, {"v": 4, "network": IPv4Network("127.0.0.0/8")}),
    (gs.ip_addresses, {"v": 6, "network": IPv6Network("::/64")}),
)
def test_produces_valid_examples_from_keyword(fn, kwargs):
    check_can_generate_examples(fn(**kwargs))


@fn_test(
    (gs.one_of, (1,)),
    (gs.one_of, (1, gs.integers())),
    (gs.tuples, (1,)),
)
def test_validates_args(fn, args):
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(fn(*args))


@fn_test(
    (gs.one_of, (gs.booleans(), gs.tuples(gs.booleans()))),
    (gs.one_of, (gs.booleans(),)),
    (gs.text, ()),
    (gs.binary, ()),
    (gs.builds, (lambda x, y: x + y, gs.integers(), gs.integers())),
)
def test_produces_valid_examples_from_args(fn, args):
    check_can_generate_examples(fn(*args))


def test_build_class_with_target_kwarg():
    NamedTupleWithTargetField = collections.namedtuple("Something", ["target"])
    check_can_generate_examples(gs.builds(NamedTupleWithTargetField, target=gs.integers()))


def test_builds_raises_with_no_target():
    with pytest.raises(TypeError):
        check_can_generate_examples(gs.builds())


@pytest.mark.parametrize("non_callable", [1, "abc", gs.integers()])
def test_builds_raises_if_non_callable_as_target_kwarg(non_callable):
    with pytest.raises(TypeError):
        check_can_generate_examples(gs.builds(target=non_callable))


@pytest.mark.parametrize("non_callable", [1, "abc", gs.integers()])
def test_builds_raises_if_non_callable_as_first_arg(non_callable):
    # If there are any positional arguments, then the target (which must be
    # callable) must be specified as the first one.
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.builds(non_callable, target=lambda x: x))


def test_tuples_raise_error_on_bad_kwargs():
    with pytest.raises(TypeError):
        gs.tuples(stuff="things")


@hegel.test
def test_has_specified_length(tc):
    xs = tc.draw(gs.lists(gs.booleans(), min_size=10, max_size=10))
    assert len(xs) == 10


@hegel.test(test_cases=100)
def test_has_upper_bound(tc):
    x = tc.draw(gs.integers(max_value=100))
    assert x <= 100


@hegel.test
def test_has_lower_bound(tc):
    x = tc.draw(gs.integers(min_value=100))
    assert x >= 100


@hegel.test
def test_is_in_bounds(tc):
    x = tc.draw(gs.integers(min_value=1, max_value=2))
    assert 1 <= x <= 2


def test_float_can_find_max_value_inf():
    assert minimal(gs.floats(max_value=math.inf), math.isinf) == float("inf")
    assert minimal(gs.floats(min_value=0.0), math.isinf) == math.inf


def test_float_can_find_min_value_inf():
    minimal(gs.floats(), lambda x: x < 0 and math.isinf(x))
    minimal(gs.floats(min_value=-math.inf, max_value=0.0), math.isinf)


def test_can_find_none_list():
    assert minimal(gs.lists(gs.none()), lambda x: len(x) >= 3) == [None] * 3


def test_produces_dictionaries_of_at_least_minimum_size():
    t = minimal(
        gs.dicts(gs.booleans(), gs.integers(), min_size=2),
    )
    assert t == {False: 0, True: 0}


@hegel.test(test_cases=50)
def test_dictionaries_respect_size(tc):
    d = tc.draw(gs.dicts(gs.integers(), gs.integers(), max_size=5))
    assert len(d) <= 5


@hegel.test(test_cases=50)
def test_dictionaries_respect_zero_size(tc):
    d = tc.draw(gs.dicts(gs.integers(), gs.integers(), max_size=0))
    assert len(d) <= 5


@hegel.test
def test_none_lists_respect_max_size(tc):
    ls = tc.draw(gs.lists(gs.none(), max_size=5))
    assert len(ls) <= 5


@hegel.test
def test_none_lists_respect_max_and_min_size(tc):
    ls = tc.draw(gs.lists(gs.none(), max_size=5, min_size=1))
    assert 1 <= len(ls) <= 5


@hegel.test
def test_iterables_are_exhaustible(tc):
    it = tc.draw(gs.iterables(gs.integers(), max_size=5, min_size=1))
    assert 1 <= len(list(it)) <= 5
    assert list(it) == []


def test_minimal_iterable():
    assert list(minimal(gs.iterables(gs.integers()))) == []


@pytest.mark.parametrize("parameter_name", ["min_value", "max_value"])
@pytest.mark.parametrize("value", [-1, 0, 1])
def test_no_infinity_for_min_max_values(value, parameter_name):
    kwargs = {"allow_infinity": False, parameter_name: value}

    @hegel.test
    def test_not_infinite(tc):
        xs = tc.draw(gs.floats(**kwargs))
        assert not math.isinf(xs)

    test_not_infinite()


@pytest.mark.parametrize("parameter_name", ["min_value", "max_value"])
@pytest.mark.parametrize("value", [-1, 0, 1])
def test_no_nan_for_min_max_values(value, parameter_name):
    kwargs = {"allow_nan": False, parameter_name: value}

    @hegel.test
    def test_not_nan(tc):
        xs = tc.draw(gs.floats(**kwargs))
        assert not math.isnan(xs)

    test_not_nan()


class Sneaky:
    """It's like a generator, but it's not a generator."""

    depth = 0
    label = 0

    def do_draw(self, tc):
        pass


@pytest.mark.parametrize("value", [5, Sneaky()])
@pytest.mark.parametrize("name", [None, "not a generator"])
@hegel.test
def test_draw_explicitly_rejects_non_generators(tc, value, name):
    with pytest.raises(InvalidArgument):
        tc.draw(value, name=name)


@hegel.test
def test_chained_filter(tc):
    x = tc.draw(gs.integers().filter(bool).filter(lambda x: x % 3))
    assert x
    assert x % 3


@pytest.mark.parametrize("version", [4, 6])
@hegel.test
def test_ipaddress_from_network_is_always_correct_version(tc, version):
    ip = tc.draw(gs.ip_addresses(v=version), name="address")
    assert ip.version == version


@hegel.test
def test_ipaddress_from_network_is_always_in_network(tc):
    network = tc.draw(gs.from_type(IPv4Network) | gs.from_type(IPv6Network))
    ip = tc.draw(gs.ip_addresses(network=network), name="address")
    assert ip in network
    assert ip.version == network.version


class AnEnum(enum.Enum):
    a = 1


@hegel.test
def test_can_sample_from_an_enum(tc):
    assert tc.draw(gs.sampled_from(AnEnum)) is AnEnum.a
