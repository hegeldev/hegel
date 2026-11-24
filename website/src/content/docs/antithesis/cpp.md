---
title: Using Hegel for C++ for Antithesis workloads
sidebar:
  label: C++
---

This is a simple test workload we call the bank test, which checks atomicity and isolation in a SQLite database and a functional correctness property
on a model of a bank. Note that some code is omitted for brevity. See [Further reading](#further-reading) for the full code.

## Bank state

```cpp
constexpr int busy_timeout_ms = 5000;

const bool inject_bug = std::getenv("BANK_INJECT_BUG") != nullptr;

struct Config {
    int64_t accounts;
    int64_t initial_balance;
    int64_t max_amount;
};

struct Workload {
    Config bank;
    int64_t min_concurrency;
    int64_t max_concurrency;
};

enum class Outcome {
    Committed,
    InsufficientFunds,
    Unknown,
};

Config config_;
int64_t total_;
std::atomic<int64_t> attempted_{0};
std::atomic<int64_t> committed_{0};
std::filesystem::path dir_;
std::filesystem::path path_;
mutable std::mutex mutex_;
mutable std::vector<SQLite::Database> pool_;
```

`busy_timeout_ms` is how long a connection waits on a locked database before it fails with `SQLITE_BUSY`.

The database has one `accounts` table with an `account_id`, a `balance` and an `opCount` for each account.

The database runs in WAL mode, which allows non-blocking reads while transfers are executing, and each read sees a consistent snapshot. Since writers are serialized, transfers must take the write lock first with `BEGIN IMMEDIATE`.

`Config` sets the number of accounts, the starting balance of each account, and the largest transfer amount.

Transfers can have one of the following outcomes (`Outcome`):

- `Committed`: `COMMIT` returned successfully.
- `InsufficientFunds`: the transaction was rolled back because the source balance was too low.
- `Unknown`: SQLite returned an error, such as `SQLITE_BUSY`. The client does not know if the transaction committed.

The test state `Bank` consists of the database path, a pool of idle connections, counts of attempted and committed transfers, and the total balance. Mutable fields of `Bank` are
atomic or guarded by a mutex since they may be modified
concurrently.

## Database helpers

```cpp
SQLite::Database open(const std::filesystem::path& path) {
    return SQLite::Database(path.string(),
                            SQLite::OPEN_READWRITE | SQLite::OPEN_CREATE |
                                SQLite::OPEN_NOMUTEX,
                            busy_timeout_ms);
}

int64_t query_int(const char* sql) const {
    PooledConnection db(*this);
    return db->execAndGet(sql).getInt64();
}
```

`open` opens a connection with the busy timeout set. `query_int` borrows a connection through `PooledConnection` and returns the integer from a query.

## Setup

```cpp
explicit Bank(const Config& config)
    : config_(config),
      total_(config.accounts * config.initial_balance) {
    std::string dir_template =
        (std::filesystem::temp_directory_path() / "sqlite_bankXXXXXX")
            .string();
    if (mkdtemp(dir_template.data()) == nullptr) {
        throw std::runtime_error("cannot create a temporary directory");
    }
    dir_ = dir_template;
    path_ = dir_ / "bank.sqlite3";

    SQLite::Database db = open(path_);
    db.exec("PRAGMA journal_mode = WAL");
    db.exec("PRAGMA synchronous = FULL");
    db.exec("CREATE TABLE accounts (account_id INTEGER PRIMARY KEY, "
            "balance INTEGER NOT NULL, opCount INTEGER NOT NULL)");
    {
        SQLite::Transaction tx(db);
        SQLite::Statement insert(
            db, "INSERT INTO accounts (account_id, balance, opCount) "
                "VALUES (?, ?, 0)");
        for (int64_t account = 0; account < config.accounts;
             account++) {
            insert.bind(1, account);
            insert.bind(2, config.initial_balance);
            insert.exec();
            insert.reset();
        }
        tx.commit();
    }
    pool_.push_back(std::move(db));
}

~Bank() {
    pool_.clear();
    std::error_code ignored;
    std::filesystem::remove_all(dir_, ignored);
}
```

`Bank` creates the database in a temporary directory, switches it to WAL mode, and creates the `accounts` table with one row per account. Each account starts with the initial balance and an `opCount` of 0. `Bank` also records the expected total `config.accounts * config.initial_balance`. The destructor closes the connections in the connection pool and deletes the database files.

`PooledConnection` takes an idle connection from the pool or opens a new one if the pool is empty, and returns it to the pool when it goes out of scope.

## Transfers

```cpp
Outcome transfer(int64_t source, int64_t dest, int64_t amount) {
    attempted_++;
    try {
        PooledConnection db(*this);
        SQLite::Transaction tx(*db,
                               SQLite::TransactionBehavior::IMMEDIATE);
        SQLite::Statement select(
            *db, "SELECT balance FROM accounts WHERE account_id = ?");
        select.bind(1, source);
        select.executeStep();
        int64_t balance = select.getColumn(0).getInt64();
        if (!inject_bug && balance < amount) {
            return Outcome::InsufficientFunds;
        }
        SQLite::Statement debit(
            *db, "UPDATE accounts SET balance = balance - ?, "
                 "opCount = opCount + 1 WHERE account_id = ?");
        debit.bind(1, amount);
        debit.bind(2, source);
        debit.exec();
        SQLite::Statement credit(
            *db, "UPDATE accounts SET balance = balance + ?, "
                 "opCount = opCount + 1 WHERE account_id = ?");
        credit.bind(1, amount);
        credit.bind(2, dest);
        credit.exec();
        tx.commit();
    } catch (const SQLite::Exception&) {
        return Outcome::Unknown;
    }
    committed_++;
    return Outcome::Committed;
}
```

`transfer` increments `attempted_`, then runs one transaction. If the source balance is less than the transfer amount, the transaction is rolled back. Otherwise, `transfer` subtracts the amount from the source, adds the amount to the destination, and increments `opCount` on both accounts. If a statement raises an exception, the transaction is rolled back when it goes out of scope.

If the transaction commits, `transfer` increments `committed_`. Any SQLite error becomes `Outcome::Unknown`.

## Properties

```cpp
void verify_total_balance() const {
    int64_t total = query_int("SELECT SUM(balance) FROM accounts");
    require(total == total_,
            "money was created or destroyed: SUM(balance) = " +
                std::to_string(total) + ", expected " +
                std::to_string(total_));
}

void verify_ops_not_lost() const {
    int64_t committed = committed_.load();
    int64_t total = query_int("SELECT SUM(opCount) FROM accounts");
    int64_t attempted = attempted_.load();
    require(total % 2 == 0, "SUM(opCount) = " + std::to_string(total) +
                                " is odd: a transfer was half-applied");
    require(total >= 2 * committed,
            "committed transfers were lost: SUM(opCount) = " +
                std::to_string(total) + " < 2 * " +
                std::to_string(committed) + " committed");
    require(
        total <= 2 * attempted,
        "phantom transfers: SUM(opCount) = " + std::to_string(total) +
            " > 2 * " + std::to_string(attempted) + " attempted");
}

void verify_no_negative() const {
    int64_t negative =
        query_int("SELECT COUNT(*) FROM accounts WHERE balance < 0");
    require(negative == 0, std::to_string(negative) +
                               " account(s) have a negative balance");
}
```

`verify_total_balance` checks that `SUM(balance)` equals the starting total (i.e. no money was created or destroyed).

`verify_ops_not_lost` checks `SUM(opCount)`. Each transfer increments two counts, so the sum must be even. The sum must be at least `2 * committed`, or a committed transfer was lost. It can be higher because some transfers can commit without the client knowing, but it cannot exceed `2 * attempted`.

`verify_no_negative` checks that no account has a balance below zero. If two transfers
both read the old balance before either withdraws, the account can be overdrawn.

## Rules and invariants

```cpp
std::vector<hegel::stateful::ConcurrentRule<Bank>> rules() {
    using Rule = hegel::stateful::ConcurrentRule<Bank>;
    return {
        Rule({.name = "transfer_rule",
              .groups = {"bank"},
              .weight = 6.0,
              .step =
                  [](hegel::TestCase& tc, Bank& bank) {
                      int64_t n = bank.config_.accounts;
                      auto account = gs::integers<int64_t>(
                          {.min_value = 0, .max_value = n - 1});
                      int64_t source = tc.draw("source", account);
                      int64_t dest = tc.draw("dest", account);
                      int64_t amount = tc.draw(
                          "amount",
                          gs::integers<int64_t>(
                              {.min_value = 0,
                               .max_value = bank.config_.max_amount}));
                      Outcome outcome =
                          bank.transfer(source, dest, amount);
                      tc.note("transfer " + std::to_string(amount) +
                              " from " + std::to_string(source) +
                              " to " + std::to_string(dest) + ": " +
                              to_string(outcome));
                  }}),
        Rule({.name = "verify_total_balance_rule",
              .groups = {"bank"},
              .step = [](hegel::TestCase&,
                         Bank& bank) { bank.verify_total_balance(); }}),
        Rule({.name = "verify_ops_not_lost_rule",
              .groups = {"bank"},
              .step = [](hegel::TestCase&,
                         Bank& bank) { bank.verify_ops_not_lost(); }}),
        Rule({.name = "verify_no_negative_rule",
              .groups = {"bank"},
              .step = [](hegel::TestCase&,
                         Bank& bank) { bank.verify_no_negative(); }}),
    };
}

std::vector<hegel::stateful::Invariant<Bank>> invariants() const {
    using Invariant = hegel::stateful::Invariant<Bank>;
    return {
        Invariant(
            "total_balance_preserved",
            [](const Bank& bank) { bank.verify_total_balance(); }),
        Invariant("no_committed_transfer_lost",
                  [](const Bank& bank) { bank.verify_ops_not_lost(); }),
        Invariant("no_negative_balance",
                  [](const Bank& bank) { bank.verify_no_negative(); }),
    };
}
```

We can now define our workload.

We are required to define actions on the state, which are called rules in Hegel. Rules can belong to
one or more groups. Only rules in the same group may execute concurrently. If no group is provided,
the rule belongs to a default group. In this case, all the rules belong to the `bank` group for
demonstration, but the group could have been omitted. Rules also have a weight, which gives Hegel
a hint of how frequently a rule should run relative to other rules in its group(s). The default
weight is 1.

We can optionally define statements about the system that should always be true, which are called
invariants. By default, invariants are sampled for performance. For example, if we had 1 million
accounts instead of 16, we might not want check `no_negative_balance` after every single round
because it requires checking all accounts. Setting `always_check = true` forces the invariant
to always be checked between rounds.

Notice that we define the verification functions both as invariants and rules. Invariants check
the state of the system at some settled state when no actions are running. When we define the
verification function as a rule, they can run while other actions are mutating the state.

For example, the invariant `total_balance_preserved` checks that after all transfers are done,
the total amount of money has not changed. The rule `verify_total_balance_rule` checks that while
transfers are executing, we never see a state in which the total amount of money has changed.

## Running the workload

```cpp
#include <limits>

void run(hegel::TestCase& tc, const Workload& workload) {
    Bank bank(workload.bank);
    hegel::stateful::run_concurrent(
        bank, tc, workload.min_concurrency, workload.max_concurrency,
        {.step_count = std::numeric_limits<int64_t>::max()});
}

HEGEL_MAIN(workload)(hegel::TestCase& tc) { run(tc, workload(tc)); }
```

`HEGEL_MAIN` turns the workload into a standalone program, which allows us to pass test settings as command line options,
such as `./sqlite_bank --seed 1 --verbosity verbose`.

Since the account balances are stored in the database, we only pass the bank handle,
which holds the database path, the connection pool, the transfer counters, and the configuration. `step_count` sets how many rounds execute within a test. Inside of Antithesis, this number should be very high to allow
the fuzzer ample time to explore a branch. Finally, `min_concurrency` and `max_concurrency` set the bounds for the number of concurrent workers. Hegel spawns some number of workers within those bounds.

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

The diagram above shows how the workload is executed. In each round, Hegel chooses a group, then gives each worker a set of rules to run.
Each worker executes its rules concurrently with other workers. Once all workers have finished executing their rules in a round, Hegel runs
the invariants.

## Further reading

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/cpp/examples/sqlite_bank).

For more information on concurrent stateful testing, see the [documentation](/cpp/classhegel_1_1stateful_1_1ConcurrentStateMachine.html).
