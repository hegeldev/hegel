// Explicit test cases: values given in advance run the test body before any
// generated test case.

#include <gtest/gtest.h>

#include <cstdint>
#include <functional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include <hegel/hegel.h>

namespace gs = hegel::generators;

namespace {
    hegel::Settings quiet_settings(uint64_t test_cases = 10) {
        return hegel::Settings{.test_cases = test_cases,
                               .verbosity = hegel::Verbosity::Quiet,
                               .database = hegel::Database::disabled()};
    }

    // Runs `body` with `cases` and returns what it wrote to stderr.
    std::string run_capturing(const std::function<void(hegel::TestCase&)>& body,
                              const std::vector<hegel::ExplicitTestCase>& cases,
                              hegel::Settings settings) {
        testing::internal::CaptureStderr();
        try {
            hegel::test(body, settings, {}, cases);
        } catch (...) {
            testing::internal::GetCapturedStderr();
            throw;
        }
        return testing::internal::GetCapturedStderr();
    }

    std::string
    failure_message(const std::function<void(hegel::TestCase&)>& body,
                    const std::vector<hegel::ExplicitTestCase>& cases) {
        std::string message;
        testing::internal::CaptureStderr();
        try {
            hegel::test(
                body,
                hegel::Settings{.test_cases = 10,
                                .database = hegel::Database::disabled()},
                {}, cases);
        } catch (const std::invalid_argument& error) {
            message = error.what();
        }
        EXPECT_EQ(testing::internal::GetCapturedStderr(), "");
        return message;
    }
} // namespace

TEST(Explicit, PositionalValuesAreDrawnInOrderBeforeGeneration) {
    std::vector<std::pair<int, std::string>> seen;
    hegel::test(
        [&seen](hegel::TestCase& tc) {
            int n = tc.draw(gs::integers<int>());
            std::string s = tc.draw(gs::text());
            seen.emplace_back(n, s);
        },
        quiet_settings(5), {},
        {hegel::ExplicitTestCase(7, "seven"),
         hegel::ExplicitTestCase(8, std::string("eight"))});
    ASSERT_GE(seen.size(), 3u);
    EXPECT_EQ(seen[0], std::make_pair(7, std::string("seven")));
    EXPECT_EQ(seen[1], std::make_pair(8, std::string("eight")));
}

TEST(Explicit, NamedValuesGoToTheDrawOfThatName) {
    std::vector<std::vector<int>> seen;
    auto named = hegel::named("y_1", 2);
    hegel::test(
        [&seen](hegel::TestCase& tc) {
            int x = tc.draw("x", gs::integers<int>());
            int y = tc.draw("y", gs::integers<int>());
            int z1 = tc.draw("z", gs::integers<int>());
            int z2 = tc.draw("z", gs::integers<int>());
            seen.push_back({x, y, z1, z2});
        },
        quiet_settings(1), {},
        {hegel::ExplicitTestCase(named, hegel::named("z_2", 4), 1, 3)});
    ASSERT_FALSE(seen.empty());
    EXPECT_EQ(seen[0], (std::vector<int>{1, 2, 3, 4}));
}

TEST(Explicit, ACaseCanBeCopied) {
    hegel::ExplicitTestCase original(1);
    hegel::ExplicitTestCase copy = original;
    std::vector<int> seen;
    hegel::test(
        [&seen](hegel::TestCase& tc) {
            seen.push_back(tc.draw(gs::integers<int>()));
        },
        quiet_settings(1), {}, {copy});
    EXPECT_EQ(seen.front(), 1);
}

TEST(Explicit, FailureIsReportedAndPropagates) {
    int calls = 0;
    try {
        (void)run_capturing(
            [&calls](hegel::TestCase& tc) {
                calls++;
                int n = tc.draw("n", gs::integers<int>());
                tc.note("checking n");
                tc.target(1.0);
                tc.event("explicit");
                tc.event_value("n", 1.0);
                if (n == 3) {
                    throw std::runtime_error("n is three");
                }
            },
            {hegel::ExplicitTestCase(1), hegel::ExplicitTestCase(3)},
            hegel::Settings{.test_cases = 10,
                            .database = hegel::Database::disabled()});
        FAIL() << "expected the explicit failure to propagate";
    } catch (const std::runtime_error& error) {
        EXPECT_EQ(std::string(error.what()), "n is three");
    }
    EXPECT_EQ(calls, 2);
}

TEST(Explicit, FailureReportShowsTheCase) {
    testing::internal::CaptureStderr();
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         (void)tc.draw("n", gs::integers<int>());
                         tc.note("checking n");
                         throw std::runtime_error("boom");
                     },
                     {"explicit_report", "file.cpp", 3},
                     hegel::Settings{.database = hegel::Database::disabled()},
                     {}, {hegel::ExplicitTestCase(hegel::named("n_1", 5))}),
                 std::runtime_error);
    EXPECT_EQ(testing::internal::GetCapturedStderr(),
              "auto n_1 = 5;\nchecking n\n");
}

TEST(Explicit, FailureWithoutDrawsHasNoBody) {
    testing::internal::CaptureStderr();
    EXPECT_THROW(
        hegel::test([](hegel::TestCase&) { throw std::logic_error(""); },
                    hegel::Settings{.database = hegel::Database::disabled()},
                    {}, {hegel::ExplicitTestCase()}),
        std::logic_error);
    EXPECT_EQ(testing::internal::GetCapturedStderr(), "");
}

TEST(Explicit, AnInvalidArgumentFromTheTestIsAFailure) {
    testing::internal::CaptureStderr();
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         (void)tc.draw("n", gs::integers<int>());
                         throw std::invalid_argument("from the test");
                     },
                     hegel::Settings{.database = hegel::Database::disabled()},
                     {}, {hegel::ExplicitTestCase(4)}),
                 std::invalid_argument);
    EXPECT_EQ(testing::internal::GetCapturedStderr(), "auto n_1 = 4;\n");
}

TEST(Explicit, QuietFailurePrintsNothing) {
    testing::internal::CaptureStderr();
    EXPECT_THROW(
        hegel::test([](hegel::TestCase&) { throw std::logic_error(""); },
                    quiet_settings(), {}, {hegel::ExplicitTestCase()}),
        std::logic_error);
    EXPECT_EQ(testing::internal::GetCapturedStderr(), "");
}

TEST(Explicit, VerboseRunsPrintEachCase) {
    std::string output = run_capturing(
        [](hegel::TestCase& tc) { (void)tc.draw(gs::booleans()); },
        {hegel::ExplicitTestCase(true)},
        hegel::Settings{.test_cases = 1,
                        .verbosity = hegel::Verbosity::Verbose,
                        .database = hegel::Database::disabled()});
    EXPECT_EQ(output.rfind("auto draw_1 = true;\n", 0), 0u) << output;
}

TEST(Explicit, SkippedWithoutTheExplicitPhase) {
    bool saw_explicit = false;
    hegel::test(
        [&saw_explicit](hegel::TestCase& tc) {
            if (tc.draw(gs::integers<int>({.min_value = 0})) == -1) {
                saw_explicit = true;
            }
        },
        hegel::Settings{.test_cases = 5,
                        .verbosity = hegel::Verbosity::Quiet,
                        .database = hegel::Database::disabled(),
                        .phases = {hegel::Phase::Generate}},
        {}, {hegel::ExplicitTestCase(-1)});
    EXPECT_FALSE(saw_explicit);
}

TEST(Explicit, MissingValueIsAnError) {
    std::string message = failure_message(
        [](hegel::TestCase& tc) { (void)tc.draw("x", gs::integers<int>()); },
        {hegel::ExplicitTestCase()});
    EXPECT_NE(message.find("no value for the draw x"), std::string::npos)
        << message;
}

TEST(Explicit, WrongTypeIsAnError) {
    std::string message = failure_message(
        [](hegel::TestCase& tc) { (void)tc.draw(gs::integers<int64_t>()); },
        {hegel::ExplicitTestCase(1)});
    EXPECT_NE(message.find("has type int, but the draw produces"),
              std::string::npos)
        << message;
}

TEST(Explicit, UnusedValuesAreAnError) {
    std::string message = failure_message(
        [](hegel::TestCase& tc) { (void)tc.draw(gs::integers<int>()); },
        {hegel::ExplicitTestCase(1, 2, hegel::named("y_1", 3))});
    EXPECT_NE(message.find("gave values that no draw took: a positional "
                           "value, y_1"),
              std::string::npos)
        << message;
}

TEST(Explicit, RejectionIsAnError) {
    std::string message =
        failure_message([](hegel::TestCase& tc) { tc.assume(false); },
                        {hegel::ExplicitTestCase()});
    EXPECT_EQ(message.rfind("Explicit test cases must not be rejected", 0), 0u)
        << message;
    EXPECT_NE(message.find("the explicit test case () was rejected"),
              std::string::npos)
        << message;
}

TEST(Explicit, RejectionAfterADrawIsAnError) {
    std::string message = failure_message(
        [](hegel::TestCase& tc) {
            int n = tc.draw(gs::integers<int>());
            tc.assume(n > 5);
        },
        {hegel::ExplicitTestCase(1)});
    EXPECT_EQ(message.rfind("Explicit test cases must not be rejected", 0), 0u)
        << message;
}

TEST(Explicit, RejectionInsideRepeatIsAnError) {
    bool ran = false;
    std::string message = failure_message(
        [&ran](hegel::TestCase& tc) {
            tc.repeat([&] {
                ran = true;
                tc.reject();
            });
        },
        {hegel::ExplicitTestCase()});
    EXPECT_TRUE(ran);
    EXPECT_EQ(message.rfind("Explicit test cases must not be rejected", 0), 0u)
        << message;
}

TEST(Explicit, RepeatRunsItsBodyOnce) {
    std::vector<int> seen;
    hegel::test(
        [&seen](hegel::TestCase& tc) {
            tc.repeat([&] { seen.push_back(tc.draw(gs::integers<int>())); });
        },
        hegel::Settings{.verbosity = hegel::Verbosity::Quiet,
                        .database = hegel::Database::disabled(),
                        .phases = {hegel::Phase::Explicit}},
        {}, {hegel::ExplicitTestCase(4)});
    EXPECT_EQ(seen, std::vector<int>{4});
}

TEST(Explicit, FailureInsideRepeatShowsTheRepetition) {
    testing::internal::CaptureStderr();
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         tc.repeat([&] {
                             (void)tc.draw("n", gs::integers<int>());
                             throw std::runtime_error("boom");
                         });
                     },
                     hegel::Settings{.database = hegel::Database::disabled()},
                     {}, {hegel::ExplicitTestCase(4)}),
                 std::runtime_error);
    EXPECT_EQ(testing::internal::GetCapturedStderr(),
              "// Repetition #1\nauto n_1 = 4;\n");
}

TEST(Explicit, CloneSharesTheValuesAndTheNamingScope) {
    std::vector<int> seen;
    testing::internal::CaptureStderr();
    EXPECT_THROW(hegel::test(
                     [&seen](hegel::TestCase& tc) {
                         seen.push_back(tc.draw("a", gs::integers<int>()));
                         hegel::TestCase worker = tc.clone();
                         seen.push_back(worker.draw(gs::integers<int>()));
                         seen.push_back(tc.draw(gs::integers<int>()));
                         worker.note("from the clone");
                         throw std::runtime_error("boom");
                     },
                     hegel::Settings{.database = hegel::Database::disabled()},
                     {}, {hegel::ExplicitTestCase(hegel::named("a_1", 1), 2, 3)}),
                 std::runtime_error);
    EXPECT_EQ(seen, (std::vector<int>{1, 2, 3}));
    EXPECT_EQ(testing::internal::GetCapturedStderr(),
              "auto a_1 = 1;\nauto draw_1 = 2;\nauto draw_2 = 3;\nfrom the "
              "clone\n");
}

TEST(Explicit, ClonesDrawFromOtherThreads) {
    std::vector<int> seen;
    hegel::test(
        [&seen](hegel::TestCase& tc) {
            auto worker = tc.spawn(
                [](hegel::TestCase& c) { return c.draw(gs::integers<int>()); });
            seen.push_back(worker.join());
            seen.push_back(tc.draw(gs::integers<int>()));
        },
        hegel::Settings{.verbosity = hegel::Verbosity::Quiet,
                        .database = hegel::Database::disabled(),
                        .phases = {hegel::Phase::Explicit}},
        {}, {hegel::ExplicitTestCase(1, 2)});
    EXPECT_EQ(seen, (std::vector<int>{1, 2}));
}

namespace {
    hegel::Settings multiple_failure_settings(
        hegel::Verbosity verbosity = hegel::Verbosity::Normal) {
        return hegel::Settings{.test_cases = 20,
                               .report_multiple_failures = true,
                               .verbosity = verbosity,
                               .database = hegel::Database::disabled(),
                               .suppress_health_check =
                                   hegel::all_health_checks()};
    }

    // Runs `body` with `cases` under `settings` and returns what it wrote to
    // stderr and the failures it raised, in order.
    std::pair<std::string, std::vector<std::string>>
    multiple_failures(const std::function<void(hegel::TestCase&)>& body,
                      const std::vector<hegel::ExplicitTestCase>& cases,
                      const hegel::Settings& settings) {
        std::vector<std::string> messages;
        testing::internal::CaptureStderr();
        try {
            hegel::test(body, settings, {}, cases);
        } catch (const hegel::MultipleFailures& error) {
            for (const std::exception_ptr& failure : error.failures()) {
                try {
                    std::rethrow_exception(failure);
                } catch (const std::exception& e) {
                    messages.emplace_back(e.what());
                }
            }
        } catch (const std::exception& error) {
            messages.emplace_back(error.what());
        }
        return {testing::internal::GetCapturedStderr(), messages};
    }
} // namespace

TEST(Explicit, FirstFailureEndsTheRunWithoutReportMultipleFailures) {
    int calls = 0;
    EXPECT_THROW((void)run_capturing(
                     [&calls](hegel::TestCase& tc) {
                         calls++;
                         (void)tc.draw(gs::integers<int>());
                         throw std::runtime_error("boom");
                     },
                     {hegel::ExplicitTestCase(1), hegel::ExplicitTestCase(2)},
                     hegel::Settings{.report_multiple_failures = false,
                                     .verbosity = hegel::Verbosity::Quiet,
                                     .database = hegel::Database::disabled()}),
                 std::runtime_error);
    EXPECT_EQ(calls, 1);
}

TEST(Explicit, FailuresAreCollectedWithTheGeneratedOnes) {
    int explicit_calls = 0;
    auto [output, messages] = multiple_failures(
        [&explicit_calls](hegel::TestCase& tc) {
            int x = tc.draw("x", gs::integers<int>({.min_value = 10}));
            if (x < 10) {
                explicit_calls++;
            }
            if (x == 1) {
                throw std::runtime_error("x is one");
            }
            if (x == 2) {
                throw std::logic_error("x is two");
            }
            throw std::domain_error("x is large");
        },
        {hegel::ExplicitTestCase(1), hegel::ExplicitTestCase(2),
         hegel::ExplicitTestCase(1)},
        multiple_failure_settings());
    EXPECT_EQ(explicit_calls, 3);
    EXPECT_EQ(messages,
              (std::vector<std::string>{"x is one", "x is two", "x is large"}));
    EXPECT_EQ(output.rfind("Property-based test failed with 3 distinct "
                           "failures.\n\nauto x_1 = 1;\n\nauto x_1 = 2;\n\nauto "
                           "x_1 = 10;\n",
                           0),
              0u)
        << output;
    EXPECT_NE(output.find("To reproduce"), std::string::npos) << output;
}

TEST(Explicit, GeneratedFailureSharingAnExplicitOriginIsDropped) {
    auto [output, messages] = multiple_failures(
        [](hegel::TestCase& tc) {
            (void)tc.draw("x", gs::integers<int>());
            throw std::runtime_error("boom");
        },
        {hegel::ExplicitTestCase(7)}, multiple_failure_settings());
    EXPECT_EQ(messages, std::vector<std::string>{"boom"});
    EXPECT_EQ(output, "auto x_1 = 7;\n");
}

TEST(Explicit, QuietFailuresPrintNothingBeforeAPassingGeneratedPhase) {
    auto [output, messages] = multiple_failures(
        [](hegel::TestCase& tc) {
            if (tc.draw("x", gs::integers<int>({.min_value = 0})) < 0) {
                throw std::runtime_error("negative");
            }
        },
        {hegel::ExplicitTestCase(-1)},
        multiple_failure_settings(hegel::Verbosity::Quiet));
    EXPECT_EQ(messages, std::vector<std::string>{"negative"});
    EXPECT_EQ(output, "");
}

TEST(Explicit, ARunErrorIsReportedAfterTheFailures) {
    auto [output, messages] = multiple_failures(
        [](hegel::TestCase& tc) {
            int x = tc.draw("x", gs::integers<int>({.min_value = 0}));
            if (x < 0) {
                throw std::runtime_error("negative");
            }
            tc.assume(false);
        },
        {hegel::ExplicitTestCase(-1)}, multiple_failure_settings());
    ASSERT_EQ(messages.size(), 2u);
    EXPECT_EQ(messages[0], "negative");
    EXPECT_EQ(messages[1].rfind("Unsatisfiable: unable to satisfy", 0), 0u)
        << messages[1];
    EXPECT_EQ(output.rfind("Property-based test failed with 2 distinct "
                           "failures.\n\nauto x = -1;\n\nUnsatisfiable: "
                           "unable to satisfy",
                           0),
              0u)
        << output;
}

TEST(Explicit, ARunErrorWithoutExplicitFailuresPropagates) {
    EXPECT_THROW((void)run_capturing(
                     [](hegel::TestCase& tc) {
                         (void)tc.draw("x", gs::integers<int>());
                         tc.assume(false);
                     },
                     {}, multiple_failure_settings(hegel::Verbosity::Quiet)),
                 hegel::Unsatisfiable);
}

TEST(Explicit, OperationsNeedingGeneratedDataAreAnError) {
    std::string message = failure_message(
        [](hegel::TestCase& tc) { hegel::stateful::Pool<int> pool(tc); },
        {hegel::ExplicitTestCase()});
    EXPECT_NE(message.find("needs generated data"), std::string::npos)
        << message;
}

namespace {
    std::vector<int> macro_seen;
} // namespace

HEGEL_EXPLICIT_TEST_CASE(explicit_macro_test, 100)
HEGEL_EXPLICIT_TEST_CASE(explicit_macro_test, hegel::named("n_1", 200))
HEGEL_TEST(explicit_macro_test,
           {.test_cases = 3,
            .verbosity = hegel::Verbosity::Quiet,
            .database = hegel::Database::disabled()})(hegel::TestCase& tc) {
    macro_seen.push_back(tc.draw("n", gs::integers<int>({.max_value = 50})));
}

TEST(Explicit, MacroRegistersCasesInOrder) {
    macro_seen.clear();
    explicit_macro_test();
    ASSERT_GE(macro_seen.size(), 2u);
    EXPECT_EQ(macro_seen[0], 100);
    EXPECT_EQ(macro_seen[1], 200);
}
