"""Span labels: opaque 64-bit identifiers for generators, computed by
libhegel's ``hegel_label_from_name`` and ``hegel_label_combine``."""

from __future__ import annotations

import functools
from collections.abc import Iterable

from hegel import _native


@functools.lru_cache(maxsize=4096)
def label_from_name(name: str) -> int:
    """The span label for a generator identified by ``name``."""
    return _native.label_from_name(name)


def combine_labels(labels: Iterable[int]) -> int:
    """The span label for a generator built from other generators: the
    labels combined in order."""
    return _combine(tuple(labels))


@functools.lru_cache(maxsize=4096)
def _combine(labels: tuple[int, ...]) -> int:
    return _native.label_combine(list(labels))
