"""Integer, float, and boolean generators."""

from __future__ import annotations

import math
import struct
from decimal import Decimal
from fractions import Fraction
from typing import TYPE_CHECKING

from hegel._labels import label_from_name
from hegel.errors import InvalidArgument
from hegel.generators.base import Generator, _PrintsValue

if TYPE_CHECKING:
    from hegel.test_case import TestCase

__all__ = ["booleans", "floats", "integers", "weighted_booleans"]

INTEGER_LABEL = label_from_name("hegel.integers")
FLOAT_LABEL = label_from_name("hegel.floats")
BOOLEAN_LABEL = label_from_name("hegel.booleans")

# The default span of an unbounded integer draw: wide enough for any value
# a test is likely to care about, and the engine still steers toward small
# magnitudes.
UNBOUNDED_MAGNITUDE = 1 << 127


def _check_int(name: str, value: object) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        if isinstance(value, float) and value.is_integer():
            return int(value)
        raise InvalidArgument(f"{name}={value!r} must be an integer")
    return value


class Integers(_PrintsValue[int]):
    def __init__(self, min_value: int | None, max_value: int | None) -> None:
        if min_value is not None and max_value is not None and min_value > max_value:
            raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
        self.min_value = min_value
        self.max_value = max_value
        # A missing bound is replaced by one UNBOUNDED_MAGNITUDE values beyond
        # the other bound (or beyond zero, when that is further).
        if min_value is None:
            lo = -UNBOUNDED_MAGNITUDE
            if max_value is not None:
                lo = min(lo, max_value - UNBOUNDED_MAGNITUDE + 1)
        else:
            lo = min_value
        if max_value is None:
            hi = UNBOUNDED_MAGNITUDE - 1
            if min_value is not None:
                hi = max(hi, min_value + UNBOUNDED_MAGNITUDE - 1)
        else:
            hi = max_value
        self._lo = lo
        self._hi = hi
        self.label = INTEGER_LABEL

    def do_draw(self, tc: TestCase) -> int:
        return tc._generate_integer(self._lo, self._hi)

    def __repr__(self) -> str:
        parts = []
        if self.min_value is not None:
            parts.append(f"min_value={self.min_value!r}")
        if self.max_value is not None:
            parts.append(f"max_value={self.max_value!r}")
        return f"integers({', '.join(parts)})"


def integers(min_value: int | None = None, max_value: int | None = None) -> Generator[int]:
    """Generate integers in ``[min_value, max_value]``, inclusive.

    Either bound may be omitted. Python integers are unbounded, but the
    values drawn are not: a missing bound is taken to be ``2**127`` values
    beyond the other bound, or beyond zero when that is further. Values
    shrink toward zero, or toward the bound nearest zero when zero is out
    of range.

    A bound that is not an integer (a ``float`` with an integral value is
    accepted) or ``min_value > max_value`` is an
    :class:`~hegel.InvalidArgument`.

    ::

        n = tc.draw(gs.integers(min_value=0, max_value=100))
    """
    if min_value is not None:
        min_value = _check_int("min_value", min_value)
    if max_value is not None:
        max_value = _check_int("max_value", max_value)
    return Integers(min_value, max_value)


class Booleans(_PrintsValue[bool]):
    def __init__(self, p: float = 0.5) -> None:
        if not (0.0 <= p <= 1.0):
            raise InvalidArgument(f"p={p!r} must be between 0 and 1")
        self.p = p
        self.label = BOOLEAN_LABEL

    def do_draw(self, tc: TestCase) -> bool:
        return tc._generate_boolean(self.p)

    def __repr__(self) -> str:
        return "booleans()" if self.p == 0.5 else f"weighted_booleans({self.p!r})"


def booleans() -> Generator[bool]:
    """Generate ``True`` or ``False``, shrinking toward ``False``."""
    return Booleans()


def weighted_booleans(p: float) -> Generator[bool]:
    """Generate ``True`` with probability ``p``, and ``False`` otherwise,
    shrinking toward ``False``. ``p`` outside ``[0, 1]`` is an
    :class:`~hegel.InvalidArgument`."""
    return Booleans(float(p))


def _sign_aware_lte(a: float, b: float) -> bool:
    if a == 0.0 and b == 0.0:
        return math.copysign(1.0, a) < 0 or math.copysign(1.0, b) > 0
    return a <= b


_SMALLEST_NORMAL = {64: struct.unpack("<d", (1 << 52).to_bytes(8, "little"))[0], 32: 2.0**-126}
_SMALLEST_SUBNORMAL = {64: 5e-324, 32: 2.0**-149}
_MAX = {
    64: struct.unpack("<d", b"\xff\xff\xff\xff\xff\xff\xef\x7f")[0],
    32: struct.unpack("<f", b"\xff\xff\x7f\x7f")[0],
}
_FLOAT_FORMATS = {64: ("<d", "<q"), 32: ("<f", "<i")}


def _next_up(value: float, width: int) -> float:
    """The smallest ``width``-bit float greater than ``value``."""
    if value == math.inf:
        return value
    if value == 0.0:
        value = 0.0  # -0.0 steps up to the smallest positive subnormal
    float_fmt, int_fmt = _FLOAT_FORMATS[width]
    (bits,) = struct.unpack(int_fmt, struct.pack(float_fmt, value))
    # The sign bit makes negative floats negative integers, whose magnitude
    # (and so the float's) shrinks as the integer decreases.
    bits += 1 if bits >= 0 else -1
    return struct.unpack(float_fmt, struct.pack(int_fmt, bits))[0]


def _next_up_normal(value: float, width: int, allow_subnormal: bool) -> float:
    """``_next_up``, skipping the subnormals when they are not allowed."""
    result = _next_up(value, width)
    if not allow_subnormal and 0 < abs(result) < _SMALLEST_NORMAL[width]:
        return 0.0 if result < 0 else _SMALLEST_NORMAL[width]
    return result


def _next_down_normal(value: float, width: int, allow_subnormal: bool) -> float:
    return -_next_up_normal(-value, width, allow_subnormal)


class Floats(_PrintsValue[float]):
    def __init__(
        self,
        min_value: float | None,
        max_value: float | None,
        allow_nan: bool | None,
        allow_infinity: bool | None,
        allow_subnormal: bool | None,
        width: int,
        exclude_min: bool,
        exclude_max: bool,
    ) -> None:
        if width not in (32, 64):
            raise InvalidArgument(f"width={width!r} must be 32 or 64")
        self.min_value = min_value
        self.max_value = max_value
        self.width = width
        self.exclude_min = exclude_min
        self.exclude_max = exclude_max
        min_name = "min_value_exclusive" if exclude_min else "min_value"
        max_name = "max_value_exclusive" if exclude_max else "max_value"
        has_min = min_value is not None
        has_max = max_value is not None
        if has_min and has_max and not (min_value <= max_value):  # type: ignore[operator]
            raise InvalidArgument(
                f"Cannot have {max_name}={max_value!r} < {min_name}={min_value!r}"
            )
        if exclude_min and min_value == math.inf:
            raise InvalidArgument("min_value_exclusive=inf leaves no values")
        if exclude_max and max_value == -math.inf:
            raise InvalidArgument("max_value_exclusive=-inf leaves no values")
        if allow_nan is None:
            allow_nan = not has_min and not has_max
        if allow_nan and (has_min or has_max):
            raise InvalidArgument("Cannot have allow_nan=True with min_value or max_value")
        smallest_normal = _SMALLEST_NORMAL[width]
        # The bounds the generated values actually obey: the given ones with
        # exclude_min/exclude_max applied. An infinite bound on its own side
        # (-inf below, inf above) bounds nothing, like a missing one.
        lo = min_value
        hi = max_value
        assumed_subnormal = allow_subnormal is None or allow_subnormal
        if lo is not None and exclude_min:
            lo = _next_up_normal(lo, width, assumed_subnormal)
        if hi is not None and exclude_max:
            hi = _next_down_normal(hi, width, assumed_subnormal)
        bounded_lo = lo is not None and lo != -math.inf
        bounded_hi = hi is not None and hi != math.inf
        if bounded_lo and bounded_hi and not _sign_aware_lte(lo, hi):  # type: ignore[arg-type]
            if exclude_min or exclude_max:
                raise InvalidArgument(
                    f"min_value_exclusive/max_value_exclusive leave no {width}-bit "
                    f"floating-point values in [{min_value!r}, {max_value!r}]"
                )
            raise InvalidArgument(
                f"There are no {width}-bit floating-point values between "
                f"min_value={min_value!r} and max_value={max_value!r}"
            )
        if allow_infinity is None:
            allow_infinity = not bounded_lo or not bounded_hi
        elif allow_infinity:
            if bounded_lo and bounded_hi:
                raise InvalidArgument(
                    "Cannot have allow_infinity=True with both min_value and max_value"
                )
        elif lo == math.inf:
            if min_value == math.inf:
                raise InvalidArgument("allow_infinity=False excludes min_value=inf")
            raise InvalidArgument(
                f"min_value_exclusive={min_value!r} leaves only inf, "
                "but allow_infinity=False excludes it"
            )
        elif hi == -math.inf:
            if max_value == -math.inf:
                raise InvalidArgument("allow_infinity=False excludes max_value=-inf")
            raise InvalidArgument(
                f"max_value_exclusive={max_value!r} leaves only -inf, "
                "but allow_infinity=False excludes it"
            )
        if allow_subnormal is None:
            if bounded_lo and bounded_hi:
                if lo == hi:
                    allow_subnormal = -smallest_normal < lo < smallest_normal  # type: ignore[operator]
                else:
                    allow_subnormal = lo < smallest_normal and hi > -smallest_normal  # type: ignore[operator]
            elif bounded_lo:
                allow_subnormal = lo < smallest_normal  # type: ignore[operator]
            elif bounded_hi:
                allow_subnormal = hi > -smallest_normal  # type: ignore[operator]
            else:
                allow_subnormal = True
        if allow_subnormal:
            if bounded_lo and lo >= smallest_normal:  # type: ignore[operator]
                raise InvalidArgument(
                    f"allow_subnormal=True, but {min_name}={min_value!r} excludes all "
                    f"values below the smallest positive normal {smallest_normal!r}"
                )
            if bounded_hi and hi <= -smallest_normal:  # type: ignore[operator]
                raise InvalidArgument(
                    f"allow_subnormal=True, but {max_name}={max_value!r} excludes all "
                    f"values above the smallest negative normal {-smallest_normal!r}"
                )
        elif bounded_lo and bounded_hi:
            contains_zero = lo <= 0.0 <= hi  # type: ignore[operator]
            if not contains_zero and hi < smallest_normal and lo > -smallest_normal:  # type: ignore[operator]
                raise InvalidArgument(
                    f"allow_subnormal=False leaves no {width}-bit floating-point "
                    f"values in [{min_value!r}, {max_value!r}]"
                )
        self.allow_nan = allow_nan
        self.allow_infinity = allow_infinity
        self.allow_subnormal = allow_subnormal
        bounded_default = not allow_nan and not allow_infinity
        if bounded_lo:
            self._min = float(min_value)  # type: ignore[arg-type]
        else:
            self._min = -_MAX[width] if bounded_default else -math.inf
        if bounded_hi:
            self._max = float(max_value)  # type: ignore[arg-type]
        else:
            self._max = _MAX[width] if bounded_default else math.inf
        self._smallest = _SMALLEST_SUBNORMAL[width] if allow_subnormal else smallest_normal
        self.label = FLOAT_LABEL

    def do_draw(self, tc: TestCase) -> float:
        return tc._generate_float(
            self.width,
            self._min,
            self._max,
            self.allow_nan,
            self.allow_infinity,
            self.exclude_min,
            self.exclude_max,
            self._smallest,
        )

    def __repr__(self) -> str:
        parts = []
        if self.min_value is not None:
            name = "min_value_exclusive" if self.exclude_min else "min_value"
            parts.append(f"{name}={self.min_value!r}")
        if self.max_value is not None:
            name = "max_value_exclusive" if self.exclude_max else "max_value"
            parts.append(f"{name}={self.max_value!r}")
        if self.width != 64:
            parts.append(f"width={self.width}")
        return f"floats({', '.join(parts)})"


def _check_float(name: str, value: object, width: int) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float, Fraction, Decimal)):
        raise InvalidArgument(f"{name}={value!r} must be a real number")
    result = float(value)
    if math.isnan(result):
        raise InvalidArgument(f"{name}={value!r} must not be NaN")
    if width == 32 and not math.isinf(result):
        narrowed = struct.unpack("<f", struct.pack("<f", result))[0]
        if narrowed != result:
            raise InvalidArgument(
                f"{name}={value!r} cannot be exactly represented as a 32-bit float"
            )
    return result


def floats(
    min_value: float | None = None,
    max_value: float | None = None,
    *,
    allow_nan: bool | None = None,
    allow_infinity: bool | None = None,
    allow_subnormal: bool | None = None,
    width: int = 64,
    min_value_exclusive: float | None = None,
    max_value_exclusive: float | None = None,
) -> Generator[float]:
    """Generate floating-point numbers in ``[min_value, max_value]``.

    Either bound may be omitted. ``min_value_exclusive`` and
    ``max_value_exclusive`` set a bound that is itself excluded, in place
    of the inclusive ``min_value`` and ``max_value``; giving both forms of
    one bound is an :class:`~hegel.InvalidArgument`. Values shrink toward
    ``0.0``, and toward simple values with few significant digits: integers
    before fractions.

    ``allow_nan``, ``allow_infinity`` and ``allow_subnormal`` default to
    whatever the bounds permit. Without bounds, NaN and the infinities are
    allowed. Setting a bound disables NaN, and asking for it with a bound
    is an :class:`~hegel.InvalidArgument`. Setting both bounds disables
    infinity; with one bound, the infinity on the unbounded side is
    allowed unless ``allow_infinity=False``. Subnormals are allowed when
    the range contains any.

    ``width`` is 64 (the default) or 32, in which case every value is
    exactly representable as a 32-bit float and the bounds must be too.

    Bounds that are NaN or not real numbers, ``min_value > max_value``, and
    any combination that leaves no values (``allow_infinity=False`` with
    ``min_value=math.inf``, or ``min_value_exclusive=1.0`` with
    ``max_value=1.0``, say) are each an :class:`~hegel.InvalidArgument`.

    ::

        probability = tc.draw(gs.floats(min_value=0.0, max_value=1.0))
        positive = tc.draw(gs.floats(min_value_exclusive=0.0))
        finite = tc.draw(gs.floats(allow_nan=False, allow_infinity=False))
    """
    if min_value is not None and min_value_exclusive is not None:
        raise InvalidArgument("Cannot give both min_value and min_value_exclusive")
    if max_value is not None and max_value_exclusive is not None:
        raise InvalidArgument("Cannot give both max_value and max_value_exclusive")
    exclude_min = min_value_exclusive is not None
    exclude_max = max_value_exclusive is not None
    if exclude_min:
        min_value = _check_float("min_value_exclusive", min_value_exclusive, width)
    elif min_value is not None:
        min_value = _check_float("min_value", min_value, width)
    if exclude_max:
        max_value = _check_float("max_value_exclusive", max_value_exclusive, width)
    elif max_value is not None:
        max_value = _check_float("max_value", max_value, width)
    return Floats(
        min_value,
        max_value,
        allow_nan,
        allow_infinity,
        allow_subnormal,
        width,
        exclude_min,
        exclude_max,
    )
