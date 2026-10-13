import pytest

import hegel
from hegel.errors import InvalidArgument


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
        # Hegel validates most arguments when the generator is built, and
        # the rest on the first draw.
        with pytest.raises(InvalidArgument):
            generator = function(*args, **kwargs)
            hegel.run(lambda tc: tc.draw(generator), report_multiple_failures=False)

    return test_raise_invalid_argument
