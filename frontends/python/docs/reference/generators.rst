Generators
==========

.. module:: hegel.generators

All generators live in ``hegel.generators``, conventionally imported as ``gs``:

.. code-block:: python

   from hegel import generators as gs

A generator is an immutable description of how to produce values. Building one
validates its arguments, so a bad argument raises
:class:`~hegel.InvalidArgument` where the generator is defined, and the same
generator can be drawn from any number of times, in any number of tests.

Numbers and booleans
--------------------

.. autofunction:: integers

.. autofunction:: floats

.. autofunction:: booleans

Text and bytes
--------------

.. autofunction:: text

.. autofunction:: characters

.. autofunction:: from_regex

.. autofunction:: from_lark

.. autofunction:: binary

Collections
-----------

.. autofunction:: lists

.. autofunction:: sets

.. autofunction:: frozensets

.. autofunction:: dicts

.. autofunction:: fixed_dicts

.. autofunction:: tuples

.. autofunction:: iterables

Choosing from values
--------------------

.. autofunction:: sampled_from

.. autofunction:: just

.. autofunction:: none

.. autofunction:: nothing

.. autofunction:: permutations

.. autofunction:: samples

.. autofunction:: subsequences

Combining generators
--------------------

.. autofunction:: one_of

.. autofunction:: optional

.. autofunction:: composite

.. autofunction:: builds

.. autofunction:: recursive

.. autofunction:: deferred

Inferring generators from types
-------------------------------

.. autofunction:: from_type

.. autofunction:: register_type_generator

Dates and times
---------------

.. module:: hegel.generators.temporal

The date and time generators live in the ``temporal`` namespace, with the
names every Hegel frontend uses for them:

.. code-block:: python

   when = tc.draw(gs.temporal.datetimes(zones=gs.temporal.zones()))

.. autofunction:: dates

.. autofunction:: times

.. autofunction:: datetimes

.. autofunction:: zoned_datetimes

.. autofunction:: instants

.. autofunction:: durations

.. autofunction:: spans

.. autoclass:: Span
   :members:

.. autofunction:: zones

.. autofunction:: zone_names

.. autofunction:: offsets

.. autofunction:: tzdb_version

Time zone data
~~~~~~~~~~~~~~

Zoned draws aim at the gaps and folds of the zone's transitions, so they
need the same time zone data as the code under test. They draw against a
:class:`ZoneSource`: by default :meth:`ZoneSource.zoneinfo`, the database
:mod:`zoneinfo` itself reads (the system's zone files, or the ``tzdata``
package), so a zone has exactly the transitions its
:class:`~zoneinfo.ZoneInfo` has. Where :mod:`zoneinfo` finds no database
at all (Windows without the ``tzdata`` package), a generator that needs
the default source's zones raises :class:`~hegel.InvalidArgument` when it
is built, saying to install ``tzdata``.

:func:`zones`, :func:`zone_names`, :func:`datetimes` and
:func:`zoned_datetimes` take a ``source`` to draw against another one, and
:func:`set_default_zone_source` changes the default. A generator reads its
source, and the names the source knows, when it is built, so build a
source once, outside the test body, and set the default before building
generators:

.. code-block:: python

   BUNDLED = gs.temporal.ZoneSource.bundled()
   zoned = gs.temporal.zoned_datetimes(source=BUNDLED)
   zones = gs.temporal.zones(source=BUNDLED)

An integration with another date and time library fills a ``ZoneSource()``
with that library's data: TZif files, :class:`ZoneData` given as fields,
or a :class:`ZoneOracle` it answers lookups for, added up front or loaded
the first time a draw needs a zone. :meth:`ZoneSource.info`,
:meth:`ZoneSource.resolve` and :meth:`ZoneSource.offset_at` read back what
the engine made of it.

.. autoclass:: ZoneSource
   :members:
   :special-members: __contains__

.. autofunction:: default_zone_source

.. autofunction:: set_default_zone_source

.. autoclass:: ZoneNameInfo

.. autoclass:: ZoneResolution

.. autoclass:: ZoneType

.. autoclass:: ZoneData

.. autoclass:: ZoneRule

.. autoclass:: RuleDate
   :members: julian, zero_based, month_week_day

.. autoclass:: ZoneOracle

.. currentmodule:: hegel.generators

The top-level names the date and time generators had before they moved,
``gs.dates``, ``gs.times``, ``gs.datetimes`` (with its ``timezones=``
argument), ``gs.timedeltas`` and ``gs.timezones``, still work, with the
arguments they were released with, but emit a :class:`DeprecationWarning`;
use the ``gs.temporal`` names instead.

Identifiers and network values
------------------------------

.. autofunction:: uuids

.. autofunction:: ip_addresses

.. autofunction:: emails

.. autofunction:: domains

.. autofunction:: urls

The generator class
-------------------

Every generator is an instance of :class:`Generator`, which provides the
methods for transforming one generator into another. Subclassing it directly is
rarely necessary: :func:`composite` covers almost every case. A subclass that
should be drawn with :meth:`TestCase.draw <hegel.TestCase.draw>` implements
``do_draw_and_print``, which prints the value as it draws it; see
:doc:`printing`.

.. autoclass:: Generator
   :members: map, filter, flat_map, print_with, print_as_repr, print_as_call, do_draw, do_draw_and_print
