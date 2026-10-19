Printing values
===============

Hegel prints drawn values with their ``repr``, laid out to fit the width of the
report. These functions and classes give control over that layout, for
generators with values that print better some other way. Most code never
needs them; :meth:`Generator.print_with <hegel.generators.Generator.print_with>`
covers the common case of a custom representation.

.. currentmodule:: hegel

.. autofunction:: pretty_repr

.. autofunction:: pretty_print

.. autoclass:: PrettyPrinter()
   :members:

.. autoclass:: Collection()
   :members:
