# Bank workload against SQLite

An example of testing an ACID database with Hegel's concurrent stateful
testing. The database is SQLite in WAL mode.

```sh
uv run python examples/sqlite_bank/main.py                    # one run per invocation
BANK_INJECT_BUG=1 uv run python examples/sqlite_bank/main.py  # a broken transfer; the run fails
```

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
not commit was either aborted or committed without the client learning of
it (a timeout or disconnection).

**Verify total balance** (a rule and an invariant). `SELECT SUM(balance)`
must equal `Total`: no money is created or destroyed.

**Verify operations are not lost** (a rule and an invariant).
`SELECT SUM(opCount)` must be even and at least `2 * Committed`. It may be
higher, because of transactions that committed without the client knowing.
If it is ever lower, a committed transaction was lost.

**Verify no balance is below zero** (a rule and an invariant).
`SELECT COUNT(*) FROM accounts WHERE balance < 0` must be 0.

The verifications run both as rules, which Hegel runs alongside transfers
on worker threads, and as invariants, which Hegel checks on the initial and
final state and sometimes between rounds, when no rule is running. The rules
test snapshot isolation while transfers compete for the database. The
invariant check on the final state confirms that no commit was lost.

## Sizing a run

Every parameter can be fixed using environment variables. Whatever is left unset
is drawn by Hegel.

| Variable | Meaning | Unset |
|---|---|---|
| `BANK_ACCOUNTS` | `N`, number of accounts | drawn, 1..=16 |
| `BANK_INITIAL_BALANCE` | `B`, starting balance per account | drawn, 0..=1000 |
| `BANK_MAX_AMOUNT` | largest transfer amount | drawn, 0..=2B+10 |
| `BANK_STEPS` | rounds per test case | 200 |
| `BANK_MIN_CONCURRENCY` | fewest worker threads | 2 |
| `BANK_MAX_CONCURRENCY` | most worker threads | 8 |
| `BANK_INJECT_BUG` | set to skip the balance check in transfers | unset |

Inside Antithesis, Hegel uses its `workload` settings profile.
