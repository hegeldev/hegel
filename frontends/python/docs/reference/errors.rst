Errors
======

.. currentmodule:: hegel

Every error Hegel raises about a run, rather than one raised by the test
itself, is a :class:`HegelError`. The exception is :class:`MultipleFailures`,
which groups the test's own exceptions when a run reports more than one
failure.

.. autoexception:: HegelError
   :show-inheritance:

.. autoexception:: InvalidArgument
   :show-inheritance:

.. autoexception:: RunError
   :show-inheritance:

.. autoexception:: FailedHealthCheck
   :show-inheritance:

.. autoexception:: Unsatisfiable
   :show-inheritance:

.. autoexception:: Flaky
   :show-inheritance:

.. autoexception:: DidNotReproduce
   :show-inheritance:

.. autoexception:: ResolutionFailed
   :show-inheritance:

.. autoexception:: BackendError
   :show-inheritance:

.. autoexception:: InternalError
   :show-inheritance:

.. autoexception:: MultipleFailures
   :show-inheritance:

Control flow
------------

These exceptions are how Hegel ends a test case early. A test should let them
propagate.

.. autoexception:: AssumeFailed
   :show-inheritance:

.. autoexception:: StopTest
   :show-inheritance:
