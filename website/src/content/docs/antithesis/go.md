---
title: Using Hegel for Go for Antithesis workloads
sidebar:
  label: Go
---

This is a simple test workload we call the bank test, which checks atomicity and isolation in a PostgreSQL-compatible database and a functional correctness property
on a model of a bank. Note that some code is omitted for brevity. See [Further reading](#further-reading) for the full code.

## Bank state

```go
const (
	totalBalance = int64(1_000_000)
	cursorBatch = 16
)

type bank struct {
	db     *sql.DB
	table  string
	nextID atomic.Int64
}
```

The test state `bank` stores the database handle, the name of the test case's table, and an atomic counter that assigns account IDs. The table has an `id` and a `balance` for each account. `totalBalance` is the amount of money distributed across the initial accounts.

## Database helpers

```go
func transaction(tc hegel.TestCase, db *sql.DB, opts *sql.TxOptions, fn func(*sql.Tx) error) {
	err := func() error {
		if opts == nil {
			opts = &sql.TxOptions{Isolation: sql.LevelSerializable}
		}
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()
		tx, err := db.BeginTx(ctx, opts)
		if err != nil {
			return fmt.Errorf("begin transaction: %w", err)
		}
		defer tx.Rollback()
		if err := fn(tx); err != nil {
			return err
		}
		return tx.Commit()
	}()
	if err != nil {
		tc.Note(fmt.Sprintf("SQL operation failed: %v", err))
	}
}
```

`transaction` begins a transaction, runs a callback against it, and commits if the callback succeeds. It defaults to serializable isolation when `opts` is `nil`. The deferred rollback cleans up an open transaction when the callback fails or the test case aborts.

## Setup

```go
count := hegel.Draw(tc, hegel.Integers(2, 100))
cuts := []int64{0, totalBalance}
for range count - 1 {
	cuts = append(cuts, hegel.Draw(tc, hegel.Integers(int64(0), totalBalance)))
}
slices.Sort(cuts)
ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
defer cancel()
tx, err := db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
if err != nil {
	tc.Errorf("begin initialization: %v", err)
}
defer tx.Rollback()
if _, err := tx.Exec(fmt.Sprintf(`CREATE TABLE %s ( id BIGINT NOT NULL, balance BIGINT NOT NULL )`, m.table)); err != nil {
	tc.Errorf("create accounts: %v", err)
}
for i := 1; i < len(cuts); i++ {
	if _, err := tx.Exec(fmt.Sprintf(`INSERT INTO %s (id, balance) VALUES ($1, $2)`, m.table), m.nextID.Add(1), cuts[i]-cuts[i-1]); err != nil {
		tc.Errorf("insert initial account: %v", err)
	}
}
if err := tx.Commit(); err != nil {
	tc.Errorf("commit initialization: %v", err)
}
```

The code above creates and fills the table. First, it draws an account count `count` between 2 and 100, then draws `count - 1` integers between 0 and `totalBalance`, inclusive. It then adds 0 and `totalBalance` to the list and sorts it. Each account gets the difference between two consecutive entries, `cuts[i] - cuts[i-1]`, so the initial balances sum to `totalBalance`.

## Properties

```go
func checkBank(tc hegel.TestCase, total, minimum sql.NullInt64, count, distinct int64) {
	if count < 1 {
		tc.Errorf("no accounts remain")
	}
	if distinct != count {
		tc.Errorf("account IDs are not unique: %d distinct IDs in %d rows", distinct, count)
	}
	if !total.Valid || total.Int64 != totalBalance {
		tc.Errorf("total balance = %v, want %d", total, totalBalance)
	}
	if !minimum.Valid || minimum.Int64 < 0 {
		tc.Errorf("negative or missing minimum balance: %v", minimum)
	}
}
```

`checkBank` checks that at least one account remains, every account ID is unique, the sum of balances equals `totalBalance`, and the lowest balance is nonnegative. The latter two properties catch some violations of atomicity and isolation. 

## Rules and invariants

```go
func (m *bank) RuleTransfer(tc hegel.TestCase) {
	transaction(tc, m.db, nil, func(tx *sql.Tx) error {
		from, to, balances, err := m.lockPair(tc, tx)
		if err != nil || len(balances) != 2 || balances[from] == 0 {
			return err
		}
		amount := hegel.Draw(tc, hegel.Integers(int64(1), balances[from]))
		if _, err := tx.Exec(fmt.Sprintf(`UPDATE %s SET balance = balance - $1 WHERE id = $2`, m.table), amount, from); err != nil {
			return fmt.Errorf("debit: %w", err)
		}
		if _, err := tx.Exec(fmt.Sprintf(`UPDATE %s SET balance = balance + $1 WHERE id = $2`, m.table), amount, to); err != nil {
			return fmt.Errorf("credit: %w", err)
		}
		return nil
	})
}

func (m *bank) RuleAdd(tc hegel.TestCase) {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	if _, err := m.db.ExecContext(ctx, fmt.Sprintf(`INSERT INTO %s (id, balance) VALUES ($1, $2)`, m.table), m.nextID.Add(1), 0); err != nil {
		tc.Note(fmt.Sprintf("add account: %v", err))
	}
}

func (m *bank) RuleMerge(tc hegel.TestCase) {
	transaction(tc, m.db, nil, func(tx *sql.Tx) error {
		from, to, balances, err := m.lockPair(tc, tx)
		if err != nil || len(balances) != 2 {
			return err
		}
		if _, err := tx.Exec(fmt.Sprintf(`UPDATE %s SET balance = balance + $1 WHERE id = $2`, m.table), balances[from], to); err != nil {
			return fmt.Errorf("merge balance: %w", err)
		}
		if _, err := tx.Exec(fmt.Sprintf(`DELETE FROM %s WHERE id = $1`, m.table), from); err != nil {
			return fmt.Errorf("delete merged account: %w", err)
		}
		return nil
	})
}

func (m *bank) InvariantQuery(tc hegel.TestCase) {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	var total, minimum sql.NullInt64
	var count, distinct int64
	err := m.db.QueryRowContext(ctx, fmt.Sprintf(`SELECT SUM(balance), MIN(balance), COUNT(*), COUNT(DISTINCT id) FROM %s`, m.table)).
		Scan(&total, &minimum, &count, &distinct)
	if err != nil {
		tc.Note(fmt.Sprintf("query validation: %v", err))
		return
	}
	checkBank(tc, total, minimum, count, distinct)
}
```

We can now define our workload.

We are required to define actions on the state, which are called rules in Hegel. The `Rule` prefix identifies a rule method to Hegel. 

`RuleTransfer` runs one transaction. `lockPair` draws two distinct account IDs and locks the corresponding rows in ID order. When both accounts are available and the source has money, the rule draws an amount up to the source balance, withdraws that amount from the source account, and deposits it in the destination account. `RuleAdd` creates a zero-balance account, and `RuleMerge` moves an account's balance then deletes it in one transaction.

We can optionally define statements about the system that should always be true, which are called invariants. By default, invariants are sampled for performance. The `Invariant` prefix identifies an invariant method to Hegel. `InvariantQuery` calls `checkBank` to check all [invariants](#properties) in one query. 

## Running the workload

```go
import "math"

func main() {
	if len(os.Args) < 2 {
		log.Fatalf("usage: %s DATABASE_URL [hegel flags]", os.Args[0])
	}
	databaseURL := os.Args[1]
	os.Args = append(os.Args[:1], os.Args[2:]...)
	db, err := sql.Open("postgres", databaseURL)
	if err != nil {
		log.Fatal(err)
	}
	defer db.Close()

	hegel.Workload(func(tc hegel.TestCase) {
		m := &bank{db: db, table: "bank_" + strings.ToLower(rand.Text())}
		hegel.RunStateful(tc, m,
			hegel.WithConcurrency(),
			hegel.WithStatefulStepCount(math.MaxInt),
			hegel.WithAlwaysCheckInvariants("InvariantQuery"),
		)
	}, hegel.SuppressHealthCheck(hegel.TooSlow))
}
```

`hegel.Workload` runs the callback as a standalone program. The program takes the database URL then Hegel's command-line options.

`WithStatefulStepCount` sets how many rounds execute within a test. Inside of Antithesis, this number should be very high to allow
the fuzzer ample time to explore a branch. `WithConcurrency` lets Hegel choose a worker count up to `GOMAXPROCS`. `WithAlwaysCheckInvariants` selects which invariants should always be checked.

<figure class="diagram">
<svg viewBox="0 0 724 200" width="100%" role="img" aria-label="Two rounds of the bank workload with three workers. Each round runs rules from the group bank. Workers run transfer and the verify rules concurrently, and the invariants run after every worker finishes the round.">
<text x="0" y="65" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">Worker 1</text>
<text x="0" y="109" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">Worker 2</text>
<text x="0" y="153" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">Worker 3</text>
<text x="92" y="20" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-white)">Round 1: group “bank”</text>
<rect x="92" y="44" width="248" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="104" y="65" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">transfer, transfer, transfer</text>
<rect x="92" y="88" width="248" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="104" y="109" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">transfer, verify_no_negative</text>
<rect x="92" y="132" width="264" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="104" y="153" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">verify_total_balance, transfer</text>
<line x1="372" y1="36" x2="372" y2="174" style="stroke:var(--sl-color-accent-high);stroke-width:2"/>
<text x="372" y="194" text-anchor="middle" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">invariants</text>
<text x="402" y="20" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-white)">Round 2: group “bank”</text>
<rect x="402" y="44" width="256" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="414" y="65" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">transfer, verify_ops_not_lost</text>
<rect x="402" y="88" width="168" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="414" y="109" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">transfer, transfer</text>
<rect x="402" y="132" width="248" height="32" rx="6" style="fill:var(--sl-color-accent-low);stroke:var(--sl-color-accent);stroke-width:1"/>
<text x="414" y="153" style="font-family:var(--__sl-font-mono);font-size:13px;fill:var(--sl-color-white)">transfer, transfer, transfer</text>
<line x1="674" y1="36" x2="674" y2="174" style="stroke:var(--sl-color-accent-high);stroke-width:2"/>
<text x="674" y="194" text-anchor="middle" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">invariants</text>
<text x="704" y="109" style="font-family:var(--__sl-font);font-size:15px;fill:var(--sl-color-gray-3)">…</text>
</svg>
</figure>

The diagram above shows how the workload is executed.
Each worker executes its rules concurrently with other workers. Once all workers have finished executing their rules in a round, Hegel runs
the invariants.

## Further reading

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/go/examples/bank).

For more information on concurrent stateful testing, see the [documentation](https://pkg.go.dev/hegel.dev/go/hegel#WithConcurrency).
