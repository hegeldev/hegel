"""The ``not_set`` sentinel used for arguments that were not passed."""

from __future__ import annotations

from typing import Any


class UniqueIdentifier:
    """A placeholder with a readable repr, compared by identity."""

    def __init__(self, identifier: str) -> None:
        self.identifier = identifier

    def __repr__(self) -> str:
        return self.identifier


not_set: Any = UniqueIdentifier("not_set")
