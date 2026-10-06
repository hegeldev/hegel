"""Generators: recipes for the values a test draws.

Import as ``from hegel import generators as gs`` and draw with
``tc.draw(gs.integers())``.
"""

from hegel.generators.base import (
    Generator,
    combine_labels,
    label_from_name,
)
from hegel.generators.collections import (
    dicts,
    fixed_dicts,
    frozensets,
    iterables,
    lists,
    permutations,
    samples,
    sets,
    subsequences,
    tuples,
)
from hegel.generators.combinators import (
    just,
    none,
    nothing,
    one_of,
    optional,
    sampled_from,
)
from hegel.generators.composite import builds, composite
from hegel.generators.from_type import from_type, register_type_generator
from hegel.generators.numeric import booleans, floats, integers, weighted_booleans
from hegel.generators.recursive import deferred, recursive
from hegel.generators.strings import (
    binary,
    characters,
    domains,
    emails,
    from_regex,
    ip_addresses,
    text,
    urls,
    uuids,
)
from hegel.generators.time import dates, datetimes, timedeltas, times, timezones

__all__ = [
    "Generator",
    "binary",
    "booleans",
    "builds",
    "characters",
    "combine_labels",
    "composite",
    "dates",
    "datetimes",
    "deferred",
    "dicts",
    "domains",
    "emails",
    "fixed_dicts",
    "floats",
    "from_regex",
    "from_type",
    "frozensets",
    "integers",
    "ip_addresses",
    "iterables",
    "just",
    "label_from_name",
    "lists",
    "none",
    "nothing",
    "one_of",
    "optional",
    "permutations",
    "recursive",
    "register_type_generator",
    "samples",
    "sampled_from",
    "sets",
    "subsequences",
    "text",
    "timedeltas",
    "times",
    "timezones",
    "tuples",
    "urls",
    "uuids",
    "weighted_booleans",
]
