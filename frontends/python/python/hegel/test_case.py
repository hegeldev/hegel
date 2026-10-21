"""The test case: the object a test body draws values from."""

from __future__ import annotations

import datetime as _dt
import sys
import threading
from collections.abc import Callable
from typing import TYPE_CHECKING, Any, NoReturn, TypeVar

from hegel import _native
from hegel._draw_names import DrawName, name_for_call
from hegel.errors import AssumeFailed, InternalError, InvalidArgument, LoopDone, StopTest
from hegel.pretty import DEFAULT_MAX_WIDTH, PrettyPrinter, pretty_repr

if TYPE_CHECKING:
    from hegel.generators.base import Generator

__all__ = ["Collection", "OutputSink", "TestCase"]

T = TypeVar("T")

OutputSink = Callable[[str], None]

# Deeper nesting than this ends the test case as too large, well before
# Python's own recursion limit would turn it into a RecursionError.
MAX_DRAW_DEPTH = 100

MIN_DATE = (1, 1, 1)
MAX_DATE = (9999, 12, 31)
MIDNIGHT = (0, 0, 0, 0)
LAST_NANOSECOND = (23, 59, 59, 999_999_999)
MIN_DATETIME = MIN_DATE + MIDNIGHT
MAX_DATETIME = MAX_DATE + LAST_NANOSECOND


_UNNAMED = DrawName("draw", repeatable=True)

# Exceptions that end a draw as part of normal control flow (a rejection,
# an abandoned test case, a misuse error) rather than as a failure of the
# draw to report.
_SIGNALS: tuple[type[BaseException], ...] = (
    AssumeFailed,
    StopTest,
    LoopDone,
    InvalidArgument,
    InternalError,
    _native.RetrySignal,
    KeyboardInterrupt,
    SystemExit,
)


def _generator_class() -> type:
    from hegel.generators.base import Generator

    return Generator


def _discard(_line: str) -> None:
    pass


class _Global:
    """State shared by every handle onto one test case: whether drawn
    values are reported, and where."""

    __slots__ = ("emit", "explicit", "sink")

    def __init__(self, emit: bool, sink: OutputSink | None, explicit: Any) -> None:
        self.emit = emit
        self.sink: OutputSink = sink if (emit and sink is not None) else _discard
        self.explicit = explicit


class _DrawState:
    """Draw-name bookkeeping for one naming scope."""

    __slots__ = ("allocated", "counts", "lock")

    def __init__(self) -> None:
        self.counts: dict[str, int] = {}
        self.allocated: set[str] = set()
        self.lock = threading.Lock()


class TestCase:
    """One execution of a property test: the object a test draws its
    inputs from, and through which it rejects inputs, adds notes to the
    failure report, steers generation and records statistics.

    A test decorated with :func:`hegel.test` receives a ``TestCase`` as
    its first argument, once per test case; tests never construct one.
    An instance belongs to the test case it was passed for and must not be
    kept and used after the test returns. It may be driven by one thread
    at a time.

    ::

        @hegel.test
        def test_addition_commutes(tc):
            x = tc.draw(gs.integers())
            y = tc.draw(gs.integers())
            tc.note(f"x + y = {x + y}")
            assert x + y == y + x
    """

    __test__ = False  # not a pytest test class

    __slots__ = (
        "_draw_state",
        "_global",
        "_handle",
        "_nested_draws",
        "_printer",
        "_printing_depth",
        "_span_depth",
    )

    def __init__(
        self,
        handle: _native.TestCase,
        emit: bool = False,
        sink: OutputSink | None = None,
        explicit: Any = None,
    ) -> None:
        self._handle = handle
        self._global = _Global(emit, sink, explicit)
        self._span_depth = 0
        self._printing_depth = 0
        self._printer: PrettyPrinter | None = None
        self._draw_state = _DrawState()
        self._nested_draws: list[tuple[str, Any]] | None = None

    @classmethod
    def _derive(cls, parent: TestCase, handle: _native.TestCase) -> TestCase:
        tc = cls.__new__(cls)
        tc._handle = handle
        tc._global = parent._global
        tc._span_depth = 0
        tc._printing_depth = 0
        tc._printer = None
        tc._draw_state = _DrawState()
        tc._nested_draws = None
        return tc

    # -- drawing ---------------------------------------------------------

    def draw(self, generator: Generator[T], name: str | None = None) -> T:
        """Draw a value from ``generator`` and report it.

        Passing something other than a :class:`~hegel.generators.Generator`
        (a generator function that was not called, say) is an
        :class:`~hegel.InvalidArgument`.

        The value is printed, as ``name = value``, into the output shown
        for a failing test case. When ``name`` is omitted it is inferred
        from the assignment the call appears in (``x = tc.draw(...)``); a
        draw that can run more than once per test case (in a loop, say) is
        numbered, as ``x_1``, ``x_2``, ... An explicit ``name`` is always
        numbered, since it typically names a draw in a helper function,
        and an unassigned draw reports as ``draw_1``, ``draw_2``, ...
        Names are inferred from the test's source, so draws in code whose
        source is unavailable report as ``draw_N``.

        Draws made inside a :func:`~hegel.composite` generator, or while
        another draw is in progress, are not printed separately: they are
        part of the outer value.
        """
        if not isinstance(generator, _generator_class()):
            raise InvalidArgument(
                f"tc.draw() takes a generator, but got {generator!r} of type "
                f"{type(generator).__name__}"
                + (". Did you forget to call it?" if callable(generator) else "")
            )
        explicit = self._global.explicit
        if explicit is None and (self._span_depth > 0 or self._printing_depth > 0):
            value = self.draw_silent(generator)
            if self._nested_draws is not None:
                # Kept in case the enclosing draw fails, to show what it drew.
                if name is None:
                    frame = sys._getframe(1)
                    inferred = name_for_call(frame.f_code.co_filename, frame.f_lineno)
                    name = "draw" if inferred is None else inferred.name
                self._nested_draws.append((name, value))
            return value
        if name is not None:
            draw_name = DrawName(str(name), repeatable=True)
        else:
            frame = sys._getframe(1)
            draw_name = name_for_call(frame.f_code.co_filename, frame.f_lineno) or _UNNAMED
        return self._draw_as(generator, draw_name)

    def _draw_as(self, generator: Generator[T], draw_name: DrawName, separator: str = " = ") -> T:
        """Draw a value reported as ``<name><separator><value>``, under the
        name allocated for ``draw_name``. Nested draws are silent: callers
        check for them first."""
        explicit = self._global.explicit
        if explicit is not None:
            return explicit.draw(self, generator, draw_name)
        display = self._allocate_display_name(draw_name)
        if display is None:
            return self.draw_silent(generator)
        self._printing_depth += 1
        outer_capture = self._nested_draws
        self._nested_draws = []
        try:
            printer = self._with_printer()
            with printer.speculate() as speculation:
                printer.text(f"{display}{separator}")
                value = self.draw_and_print(generator, printer)
                printer.hard_break()
                speculation.commit()
            return value
        except _SIGNALS:
            raise
        except BaseException as error:
            self._report_failed_draw(f"{display}{separator}", generator, error)
            raise
        finally:
            self._printing_depth -= 1
            self._nested_draws = outer_capture

    def _report_failed_draw(
        self, prefix: str, generator: Generator[Any], error: BaseException
    ) -> None:
        """Report a draw that raised: the draw itself, then the values
        drawn inside it (a composite's, say) before it did."""
        printer = self._with_printer()
        printer.text(f"{prefix}<{type(error).__name__} raised while drawing from {generator!r}>")
        printer.hard_break()
        for name, value in self._nested_draws or ():
            printer.text(f"  {name} = {pretty_repr(value)}")
            printer.hard_break()

    def draw_silent(self, generator: Generator[T]) -> T:
        """Draw a value from ``generator`` without printing it in the
        failure report. Use it for values that would only clutter the
        report, or in a custom :class:`~hegel.generators.Generator`'s
        ``do_draw``."""
        explicit = self._global.explicit
        if explicit is not None:
            return explicit.draw_silent(self, generator)
        self.start_span(generator.label)
        value = generator.do_draw(self)
        self.stop_span(False)
        return value

    def draw_and_print(self, generator: Generator[T], printer: PrettyPrinter) -> T:
        """Draw a value, printing its representation through ``printer``.
        This is how a custom generator's ``do_draw_and_print`` draws its
        components, so they print as part of its own value."""
        self.start_span(generator.label)
        value = generator.do_draw_and_print(self, printer)
        self.stop_span(False)
        return value

    def _allocate_display_name(self, draw_name: DrawName) -> str | None:
        """Count a draw under ``draw_name`` and return the name it is
        reported as (``None`` when this test case reports nothing)."""
        state = self._draw_state
        name = draw_name.name
        with state.lock:
            count = state.counts.get(name, 0) + 1
            state.counts[name] = count
            if not self._global.emit:
                return None
            if draw_name.repeatable or count > 1 or name in state.allocated:
                # A name inferred as drawn once that is drawn again (from a
                # helper called twice, say) carries on from its count.
                suffix = count
                candidate = f"{name}_{suffix}"
                while candidate in state.allocated:
                    suffix += 1
                    candidate = f"{name}_{suffix}"
            else:
                candidate = name
            state.allocated.add(candidate)
            return candidate

    # -- control flow ----------------------------------------------------

    def assume(self, condition: object) -> None:
        """Reject the current test case unless ``condition`` is true.

        A rejected test case is neither a pass nor a failure: it is
        abandoned and does not count toward ``test_cases``. Use it for
        inputs the property does not apply to::

            x = tc.draw(gs.integers())
            y = tc.draw(gs.integers())
            tc.assume(y != 0)
            assert (x // y) * y + x % y == x

        Rejecting most test cases makes a run slow and fails the
        ``filter_too_much`` health check; a generator that only produces
        valid inputs is better where one is easy to write.
        """
        if not condition:
            self.reject()

    def reject(self) -> None:
        """Reject the current test case unconditionally, as
        ``tc.assume(False)`` does. It never returns: it raises
        :class:`~hegel.AssumeFailed`, which Hegel catches."""
        explicit = self._global.explicit
        if explicit is not None:
            explicit.reject()
        raise AssumeFailed()

    def note(self, message: str) -> None:
        """Add ``message`` to the output of this test case, between the
        drawn values in the order it was recorded.

        The output is shown for the failing test case Hegel reports, and
        for every test case at ``verbosity="verbose"`` and above; otherwise
        notes cost nothing. Use it for intermediate values that help explain
        a failure::

            result = compute(x)
            tc.note(f"intermediate result: {result!r}")
        """
        explicit = self._global.explicit
        if explicit is not None:
            explicit.note(str(message))
            return
        if not self._global.emit:
            return
        self._handle.note(str(message))

    def target(self, score: float, label: str = "") -> None:
        """Guide generation toward test cases that maximise ``score``.

        ``score`` is an ``int`` or ``float`` and ``label`` names the
        quantity being maximised, so one test can target several. After a
        warm-up, the engine's targeting phase searches around the
        highest-scoring test cases for higher scores. This helps find
        failures that need an extreme value of some measure (a
        long queue, a deep tree). Each label can be targeted at most once
        per test case; a second call with the same label, or a score or
        label of the wrong type, is an :class:`~hegel.InvalidArgument`.

        ::

            queue_length = simulate(tc)
            tc.target(queue_length, label="queue length")
        """
        if not isinstance(label, str):
            raise InvalidArgument(f"label={label!r} must be a string")
        if isinstance(score, bool) or not isinstance(score, (int, float)):
            raise InvalidArgument(f"score={score!r} must be an int or float")
        if self._global.explicit is not None:
            return
        self._handle.target(float(score), label)

    def event(self, label: str) -> None:
        """Record that ``label`` happened in this test case.

        With ``show_statistics=True`` (or ``HEGEL_STATISTICS=1``), the end
        of the run reports the percentage of test cases recording each
        event, which shows whether the inputs exercise what they should::

            tc.event("empty" if not xs else "non-empty")
        """
        if self._global.explicit is not None:
            return
        self._handle.event(str(label))

    def event_value(self, label: str, value: float) -> None:
        """Record a numeric observation ``value`` under ``label``.

        With ``show_statistics=True`` the end of the run reports the count,
        minimum, median, mean, 90th percentile and maximum of the values
        recorded under each label. ``value`` must be an ``int`` or
        ``float``, or it is an :class:`~hegel.InvalidArgument`.

        ::

            tc.event_value("list length", len(xs))
        """
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise InvalidArgument(f"value={value!r} must be an int or float")
        if self._global.explicit is not None:
            return
        self._handle.event_value(float(value), str(label))

    def repeat(self, body: Callable[[], Any]) -> NoReturn:
        """Run ``body`` an engine-chosen number of times, then end the test
        case successfully.

        ``body`` takes no arguments and draws from this test case (a
        closure over ``tc``). A repetition that rejects (with
        :meth:`assume`) is skipped and the next one runs. A failure in any
        repetition fails the test case, and shrinking reduces the number of
        repetitions along with the values each drew. Each repetition is
        reported under a ``// Repetition #N`` heading.

        ``repeat`` never returns normally: it raises an internal
        control-flow signal that ends the test body, so code after it does
        not run. It suits workload-style tests that perform a sequence of
        operations until the engine decides to stop::

            @hegel.test
            def test_store(tc):
                store = Store()

                def step():
                    key = tc.draw(gs.text())
                    store.put(key, tc.draw(gs.integers()))
                    assert store.get(key) is not None

                tc.repeat(step)
        """
        from hegel.generators import booleans, integers

        min_size = self.draw_silent(integers(min_value=0, max_value=1 << 40))
        collection = Collection(self, min_size, None)
        iteration = 0
        while collection.more():
            iteration += 1
            self.note(f"// Repetition #{iteration}")
            outer = self._enter_block(2)
            try:
                body()
            except AssumeFailed:
                pass
            except (StopTest, InvalidArgument, InternalError, LoopDone):
                raise
            except BaseException:
                # Back on the test's own handle before the draw that marks
                # the failing repetition.
                self._restore_handle(outer)
                self.draw_silent(booleans())
                raise
            finally:
                self._restore_handle(outer)
        raise LoopDone()

    # -- handles and output ----------------------------------------------

    def _child(self, extra_indent: int = 2) -> TestCase:
        """An instance for an indented section of this one's output (a
        stateful rule body, say): it draws from the same choice stream, and
        every line it prints is nested ``extra_indent`` columns further in."""
        return TestCase._derive(self, self._handle.block(extra_indent))

    def _clone(self) -> TestCase:
        """A handle with an independent choice stream, for another thread."""
        return TestCase._derive(self, self._handle.clone())

    def _set_worker(self, index: int) -> None:
        self._handle.set_worker(index)

    def _enter_block(self, indent: int) -> _native.TestCase:
        block = self._handle.block(indent)
        return self._restore_handle(block)

    def _restore_handle(self, handle: _native.TestCase) -> _native.TestCase:
        self._printer = None
        previous = self._handle
        self._handle = handle
        return previous

    def _with_printer(self) -> PrettyPrinter:
        if self._printer is None:
            self._printer = PrettyPrinter(self._handle.printer(DEFAULT_MAX_WIDTH))
        return self._printer

    def _emit_rendered_output(self) -> None:
        """Render this instance's document and push it, line by line,
        through the output sink."""
        if not self._global.emit:
            return
        try:
            output = PrettyPrinter(self._handle.printer(DEFAULT_MAX_WIDTH)).value()
        except Exception as error:  # nocov
            self._global.sink(
                "Failed to render this test case's drawn values "
                f"({error}). This indicates a bug in printing code the test "
                "uses: check any do_draw_and_print method for unbalanced "
                "begin_group/end_group calls."
            )
            return
        for line in output.splitlines():
            self._global.sink(line)

    # -- spans -----------------------------------------------------------

    def start_span(self, label: int) -> None:
        """Open a span labelled ``label`` grouping the draws until the
        matching :meth:`stop_span`. Spans tell the shrinker which choices
        belong together; :meth:`draw` and :meth:`draw_silent` open one per
        draw, so only a low-level custom generator needs this. Nesting
        deeper than ``MAX_DRAW_DEPTH`` abandons the test case as too
        large."""
        if self._span_depth >= MAX_DRAW_DEPTH:
            raise StopTest(f"draws nested more than {MAX_DRAW_DEPTH} deep")
        self._span_depth += 1
        try:
            self._handle.start_span(label)
        except BaseException:
            self._span_depth -= 1
            raise

    def stop_span(self, discard: bool = False) -> None:
        """Close the span opened by the matching :meth:`start_span`. With
        ``discard=True`` the shrinker treats the span's draws as thrown
        away (a filtered-out attempt, say)."""
        if self._span_depth <= 0:
            raise InternalError("stop_span called with no open span")
        self._span_depth -= 1
        self._handle.stop_span(discard)

    @property
    def _open_span_depth(self) -> int:
        return self._span_depth

    def _reset_open_spans_to(self, depth: int) -> None:
        if self._span_depth < depth:
            raise InternalError("open span depth decreased unexpectedly")
        self._span_depth = depth

    # -- primitive draws -------------------------------------------------
    # The engine's primitive choices, which the built-in generators are made
    # of. Private, as in the Rust library: a custom generator draws through
    # draw() or draw_silent().

    def _generate_boolean(self, p: float = 0.5, forced: bool | None = None) -> bool:
        return self._handle.generate_boolean(p, forced)

    def _generate_integer(self, min_value: int, max_value: int) -> int:
        return self._handle.generate_integer(min_value, max_value)

    def _generate_float(
        self,
        width: int,
        min_value: float,
        max_value: float,
        allow_nan: bool,
        allow_infinity: bool,
        exclude_min: bool,
        exclude_max: bool,
        smallest_nonzero_magnitude: float,
    ) -> float:
        return self._handle.generate_float(
            width,
            min_value,
            max_value,
            allow_nan,
            allow_infinity,
            exclude_min,
            exclude_max,
            smallest_nonzero_magnitude,
        )

    def _generate_bytes(self, min_size: int, max_size: int) -> bytes:
        return self._handle.generate_bytes(min_size, max_size)

    def _generate_string(self, generator: _native.StringGenerator) -> str:
        return self._handle.generate_string(generator).decode("utf-8", "surrogatepass")

    def _generate_date(self, min_value: _dt.date, max_value: _dt.date) -> _dt.date:
        year, month, day = self._handle.generate_date(
            (min_value.year, min_value.month, min_value.day),
            (max_value.year, max_value.month, max_value.day),
        )
        return _dt.date(year, month, day)

    def _generate_time(self, min_value: _dt.time, max_value: _dt.time) -> _dt.time:
        hour, minute, second, nanosecond = self._handle.generate_time(
            (min_value.hour, min_value.minute, min_value.second, min_value.microsecond * 1000),
            (max_value.hour, max_value.minute, max_value.second, max_value.microsecond * 1000),
        )
        return _dt.time(hour, minute, second, nanosecond // 1000)

    def _generate_datetime(self, min_value: _dt.datetime, max_value: _dt.datetime) -> _dt.datetime:
        result = self._handle.generate_datetime(
            (
                min_value.year,
                min_value.month,
                min_value.day,
                min_value.hour,
                min_value.minute,
                min_value.second,
                min_value.microsecond * 1000,
            ),
            (
                max_value.year,
                max_value.month,
                max_value.day,
                max_value.hour,
                max_value.minute,
                max_value.second,
                max_value.microsecond * 1000,
            ),
        )
        year, month, day, hour, minute, second, nanosecond = result
        return _dt.datetime(year, month, day, hour, minute, second, nanosecond // 1000)

    def _generate_timedelta(
        self, min_value: _dt.timedelta, max_value: _dt.timedelta
    ) -> _dt.timedelta:
        seconds, nanoseconds = self._handle.generate_duration(
            _duration_of(min_value), _duration_of(max_value)
        )
        return _dt.timedelta(seconds=seconds, microseconds=nanoseconds // 1000)

    def _generate_timezone(self, spec: _native.TimeZoneSpec) -> str:
        return self._handle.generate_timezone(spec)

    def _generate_zoned_datetime(
        self, tz_spec: _native.TimeZoneSpec, zoned_spec: _native.ZonedSpec
    ) -> tuple[str, _dt.datetime]:
        """Draw a zone and a wall-clock time in it, as the zone's name and a
        naive datetime carrying the fold. A wall time outside the years
        :mod:`datetime` represents, which instant bounds near its limits can
        reach in a zone with a large offset, rejects the test case."""
        key, civil, fold, _, _, _, _ = self._handle.generate_zoned_datetime(tz_spec, zoned_spec)
        year, month, day, hour, minute, second, nanosecond = civil
        if not _dt.MINYEAR <= year <= _dt.MAXYEAR:
            self.reject()
        value = _dt.datetime(year, month, day, hour, minute, second, nanosecond // 1000)
        return key, value.replace(fold=int(fold))

    def _generate_uuid(self, version: int | None = None) -> bytes:
        return self._handle.generate_uuid(version)

    def _generate_ipv4(self) -> bytes:
        return self._handle.generate_ipv4()

    def _generate_ipv6(self) -> bytes:
        return self._handle.generate_ipv6()

    def __repr__(self) -> str:
        return "TestCase()"


class Collection:
    """A low-level helper for custom collection generators: the engine
    decides how many elements to draw, within ``[min_size, max_size]``
    (``max_size=None`` for no fixed limit), so the shrinker can remove
    elements cleanly. Call :meth:`more` before drawing each element::

        collection = hegel.Collection(tc, min_size, max_size)
        result = []
        while collection.more():
            result.append(tc.draw_silent(elements))
    """

    __slots__ = ("_finished", "_handle", "_max_size", "_min_size", "_tc")

    def __init__(self, tc: TestCase, min_size: int, max_size: int | None) -> None:
        self._tc = tc
        self._min_size = min_size
        self._max_size = max_size
        self._handle: _native.Collection | None = None
        self._finished = False

    def _ensure(self) -> _native.Collection:
        if self._handle is None:
            self._handle = self._tc._handle.new_collection(self._min_size, self._max_size)
        return self._handle

    def more(self) -> bool:
        """Whether to draw another element."""
        if self._finished:
            return False
        handle = self._ensure()
        try:
            result = handle.more(self._tc._handle)
        except BaseException:
            self._finished = True
            raise
        if not result:
            self._finished = True
        return result

    def reject(self, why: str | None = None) -> None:
        """Discard the element just drawn (a duplicate, say) so it does not
        count toward the size."""
        if self._finished:
            return
        self._ensure().reject(self._tc._handle, why)


def _duration_of(delta: _dt.timedelta) -> tuple[int, int]:
    """A timedelta as the engine's ``(seconds, nanoseconds)`` of one sign."""
    microseconds = delta // _dt.timedelta(microseconds=1)
    seconds, remainder = divmod(abs(microseconds), 1_000_000)
    sign = -1 if microseconds < 0 else 1
    return sign * seconds, sign * remainder * 1000
