# Changelog

## 0.1.0 - unreleased

Initial release of Hegel for Python: bindings to libhegel (`hegeltest-c` 0.45.0)
through a pyo3 extension module, the `hegel` API (`@hegel.test`, generators,
explicit test cases, failure reproduction, sequential and concurrent stateful
testing), and the `hegel.hypothesis` namespace reimplementing Hypothesis's API on
top of it.
