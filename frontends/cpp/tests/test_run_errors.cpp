#include <gtest/gtest.h>

#include <engine.h>

#include <hegel/errors.h>
#include <hegel/settings.h>

#include <hegel.h>
#include <hegel/hegel.h>

#include <optional>
#include <string>

using hegel::HealthCheck;

TEST(RunErrorKinds, HealthCheckFlagsMapToHealthChecks) {
    EXPECT_EQ(hegel::impl::health_check_from_flag(HEGEL_HC_FILTER_TOO_MUCH),
              HealthCheck::FilterTooMuch);
    EXPECT_EQ(hegel::impl::health_check_from_flag(HEGEL_HC_TOO_SLOW),
              HealthCheck::TooSlow);
    EXPECT_EQ(
        hegel::impl::health_check_from_flag(HEGEL_HC_TEST_CASES_TOO_LARGE),
        HealthCheck::TestCasesTooLarge);
    EXPECT_EQ(
        hegel::impl::health_check_from_flag(HEGEL_HC_LARGE_INITIAL_TEST_CASE),
        HealthCheck::LargeInitialTestCase);
    EXPECT_EQ(hegel::impl::health_check_from_flag(0), std::nullopt);
}

TEST(RunErrorKinds, EachKindRaisesItsError) {
    using hegel::impl::raise_run_error;
    try {
        raise_run_error("slow",
                        {HEGEL_RUN_ERROR_HEALTH_CHECK, HEGEL_HC_TOO_SLOW});
        FAIL();
    } catch (const hegel::FailedHealthCheck& e) {
        EXPECT_EQ(e.health_check(), HealthCheck::TooSlow);
        EXPECT_EQ(std::string(e.what()), "slow");
    }
    EXPECT_THROW(raise_run_error("flaky", {HEGEL_RUN_ERROR_FLAKY, 0}),
                 hegel::Flaky);
    EXPECT_THROW(
        raise_run_error("diverged", {HEGEL_RUN_ERROR_NONDETERMINISTIC, 0}),
        hegel::Flaky);
    EXPECT_THROW(raise_run_error("none", {HEGEL_RUN_ERROR_UNSATISFIABLE, 0}),
                 hegel::Unsatisfiable);
    try {
        raise_run_error("bug", {HEGEL_RUN_ERROR_INTERNAL, 0});
        FAIL();
    } catch (const hegel::FailedHealthCheck&) {
        FAIL();
    } catch (const hegel::Flaky&) {
        FAIL();
    } catch (const hegel::Unsatisfiable&) {
        FAIL();
    } catch (const hegel::RunError& e) {
        EXPECT_EQ(std::string(e.what()), "bug");
    }
}

TEST(RunErrorKinds, ARunThatRejectsEveryCaseRaisesARunError) {
    namespace gs = hegel::generators;
    hegel::Settings settings;
    settings.test_cases = 5;
    settings.verbosity = hegel::Verbosity::Verbose;
    settings.database = hegel::Database::disabled();
    testing::internal::CaptureStderr();
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         (void)tc.draw(gs::text({.max_size = 1}));
                         (void)tc.draw(gs::none());
                         tc.reject();
                     },
                     settings),
                 hegel::RunError);
    EXPECT_NE(testing::internal::GetCapturedStderr().find("std::monostate"),
              std::string::npos);
}
