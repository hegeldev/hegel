"""Example database objects accepted by ``settings(database=...)``.

Hegel keeps its own example database on disk, so these classes only carry
the directory the engine should use. An in-memory database is a temporary
directory that lives as long as the object does.
"""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

__all__ = [
    "DirectoryBasedExampleDatabase",
    "ExampleDatabase",
    "InMemoryExampleDatabase",
    "ReadOnlyDatabase",
]


class ExampleDatabase:
    """Base class: ``_hegel_path`` is the directory handed to the engine."""

    _hegel_path: str = ""

    def __repr__(self) -> str:
        return f"{type(self).__name__}()"


class DirectoryBasedExampleDatabase(ExampleDatabase):
    def __init__(self, path: str | os.PathLike[str]) -> None:
        self.path = Path(path)
        self._hegel_path = str(self.path)

    def __repr__(self) -> str:
        return f"DirectoryBasedExampleDatabase({str(self.path)!r})"

    def __eq__(self, other: object) -> bool:
        return isinstance(other, DirectoryBasedExampleDatabase) and other.path == self.path

    def __hash__(self) -> int:
        return hash(self.path)


class InMemoryExampleDatabase(ExampleDatabase):
    """A database that does not outlive the process."""

    def __init__(self) -> None:
        self._tempdir = tempfile.TemporaryDirectory(prefix="hegel-db-")
        self._hegel_path = self._tempdir.name

    def __repr__(self) -> str:
        return "InMemoryExampleDatabase({})"


class ReadOnlyDatabase(ExampleDatabase):
    """Wraps another database; Hegel has no read-only mode, so this shares
    the wrapped database's directory."""

    def __init__(self, db: ExampleDatabase) -> None:
        self._wrapped = db
        self._hegel_path = db._hegel_path

    def __repr__(self) -> str:
        return f"ReadOnlyDatabase({self._wrapped!r})"
