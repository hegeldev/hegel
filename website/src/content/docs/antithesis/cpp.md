---
title: Using Hegel for C++ for Antithesis workloads
sidebar:
  label: C++
---

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

## Further reading

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/cpp/examples/sqlite_bank).

For more information on concurrent stateful testing, see the [documentation](/cpp/classhegel_1_1stateful_1_1ConcurrentStateMachine.html).
