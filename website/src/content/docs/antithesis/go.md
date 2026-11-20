---
title: Using Hegel for Go for Antithesis workloads
sidebar:
  label: Go
---

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

## Transfers

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
```

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

## Rules and invariants

```go
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
		)
	}, hegel.SuppressHealthCheck(hegel.TooSlow))
}
```

## Further reading

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/go/examples/bank).

For more information on concurrent stateful testing, see the [documentation](https://pkg.go.dev/hegel.dev/go/hegel#WithConcurrency).
