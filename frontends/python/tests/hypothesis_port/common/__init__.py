import sys
from collections import namedtuple

from hegel.generators import (
    binary,
    booleans,
    builds,
    dicts,
    fixed_dicts,
    floats,
    frozensets,
    integers,
    just,
    lists,
    none,
    one_of,
    recursive,
    sampled_from,
    sets,
    text,
    tuples,
)

__all__ = ["standard_types"]

OrderedPair = namedtuple("OrderedPair", ("left", "right"))
ABC = namedtuple("ABC", ("a", "b", "c"))


def abc(x, y, z):
    return builds(ABC, x, y, z)


standard_types = [
    lists(none(), max_size=0),
    tuples(),
    sets(none(), max_size=0),
    frozensets(none(), max_size=0),
    fixed_dicts({}),
    abc(booleans(), booleans(), booleans()),
    abc(booleans(), booleans(), integers()),
    fixed_dicts({"a": integers(), "b": booleans()}),
    dicts(booleans(), integers()),
    dicts(text(), booleans()),
    one_of(integers(), tuples(booleans())),
    sampled_from(range(10)),
    one_of(just("a"), just("b"), just("c")),
    sampled_from(("a", "b", "c")),
    integers(),
    integers(min_value=3),
    integers(min_value=(-(2**32)), max_value=(2**64)),
    floats(),
    floats(min_value=-2.0, max_value=3.0),
    floats(),
    floats(min_value=-2.0),
    floats(),
    floats(max_value=-0.0),
    floats(),
    floats(min_value=0.0),
    floats(min_value=3.14, max_value=3.14),
    text(),
    binary(),
    booleans(),
    tuples(booleans(), booleans()),
    frozensets(integers()),
    sets(frozensets(booleans())),
    lists(lists(booleans())),
    lists(floats(0.0, 0.0)),
    integers().flat_map(
        lambda right: integers(min_value=0).map(lambda length: OrderedPair(right - length, right))
    ),
    integers().flat_map(lambda v: lists(just(v))),
    integers().filter(lambda x: abs(x) > 100),
    floats(min_value=-sys.float_info.max, max_value=sys.float_info.max),
    none(),
    booleans().flat_map(lambda x: booleans() if x else floats()),
    recursive(booleans(), lambda x: lists(x, max_size=3), max_leaves=10),
]
