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

.. autofunction:: dates

.. autofunction:: times

.. autofunction:: datetimes

.. autofunction:: timedeltas

.. autofunction:: timezones

.. autofunction:: timezone_keys

.. autofunction:: tzdb_version

.. autofunction:: utc_offsets

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
rarely necessary: :func:`composite` covers almost every case.

.. autoclass:: Generator
   :members: map, filter, flat_map, print_with, print_as_call, do_draw, do_draw_and_print
