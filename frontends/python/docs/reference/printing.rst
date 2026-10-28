Printing values
===============

.. currentmodule:: hegel

When a test fails, Hegel replays the minimal failing example and prints every
drawn value as a ``name = value`` line, laid out by the engine to fit the width
of the report::

    records = [Record(name='', tags=None, scores={}),
     Record(name='', tags=None, scores={}),
     Record(name='', tags=None, scores={})]

Printing is the generator's job
-------------------------------

A value cannot always print itself usefully: a drawn function has no helpful
``repr``, and some values can only be described after they are used. The
generator that drew a value can always describe it, so in Hegel printing is a
capability of the generator, not the value. A printable generator draws and
prints in one step, in its
:meth:`~hegel.generators.Generator.do_draw_and_print` method, writing to a
:class:`PrettyPrinter` as it goes. :meth:`TestCase.draw` needs a printable
generator, so that the report can say what was drawn;
:meth:`TestCase.draw_silent` accepts any generator and reports nothing.

Every generator Hegel provides is printable:

- Every leaf generator prints the ``repr`` of what it draws: integers, floats,
  booleans, text and characters, regexes, bytes, dates and times, UUIDs, IP
  addresses, emails, URLs and grammars.
- Structural generators print as they draw, element by element and argument by
  argument: :func:`~hegel.generators.lists`, :func:`~hegel.generators.sets`,
  :func:`~hegel.generators.dicts`, :func:`~hegel.generators.fixed_dicts`,
  :func:`~hegel.generators.tuples`, :func:`~hegel.generators.optional`,
  :func:`~hegel.generators.one_of`, :meth:`~hegel.generators.Generator.filter`,
  :meth:`~hegel.generators.Generator.flat_map`,
  :func:`~hegel.generators.recursive`, :func:`~hegel.generators.deferred` and
  :func:`~hegel.generators.builds`. An element's own printing (a
  :meth:`~hegel.generators.Generator.print_with` function, a custom generator)
  shows up inside its container. They are printable whenever their components
  are.
- Value-producing generators print the ``repr`` of the finished value:
  :meth:`~hegel.generators.Generator.map`, :func:`~hegel.generators.just`,
  :func:`~hegel.generators.sampled_from`, :func:`composite` and the stateful
  pools. A composite's inner draws never appear in the report, only the value
  it returns.

Python has no static types to erase, so a generator keeps its own printing
wherever it is passed: there is nothing like the Rust library's ``boxed``
to worry about.

When a draw is not printable
----------------------------

A :class:`~hegel.generators.Generator` subclass that implements only
``do_draw`` cannot print what it draws. Drawing it with :meth:`TestCase.draw`,
directly or inside a structure, raises :class:`NotPrintable` on the first test
case that reaches it. The fixes, in order of preference:

- Implement ``do_draw_and_print`` (below), so the generator prints itself and
  gets ``do_draw`` for free.
- :meth:`~hegel.generators.Generator.print_with`: print a custom representation
  from a function taking the value and the printer, such as
  ``.print_with(lambda value, printer: printer.text(f"make({value.id})"))``.
  It is also the way to mask a secret.
- :meth:`~hegel.generators.Generator.print_as_repr`: print the drawn value's
  ``repr``.
- :meth:`~hegel.generators.Generator.print_as_call`: on a
  :meth:`~hegel.generators.Generator.map` of a printable generator, print the
  input instead of the output, as ``Point((0, 0))``.
- :meth:`TestCase.draw_silent`: draw without reporting the value, when it is
  not worth reporting.

``print_with`` and ``print_as_repr`` print a finished value: they are escape
hatches for generators that do not print themselves, not the way built-in
generators print.

Writing a printable generator
-----------------------------

A printable generator implements ``do_draw_and_print``. It draws its
components with :meth:`TestCase.draw_and_print`, passing the printer on, and
writes its own syntax around them::

    class Coords(gs.Generator[tuple[int, int]]):
        def do_draw_and_print(self, tc, printer):
            with printer.group("Coord(", ")"):
                x = tc.draw_and_print(gs.integers(0, 9), printer)
                printer.separator()
                y = tc.draw_and_print(gs.integers(0, 9), printer)
            return (x, y)

``tc.draw(gs.lists(Coords(), min_size=1))`` then reports ``xs = [Coord(0, 0)]``.
When the generator is drawn silently, the same method runs with a printer that
prints nothing, and its components are drawn silently too, so it draws the
same choices either way.

A generator whose value can only be described after it is drawn opens a
:meth:`deferred <PrettyPrinter.deferred>` region at draw time and hands it to
the value, which records into it as it is used::

    class Recorder:
        def __init__(self, tc, printer):
            self._tc, self._printer, self._first = tc, printer, True

        def next(self):
            value = self._tc.draw_silent(gs.integers(0, 10))
            if not self._first:
                self._printer.separator()
            self._first = False
            self._printer.repr(value)
            return value


    class Recorders(gs.Generator[Recorder]):
        def do_draw_and_print(self, tc, printer):
            printer.begin_group(len("Recorder(["), "Recorder([")
            slot = printer.deferred()
            printer.end_group("])")
            return Recorder(tc, slot)

``r = tc.draw(Recorders())`` followed by two calls of ``r.next()`` reports
``r = Recorder([0, 0])``, in the place where ``r`` was drawn.

The printer
-----------

:class:`PrettyPrinter` is the write surface a generator prints through. The
engine lays its output out with an Oppen-style pretty-printer: :meth:`text
<PrettyPrinter.text>` emits literal text, :meth:`breakable
<PrettyPrinter.breakable>` marks a point that renders as a separator when the
enclosing group fits on one line and as a newline plus indentation when it
does not, and :meth:`begin_group <PrettyPrinter.begin_group>` /
:meth:`end_group <PrettyPrinter.end_group>` (or the :meth:`group
<PrettyPrinter.group>` context manager) delimit the groups those decisions are
made over. A combinator that may reject a draw prints each attempt into a
:meth:`speculation <PrettyPrinter.speculate>` and commits only the accepted
one.

A :class:`Document` is a standalone document to print into and render, for
trying out or testing printing code outside a test case::

    document = hegel.Document(max_width=8)
    document.printer.seq("{", "}", [1, 2, 3], lambda p, n: p.repr(n))
    assert document.finish() == "{1,\n 2,\n 3}"

.. autoclass:: PrettyPrinter()
   :members:

.. autoclass:: Speculation()
   :members: printer, commit, abort

.. autoclass:: Document
   :members:

.. autofunction:: pretty_repr

.. autofunction:: pretty_print
