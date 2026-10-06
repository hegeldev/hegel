import pytest

from hegel.hypothesis import given, settings
from hegel.hypothesis.errors import InvalidArgument


def e(a, *args, **kwargs):
    return (a, args, kwargs)


def e_to_str(elt):
    f, args, kwargs = getattr(elt, "values", elt)
    bits = list(map(repr, args))
    bits.extend(sorted(f"{k}={v!r}" for k, v in kwargs.items()))
    return "{}({})".format(f.__name__, ", ".join(bits))


def argument_validation_test(bad_args):
    @pytest.mark.parametrize(
        ("function", "args", "kwargs"), bad_args, ids=list(map(e_to_str, bad_args))
    )
    def test_raise_invalid_argument(function, args, kwargs):
        @given(function(*args, **kwargs))
        @settings(report_multiple_bugs=False)
        def test(x):
            pass

        with pytest.raises(InvalidArgument):
            test()

    return test_raise_invalid_argument
