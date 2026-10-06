"""Strategies for internet-related data (domains and URLs)."""

from __future__ import annotations

from hegel import generators as gs
from hegel.hypothesis.strategies import SearchStrategy

__all__ = ["domains", "urls"]


def domains(*, max_length: int = 255, max_element_length: int = 63) -> SearchStrategy[str]:
    return gs.domains(max_length=max_length)


def urls() -> SearchStrategy[str]:
    return gs.urls()
