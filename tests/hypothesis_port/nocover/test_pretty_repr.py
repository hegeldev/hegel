# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import pytest

import hegel
from hegel import generators as gs


def foo(x):
    pass


def bar(x):
    pass


def baz(x):
    pass


fns = [foo, bar, baz]


@hegel.composite
def builds_ignoring_invalid(tc, target, *args, **kwargs):
    # Hegel validates generator arguments eagerly, so an invalid combination
    # raises as the generator is built.
    args = tc.draw(gs.tuples(*args))
    kwargs = tc.draw(gs.fixed_dicts(kwargs))
    try:
        return target(*args, **kwargs)
    except hegel.InvalidArgument:
        tc.reject()


size_strategies = {
    "min_size": gs.integers(min_value=0, max_value=100),
    "max_size": gs.integers(min_value=0, max_value=100) | gs.none(),
}

values = gs.integers() | gs.text()


# Hegel has no randoms(), complex_numbers(), fractions() or decimals().
Strategies = gs.recursive(
    gs.one_of(
        gs.sampled_from([gs.none(), gs.booleans(), gs.floats()]),
        gs.builds(gs.just, values),
        gs.builds(gs.sampled_from, gs.lists(values, min_size=1)),
        builds_ignoring_invalid(gs.floats, gs.floats(), gs.floats()),
    ),
    lambda x: gs.one_of(
        builds_ignoring_invalid(gs.lists, x, **size_strategies),
        builds_ignoring_invalid(gs.sets, x, **size_strategies),
        builds_ignoring_invalid(lambda v: gs.tuples(*v), gs.lists(x)),
        builds_ignoring_invalid(lambda v: gs.one_of(*v), gs.lists(x, min_size=1)),
        builds_ignoring_invalid(gs.dicts, x, x, **size_strategies),
        gs.builds(lambda s, f: s.map(f), x, gs.sampled_from(fns)),
    ),
)


strategy_globals = {k: getattr(gs, k) for k in dir(gs)}

strategy_globals["inf"] = float("inf")
strategy_globals["nan"] = float("nan")
strategy_globals["foo"] = foo
strategy_globals["bar"] = bar
strategy_globals["baz"] = baz


# builds_ignoring_invalid rejects often enough that, about one run in a
# hundred, 50 inputs are rejected before 10 are accepted.
@hegel.test(suppress_health_check=[hegel.HealthCheck.filter_too_much])
def test_repr_evals_to_thing_with_same_repr(tc):
    strategy = tc.draw(Strategies)
    r = repr(strategy)
    via_eval = eval(r, strategy_globals)
    r2 = repr(via_eval)
    assert r == r2


@pytest.mark.parametrize(
    "r",
    [
        "none().filter(foo).map(bar)",
        "just(1).filter(foo).map(bar)",
        "sampled_from([1, 2, 3]).filter(foo).map(bar)",
    ],
)
def test_sampled_transform_reprs(r):
    assert repr(eval(r, strategy_globals)) == r
