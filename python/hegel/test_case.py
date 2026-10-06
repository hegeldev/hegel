"""The test case: the object a test body draws values from."""

from __future__ import annotations

import datetime as _dt
import sys
import threading
from collections.abc import Callable
from typing import TYPE_CHECKING, Any, TypeVar

from hegel import _native
from hegel._draw_names import name_for_call
from hegel.errors import AssumeFailed, InternalError, InvalidArgument, LoopDone, StopTest
from hegel.pretty import DEFAULT_MAX_WIDTH, PrettyPrinter

if TYPE_CHECKING:
    from hegel.generators.base import Generator

__all__ = ["Collection", "OutputSink", "TestCase"]

T = TypeVar("T")

OutputSink = Callable[[str], None]

MIN_DATE = (1, 1, 1)
MAX_DATE = (9999, 12, 31)
MIDNIGHT = (0, 0, 0, 0)
LAST_NANOSECOND = (23, 59, 59, 999_999_999)
MIN_DATETIME = MIN_DATE + MIDNIGHT
MAX_DATETIME = MAX_DATE + LAST_NANOSECOND


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

    __slots__ = ("allocated", "counts", "lock", "unnamed")

    def __init__(self) -> None:
        self.counts: dict[str, int] = {}
        self.allocated: set[str] = set()
        self.unnamed = 0
        self.lock = threading.Lock()


class TestCase:
    """One execution of a property test: draws values from the engine,
    records notes, assumptions, and targets.

    Test bodies receive an instance; they never construct one. Each
    instance owns a libhegel handle and may be driven by one thread at a
    time. For another thread, :meth:`clone` a handle with an independent
    choice stream.
    """

    __test__ = False  # not a pytest test class

    __slots__ = (
        "_draw_state",
        "_global",
        "_handle",
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

    @classmethod
    def _derive(cls, parent: TestCase, handle: _native.TestCase) -> TestCase:
        tc = cls.__new__(cls)
        tc._handle = handle
        tc._global = parent._global
        tc._span_depth = 0
        tc._printing_depth = 0
        tc._printer = None
        tc._draw_state = _DrawState()
        return tc

    # -- drawing ---------------------------------------------------------

    def draw(self, generator: Generator[T], name: str | None = None) -> T:
        """Draw a value from ``generator`` and report it.

        The value is printed, as ``name = value``, into the output shown
        for a failing test case. When ``name`` is omitted it is inferred
        from the assignment the call appears in (``x = tc.draw(...)``), and
        an unassigned draw reports as ``Draw N: value``.
        """
        explicit = self._global.explicit
        if explicit is None and (self._span_depth > 0 or self._printing_depth > 0):
            return self.draw_silent(generator)
        if name is None:
            frame = sys._getframe(1)
            name = name_for_call(frame.f_code.co_filename, frame.f_lineno)
        if explicit is not None:
            return explicit.draw(self, generator, name)
        display = self._allocate_display_name(name)
        if display is None:
            return self.draw_silent(generator)
        self._printing_depth += 1
        try:
            printer = self._with_printer()
            with printer.speculate() as speculation:
                printer.text(display)
                value = self.draw_and_print(generator, printer)
                printer.hard_break()
                speculation.commit()
            return value
        finally:
            self._printing_depth -= 1

    def draw_silent(self, generator: Generator[T]) -> T:
        """Draw a value without reporting it. The draw still runs inside a
        span labelled with the generator's label."""
        explicit = self._global.explicit
        if explicit is not None:
            return explicit.draw_silent(self, generator)
        self.start_span(generator.label)
        value = generator.do_draw(self)
        self.stop_span(False)
        return value

    def draw_and_print(self, generator: Generator[T], printer: PrettyPrinter) -> T:
        """Draw a value, printing its representation through ``printer``.
        This is how compositional generators draw their components."""
        self.start_span(generator.label)
        value = generator.do_draw_and_print(self, printer)
        self.stop_span(False)
        return value

    def _allocate_display_name(self, name: str | None) -> str | None:
        state = self._draw_state
        with state.lock:
            if name is None:
                state.unnamed += 1
                if not self._global.emit:
                    return None
                return f"Draw {state.unnamed}: "
            count = state.counts.get(name, 0) + 1
            state.counts[name] = count
            if not self._global.emit:
                return None
            candidate = name if count == 1 else f"{name}_{count}"
            suffix = count
            while candidate in state.allocated:
                suffix += 1
                candidate = f"{name}_{suffix}"
            state.allocated.add(candidate)
            return f"{candidate} = "

    # -- control flow ----------------------------------------------------

    def assume(self, condition: object) -> None:
        """Reject the current test case unless ``condition`` holds."""
        if not condition:
            self.reject()

    def reject(self) -> None:
        """Reject the current test case."""
        explicit = self._global.explicit
        if explicit is not None:
            explicit.reject()
        raise AssumeFailed()

    def note(self, message: str) -> None:
        """Record a message shown with the drawn values of a failing test
        case (or in verbose mode)."""
        explicit = self._global.explicit
        if explicit is not None:
            explicit.note(str(message))
            return
        if not self._global.emit:
            return
        self._handle.note(str(message))

    def target(self, score: float, label: str = "") -> None:
        """Guide generation toward maximising ``score`` (once per label per
        test case)."""
        if self._global.explicit is not None:
            return
        self._handle.target(float(score), label)

    def event(self, label: str) -> None:
        """Record an event for the run's statistics."""
        if self._global.explicit is not None:
            return
        self._handle.event(str(label))

    def event_value(self, label: str, value: float) -> None:
        """Record a numeric observation for the run's statistics."""
        if self._global.explicit is not None:
            return
        self._handle.event_value(float(value), str(label))

    def repeat(self, body: Callable[[], Any]) -> None:
        """Run ``body`` an engine-chosen number of times, then end the test
        case. Each repetition is reported under a ``// Repetition #N``
        heading. This never returns normally: it raises an internal
        control-flow signal that ends the test body successfully."""
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
                self._restore_handle(outer)
                self.draw_silent(booleans())
                raise
            finally:
                self._restore_handle(outer)
        raise LoopDone()

    # -- handles and output ----------------------------------------------

    def child(self, extra_indent: int = 2) -> TestCase:
        """An instance for an indented section of this one's output (a
        stateful rule body, say): it draws from the same choice stream, and
        every line it prints is nested ``extra_indent`` columns further in."""
        return TestCase._derive(self, self._handle.block(extra_indent))

    def clone(self) -> TestCase:
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
                "uses: check any __hegel_pretty__ method for unbalanced "
                "begin_group/end_group calls."
            )
            return
        for line in output.splitlines():
            self._global.sink(line)

    @property
    def is_nondeterministic(self) -> bool:
        return self._handle.is_nondeterministic()

    # -- spans -----------------------------------------------------------

    def start_span(self, label: int) -> None:
        """Open a span labelled ``label`` grouping the draws until the
        matching :meth:`stop_span`."""
        self._span_depth += 1
        try:
            self._handle.start_span(label)
        except BaseException:
            self._span_depth -= 1
            raise

    def stop_span(self, discard: bool = False) -> None:
        if self._span_depth <= 0:
            raise InternalError("stop_span called with no open span")
        self._span_depth -= 1
        self._handle.stop_span(discard)

    @property
    def open_span_depth(self) -> int:
        return self._span_depth

    def _reset_open_spans_to(self, depth: int) -> None:
        if self._span_depth < depth:
            raise InternalError("open span depth decreased unexpectedly")
        self._span_depth = depth

    # -- primitive draws -------------------------------------------------

    def generate_boolean(self, p: float = 0.5, forced: bool | None = None) -> bool:
        return self._handle.generate_boolean(p, forced)

    def generate_integer(self, min_value: int, max_value: int) -> int:
        return self._handle.generate_integer(min_value, max_value)

    def generate_float(
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

    def generate_bytes(self, min_size: int, max_size: int) -> bytes:
        return self._handle.generate_bytes(min_size, max_size)

    def generate_string(self, generator: _native.StringGenerator) -> str:
        return self._handle.generate_string(generator).decode("utf-8", "surrogatepass")

    def generate_date(self, min_value: _dt.date, max_value: _dt.date) -> _dt.date:
        year, month, day = self._handle.generate_date(
            (min_value.year, min_value.month, min_value.day),
            (max_value.year, max_value.month, max_value.day),
        )
        return _dt.date(year, month, day)

    def generate_time(self, min_value: _dt.time, max_value: _dt.time) -> _dt.time:
        hour, minute, second, nanosecond = self._handle.generate_time(
            (min_value.hour, min_value.minute, min_value.second, min_value.microsecond * 1000),
            (max_value.hour, max_value.minute, max_value.second, max_value.microsecond * 1000),
        )
        return _dt.time(hour, minute, second, nanosecond // 1000)

    def generate_datetime(self, min_value: _dt.datetime, max_value: _dt.datetime) -> _dt.datetime:
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

    def generate_uuid(self, version: int | None = None) -> bytes:
        return self._handle.generate_uuid(version)

    def generate_ipv4(self) -> bytes:
        return self._handle.generate_ipv4()

    def generate_ipv6(self) -> bytes:
        return self._handle.generate_ipv6()

    def __repr__(self) -> str:
        return "TestCase()"


class Collection:
    """A backend-managed collection: asks the engine whether to draw
    another element, within ``[min_size, max_size]``."""

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
