"""Stand-ins for the Hypothesis internals its tests import: enough of
``hypothesis.internal.floats``, ``.reflection`` and ``.compat`` for the
ported tests that only use them as helpers."""

import math
import struct
import sys
import typing

PYPY = False
GRAALPY = False
ExceptionGroup = ExceptionGroup  # noqa: PLW0127

SIGNALING_NAN = struct.unpack("!d", b"\x7f\xf8\x00\x00\x00\x00\x00\x01")[0]


def next_up(value: float, width: int = 64) -> float:
    if math.isnan(value) or (math.isinf(value) and value > 0):
        return value
    if value == 0.0 and math.copysign(1.0, value) < 0:
        return 0.0
    fmt = {64: "<d", 32: "<f", 16: "<e"}[width]
    bits_fmt = {64: "<Q", 32: "<I", 16: "<H"}[width]
    bits = struct.unpack(bits_fmt, struct.pack(fmt, value))[0]
    if value >= 0:
        bits += 1
    else:
        bits -= 1
    return struct.unpack(fmt, struct.pack(bits_fmt, bits))[0]


def next_down(value: float, width: int = 64) -> float:
    return -next_up(-value, width)


def next_down_normal(value: float, width: int, *, allow_subnormal: bool) -> float:
    candidate = next_down(value, width)
    smallest = {64: sys.float_info.min, 32: 2**-126, 16: 2**-14}[width]
    if not allow_subnormal and 0 < abs(candidate) < smallest:
        return 0.0 if candidate > 0 else -smallest
    return candidate


def next_up_normal(value: float, width: int, *, allow_subnormal: bool) -> float:
    return -next_down_normal(-value, width, allow_subnormal=allow_subnormal)


def get_pretty_function_description(f: object) -> str:
    name = getattr(f, "__name__", None)
    if name is None:
        return repr(f)
    return name


def nicerepr(v: object) -> str:
    if callable(v) and not isinstance(v, type):
        return get_pretty_function_description(v)
    return repr(v)


get_type_hints = typing.get_type_hints


def float_to_int(value: float, width: int = 64) -> int:
    fmt, bits = {64: ("<d", "<Q"), 32: ("<f", "<I"), 16: ("<e", "<H")}[width]
    return struct.unpack(bits, struct.pack(fmt, value))[0]


def int_to_float(value: int, width: int = 64) -> float:
    fmt, bits = {64: ("<d", "<Q"), 32: ("<f", "<I"), 16: ("<e", "<H")}[width]
    return struct.unpack(fmt, struct.pack(bits, value))[0]


def float_of(x: float, width: int) -> float:
    if width == 64:
        return float(x)
    fmt = {32: "<f", 16: "<e"}[width]
    return struct.unpack(fmt, struct.pack(fmt, float(x)))[0]


def is_negative(value: float) -> bool:
    return math.copysign(1.0, value) < 0
