"""Strategies for internet-related data (domains and URLs)."""

from __future__ import annotations

from hegel import generators as gs
from hegel.generators.base import Generator
from hegel.generators.strings import DOMAIN_LABEL
from hegel.hypothesis.errors import InvalidArgument
from hegel.hypothesis.strategies import SearchStrategy
from hegel.hypothesis.strategies._base import defines_strategy
from hegel.test_case import TestCase

__all__ = ["domains", "urls"]


def _clean_input(minimum: int, maximum: int, value: int | None, name: str) -> int:
    if value is None:
        return maximum
    if not isinstance(value, int) or isinstance(value, bool):
        raise InvalidArgument(f"Expected integer but {name} is a {type(value).__name__}")
    if not minimum <= value <= maximum:
        raise InvalidArgument(f"Invalid value {minimum!r} < {name}={value!r} < {maximum!r}")
    return value


class _Domains(Generator[str]):
    """RFC 1035 domain names whose labels are at most ``max_element_length``
    characters: a top-level domain from the engine's list, extended with
    labels drawn from a regex while the name fits in ``max_length``."""

    def __init__(self, max_length: int, max_element_length: int) -> None:
        self.max_length = max_length
        self.max_element_length = max_element_length
        if max_element_length == 1:
            label_regex = r"[a-zA-Z]"
        elif max_element_length == 2:
            label_regex = r"[a-zA-Z][a-zA-Z0-9]?"
        else:
            label_regex = r"[a-zA-Z]([a-zA-Z0-9\-]{0," f"{max_element_length - 2}" r"}[a-zA-Z0-9])?"
        self.tld = (
            gs.domains(max_length=255)
            .map(lambda d: d.rsplit(".", 1)[-1])
            .filter(lambda tld: len(tld) + 2 <= max_length)
        )
        self.labels = gs.lists(
            gs.from_regex(label_regex, fullmatch=True).filter(
                lambda label: len(label) < 4 or label[2:4] != "--"
            ),
            min_size=1,
            max_size=126,
        )
        self.label = DOMAIN_LABEL

    def do_draw(self, tc: TestCase) -> str:
        domain = tc.draw_silent(self.tld)
        for sub_domain in tc.draw_silent(self.labels):
            if len(domain) + len(sub_domain) >= self.max_length:
                break
            domain = sub_domain + "." + domain
        return domain

    def __repr__(self) -> str:
        return (
            f"domains(max_length={self.max_length}, max_element_length={self.max_element_length})"
        )


@defines_strategy
def domains(
    *, max_length: int | None = 255, max_element_length: int | None = 63
) -> SearchStrategy[str]:
    """Fully qualified domain names of at most ``max_length`` characters,
    each label at most ``max_element_length`` characters."""
    max_length = _clean_input(4, 255, max_length, "max_length")
    max_element_length = _clean_input(1, 63, max_element_length, "max_element_length")
    if max_element_length == 63:
        return SearchStrategy(gs.domains(max_length=max_length))
    return SearchStrategy(_Domains(max_length, max_element_length))


@defines_strategy
def urls() -> SearchStrategy[str]:
    return SearchStrategy(gs.urls())
