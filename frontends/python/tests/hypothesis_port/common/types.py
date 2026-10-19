"""Stand-in for ``hypothesis.strategies._internal.types``: a read-only
view of the types ``from_type`` knows about, plus the small predicates the
lookup tests use."""

import collections
import collections.abc
import importlib
import typing
from collections.abc import Mapping

from hegel import generators as gs

_from_type = importlib.import_module("hegel.generators.from_type")


class _TypeLookup(Mapping):
    """The registered and built-in types, registered ones first."""

    def _mapping(self):
        return {**_from_type._registry, **_from_type._BUILTINS, **_GENERIC_ORIGINS}

    def __getitem__(self, key):
        return self._mapping()[key]

    def __iter__(self):
        return iter(self._mapping())

    def __len__(self):
        return len(self._mapping())

    def __contains__(self, key):
        try:
            return key in self._mapping()
        except TypeError:
            return False


# The generic origins from_type handles without a registry entry.
_GENERIC_ORIGINS = {
    origin: None
    for origin in (
        list,
        set,
        frozenset,
        dict,
        tuple,
        collections.deque,
        collections.OrderedDict,
        collections.abc.Sequence,
        collections.abc.MutableSequence,
        collections.abc.Set,
        collections.abc.MutableSet,
        collections.abc.Mapping,
        collections.abc.MutableMapping,
        collections.abc.Iterable,
        collections.abc.Iterator,
        collections.abc.Collection,
    )
}

_global_type_lookup = _TypeLookup()

# The builtin types with a generator of their own.
core_types = {t for t in _from_type._BUILTINS if isinstance(t, type)}

typing_root_type = (typing._Final, typing._GenericAlias)  # type: ignore[attr-defined]

_NOT_GENERATORS = {
    "Generator",
    "combine_labels",
    "composite",
    "label_from_name",
    "register_type_generator",
}
_all_generators = {name: getattr(gs, name) for name in gs.__all__ if name not in _NOT_GENERATORS}


def can_cast(type, value):
    try:
        type(value)
        return True
    except Exception:
        return False


def _can_hash(val):
    try:
        hash(val)
        return True
    except Exception:
        return False
