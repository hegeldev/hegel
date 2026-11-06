RELEASE_TYPE: patch

This patch adds `@hegel.main`, which turns a function that takes a test case into the entry point of a program. Calling the decorated function reads the command line and runs one test case. Each setting except `test_cases` is a command-line option named after its `hegel.toml` key (`--seed`, `--phases`, `--backend` and so on), and `--profile` selects a settings profile.

```python
@hegel.main
def workload(tc):
    n = tc.draw(gs.integers())
    assert n + 0 == n


if __name__ == "__main__":
    workload()
```
