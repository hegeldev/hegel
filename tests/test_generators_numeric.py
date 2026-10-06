from __future__ import annotations

import math
import struct

import pytest

import hegel
from hegel import generators as gs
from hegel.generators.numeric import UNBOUNDED_MAGNITUDE, Booleans, Integers
from tests.helpers import failing, notes, run_test


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), value

    run_test(body, test_cases=test_cases)


def test_integers_bounds_and_repr():
    check_all(gs.integers(-5, 5), lambda x: -5 <= x <= 5 and isinstance(x, int))
    check_all(gs.integers(min_value=10), lambda x: x >= 10)
    check_all(gs.integers(max_value=-10), lambda x: x <= -10)
    check_all(gs.integers(), lambda x: abs(x) <= UNBOUNDED_MAGNITUDE)
    check_all(gs.integers(2.0, 3.0), lambda x: x in (2, 3))
    assert repr(gs.integers()) == "integers()"
    assert repr(gs.integers(1, 2)) == "integers(min_value=1, max_value=2)"
    assert repr(gs.integers(max_value=2)) == "integers(max_value=2)"
    with pytest.raises(hegel.InvalidArgument):
        gs.integers(3, 2)
    with pytest.raises(hegel.InvalidArgument):
        gs.integers(1.5)
    with pytest.raises(hegel.InvalidArgument):
        gs.integers(max_value=True)
    with pytest.raises(hegel.InvalidArgument):
        gs.integers("1")


def test_integer_bounds_beyond_unbounded_magnitude():
    huge = UNBOUNDED_MAGNITUDE * 4
    assert Integers(None, -huge)._lo == -huge
    assert Integers(huge, None)._hi == huge
    check_all(gs.integers(min_value=huge), lambda x: x >= huge)
    check_all(gs.integers(max_value=-huge), lambda x: x <= -huge)
    check_all(gs.integers(-(10**40), 10**40), lambda x: abs(x) <= 10**40)


def test_integers_shrink_toward_zero_and_bounds():
    assert hegel.find_example(gs.integers(), lambda x: x < -10) == -11
    assert hegel.find_example(gs.integers(5, 100)) == 5
    assert hegel.find_example(gs.integers(-100, -5)) == -5
    assert hegel.find_example(gs.integers(2**70, 2**71)) == 2**70


def test_booleans():
    check_all(gs.booleans(), lambda b: isinstance(b, bool))
    check_all(gs.weighted_booleans(1.0), lambda b: b is True)
    check_all(gs.weighted_booleans(0), lambda b: b is False)
    assert repr(gs.booleans()) == "booleans()"
    assert repr(gs.weighted_booleans(0.25)) == "weighted_booleans(0.25)"
    assert hegel.find_example(gs.booleans()) is False
    assert hegel.find_example(gs.booleans(), lambda b: b) is True
    with pytest.raises(hegel.InvalidArgument):
        Booleans(1.5)


def test_floats_default_allow_everything():
    seen = set()

    def body(tc):
        x = tc.draw(gs.floats())
        seen.add("nan" if math.isnan(x) else "inf" if math.isinf(x) else "finite")

    run_test(body, test_cases=500)
    assert "finite" in seen


def test_floats_bounded():
    check_all(gs.floats(0.0, 1.0), lambda x: 0.0 <= x <= 1.0)
    check_all(gs.floats(min_value=1e300), lambda x: x >= 1e300)
    check_all(gs.floats(max_value=-1e300), lambda x: x <= -1e300)
    check_all(gs.floats(0.0, 1.0, exclude_min=True, exclude_max=True), lambda x: 0.0 < x < 1.0)
    check_all(gs.floats(allow_nan=False), lambda x: not math.isnan(x))
    check_all(gs.floats(allow_nan=False, allow_infinity=False), lambda x: math.isfinite(x))
    check_all(gs.floats(1, 2), lambda x: 1 <= x <= 2)
    check_all(
        gs.floats(allow_subnormal=False),
        lambda x: x == 0 or math.isnan(x) or abs(x) >= 2.2250738585072014e-308,
    )
    check_all(gs.floats(min_value=0.0, allow_infinity=True), lambda x: x >= 0.0)


def test_floats_width_32():
    def representable(x):
        return math.isnan(x) or struct.unpack("<f", struct.pack("<f", x))[0] == x

    check_all(gs.floats(width=32), representable)
    check_all(gs.floats(0.0, 1.0, width=32), lambda x: 0.0 <= x <= 1.0 and representable(x))
    check_all(gs.floats(min_value=0.5, width=32), lambda x: x >= 0.5)
    check_all(gs.floats(max_value=-0.5, width=32), lambda x: x <= -0.5)
    assert repr(gs.floats(width=32)) == "floats(width=32)"
    assert repr(gs.floats(0.0, 1.0)) == "floats(min_value=0.0, max_value=1.0)"
    assert repr(gs.floats(max_value=1.0)) == "floats(max_value=1.0)"


def test_floats_shrink_to_simple_values():
    assert hegel.find_example(gs.floats(), lambda x: x > 1.5) == 2.0
    assert hegel.find_example(gs.floats(allow_nan=False, allow_infinity=False)) == 0.0
    assert math.isnan(hegel.find_example(gs.floats(), math.isnan))


@pytest.mark.parametrize(
    "kwargs",
    [
        dict(min_value=2.0, max_value=1.0),
        dict(min_value=0.0, max_value=-0.0),
        dict(min_value=1.0, max_value=1.0, exclude_min=True),
        dict(exclude_min=True),
        dict(exclude_max=True),
        dict(min_value=math.inf, exclude_min=True),
        dict(max_value=-math.inf, exclude_max=True),
        dict(min_value=0.0, allow_nan=True),
        dict(min_value=0.0, max_value=1.0, allow_infinity=True),
        dict(min_value=1.0, allow_subnormal=True),
        dict(max_value=-1.0, allow_subnormal=True),
        dict(min_value=1e-310, max_value=2e-310, allow_subnormal=False),
        dict(width=16),
        dict(min_value=math.nan),
        dict(min_value="x"),
        dict(min_value=0.1, width=32),
    ],
)
def test_floats_invalid_arguments(kwargs):
    with pytest.raises(hegel.InvalidArgument):
        gs.floats(**kwargs)


def test_floats_subnormal_defaults():
    assert gs.floats(1e-310, 1e-310).allow_subnormal is True
    assert gs.floats(1.0, 1.0).allow_subnormal is False
    assert gs.floats(-1.0, 1.0).allow_subnormal is True
    assert gs.floats(1.0, 2.0).allow_subnormal is False
    assert gs.floats(min_value=1.0).allow_subnormal is False
    assert gs.floats(max_value=-1.0).allow_subnormal is False
    assert gs.floats(max_value=1.0).allow_subnormal is True
    assert gs.floats(-1.0, 1.0, allow_subnormal=False).allow_subnormal is False
    assert gs.floats(min_value=math.inf).allow_infinity is True
    check_all(gs.floats(min_value=math.inf), lambda x: x == math.inf)


def test_floats_accept_integers_and_decimals():
    import decimal

    assert gs.floats(decimal.Decimal("0.5"), 1).min_value == 0.5
    check_all(gs.floats(0, 1), lambda x: 0 <= x <= 1)


def test_failure_reports_floats():
    def body(tc):
        x = tc.draw(gs.floats(allow_nan=False, allow_infinity=False))
        assert x <= 100

    assert "x = " in notes(failing(body))[0]
