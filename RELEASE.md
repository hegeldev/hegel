RELEASE_TYPE: patch

Hegel for Python now ships a `py.typed` marker, so type checkers use its inline
annotations, and `hegel.__version__` is read from the installed package
metadata rather than written out by hand.
