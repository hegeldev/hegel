> [!IMPORTANT]
> We're excited you're checking out Hegel! Hegel is in beta, and we'd love for you to try it and [report any feedback](https://github.com/hegeldev/hegel/issues/new).
>
> As part of our beta, we may make breaking changes if it makes Hegel a better property-based testing library. If that instability bothers you, please check back in a few months for a stable release!
>
> See https://hegel.dev/compatibility for more details.

# Hegel for Python

* [Package](https://pypi.org/project/hegeltest/)
* [Documentation](https://hegel.readthedocs.io)
* [Website](https://hegel.dev)

`hegel-python` is a property-based testing library for Python. `hegel-python` is based on [Hypothesis](https://github.com/hypothesisworks/hypothesis), using the [Hegel](https://hegel.dev/) protocol. If you are coming from Hypothesis, [the documentation](https://hegel.readthedocs.io/en/latest/explanation/comparison-with-hypothesis.html) sets out how the two compare.

## Installation

To install: `pip install hegeltest`. The package is imported as `hegel` and needs Python 3.11 or later.

## Quickstart

Here's a quick example of how to write a Hegel test:

```python
import hegel
from hegel import generators as gs


def my_sort(ls):
    return sorted(set(ls))


@hegel.test
def test_matches_builtin(tc):
    ls = tc.draw(gs.lists(gs.integers()))
    assert my_sort(ls) == sorted(ls)
```

This test will fail when run with `pytest`! Hegel will produce a minimal failing test case for us:

```
_____________________________ test_matches_builtin _____________________________
test_readme.py:12: in test_matches_builtin
    assert my_sort(ls) == sorted(ls)
E   assert [0] == [0, 0]
E   Falsifying example:
E   ls = [0, 0]
E   To reproduce this failure, add this decorator below @hegel.test:
E       @hegel.reproduce_failure("AXicY2VgYGBkZOBiZEBhMAAAAd8AIQ==")
```

Hegel reports the minimal example showing that our sort is incorrectly dropping duplicates. If we change `my_sort()` to `sorted(ls)`, this test will then pass (because it's just comparing the standard sort against itself).

## Relationship to Hypothesis

The short version is "We're still figuring that out". Hegel and Hypothesis are both very similar libraries, built on very similar ideas. Hegel is more optimised for the sorts of workloads that you are likely to want to run on [Antithesis](https://antithesis.com/) and we are providing it partly because many people want to write their workloads in Python, and avoiding writing a Hegel for Python just because Hypothesis already existed seemed silly. 

As a rule of thumb: If you just want to have tests that run as part of your CI and already have Hypothesis tests, you probably want to keep using Hypothesis. If you want to write a workload for Antithesis, you definitely want to use Hegel. Otherwise... probably stick to Hypothesis for now, but there's not a huge amount in it and in some cases Hegel will be better and you should feel free toe xperiment.

The answer may change in future, but if and when it does it will be very easy to get an LLM to port your tests from Hypothesis to Hegel.
