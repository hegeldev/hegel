// A program defined by HEGEL_MAIN, run by tests/test_main.cpp.
//
// HEGEL_MAIN_SUBJECT_BODY selects what the test case does: `pass` (the
// default), `fail`, `empty_message`, `reject` or `throw_int`. Each test case
// prints one `subject: test case` line to stdout. HEGEL_MAIN_SUBJECT_STEPS sets
// Settings::stateful_step_count.
//
// The build compiles this file twice. The second program defines
// HEGEL_MAIN_SUBJECT_REPRODUCE: it uses the form of HEGEL_MAIN without
// settings and registers a blob that is not valid with
// HEGEL_REPRODUCE_FAILURE.

#include <cstdio>
#include <cstdlib>
#include <stdexcept>
#include <string>

#include <hegel/hegel.h>

namespace gs = hegel::generators;

namespace {
    std::string env(const char* name, const char* fallback) {
        const char* value = std::getenv(name);
        return value == nullptr ? fallback : value;
    }

    void body(hegel::TestCase& tc) {
        std::printf("subject: test case\n");
        std::fflush(stdout);
        int n = tc.draw("n", gs::integers<int>());
        std::string kind = env("HEGEL_MAIN_SUBJECT_BODY", "pass");
        if (kind == "fail") {
            throw std::runtime_error("failed with n=" + std::to_string(n));
        }
        if (kind == "empty_message") {
            throw std::runtime_error("");
        }
        if (kind == "reject") {
            tc.assume(false);
        }
        if (kind == "throw_int") {
            throw n;
        }
    }
} // namespace

#ifdef HEGEL_MAIN_SUBJECT_REPRODUCE

HEGEL_REPRODUCE_FAILURE(subject_workload, "not a blob")
HEGEL_MAIN(subject_workload)(hegel::TestCase& tc) { body(tc); }

#else

HEGEL_MAIN(subject_workload,
           {.test_cases = 50,
            .database = hegel::Database::disabled(),
            .stateful_step_count = std::stoll(
                env("HEGEL_MAIN_SUBJECT_STEPS", "50"))})(hegel::TestCase& tc) {
    body(tc);
}

#endif
