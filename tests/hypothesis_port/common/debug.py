from typing import TypeVar

from hegel.hypothesis import HealthCheck, Phase, Verbosity, given, settings as Settings
from hegel.hypothesis.control import current_build_context, currently_in_test_context
from hegel.hypothesis.errors import NoSuchExample, Unsatisfiable
from hegel.hypothesis.strategies import SearchStrategy

from tests.hypothesis_port.common.utils import no_shrink

TIME_INCREMENT = 0.00001
T = TypeVar("T")


class Found(Exception):
    pass


def minimal(
    definition: SearchStrategy[T],
    condition=lambda x: True,
    settings: Settings | None = None,
) -> T:
    definition.validate()
    result = None

    def wrapped_condition(x):
        return condition(x)

    if settings is None:
        settings = Settings(max_examples=500, phases=(Phase.generate, Phase.shrink))

    verbosity = settings.verbosity
    if verbosity == Verbosity.normal:
        verbosity = Verbosity.quiet

    @given(definition)
    @Settings(
        parent=settings,
        suppress_health_check=list(HealthCheck),
        report_multiple_bugs=False,
        derandomize=True,
        database=None,
        verbosity=verbosity,
        deadline=None,
    )
    def inner(x):
        if wrapped_condition(x):
            nonlocal result
            result = x
            raise Found

    try:
        inner()
    except Found:
        return result
    raise Unsatisfiable(
        f"Could not find any examples from {definition!r} that satisfied "
        f"{getattr(condition, '__name__', repr(condition))}"
    )


def find_any(definition, condition=lambda _: True, settings=None):
    # If nested within an existing @given
    if currently_in_test_context():
        tc = current_build_context().tc
        while True:
            if condition(s := tc.draw_silent(definition)):
                return s

    # If top-level
    settings = settings or Settings.default
    return minimal(
        definition,
        condition,
        settings=Settings(
            settings, phases=no_shrink, max_examples=max(1000, settings.max_examples)
        ),
    )


def assert_no_examples(strategy, condition=lambda _: True):
    try:
        assert_all_examples(strategy, lambda val: not condition(val))
    except (Unsatisfiable, NoSuchExample):
        pass


def assert_all_examples(strategy, predicate, settings=None):
    """Asserts that all examples of the given strategy match the predicate."""
    if currently_in_test_context():
        tc = current_build_context().tc
        for _ in range(20):
            s = tc.draw_silent(strategy)
            msg = f"Found {s!r} using strategy {strategy} which does not match"
            assert predicate(s), msg

    else:

        @given(strategy)
        @Settings(parent=settings, database=None, deadline=None)
        def assert_examples(s):
            msg = f"Found {s!r} using strategy {strategy} which does not match"
            assert predicate(s), msg

        assert_examples()


def assert_simple_property(strategy, predicate, settings=None):
    """Like assert_all_examples, for simple constant properties that a few
    examples verify adequately."""
    assert_all_examples(
        strategy,
        predicate,
        Settings(parent=settings, max_examples=15, suppress_health_check=list(HealthCheck)),
    )


def check_can_generate_examples(strategy, settings=None):
    """Generates a few examples from the strategy, checking only that no
    error is raised."""
    assert_simple_property(
        strategy,
        lambda _: True,
        settings=Settings(parent=settings, phases=(Phase.generate,)),
    )
