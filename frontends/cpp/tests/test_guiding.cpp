// Guiding generation: the TestCase controls that steer the engine rather than
// produce values — reject(), target(), and repeat().

#include <gtest/gtest.h>

#include <functional>
#include <limits>
#include <stdexcept>
#include <string>

#include <hegel/hegel.h>

namespace gs = hegel::generators;

// ---------------------------------------------------------------------------
// reject()
// ---------------------------------------------------------------------------

// reject() discards the current case as invalid, the same as assume(false).
// A run where some cases reject still completes without reporting a failure.
TEST(Reject, DiscardsCaseNotFailure) {
    EXPECT_NO_THROW(hegel::test(
        [](hegel::TestCase& tc) {
            int n = tc.draw(gs::integers<int>());
            if (n % 2 != 0) {
                tc.reject();
            }
            // Only even values reach here; the property trivially holds.
        },
        hegel::Settings{.test_cases = 100,
                        .derandomize = true,
                        .database = hegel::Database::disabled()}));
}

// reject() does not return: nothing after it runs. If it returned, the throw
// below would surface as a failure and hegel::test() would re-raise it.
// Rejecting every case leaves the run without a valid input, which the
// engine reports as Unsatisfiable.
TEST(Reject, DoesNotReturn) {
    try {
        hegel::test(
            [](hegel::TestCase& tc) {
                (void)tc.draw(gs::integers<int>());
                tc.reject();
                throw std::runtime_error("reject() returned"); // unreachable
            },
            hegel::Settings{
                .test_cases = 20,
                .derandomize = true,
                .database = hegel::Database::disabled(),
                .suppress_health_check = {hegel::HealthCheck::FilterTooMuch}});
        FAIL() << "expected an Unsatisfiable error";
    } catch (const std::runtime_error& error) {
        std::string message = error.what();
        EXPECT_NE(message.find("Unsatisfiable"), std::string::npos) << message;
        EXPECT_EQ(message.find("reject() returned"), std::string::npos)
            << message;
    }
}

// ---------------------------------------------------------------------------
// target()
// ---------------------------------------------------------------------------

// A run that records a targeting observation each case completes normally.
// Phase::Target is enabled by default, so the observation is honored.
TEST(Targeting, RecordsObservation) {
    EXPECT_NO_THROW(hegel::test(
        [](hegel::TestCase& tc) {
            int n = tc.draw(gs::integers<int>({.min_value = 0}));
            tc.target(static_cast<double>(n));
        },
        hegel::Settings{.test_cases = 100,
                        .derandomize = true,
                        .database = hegel::Database::disabled()}));
}

// The labelled form is accepted the same way. Distinct labels can be recorded
// on the same case.
TEST(Targeting, LabelledObservations) {
    EXPECT_NO_THROW(hegel::test(
        [](hegel::TestCase& tc) {
            int n = tc.draw(gs::integers<int>({.min_value = 0}));
            tc.target(static_cast<double>(n), "size");
            tc.target(static_cast<double>(-n), "negated");
        },
        hegel::Settings{.test_cases = 100,
                        .derandomize = true,
                        .database = hegel::Database::disabled()}));
}

// A non-finite score is a usage error: the engine rejects it and the C++ layer
// surfaces it as std::invalid_argument.
TEST(Targeting, NonFiniteScoreThrows) {
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         (void)tc.draw(gs::integers<int>());
                         tc.target(std::numeric_limits<double>::infinity());
                     },
                     hegel::Settings{.test_cases = 1,
                                     .derandomize = true,
                                     .database = hegel::Database::disabled()}),
                 std::invalid_argument);
}

// Each label may be recorded at most once per case; a duplicate is a usage
// error surfaced as std::invalid_argument.
TEST(Targeting, DuplicateLabelThrows) {
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         (void)tc.draw(gs::integers<int>());
                         tc.target(1.0, "dup");
                         tc.target(2.0, "dup");
                     },
                     hegel::Settings{.test_cases = 1,
                                     .derandomize = true,
                                     .database = hegel::Database::disabled()}),
                 std::invalid_argument);
}

// ---------------------------------------------------------------------------
// repeat()
// ---------------------------------------------------------------------------

// repeat() runs its body in an engine-managed loop that can iterate more than
// once, and control returns to the caller after the loop completes.
TEST(Loop, RunsBodyAndReturns) {
    int max_iters = 0;
    bool returned = false;
    hegel::test(
        [&](hegel::TestCase& tc) {
            int iters = 0;
            tc.repeat([&] {
                (void)tc.draw(gs::integers<int>());
                ++iters;
            });
            // Reached only because repeat() returns after the loop.
            returned = true;
            if (iters > max_iters) {
                max_iters = iters;
            }
        },
        hegel::Settings{.test_cases = 10,
                        .derandomize = true,
                        .database = hegel::Database::disabled()});
    EXPECT_TRUE(returned);
    EXPECT_GT(max_iters, 1) << "repeat() never iterated more than once";
}

// A failure raised inside a repeat() iteration is reported like any other
// failing property.
TEST(Loop, FailureInsideIsReported) {
    try {
        hegel::test(
            [](hegel::TestCase& tc) {
                tc.repeat([&] {
                    (void)tc.draw(gs::integers<int>());
                    throw std::runtime_error("boom");
                });
            },
            hegel::Settings{.test_cases = 200,
                            .derandomize = true,
                            .database = hegel::Database::disabled()});
        FAIL() << "expected the failure inside repeat() to be re-raised";
    } catch (const std::runtime_error& e) {
        EXPECT_EQ(std::string(e.what()), "boom");
    }
}

// Each repetition's draws print indented under its `// Repetition #N` note,
// and a rejected repetition still counts as one.
TEST(Loop, RepetitionsPrintIndented) {
    testing::internal::CaptureStderr();
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         int total = 0;
                         tc.repeat([&] {
                             int n = tc.draw(
                                 "n", gs::integers<int>(
                                          {.min_value = 0, .max_value = 5}));
                             tc.assume(n != 1);
                             total += n;
                             if (total > 5) {
                                 throw std::runtime_error("boom");
                             }
                         });
                     },
                     hegel::Settings{.test_cases = 200,
                                     .derandomize = true,
                                     .database = hegel::Database::disabled()}),
                 std::runtime_error);
    std::string out = testing::internal::GetCapturedStderr();
    EXPECT_NE(out.find("// Repetition #1\n  auto n_1 = "), std::string::npos)
        << out;
    EXPECT_NE(out.find("// Repetition #2\n  auto n_2 = "), std::string::npos)
        << out;
}

// A usage error inside a repetition propagates untouched.
TEST(Loop, UsageErrorInsideIsReraised) {
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         tc.repeat([&] {
                             (void)tc.draw(gs::integers<int>(
                                 {.min_value = 1, .max_value = 0}));
                         });
                     },
                     hegel::Settings{.test_cases = 5,
                                     .derandomize = true,
                                     .database = hegel::Database::disabled()}),
                 std::invalid_argument);
}

// reject() inside an iteration discards that iteration and the loop continues;
// the run completes without reporting a failure.
TEST(Loop, RejectInsideContinues) {
    bool completed = false;
    hegel::test(
        [&](hegel::TestCase& tc) {
            tc.repeat([&] {
                int n = tc.draw(
                    gs::integers<int>({.min_value = 0, .max_value = 3}));
                if (n % 2 != 0) {
                    tc.reject();
                }
            });
            completed = true;
        },
        hegel::Settings{
            .test_cases = 3,
            .derandomize = true,
            .database = hegel::Database::disabled(),
            .suppress_health_check = {hegel::HealthCheck::FilterTooMuch}});
    EXPECT_TRUE(completed);
}

// ---------------------------------------------------------------------------
// event() / event_value()
// ---------------------------------------------------------------------------

namespace {
    std::string
    run_capturing_stderr(const std::function<void(hegel::TestCase&)>& body,
                         bool show_statistics) {
        testing::internal::CaptureStderr();
        hegel::test(body,
                    hegel::Settings{.test_cases = 50,
                                    .derandomize = true,
                                    .show_statistics = show_statistics,
                                    .database = hegel::Database::disabled()});
        return testing::internal::GetCapturedStderr();
    }
} // namespace

TEST(Events, StatisticsReportEventsAndValues) {
    std::string output = run_capturing_stderr(
        [](hegel::TestCase& tc) {
            int n =
                tc.draw(gs::integers<int>({.min_value = 0, .max_value = 9}));
            if (n == 0) {
                tc.event("zero");
            }
            tc.event_value("n", static_cast<double>(n));
        },
        true);
    EXPECT_NE(output.find("Statistics (over "), std::string::npos) << output;
    EXPECT_NE(output.find("* zero: "), std::string::npos) << output;
    EXPECT_NE(output.find("* n: count "), std::string::npos) << output;
}

TEST(Events, NotReportedByDefault) {
    std::string output = run_capturing_stderr(
        [](hegel::TestCase& tc) {
            (void)tc.draw(gs::booleans());
            tc.event("always");
        },
        false);
    EXPECT_EQ(output.find("Statistics"), std::string::npos) << output;
}

TEST(Events, NonFiniteValueThrows) {
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         tc.event_value(
                             "bad", std::numeric_limits<double>::quiet_NaN());
                     },
                     hegel::Settings{.test_cases = 1,
                                     .database = hegel::Database::disabled()}),
                 std::invalid_argument);
}
