# Run tests on Antithesis

[Antithesis](https://antithesis.com) runs software in a deterministic
simulation and fuzzes it. Hegel tests can be used to write Antithesis workloads.

When running inside Antithesis, Hegel automatically uses the `workload` settings 
profile. The `workload` profile:

- uses the `urandom` {class}`~hegel.Backend`, which takes every choice from the
  Antithesis fuzzer rather than from Hegel's own random number generator
- turns off the example database
- suppresses every health check

Each test's outcome is reported as an Antithesis assertion, so failures appear 
in the Antithesis triage report.

## Write a workload program

{func}`hegel.main` turns a function that takes a test case into a program.
Each run of the program runs one test case.

```python
import hegel
from hegel import generators as gs


@hegel.main
def workload(tc):
    n = tc.draw(gs.integers())
    assert n + 0 == n


if __name__ == "__main__":
    workload()
```

Each setting is a command-line option named after its `hegel.toml` key. To
run the program with the `workload` profile outside Antithesis:

```bash
python workload.py --profile=workload
```

If the test case fails, the program exits with status 1 and prints a
`rerun with: --failure-blob BLOB` line. Run the program with that option to
replay the failure.
