#include <gtest/gtest-spi.h>
#include <gtest/gtest.h>

#include <cstdint>
#include <cstdlib>
#include <filesystem>
#include <hegel/hegel.h>
#include <hegel/internal.h>
#include <map>
#include <stdexcept>
#include <string>
#include <vector>

namespace gs = hegel::generators;
using hegel::Database;
using hegel::HealthCheck;
using hegel::NondeterminismStrictness;
using hegel::Settings;

TEST(Settings, DefaultRuns100TestCases) {
    int count = 0;
    hegel::test([&count](hegel::TestCase& tc) {
        tc.draw(gs::integers<int>());
        count++;
    });
    EXPECT_EQ(count, 100);
}

TEST(Settings, CustomTestCases) {
    int count = 0;
    hegel::test(
        [&count](hegel::TestCase& tc) {
            tc.draw(gs::integers<int>());
            count++;
        },
        Settings{.test_cases = 17});
    EXPECT_EQ(count, 17);
}

TEST(Settings, SeedDeterminism) {
    auto run = []() {
        std::vector<int32_t> seen;
        hegel::test(
            [&seen](hegel::TestCase& tc) {
                seen.push_back(tc.draw(gs::integers<int32_t>()));
            },
            Settings{.test_cases = 25, .seed = 42});
        return seen;
    };
    auto a = run();
    auto b = run();
    EXPECT_EQ(a, b);
}

TEST(Settings, DerandomizeProducesRepeatableRuns) {
    auto run = []() {
        std::vector<int32_t> seen;
        hegel::test(
            [&seen](hegel::TestCase& tc) {
                seen.push_back(tc.draw(gs::integers<int32_t>()));
            },
            Settings{.test_cases = 25,
                     .derandomize = true,
                     .database = Database::disabled()});
        return seen;
    };
    auto a = run();
    auto b = run();
    EXPECT_EQ(a, b);
}

TEST(Settings, SuppressFilterTooMuch) {
    // Without suppression, the hypothesis filter_too_much health check fires
    // here because ~10% of draws are rejected. We suppress it and expect
    // the test to pass cleanly.
    hegel::test(
        [](hegel::TestCase& tc) {
            int n =
                tc.draw(gs::integers<int>({.min_value = 0, .max_value = 100}));
            tc.assume(n < 90);
        },
        Settings{.suppress_health_check = {HealthCheck::FilterTooMuch}});
}

TEST(Settings, SuppressMultiple) {
    hegel::test(
        [](hegel::TestCase& tc) {
            int n =
                tc.draw(gs::integers<int>({.min_value = 0, .max_value = 100}));
            tc.assume(n < 90);
        },
        Settings{.suppress_health_check = {HealthCheck::FilterTooMuch,
                                           HealthCheck::TooSlow}});
}

TEST(Settings, SuppressAllWithAllHealthChecks) {
    hegel::test(
        [](hegel::TestCase& tc) {
            int n =
                tc.draw(gs::integers<int>({.min_value = 0, .max_value = 100}));
            tc.assume(n < 90);
        },
        Settings{.suppress_health_check = hegel::all_health_checks()});
}

TEST(Settings, SuppressDataTooLarge) {
    hegel::test(
        [](hegel::TestCase& tc) {
            bool do_big = tc.draw(gs::booleans());
            if (do_big) {
                for (int i = 0; i < 100; ++i) {
                    (void)tc.draw(gs::integers<int32_t>());
                }
            }
        },
        Settings{.test_cases = 15,
                 .suppress_health_check = {HealthCheck::TestCasesTooLarge,
                                           HealthCheck::TooSlow,
                                           HealthCheck::LargeInitialTestCase}});
}

TEST(Settings, SuppressLargeBaseExample) {
    hegel::test(
        [](hegel::TestCase& tc) {
            for (int i = 0; i < 10; ++i) {
                (void)tc.draw(gs::vectors(gs::integers<int32_t>(),
                                          {.min_size = 50, .max_size = 50}));
            }
        },
        Settings{.test_cases = 15,
                 .suppress_health_check = {HealthCheck::LargeInitialTestCase,
                                           HealthCheck::TestCasesTooLarge,
                                           HealthCheck::TooSlow}});
}

static int global_counter = 1;

TEST(FlakyReporting, FlakyReplay) {
    global_counter = 1;
    try {
        hegel::test(
            [&](hegel::TestCase& tc) {
                auto x = tc.draw(gs::integers<int>());
                if (global_counter == 1) {
                    global_counter--;
                    throw std::runtime_error("first test case fails");
                }
            },
            {.test_cases = 1,
             .nondeterminism_strictness = NondeterminismStrictness::Error,
             .suppress_health_check = {HealthCheck::LargeInitialTestCase}});
        FAIL() << "Expected hegel::Flaky";
    } catch (const hegel::Flaky& e) {
        EXPECT_NE(
            std::string(e.what()).find("Your test produced different outcomes"),
            std::string::npos)
            << "Unexpected error message: " << e.what();
    }
}

TEST(FlakyReporting, FlakyGeneration) {
    global_counter = 0;
    try {
        hegel::test(
            [&](hegel::TestCase& tc) {
                tc.draw(gs::integers<int>({.min_value = global_counter,
                                           .max_value = global_counter + 1}));
                global_counter++;
            },
            {.test_cases = 10,
             .nondeterminism_strictness = NondeterminismStrictness::Error,
             .suppress_health_check = {HealthCheck::LargeInitialTestCase}});
        FAIL() << "Expected hegel::Flaky";
    } catch (const hegel::Flaky& e) {
        EXPECT_NE(std::string(e.what()).find(
                      "Your data generation is non-deterministic"),
                  std::string::npos)
            << "Unexpected error message: " << e.what();
    }
}

// A failure that never replays is reported unconfirmed, without a
// reproduce blob: the first one from its outcome alone (its case revealed
// the nondeterminism, so the engine had not asked for its output), a later
// one from the output the engine had the run capture. By the thirtieth
// case the engine has finished replaying the first failure and is
// generating again.
TEST(FlakyReporting, QuietReportsUnconfirmedFailures) {
    int calls = 0;
    testing::internal::CaptureStderr();
    std::string message;
    size_t failure_count = 0;
    try {
        hegel::test(
            [&](hegel::TestCase& tc) {
                auto x = tc.draw(gs::integers<int>());
                tc.note("saw " + std::to_string(x));
                calls++;
                if (calls == 1) {
                    throw std::runtime_error("first case fails");
                }
                if (calls == 30) {
                    throw std::logic_error("thirtieth case fails");
                }
            },
            {.test_cases = 30,
             .report_multiple_failures = true,
             .stop_on_first_failure = false,
             .database = Database::disabled(),
             .suppress_health_check = hegel::all_health_checks()});
    } catch (const hegel::MultipleFailures& e) {
        message = e.what();
        failure_count = e.failures().size();
    }
    std::string output = testing::internal::GetCapturedStderr();
    EXPECT_NE(message.find("2 distinct failures"), std::string::npos)
        << message;
    EXPECT_EQ(failure_count, 2u);
    EXPECT_NE(output.find("note: unconfirmed failure"), std::string::npos)
        << output;
    EXPECT_NE(output.find("Exception: std::runtime_error: first case fails"),
              std::string::npos)
        << output;
    EXPECT_NE(output.find("Exception: std::logic_error: thirtieth case fails"),
              std::string::npos)
        << output;
    EXPECT_NE(output.find("saw "), std::string::npos) << output;
    EXPECT_EQ(output.find("rerun with:"), std::string::npos) << output;
}

TEST(FlakyReporting, WarnStrictnessKeepsRunning) {
    global_counter = 0;
    EXPECT_NO_THROW(hegel::test(
        [&](hegel::TestCase& tc) {
            tc.draw(gs::integers<int>({.min_value = global_counter,
                                       .max_value = global_counter + 1}));
            global_counter++;
        },
        {.test_cases = 10,
         .nondeterminism_strictness = NondeterminismStrictness::Warn,
         .database = Database::disabled(),
         .suppress_health_check = {HealthCheck::LargeInitialTestCase}}));
    EXPECT_GE(global_counter, 10);
}

TEST(Settings, StopOnFirstFailureOffUsesTheWholeBudget) {
    int calls = 0;
    try {
        hegel::test(
            [&](hegel::TestCase& tc) {
                (void)tc.draw(gs::integers<int>());
                calls++;
                throw std::runtime_error("always fails");
            },
            {.test_cases = 10,
             .stop_on_first_failure = false,
             .database = Database::disabled(),
             .phases = {hegel::Phase::Generate}});
        FAIL() << "Expected std::runtime_error";
    } catch (const std::runtime_error& e) {
        EXPECT_EQ(std::string(e.what()), "always fails");
    }
    EXPECT_GE(calls, 10);
}

std::optional<int> failed_value = std::nullopt;

HEGEL_TEST(check_database)(hegel::TestCase& tc) {
    int64_t n = tc.draw(gs::integers<int64_t>());
    if (!(n < 1'000'000)) {
        failed_value = {n};
        throw std::runtime_error("n >= 1_000_000");
    }
}

TEST(Settings, DatabaseReplaysFailure) {
    namespace fs = std::filesystem;

    fs::path db_path = fs::temp_directory_path() /
                       ("hegel_db_settings_test_" + std::to_string(::getpid()));
    fs::remove_all(db_path);
    fs::create_directories(db_path);

    auto run_once = [&](const std::string& key,
                        const std::vector<hegel::Phase> phases) {
        try {
            check_database(Settings{
                .database = Database::from_path(db_path.string()),
                .database_key = key,
                .phases = phases,
            });
        } catch (const std::runtime_error&) { // NOLINT(bugprone-empty-catch)
            // expected: the property fails and hegel::test() rethrows
        }
    };

    // First run fails and shrinks to the minimal failing value.
    run_once("replay-key", hegel::all_phases());
    int64_t shrunk_value = failed_value.value();
    EXPECT_EQ(shrunk_value, 1'000'000);

    failed_value = std::nullopt;

    // Second run with the same key replays the shrunk failure first.
    run_once("replay-key", {hegel::Phase::Reuse});
    int64_t replayed_value = failed_value.value();
    EXPECT_EQ(shrunk_value, replayed_value);

    fs::remove_all(db_path);
}

// ---------------------------------------------------------------------------
// The database key hegel::test() derives when the caller sets none
// ---------------------------------------------------------------------------

namespace {
    std::optional<int64_t> derived_key_value;

    void fail_above_million(hegel::TestCase& tc) {
        int64_t n = tc.draw(gs::integers<int64_t>());
        if (n >= 1'000'000) {
            derived_key_value = n;
            throw std::runtime_error("n >= 1_000_000");
        }
    }

    // Runs the property against `db_path` and reports the value it failed on,
    // or nothing if it passed. An empty `key` leaves Settings::database_key
    // unset, so hegel::test() derives one.
    std::optional<int64_t> run_against(const std::string& db_path,
                                       const std::vector<hegel::Phase>& phases,
                                       const std::string& key = "") {
        derived_key_value = std::nullopt;
        Settings settings{.database = Database::from_path(db_path),
                          .phases = phases};
        if (!key.empty()) {
            settings.database_key = key;
        }
        try {
            hegel::test(fail_above_million, settings);
        } catch (const std::runtime_error&) { // NOLINT(bugprone-empty-catch)
            // expected: the property fails and hegel::test() rethrows
        }
        return derived_key_value;
    }
} // namespace

// A run that sets no key still persists its counterexample, and a later run
// replays it. The key is the file and the enclosing GoogleTest test, so
// naming it explicitly reaches the same stored example.
TEST(Settings, DerivedDatabaseKeyReplaysFailure) {
    namespace fs = std::filesystem;

    fs::path db_path = fs::temp_directory_path() /
                       ("hegel_db_derived_key_" + std::to_string(::getpid()));
    fs::remove_all(db_path);
    fs::create_directories(db_path);

    std::optional<int64_t> shrunk =
        run_against(db_path.string(), hegel::all_phases());
    ASSERT_TRUE(shrunk.has_value());
    EXPECT_EQ(shrunk.value(), 1'000'000);

    // Reuse alone runs only what the database holds under the same key.
    EXPECT_EQ(run_against(db_path.string(), {hegel::Phase::Reuse}), shrunk);

    // The derived key spells out the file and the GoogleTest test.
    EXPECT_EQ(run_against(db_path.string(), {hegel::Phase::Reuse},
                          __FILE__
                          "::Settings.DerivedDatabaseKeyReplaysFailure"),
              shrunk);

    // Another key is another scope, and holds nothing to replay.
    EXPECT_FALSE(
        run_against(db_path.string(), {hegel::Phase::Reuse}, "other-key")
            .has_value());

    fs::remove_all(db_path);
}

static int macro_inline_count = 0;

HEGEL_TEST(macro_inline_settings,
           {.test_cases = 17,
            .database = Database::disabled()})(hegel::TestCase& tc) {
    tc.draw(gs::integers<int>());
    macro_inline_count++;
}

TEST(Settings, HegelTestMacroInlineSettings) {
    macro_inline_count = 0;
    macro_inline_settings();
    EXPECT_EQ(macro_inline_count, 17);

    // A Settings passed at the call site replaces the inline default.
    macro_inline_count = 0;
    macro_inline_settings({.test_cases = 5, .database = Database::disabled()});
    EXPECT_EQ(macro_inline_count, 5);
}

TEST(Settings, EmptyPhasesRunNothing) {
    int count = 0;
    hegel::test(
        [&count](hegel::TestCase& tc) {
            tc.draw(gs::integers<int>());
            count++;
        },
        Settings{.database = Database::disabled(), .phases = {}});
    EXPECT_EQ(count, 0);
}

TEST(Settings, GenerateOnlyPhaseRunsFullBudget) {
    int count = 0;
    hegel::test(
        [&count](hegel::TestCase& tc) {
            tc.draw(gs::integers<int>());
            count++;
        },
        Settings{.database = Database::disabled(),
                 .phases = {hegel::Phase::Generate}});
    EXPECT_EQ(count, 100);
}

TEST(Settings, UrandomBackendRuns) {
    int count = 0;
    hegel::test(
        [&count](hegel::TestCase& tc) {
            tc.draw(gs::integers<int>());
            count++;
        },
        Settings{.test_cases = 10,
                 .database = Database::disabled(),
                 .backend = hegel::Backend::Urandom});
    EXPECT_EQ(count, 10);
}

TEST(Settings, DefaultBackendRuns) {
    int count = 0;
    hegel::test(
        [&count](hegel::TestCase& tc) {
            tc.draw(gs::integers<int>());
            count++;
        },
        Settings{.test_cases = 10,
                 .database = Database::disabled(),
                 .backend = hegel::Backend::Default});
    EXPECT_EQ(count, 10);
}

// With Database::unset() the run keeps the profile's database, which is
// `.hegel` outside CI. Run inside a temporary working directory so the database
// artifacts don't leak into the build tree.
TEST(Settings, UnsetDatabaseUsesEngineDefault) {
    namespace fs = std::filesystem;

    fs::path work = fs::temp_directory_path() /
                    ("hegel_unset_db_test_" + std::to_string(::getpid()));
    fs::remove_all(work);
    fs::create_directories(work);
    fs::path prev = fs::current_path();
    fs::current_path(work);

    int count = 0;
    hegel::test(
        [&count](hegel::TestCase& tc) {
            tc.draw(gs::integers<int>());
            count++;
        },
        Settings{.test_cases = 5, .database = Database::unset()});

    fs::current_path(prev);
    fs::remove_all(work);
    EXPECT_EQ(count, 5);
}

// ---------------------------------------------------------------------------
// Reproduce-failure blobs
// ---------------------------------------------------------------------------

TEST(FailureBlobs, BlobDoesNotReproduceFailure) {
    try {
        hegel::test(
            [&](hegel::TestCase& tc) {
                int n = tc.draw(gs::integers<int>());
                if (n < 50) {
                    throw std::runtime_error("fail");
                }
            },
            {}, {"AAEAAAAACgEAAAAy"});
        FAIL();
    } catch (const hegel::DidNotReproduce& e) {
        EXPECT_NE(std::string(e.what()).find(
                      "The failure blob did not reproduce a failure"),
                  std::string::npos)
            << e.what();
    }
}

TEST(FailureBlobs, InvalidBlob) {
    try {
        hegel::test(
            [&](hegel::TestCase& tc) {
                int n = tc.draw(gs::integers<int>());
                if (n < 50) {
                    throw std::runtime_error("fail");
                }
            },
            {}, {"A"});
        FAIL();
    } catch (const std::invalid_argument& e) {
        EXPECT_NE(std::string(e.what()).find("could not be decoded"),
                  std::string::npos)
            << e.what();
    }
}

HEGEL_REPRODUCE_FAILURE(obvious_fail, "AAEAAAAACgEAAAAy", "AAEAAAAACgEAAAAA")
HEGEL_TEST(obvious_fail,
           {.phases = {hegel::Phase::Explicit}})(hegel::TestCase& tc) {
    int n = tc.draw(gs::integers<int>());
    if (n < 50) {
        throw std::runtime_error("fail");
    }
}

// The first blob (n = 50) passes; the second (n = 0) fails. Every blob is
// replayed, so the failure is found.
TEST(FailureBlobs, BlobReproduceFailure) {
    try {
        obvious_fail();
        FAIL();
    } catch (const std::runtime_error& e) {
        EXPECT_EQ(std::string(e.what()), "fail");
    }
}

TEST(FailureBlobs, EveryReproducedBlobIsReported) {
    try {
        hegel::test(
            [&](hegel::TestCase& tc) {
                int n = tc.draw(gs::integers<int>());
                if (n < 50) {
                    throw std::runtime_error("fail");
                }
            },
            {.verbosity = hegel::Verbosity::Quiet},
            {"AAEAAAAACgEAAAAA", "AAEAAAAACgEAAAAA"});
        FAIL();
    } catch (const hegel::MultipleFailures& e) {
        EXPECT_EQ(e.failures().size(), 2u);
    }
}

// ---------------------------------------------------------------------------
// Run errors
// ---------------------------------------------------------------------------

TEST(RunErrors, FilterTooMuchIsAFailedHealthCheck) {
    try {
        hegel::test(
            [](hegel::TestCase& tc) {
                int n = tc.draw(
                    gs::integers<int>({.min_value = 0, .max_value = 100}));
                tc.assume(n == 1000);
            },
            {.database = Database::disabled(), .suppress_health_check = {}});
        FAIL() << "Expected hegel::FailedHealthCheck";
    } catch (const hegel::FailedHealthCheck& e) {
        EXPECT_EQ(e.health_check(), HealthCheck::FilterTooMuch);
    }
}

TEST(RunErrors, NoValidTestCaseIsUnsatisfiable) {
    EXPECT_THROW(
        hegel::test([](hegel::TestCase& tc) { tc.reject(); },
                    {.database = Database::disabled(),
                     .suppress_health_check = hegel::all_health_checks()}),
        hegel::Unsatisfiable);
}

// ---------------------------------------------------------------------------
// Settings profiles
// ---------------------------------------------------------------------------

namespace {
    int count_cases(const Settings& settings) {
        int count = 0;
        hegel::test(
            [&count](hegel::TestCase& tc) {
                tc.draw(gs::integers<int>());
                count++;
            },
            settings);
        return count;
    }
} // namespace

TEST(Profiles, UnsetFieldsKeepTheProfile) {
    hegel::register_profile("cpp_unset_fields",
                            {.test_cases = 20,
                             .database = Database::disabled(),
                             .phases = {hegel::Phase::Generate}});
    EXPECT_EQ(count_cases({.profile = "cpp_unset_fields"}), 20);
    EXPECT_EQ(count_cases({.test_cases = 7, .profile = "cpp_unset_fields"}), 7);
}

TEST(Profiles, ProfilePhasesAreKept) {
    hegel::register_profile("cpp_no_phases",
                            {.database = Database::disabled(), .phases = {}});
    EXPECT_EQ(count_cases({.profile = "cpp_no_phases"}), 0);
    EXPECT_EQ(count_cases({.phases = {hegel::Phase::Generate},
                           .profile = "cpp_no_phases"}),
              100);
}

TEST(Profiles, ProfileVerbosityAndPrintBlobAreKept) {
    auto failing = [](hegel::TestCase& tc) {
        int n = tc.draw(gs::integers<int>());
        if (n > 10) {
            throw std::runtime_error("too big");
        }
    };
    hegel::register_profile("cpp_quiet", {.verbosity = hegel::Verbosity::Quiet,
                                          .database = Database::disabled()});
    testing::internal::CaptureStderr();
    EXPECT_THROW(hegel::test(failing, {.profile = "cpp_quiet"}),
                 std::runtime_error);
    EXPECT_EQ(testing::internal::GetCapturedStderr(), "");

    hegel::register_profile(
        "cpp_no_blob", {.print_blob = false, .database = Database::disabled()});
    testing::internal::CaptureStderr();
    EXPECT_THROW(hegel::test(failing, {.profile = "cpp_no_blob"}),
                 std::runtime_error);
    std::string output = testing::internal::GetCapturedStderr();
    EXPECT_NE(output.find("Exception: std::runtime_error"), std::string::npos)
        << output;
    EXPECT_EQ(output.find("rerun with:"), std::string::npos) << output;
}

TEST(Profiles, ProfileHealthChecksCanBeCleared) {
    hegel::register_profile(
        "cpp_suppress_all",
        {.database = Database::disabled(),
         .suppress_health_check = hegel::all_health_checks()});
    auto filtering = [](hegel::TestCase& tc) {
        int n = tc.draw(gs::integers<int>({.min_value = 0, .max_value = 100}));
        tc.assume(n < 5);
    };
    EXPECT_NO_THROW(hegel::test(filtering, {.profile = "cpp_suppress_all"}));
    EXPECT_THROW(hegel::test(filtering, {.suppress_health_check = {},
                                         .profile = "cpp_suppress_all"}),
                 hegel::FailedHealthCheck);
}

TEST(Profiles, DefaultProfileCanBeSet) {
    hegel::register_profile("cpp_default_twelve",
                            {.test_cases = 12,
                             .database = Database::disabled(),
                             .phases = {hegel::Phase::Generate}});
    hegel::set_default_profile("cpp_default_twelve");
    int count = count_cases({});
    hegel::set_default_profile("");
    EXPECT_EQ(count, 12);
}

TEST(Profiles, UnknownProfileIsRejected) {
    EXPECT_THROW(count_cases({.profile = "cpp_no_such_profile"}),
                 std::invalid_argument);
}

TEST(Profiles, InvalidNamesAreRejected) {
    EXPECT_THROW(hegel::register_profile("default", {}), std::invalid_argument);
    EXPECT_THROW(hegel::register_profile("not a name", {}),
                 std::invalid_argument);
    EXPECT_THROW(hegel::set_default_profile("default"), std::invalid_argument);
}

TEST(Settings, StepCountBelowOneIsRejected) {
    EXPECT_THROW(count_cases({.stateful_step_count = 0}),
                 std::invalid_argument);
    EXPECT_THROW(
        hegel::register_profile("cpp_bad_steps", {.stateful_step_count = 0}),
        std::invalid_argument);
}

TEST(Settings, EngineVersionIsKnown) {
    EXPECT_FALSE(hegel::engine_version().empty());
}

TEST(Settings, ShowStatisticsAndUnboundedChoicesRun) {
    EXPECT_EQ(count_cases(
                  {.test_cases = 10,
                   .nondeterminism_strictness = NondeterminismStrictness::Quiet,
                   .show_statistics = true,
                   .unbounded_choices = true,
                   .database = Database::disabled()}),
              10);
}

// Database::default_location() uses `.hegel` in the working directory, also
// where the profile disables the database.
TEST(Settings, DefaultLocationDatabaseOverridesTheProfile) {
    namespace fs = std::filesystem;

    fs::path work = fs::temp_directory_path() /
                    ("hegel_default_db_test_" + std::to_string(::getpid()));
    fs::remove_all(work);
    fs::create_directories(work);
    fs::path prev = fs::current_path();
    fs::current_path(work);

    try {
        hegel::test(
            [](hegel::TestCase& tc) {
                if (tc.draw(gs::integers<int>()) > 10) {
                    throw std::runtime_error("too big");
                }
            },
            {.verbosity = hegel::Verbosity::Quiet,
             .database = Database::default_location(),
             .profile = "ci"});
    } catch (const std::runtime_error&) { // NOLINT(bugprone-empty-catch)
    }
    bool exists = fs::exists(work / ".hegel");

    fs::current_path(prev);
    fs::remove_all(work);
    EXPECT_TRUE(exists);
}

// ---------------------------------------------------------------------------
// Database keys
// ---------------------------------------------------------------------------

namespace {
    std::vector<int32_t> derandomized_draws(const hegel::TestLocation& where) {
        std::vector<int32_t> seen;
        hegel::test(
            [&seen](hegel::TestCase& tc) {
                seen.push_back(tc.draw(gs::integers<int32_t>()));
            },
            where,
            {.test_cases = 10,
             .derandomize = true,
             .database = Database::disabled()});
        return seen;
    }
} // namespace

// A derandomized run seeds from its database key, so the key is derived also
// where the database is disabled: each test sees its own inputs.
TEST(Settings, DerivedKeySeedsDerandomizedRuns) {
    hegel::TestLocation first{"first", "file.cpp", 1};
    hegel::TestLocation second{"second", "file.cpp", 2};
    EXPECT_EQ(derandomized_draws(first), derandomized_draws(first));
    EXPECT_NE(derandomized_draws(first), derandomized_draws(second));
}

TEST(Settings, LocationWithoutANameRuns) {
    EXPECT_EQ(derandomized_draws({"", "file.cpp", 1}).size(), 10u);
}

// HEGEL_TEST and hegel::test() derive the same key, `<file>::<name>`.
TEST(Settings, MacroKeyMatchesTheDerivedKey) {
    namespace fs = std::filesystem;

    fs::path db_path = fs::temp_directory_path() /
                       ("hegel_db_macro_key_" + std::to_string(::getpid()));
    fs::remove_all(db_path);
    fs::create_directories(db_path);

    failed_value = std::nullopt;
    try {
        check_database({.verbosity = hegel::Verbosity::Quiet,
                        .database = Database::from_path(db_path.string())});
    } catch (const std::runtime_error&) { // NOLINT(bugprone-empty-catch)
    }
    ASSERT_TRUE(failed_value.has_value());
    int64_t shrunk = *failed_value;

    failed_value = std::nullopt;
    try {
        hegel::test(
            [](hegel::TestCase& tc) {
                int64_t n = tc.draw(gs::integers<int64_t>());
                if (n >= 1'000'000) {
                    failed_value = n;
                    throw std::runtime_error("n >= 1_000_000");
                }
            },
            {"check_database", __FILE__, 1},
            {.verbosity = hegel::Verbosity::Quiet,
             .database = Database::from_path(db_path.string()),
             .phases = {hegel::Phase::Reuse}});
    } catch (const std::runtime_error&) { // NOLINT(bugprone-empty-catch)
    }
    fs::remove_all(db_path);
    EXPECT_EQ(failed_value, shrunk);
}
