// HEGEL_MAIN, through the programs that tests/main_subject.cpp defines.

#include <gtest/gtest.h>

#include <regex>
#include <string>
#include <utility>
#include <vector>

#include "common/subprocess.h"

using hegel::tests::common::run_subject;
using hegel::tests::common::SubprocessResult;

#if !defined(HEGEL_MAIN_SUBJECT_BIN) ||                                        \
    !defined(HEGEL_MAIN_REPRODUCE_SUBJECT_BIN)
#error                                                                         \
    "HEGEL_MAIN_SUBJECT_BIN and HEGEL_MAIN_REPRODUCE_SUBJECT_BIN must be defined at build time"
#endif

namespace {
    using Env = std::vector<std::pair<std::string, std::string>>;

    SubprocessResult run_main(const std::vector<std::string>& args,
                              const std::string& body = "pass", Env env = {}) {
        env.emplace_back("HEGEL_MAIN_SUBJECT_BODY", body);
        return run_subject(HEGEL_MAIN_SUBJECT_BIN, args, env);
    }

    SubprocessResult run_reproduce_main(const std::vector<std::string>& args) {
        return run_subject(HEGEL_MAIN_REPRODUCE_SUBJECT_BIN, args,
                           {{"HEGEL_MAIN_SUBJECT_BODY", "fail"},
                            {"HEGEL_DATABASE", "disabled"}});
    }

    size_t test_cases_run(const SubprocessResult& r) {
        size_t count = 0;
        for (size_t at = r.stdout_data.find("test case\n");
             at != std::string::npos;
             at = r.stdout_data.find("test case\n", at + 1)) {
            count++;
        }
        return count;
    }

    std::string blob_in(const std::string& report) {
        std::smatch match;
        if (!std::regex_search(
                report, match,
                std::regex(R"(rerun with: --failure-blob (\S+))"))) {
            return "";
        }
        return match[1];
    }

    std::string failing_blob(const std::string& body = "fail") {
        return blob_in(run_main({"--phases", "generate"}, body).stderr_data);
    }

    bool contains(const std::string& text, const std::string& part) {
        return text.find(part) != std::string::npos;
    }
} // namespace

TEST(Main, RunsOneTestCaseWhateverTheSettings) {
    SubprocessResult r = run_main({}, "pass", {{"HEGEL_TEST_CASES", "50"}});
    EXPECT_EQ(r.exit_code, 0) << r.stderr_data;
    EXPECT_EQ(test_cases_run(r), 1u) << r.stdout_data;
}

TEST(Main, HelpPrintsUsage) {
    for (const char* help : {"--help", "-h"}) {
        SubprocessResult r = run_main({"--seed", "1", help});
        EXPECT_EQ(r.exit_code, 0);
        EXPECT_TRUE(contains(r.stdout_data, "Usage: ")) << r.stdout_data;
        EXPECT_TRUE(contains(r.stdout_data, "--failure-blob BLOB"));
        EXPECT_EQ(test_cases_run(r), 0u);
    }
}

TEST(Main, UsageErrors) {
    struct Case {
        std::vector<std::string> args;
        std::string message;
    };
    std::vector<Case> cases = {
        {{"positional"}, "unknown argument: positional"},
        {{"--"}, "unknown argument: --"},
        {{"--test-cases", "5"}, "--test-cases cannot be set"},
        {{"--test_cases=5"}, "unknown argument: --test_cases"},
        {{"--no-such-setting", "1"}, "unknown argument: --no-such-setting"},
        {{"--no-such-setting"}, "unknown argument: --no-such-setting"},
        {{"--seed"}, "--seed requires a value"},
        {{"--seed", "--derandomize", "true"}, "--seed requires a value"},
        {{"--seed=nope"}, "--seed expects an integer"},
        {{"--database-key="}, "--database-key expects a non-empty key"},
        {{"--profile"}, "--profile requires a value"},
        {{"--profile="}, "--profile requires a value"},
        {{"--profile", "ci", "--profile=ci"},
         "--profile can be given only once"},
        {{"--failure-blob"}, "--failure-blob requires a value"},
        {{"--failure-blob=a", "--failure-blob=b"},
         "--failure-blob can be given only once"},
        {{"--profile", "no-such-profile"}, "--profile: "},
        {{"--profile=default"}, "--profile: "},
    };
    for (const Case& c : cases) {
        SubprocessResult r = run_main(c.args);
        EXPECT_EQ(r.exit_code, 2) << c.args.front();
        EXPECT_EQ(test_cases_run(r), 0u) << c.args.front();
        EXPECT_TRUE(contains(r.stderr_data, "error: " + c.message))
            << r.stderr_data;
        EXPECT_TRUE(contains(r.stderr_data, "Usage: "));
    }
}

TEST(Main, ProfileApplies) {
    SubprocessResult r = run_main({"--profile=workload"});
    EXPECT_EQ(r.exit_code, 0) << r.stderr_data;
}

TEST(Main, FailureReportNamesTheProgramAndTheOption) {
    SubprocessResult r = run_main({"--phases", "generate"}, "fail");
    EXPECT_EQ(r.exit_code, 1);
    EXPECT_TRUE(contains(r.stderr_data, "--- Failure: subject_workload ("))
        << r.stderr_data;
    EXPECT_TRUE(contains(r.stderr_data, "main_subject.cpp:")) << r.stderr_data;
    EXPECT_NE(blob_in(r.stderr_data), "") << r.stderr_data;
}

TEST(Main, FailureBlobReplaysTheFailure) {
    std::string blob = failing_blob();
    ASSERT_NE(blob, "");
    SubprocessResult r = run_main({"--failure-blob", blob}, "fail");
    EXPECT_EQ(r.exit_code, 1);
    EXPECT_TRUE(contains(r.stderr_data, "auto n = ")) << r.stderr_data;
    EXPECT_TRUE(contains(r.stderr_data,
                         "Exception: std::runtime_error: failed with n="))
        << r.stderr_data;
}

TEST(Main, ReplayWithAnEmptyMessage) {
    std::string blob = failing_blob("empty_message");
    ASSERT_NE(blob, "");
    SubprocessResult r = run_main({"--failure-blob", blob}, "empty_message");
    EXPECT_EQ(r.exit_code, 1);
    EXPECT_TRUE(contains(r.stderr_data, "Exception: std::runtime_error\n"))
        << r.stderr_data;
}

TEST(Main, FailureBlobThatDoesNotReproduce) {
    std::string blob = failing_blob();
    ASSERT_NE(blob, "");
    SubprocessResult r = run_main({"--failure-blob=" + blob}, "pass");
    EXPECT_EQ(r.exit_code, 1);
    EXPECT_TRUE(contains(r.stderr_data, "error: The failure blob did not "
                                        "reproduce an error"))
        << r.stderr_data;
}

TEST(Main, InvalidRegisteredBlobIsReported) {
    SubprocessResult r = run_reproduce_main({});
    EXPECT_EQ(r.exit_code, 1);
    EXPECT_TRUE(contains(r.stderr_data, "error: ")) << r.stderr_data;
    EXPECT_EQ(test_cases_run(r), 0u);
}

TEST(Main, FailureBlobOptionComesBeforeRegisteredBlobs) {
    std::string blob = failing_blob();
    ASSERT_NE(blob, "");
    SubprocessResult r = run_reproduce_main({"--failure-blob", blob});
    EXPECT_EQ(r.exit_code, 1);
    EXPECT_TRUE(contains(r.stderr_data, "reproduced an error"))
        << r.stderr_data;
}

TEST(Main, OptionsSetVerbosityAndPrintBlob) {
    SubprocessResult quiet = run_main({"--verbosity", "quiet"}, "fail");
    EXPECT_EQ(quiet.exit_code, 1);
    EXPECT_FALSE(contains(quiet.stderr_data, "Failure")) << quiet.stderr_data;

    SubprocessResult no_blob = run_main({"--print-blob", "false"}, "fail");
    EXPECT_EQ(no_blob.exit_code, 1);
    EXPECT_FALSE(contains(no_blob.stderr_data, "rerun with:"))
        << no_blob.stderr_data;

    EXPECT_EQ(run_main({"--verbosity=verbose"}).exit_code, 0);
    EXPECT_EQ(run_main({"--verbosity=debug"}).exit_code, 0);
}

TEST(Main, RunErrorIsPrinted) {
    SubprocessResult r = run_main({}, "reject");
    EXPECT_EQ(r.exit_code, 1);
    EXPECT_TRUE(contains(r.stderr_data, "error: Hegel run error: "))
        << r.stderr_data;
}

TEST(Main, FailureWithANonStandardException) {
    SubprocessResult r = run_main({"--phases", "generate"}, "throw_int");
    EXPECT_EQ(r.exit_code, 1);
    EXPECT_TRUE(contains(r.stderr_data, "Exception: int")) << r.stderr_data;
}

TEST(Main, InvalidInlineSettings) {
    SubprocessResult r =
        run_main({}, "pass", {{"HEGEL_MAIN_SUBJECT_STEPS", "0"}});
    EXPECT_EQ(r.exit_code, 1);
    EXPECT_TRUE(contains(r.stderr_data, "error: Settings::stateful_step_count"))
        << r.stderr_data;
}
