"""The pseudo-random number generators Hypothesis-style tests manage.

The global ``random`` module, NumPy's global generator once NumPy is
imported, and every generator passed to :func:`register_random` are seeded
with 0 for each test case and restored afterwards, so a test that uses them
behaves the same on every run and leaves their state as it found it.
``random_module()`` seeds them with a drawn seed instead.
"""

from __future__ import annotations

import contextlib
import gc
import random
import sys
import warnings
from collections.abc import Callable, Iterator
from itertools import count
from typing import Any
from weakref import WeakValueDictionary

from hegel.hypothesis.errors import HypothesisWarning, InvalidArgument

__all__ = ["RANDOMS_TO_MANAGE", "deterministic_PRNG", "get_seeder_and_restorer", "register_random"]

_keys = count()
RANDOMS_TO_MANAGE: WeakValueDictionary[int, Any] = WeakValueDictionary({next(_keys): random})


def _refcount_of_argument(r: Any) -> int:
    return sys.getrefcount(r)


# What sys.getrefcount reports, inside a function, for an argument nothing
# else refers to.
_UNREFERENCED_COUNT = _refcount_of_argument(object())


class _NumpyRandom:
    """NumPy's global generator, under the stdlib's method names."""

    def __init__(self) -> None:
        import numpy.random

        self.seed = numpy.random.seed
        self.getstate = numpy.random.get_state
        self.setstate = numpy.random.set_state


_numpy_random: _NumpyRandom | None = None


def register_random(r: Any) -> None:
    """Register (a weak reference to) ``r``, an object with ``seed``,
    ``getstate`` and ``setstate`` methods such as a :class:`random.Random`,
    to be seeded and restored with the global ``random`` module for every
    test case. ``r`` must be referenced elsewhere for as long as it should
    be managed: ``register_random(random.Random())`` is an error."""
    if not (hasattr(r, "seed") and hasattr(r, "getstate") and hasattr(r, "setstate")):
        raise InvalidArgument(f"r={r!r} does not have all the required methods")
    # A loop, not any(...): a generator closing over r would add a
    # reference to it, hiding the unreferenced case below.
    managed = None
    for managed in list(RANDOMS_TO_MANAGE.values()):
        if managed is r:
            return
    del managed
    gc.collect()
    if not gc.get_referrers(r):
        if sys.getrefcount(r) <= _UNREFERENCED_COUNT:
            raise ReferenceError(
                f"`register_random` was passed `r={r}` which will be garbage collected "
                "immediately after `register_random` creates a weakref to it. Keep a "
                "reference to it for as long as it should be managed."
            )
        warnings.warn(
            "It looks like `register_random` was passed an object that could be garbage "
            "collected immediately after `register_random` creates a weakref to it. Keep "
            "a reference to it for as long as it should be managed.",
            HypothesisWarning,
            stacklevel=2,
        )
    RANDOMS_TO_MANAGE[next(_keys)] = r


def get_seeder_and_restorer(seed: int = 0) -> tuple[Callable[[], None], Callable[[], None]]:
    """A function seeding every managed generator with ``seed``, and one
    restoring the states they had when it ran."""
    global _numpy_random
    if "numpy" in sys.modules and _numpy_random is None:
        _numpy_random = _NumpyRandom()
        RANDOMS_TO_MANAGE[next(_keys)] = _numpy_random
    states: dict[int, Any] = {}

    def seed_all() -> None:
        for key, r in list(RANDOMS_TO_MANAGE.items()):
            states[key] = r.getstate()
            r.seed(seed)

    def restore_all() -> None:
        for key, state in states.items():
            r = RANDOMS_TO_MANAGE.get(key)
            if r is not None:
                r.setstate(state)
        states.clear()

    return seed_all, restore_all


@contextlib.contextmanager
def deterministic_PRNG(seed: int = 0) -> Iterator[None]:
    """Seed every managed generator with ``seed`` for the block, restoring
    their states afterwards."""
    seed_all, restore_all = get_seeder_and_restorer(seed)
    seed_all()
    try:
        yield
    finally:
        restore_all()
