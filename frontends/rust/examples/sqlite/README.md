# Bank workload against SQLite

An example of testing an ACID database with Hegel's concurrent stateful testing.
The package lives outside the Cargo workspace so that compiling SQLite is only
ever paid for by someone running this example. It runs two ways:

```sh
cd frontends/rust/examples/sqlite
cargo test --release                       # a Hegel test suite (multiple test cases)
cargo run --release                        # a single workload (one test case)
cargo run --release --features inject-bug  # a broken transfer; the run fails
```
