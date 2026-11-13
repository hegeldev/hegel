// Bank workload that tests an ACID database with Hegel's concurrent stateful
// testing. Each run of the program runs the workload once.
//
// The state is a file-backed SQLite database in WAL mode with one
// `accounts(account_id, balance, opCount)` table, the workload's client-side
// counts (attempted, committed, total), and a pool of idle connections that
// rules use for each transaction.
//
// In WAL mode, the verification rules can read while transfers are running,
// without blocking them or failing with SQLITE_BUSY, and each read sees a
// consistent snapshot. Writes are still serialized, so transfers take the
// write lock at the start with `BEGIN IMMEDIATE`.

#include <algorithm>
#include <atomic>
#include <cstdint>
#include <cstdlib>
#include <filesystem>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include <unistd.h>

#include <SQLiteCpp/SQLiteCpp.h>
#include <hegel/hegel.h>

namespace gs = hegel::generators;

namespace {

    /// How many milliseconds a connection waits on a locked database before
    /// it fails with SQLITE_BUSY. A transfer that fails this way counts as
    /// attempted, but not known to have committed.
    constexpr int busy_timeout_ms = 5000;

    /// Set `BANK_INJECT_BUG` to skip the balance check in transfers. The run
    /// then fails the "no negative balance" check.
    const bool inject_bug = std::getenv("BANK_INJECT_BUG") != nullptr;

    struct Config {
        int64_t accounts;        ///< `N`, the number of accounts.
        int64_t initial_balance; ///< `B`, the initial balance of every account.
        /// The largest amount a transfer can move. It can be larger than `B`,
        /// so that some transfers have insufficient funds.
        int64_t max_amount;
    };

    struct Workload {
        Config bank;
        int64_t min_concurrency;
        int64_t max_concurrency;
    };

    std::optional<int64_t> env_int(const char* name, int64_t minimum) {
        const char* raw = std::getenv(name);
        if (raw == nullptr || *raw == '\0') {
            return std::nullopt;
        }
        int64_t value = 0;
        try {
            size_t used = 0;
            value = std::stoll(raw, &used);
            if (raw[used] != '\0') {
                throw std::invalid_argument(raw);
            }
        } catch (const std::logic_error&) {
            throw std::invalid_argument(std::string(name) + "=" + raw +
                                        " is not an integer");
        }
        if (value < minimum) {
            throw std::invalid_argument(
                std::string(name) + "=" + std::to_string(value) +
                " must be at least " + std::to_string(minimum));
        }
        return value;
    }

    int64_t env_int_or_draw(hegel::TestCase& tc, const char* name,
                            const char* draw_name, int64_t minimum,
                            int64_t max_value) {
        if (std::optional<int64_t> value = env_int(name, minimum)) {
            return *value;
        }
        return tc.draw(draw_name,
                       gs::integers<int64_t>(
                           {.min_value = minimum, .max_value = max_value}));
    }

    /// The number of rounds per test case: `BANK_STEPS`, or 200 if it is
    /// unset.
    int64_t steps_from_env() { return env_int("BANK_STEPS", 1).value_or(200); }

    /// Reads the workload from the environment, and draws a value for each
    /// unset variable:
    ///
    /// - `BANK_ACCOUNTS`: `N`, at least 1. Drawn from 1..16.
    /// - `BANK_INITIAL_BALANCE`: `B`, at least 0. Drawn from 0..1000.
    /// - `BANK_MAX_AMOUNT`: at least 0. Drawn from 0..2B+10.
    /// - `BANK_MIN_CONCURRENCY`: at least 1. Defaults to 2.
    /// - `BANK_MAX_CONCURRENCY`: at least the minimum. Defaults to 8.
    Workload workload(hegel::TestCase& tc) {
        int64_t accounts =
            env_int_or_draw(tc, "BANK_ACCOUNTS", "accounts", 1, 16);
        int64_t initial_balance = env_int_or_draw(tc, "BANK_INITIAL_BALANCE",
                                                  "initial_balance", 0, 1000);
        int64_t max_amount = env_int_or_draw(
            tc, "BANK_MAX_AMOUNT", "max_amount", 0, 2 * initial_balance + 10);
        int64_t min_concurrency =
            env_int("BANK_MIN_CONCURRENCY", 1).value_or(2);
        int64_t max_concurrency =
            env_int("BANK_MAX_CONCURRENCY", min_concurrency)
                .value_or(std::max<int64_t>(8, min_concurrency));
        return {.bank = {.accounts = accounts,
                         .initial_balance = initial_balance,
                         .max_amount = max_amount},
                .min_concurrency = min_concurrency,
                .max_concurrency = max_concurrency};
    }

    /// What the client observed for a transfer attempt.
    enum class Outcome {
        Committed,         ///< `COMMIT` returned successfully.
        InsufficientFunds, ///< Rolled back because the source balance was
                           ///< insufficient.
        Unknown, ///< SQLite returned an error, such as SQLITE_BUSY. Whether
                 ///< the transaction committed is unknown.
    };

    const char* to_string(Outcome outcome) {
        switch (outcome) {
        case Outcome::Committed:
            return "Committed";
        case Outcome::InsufficientFunds:
            return "Insufficient_funds";
        case Outcome::Unknown:
            return "Unknown";
        }
        return "";
    }

    void require(bool condition, const std::string& message) {
        if (!condition) {
            throw std::runtime_error(message);
        }
    }

    SQLite::Database open(const std::filesystem::path& path) {
        return SQLite::Database(path.string(),
                                SQLite::OPEN_READWRITE | SQLite::OPEN_CREATE |
                                    SQLite::OPEN_NOMUTEX,
                                busy_timeout_ms);
    }

    /// Each verify_* check is both a rule and an invariant. As rules, they
    /// run while transfers are running, and check what a reader sees during a
    /// write. As invariants, they run on the final state and sometimes between
    /// rounds, so a lost commit is caught by the end of the run.
    class Bank : public hegel::stateful::ConcurrentStateMachine<Bank> {
      public:
        /// Creates the `accounts` table with `N` rows of balance `B` and
        /// `opCount` 0.
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

        Bank(const Bank&) = delete;
        Bank& operator=(const Bank&) = delete;

        /// The transfer transaction. Returns what the client observed.
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

        /// No money is created or destroyed.
        void verify_total_balance() const {
            int64_t total = query_int("SELECT SUM(balance) FROM accounts");
            require(total == total_,
                    "money was created or destroyed: SUM(balance) = " +
                        std::to_string(total) + ", expected " +
                        std::to_string(total_));
        }

        /// No committed transfer is lost. `committed` is read before the
        /// snapshot, so every transfer it counts must appear in the snapshot.
        /// `attempted` is read after the snapshot, so it counts every
        /// transfer that appears in it.
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

        /// No account balance is below zero.
        void verify_no_negative() const {
            int64_t negative =
                query_int("SELECT COUNT(*) FROM accounts WHERE balance < 0");
            require(negative == 0, std::to_string(negative) +
                                       " account(s) have a negative balance");
        }

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

      private:
        /// A connection taken from the pool, or a new one if the pool is
        /// empty. Rules run concurrently on worker threads, so each rule takes
        /// its own connection. The connection goes back to the pool at the end
        /// of its scope.
        class PooledConnection {
          public:
            explicit PooledConnection(const Bank& bank) : bank_(bank) {
                {
                    std::lock_guard<std::mutex> lock(bank_.mutex_);
                    if (!bank_.pool_.empty()) {
                        db_.emplace(std::move(bank_.pool_.back()));
                        bank_.pool_.pop_back();
                    }
                }
                if (!db_.has_value()) {
                    db_.emplace(open(bank_.path_));
                }
            }

            ~PooledConnection() {
                std::lock_guard<std::mutex> lock(bank_.mutex_);
                bank_.pool_.push_back(std::move(*db_));
            }

            PooledConnection(const PooledConnection&) = delete;
            PooledConnection& operator=(const PooledConnection&) = delete;

            SQLite::Database& operator*() { return *db_; }
            SQLite::Database* operator->() { return &*db_; }

          private:
            const Bank& bank_;
            std::optional<SQLite::Database> db_;
        };

        int64_t query_int(const char* sql) const {
            PooledConnection db(*this);
            return db->execAndGet(sql).getInt64();
        }

        Config config_;
        int64_t total_;
        std::atomic<int64_t> attempted_{0};
        std::atomic<int64_t> committed_{0};
        std::filesystem::path dir_;
        std::filesystem::path path_;
        mutable std::mutex mutex_;
        mutable std::vector<SQLite::Database> pool_;
    };

    void run(hegel::TestCase& tc, const Workload& workload) {
        tc.note(
            "workload: accounts = " + std::to_string(workload.bank.accounts) +
            ", initial_balance = " +
            std::to_string(workload.bank.initial_balance) +
            ", max_amount = " + std::to_string(workload.bank.max_amount) +
            ", concurrency = [" + std::to_string(workload.min_concurrency) +
            ", " + std::to_string(workload.max_concurrency) + "]");
        Bank bank(workload.bank);
        hegel::stateful::run_concurrent(bank, tc, workload.min_concurrency,
                                        workload.max_concurrency,
                                        {.step_count = steps_from_env()});
    }

} // namespace

HEGEL_MAIN(workload)(hegel::TestCase& tc) { run(tc, workload(tc)); }
