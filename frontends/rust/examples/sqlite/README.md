# Bank workload against SQLite

An example of testing an ACID database with Hegel's concurrent stateful
testing. The database is SQLite in WAL mode; the workload is a classic one
for checking transactional guarantees under concurrency.

The package lives outside the Cargo workspace so that compiling SQLite is
only ever paid for by someone running this example. It runs two ways:

```sh
cd frontends/rust/examples/sqlite
cargo test --release                       # a Hegel test: 10 test cases
cargo run --release                        # the binary: one run per invocation
cargo run --release --features inject-bug  # a broken transfer; the run fails
cargo run --release -- --help              # Hegel's own flags
```

The test is an ordinary `#[hegel::test]` over the workload, fixed at 10 test
cases, so a failure is shrunk and reported like any other. The binary is a
`#[hegel::main]` program: every invocation runs exactly one test case, and a
failure is shrunk, reported, and persisted to the failure database so that
the next invocation replays it.

## The workload

**Init.** Create `accounts(account_id, balance, opCount)` with `N` rows, each
with balance `B` and `opCount` 0. Remember `Total = N * B`, and keep two
client-side counters: `Attempted` and `Committed`.

**Transfer** (a rule, run concurrently). Pick a random source `s`,
destination `d` and amount `a`, increment `Attempted`, and run one
transaction:

```
IF s.balance < a
  ABORT
ELSE
  s.balance -= a; d.balance += a
  s.opCount += 1; d.opCount += 1
```

If the transaction commits, increment `Committed`. A transaction that does
not commit was either aborted, or committed without the client learning of
it (a timeout or disconnection). The workload does not try to tell these
apart, since it is not always possible to.

**Verify total balance** (a rule and an invariant). `SELECT SUM(balance)`
must equal `Total`: no money is created or destroyed.

**Verify operations are not lost** (a rule and an invariant).
`SELECT SUM(opCount)` must be even and at least `2 * Committed`. It may be
higher, because of transactions that committed without the client knowing.
If it is ever lower, a committed transaction was lost.

**Verify no balance is below zero** (a rule and an invariant).
`SELECT COUNT(*) FROM accounts WHERE balance < 0` must be 0.

The verifications run both as rules, which Hegel interleaves with transfers
on worker threads, and as invariants, which Hegel checks on the initial and
final state and samples between rounds when no rule is running. The rules
test snapshot isolation under contention; the final invariant check is the
definitive test that no commit was lost.

## Sizing a run

Every test case draws its own bank: `N` accounts in `1..=16`, a starting
balance `B` in `0..=1000`, and a largest transfer amount in `0..=2B+10`,
larger than `B` so that the insufficient-funds path is exercised regularly.
It then runs 200 rounds on 2 to 8 worker threads. The ranges are in
`WorkloadConfig::draw` in `src/lib.rs`.

Inside Antithesis, Hegel detects `ANTITHESIS_OUTPUT_DIR` and switches to its
`workload` profile: every draw is handed to the fuzzer, the failure database
is disabled, and wall-clock health checks are suppressed.
