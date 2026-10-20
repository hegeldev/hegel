Tests and test cases
====================

.. module:: hegel

Defining tests
--------------

.. autofunction:: hegel.test

.. autofunction:: hegel.explicit_test_case

.. autofunction:: hegel.reproduce_failure

.. autofunction:: hegel.run

.. autofunction:: hegel.composite

The test case
-------------

.. autoclass:: hegel.TestCase()
   :members: draw, draw_silent, assume, reject, note, target, event, event_value, repeat

.. autoclass:: hegel.Collection()
   :members:

Settings
--------

:doc:`settings` describes how settings are resolved, the profiles, and
`hegel.toml`.

.. autoclass:: hegel.Settings
   :members: from_profile, register_profile, set_default_profile, replace

.. autoclass:: hegel.Verbosity
   :members:
   :undoc-members:

.. autoclass:: hegel.Phase
   :members:
   :undoc-members:

.. autoclass:: hegel.HealthCheck
   :members:
   :undoc-members:

.. autoclass:: hegel.Backend
   :members:
   :undoc-members:

.. autoclass:: hegel.NondeterminismStrictness
   :members:
   :undoc-members:

Explicit test cases and locations
---------------------------------

.. autoclass:: hegel.ExplicitCase

.. autoclass:: hegel.TestLocation

Versions
--------

.. autodata:: hegel.__version__
   :no-value:

.. autofunction:: hegel.engine_version
