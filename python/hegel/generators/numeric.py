"""Integer, float, and boolean generators."""

from __future__ import annotations

import math
import struct
from typing import TYPE_CHECKING

from hegel.errors import InvalidArgument
from hegel.generators.base import Generator, label_from_name

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


class Integers(Generator[int]):
    def __init__(self, min_value: int | None, max_value: int | None) -> None:
        if min_value is not None and max_value is not None and min_value > max_value:
            raise InvalidArgument(f"Cannot have max_value={max_value!r} < min_value={min_value!r}")
        self.min_value = min_value
        self.max_value = max_value
        if min_value is None:
            lo = -UNBOUNDED_MAGNITUDE if max_value is None else min(-UNBOUNDED_MAGNITUDE, max_value)
        else:
            lo = min_value
        if max_value is None:
            hi = (
                UNBOUNDED_MAGNITUDE - 1
                if min_value is None
                else max(UNBOUNDED_MAGNITUDE - 1, min_value)
            )
        else:
            hi = max_value
        self._lo = lo
        self._hi = hi
        self.label = INTEGER_LABEL

    def do_draw(self, tc: TestCase) -> int:
        return tc.generate_integer(self._lo, self._hi)

    def __repr__(self) -> str:
        parts = []
        if self.min_value is not None:
            parts.append(f"min_value={self.min_value!r}")
        if self.max_value is not None:
            parts.append(f"max_value={self.max_value!r}")
        return f"integers({', '.join(parts)})"


def integers(min_value: int | None = None, max_value: int | None = None) -> Generator[int]:
    """Generate integers in ``[min_value, max_value]``; either bound may be
    omitted."""
    if min_value is not None:
        min_value = _check_int("min_value", min_value)
    if max_value is not None:
        max_value = _check_int("max_value", max_value)
    return Integers(min_value, max_value)


class Booleans(Generator[bool]):
    def __init__(self, p: float = 0.5) -> None:
        if not (0.0 <= p <= 1.0):
            raise InvalidArgument(f"p={p!r} must be between 0 and 1")
        self.p = p
        self.label = BOOLEAN_LABEL

    def do_draw(self, tc: TestCase) -> bool:
        return tc.generate_boolean(self.p)

    def __repr__(self) -> str:
        return "booleans()" if self.p == 0.5 else f"weighted_booleans({self.p!r})"


def booleans() -> Generator[bool]:
    """Generate ``True`` or ``False``."""
    return Booleans()


def weighted_booleans(p: float) -> Generator[bool]:
    """Generate ``True`` with probability ``p``."""
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


class Floats(Generator[float]):
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
        has_min = min_value is not None
        has_max = max_value is not None
        if has_min and has_max:
            if not (min_value <= max_value):  # type: ignore[operator]
                raise InvalidArgument(
                    f"Cannot have max_value={max_value!r} < min_value={min_value!r}"
                )
            if not _sign_aware_lte(min_value, max_value):  # type: ignore[arg-type]
                raise InvalidArgument(
                    f"There are no {width}-bit floating-point values between "
                    f"min_value={min_value!r} and max_value={max_value!r}"
                )
            if (min_value == max_value) and (exclude_min or exclude_max):
                raise InvalidArgument(
                    f"exclude_min/exclude_max leave no {width}-bit floating-point "
                    f"values in [{min_value!r}, {max_value!r}]"
                )
        if exclude_min and not has_min:
            raise InvalidArgument("Cannot exclude min_value=None")
        if exclude_max and not has_max:
            raise InvalidArgument("Cannot exclude max_value=None")
        if exclude_min and min_value == math.inf:
            raise InvalidArgument("exclude_min with min_value=inf leaves no values")
        if exclude_max and max_value == -math.inf:
            raise InvalidArgument("exclude_max with max_value=-inf leaves no values")
        if allow_nan is None:
            allow_nan = not has_min and not has_max
        if allow_infinity is None:
            allow_infinity = not has_min or not has_max
        if allow_nan and (has_min or has_max):
            raise InvalidArgument("Cannot have allow_nan=True with min_value or max_value")
        if allow_infinity and has_min and has_max:
            raise InvalidArgument(
                "Cannot have allow_infinity=True with both min_value and max_value"
            )
        smallest_normal = _SMALLEST_NORMAL[width]
        if allow_subnormal is None:
            if has_min and has_max:
                if min_value == max_value:
                    allow_subnormal = -smallest_normal < min_value < smallest_normal  # type: ignore[operator]
                else:
                    allow_subnormal = min_value < smallest_normal and max_value > -smallest_normal  # type: ignore[operator]
            elif has_min:
                allow_subnormal = min_value < smallest_normal  # type: ignore[operator]
            elif has_max:
                allow_subnormal = max_value > -smallest_normal  # type: ignore[operator]
            else:
                allow_subnormal = True
        if allow_subnormal:
            if has_min and min_value >= smallest_normal:  # type: ignore[operator]
                raise InvalidArgument(
                    f"allow_subnormal=True, but min_value={min_value!r} excludes all "
                    f"values below the smallest positive normal {smallest_normal!r}"
                )
            if has_max and max_value <= -smallest_normal:  # type: ignore[operator]
                raise InvalidArgument(
                    f"allow_subnormal=True, but max_value={max_value!r} excludes all "
                    f"values above the smallest negative normal {-smallest_normal!r}"
                )
        elif has_min and has_max:
            contains_zero = min_value <= 0.0 <= max_value  # type: ignore[operator]
            if not contains_zero and max_value < smallest_normal and min_value > -smallest_normal:  # type: ignore[operator]
                raise InvalidArgument(
                    f"allow_subnormal=False leaves no {width}-bit floating-point "
                    f"values in [{min_value!r}, {max_value!r}]"
                )
        self.allow_nan = allow_nan
        self.allow_infinity = allow_infinity
        self.allow_subnormal = allow_subnormal
        bounded_default = not allow_nan and not allow_infinity
        if has_min:
            self._min = float(min_value)  # type: ignore[arg-type]
        else:
            self._min = -_MAX[width] if bounded_default else -math.inf
        if has_max:
            self._max = float(max_value)  # type: ignore[arg-type]
        else:
            self._max = _MAX[width] if bounded_default else math.inf
        self._smallest = _SMALLEST_SUBNORMAL[width] if allow_subnormal else smallest_normal
        self.label = FLOAT_LABEL

    def do_draw(self, tc: TestCase) -> float:
        return tc.generate_float(
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
            parts.append(f"min_value={self.min_value!r}")
        if self.max_value is not None:
            parts.append(f"max_value={self.max_value!r}")
        if self.width != 64:
            parts.append(f"width={self.width}")
        return f"floats({', '.join(parts)})"


def _check_float(name: str, value: object, width: int) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        try:
            value = float(value)  # type: ignore[arg-type]
        except (TypeError, ValueError):
            raise InvalidArgument(f"{name}={value!r} must be a real number") from None
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
    exclude_min: bool = False,
    exclude_max: bool = False,
) -> Generator[float]:
    """Generate floating-point numbers.

    Without bounds, NaN and the infinities are allowed. Setting a bound
    disables NaN; setting both disables infinity. ``width`` is 64 (the
    default) or 32, in which case every value is exactly representable as a
    32-bit float.
    """
    if min_value is not None:
        min_value = _check_float("min_value", min_value, width)
    if max_value is not None:
        max_value = _check_float("max_value", max_value, width)
    return Floats(
        min_value,
        max_value,
        allow_nan,
        allow_infinity,
        allow_subnormal,
        width,
        bool(exclude_min),
        bool(exclude_max),
    )
