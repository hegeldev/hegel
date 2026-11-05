RELEASE_TYPE: patch

This patch adds `HEGEL_MAIN` for running a Hegel test as a standalone binary. 
Each setting except `test_cases` is a command-line option named after its `hegel.toml` 
key (`--seed`, `--phases`, `--backend` and so on).

```cpp
HEGEL_MAIN(workload)(hegel::TestCase& tc) {
    auto n = tc.draw("n", gs::integers<int>());
    if (n + 0 != n) {
        throw std::runtime_error("n + 0 != n");
    }
}
```
