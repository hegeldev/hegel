"""Span labels: opaque 64-bit identifiers for generators.

These are computed in Python rather than by calling libhegel's
``hegel_label_from_name`` and ``hegel_label_combine``: every generator
computes its label when constructed, often at import time, and a native call
for each costs more than the hash. ``tests/test_labels.py`` checks that both
agree with libhegel's.
"""

from __future__ import annotations

from collections.abc import Iterable

_FNV_OFFSET_BASIS = 0xCBF29CE484222325
_FNV_PRIME = 0x100000001B3
_MASK = (1 << 64) - 1


def _fnv1a(hash_value: int, data: bytes) -> int:
    for byte in data:
        hash_value ^= byte
        hash_value = (hash_value * _FNV_PRIME) & _MASK
    return hash_value


def label_from_name(name: str) -> int:
    """The span label for a generator identified by ``name``: the 64-bit
    FNV-1a hash of its UTF-8 bytes, as libhegel's ``hegel_label_from_name``
    computes it."""
    return _fnv1a(_FNV_OFFSET_BASIS, name.encode("utf-8", "surrogatepass"))


def combine_labels(labels: Iterable[int]) -> int:
    """The span label for a generator built from other generators: the
    labels combined in order, as libhegel's ``hegel_label_combine`` does."""
    hash_value = _FNV_OFFSET_BASIS
    for label in labels:
        hash_value = _fnv1a(hash_value, (label & _MASK).to_bytes(8, "little"))
    return hash_value
