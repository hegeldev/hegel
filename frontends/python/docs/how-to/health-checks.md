# Suppress a health check

Hegel watches each run for signs that a test is set up in a way that will make
it slow or ineffective, and fails the run with a
{class}`~hegel.FailedHealthCheck` when it sees one. For example, a test that
throws away almost every input it generates:

```python
@hegel.test
def test_multiples_of_1000(tc):
    n = tc.draw(gs.integers())
    tc.assume(n % 1000 == 0)
    ...
```

fails with a `FailedHealthCheck` whose message reads:

```
FailedHealthCheck: FilterTooMuch — it looks like this test is filtering out too
many inputs. 50 inputs were rejected by assume() or a filter while only 1 valid
inputs were generated. If this is expected, suppress the FilterTooMuch health
check with the suppress_health_check setting.
```

The best fix is usually to change the test, so that it generates the inputs it
wants directly. Here, `gs.integers().map(lambda n: n * 1000)` generates only
multiples of 1000. But when the behaviour is expected, suppress the health
check with the `suppress_health_check` setting:

```python
from hegel import HealthCheck


@hegel.test(suppress_health_check=[HealthCheck.filter_too_much])
def test_multiples_of_1000(tc):
    ...
```

The setting takes a collection of {class}`~hegel.HealthCheck` members, or their
names (`suppress_health_check=["filter_too_much"]`).
`suppress_health_check=HealthCheck.all()` suppresses every health check.

To suppress a health check for every test, set it in a profile in
`hegel.toml`:

```toml
[profiles.development]
suppress_health_check = ["too_slow"]
```

## The health checks

`filter_too_much`
: Too many test cases are being discarded, by
  {meth}`~hegel.TestCase.assume` or {meth}`~hegel.generators.Generator.filter`.

`too_slow`
: The first test cases are taking too long: more than 30 seconds in all
  before ten valid ones, counting the test body and its generators.

`test_cases_too_large`
: The test cases being generated are too large: they make too many choices.
  Suppressing this check also removes the limit on how many choices a test
  case can make.

`large_initial_test_case`
: The smallest input the test can generate is already very large, which
  usually means a generator has a large minimum size.
