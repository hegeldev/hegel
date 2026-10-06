"""hegel.hypothesis strategies wrap Hegel generators rather than being them."""

import pytest

import hegel
from hegel import generators as gs
from hegel.hypothesis import strategies as st


def test_strategies_wrap_generators():
    s = st.integers()
    assert isinstance(s, st.SearchStrategy)
    assert not isinstance(s, gs.Generator)
    assert isinstance(s.generator, gs.Generator)
    s.validate()
    assert s.is_empty is False
    assert st.nothing().is_empty is True
    assert isinstance(s.flatmap(lambda n: st.just(n)).generator, gs.Generator)


def test_example_is_minimal():
    assert st.integers(5, 5).example() == 5
    assert st.lists(st.integers(), min_size=2).example() == [0, 0]
    with pytest.raises(hegel.InvalidArgument, match="produced no example"):
        st.integers().filter(lambda x: False).example()


class _Pairs(st.SearchStrategy):
    def do_draw(self, data):
        return (data.draw(st.integers()), data.draw(st.booleans()))


def test_custom_strategy_draws_through_a_data_object():
    value = _Pairs().example()
    assert value == (0, False)
    assert repr(_Pairs()) == "_Pairs()"
