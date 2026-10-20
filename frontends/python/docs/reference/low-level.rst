Low-level tools
===============

Building blocks for custom generators that need finer control over how the
engine draws, and so how it shrinks, than :func:`~hegel.generators.composite`
and the standard generators give. Most tests never need them.

.. autoclass:: hegel.Collection()
   :members:

.. autofunction:: hegel.generators.weighted_booleans

.. automethod:: hegel.TestCase.draw_and_print

.. automethod:: hegel.TestCase.start_span

.. automethod:: hegel.TestCase.stop_span
