"""Hypothesis's optional integrations. None are provided on Hegel: importing
one raises :class:`ModuleNotFoundError` saying so, so code that tolerates a
missing integration (``pytest.importorskip``, ``except ImportError``) still
does."""

from __future__ import annotations


def _unavailable(module: str) -> ModuleNotFoundError:
    # No ``name=``: ``from hegel.hypothesis.extra import numpy`` swallows a
    # ModuleNotFoundError naming the submodule and raises a bare "cannot
    # import name" ImportError in its place.
    name = module.replace("hegel.hypothesis", "hypothesis", 1)
    return ModuleNotFoundError(
        f"{module} is not available: hegel.hypothesis does not provide {name}. "
        "See docs/comparison-with-hypothesis.md in hegel-python for what it covers."
    )
