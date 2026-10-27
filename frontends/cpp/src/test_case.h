#pragma once

/**
 * @cond INTERNAL
 */

#include <hegel.h>
#include <hegel/explicit.h>

#include <cstdint>
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <vector>

namespace hegel::impl::test_case {

    // The display names the draws of a test case have taken. Shared by a test
    // case, its blocks and its clones, whose draws print into one sequence of
    // names.
    struct DrawNames {
        std::mutex mutex;
        std::map<std::string, int> counts;
    };

    // The state of an explicit test case: the values it was given, which
    // of them the draws took, and what it printed.
    struct ExplicitState {
        explicit ExplicitState(const ExplicitTestCase& test_case)
            : test_case(test_case), used(test_case.values().size(), false) {}

        const ExplicitTestCase& test_case;
        std::vector<bool> used;
        std::vector<std::string> lines;
    };

    // Per-case runtime state.
    // Generators reach `tc` through TestCase::data() to drive the typed draw
    // primitives. (The error-reporting context is per-thread.)
    struct TestCaseData {
        TestCaseData(hegel_test_case_t* handle, bool emit,
                     int64_t stateful_step_count)
            : tc(handle), emit(emit), stateful_step_count(stateful_step_count) {
        }

        // Releases the owned libhegel handles. The owning TestCase's
        // unique_ptr runs this on scope exit.
        ~TestCaseData();

        TestCaseData(const TestCaseData&) = delete;
        TestCaseData& operator=(const TestCaseData&) = delete;

        hegel_test_case_t* tc;
        // Whether notes and drawn values are recorded in the engine's
        // document for this case: when its output can be shown (the final
        // replay, a case the engine stamped for capture, or any case at
        // Verbosity::Verbose and above), and never under Verbosity::Quiet.
        bool emit;
        // Settings::stateful_step_count for this run, read when the case
        // creates a state machine.
        int64_t stateful_step_count;
        // Draws in progress on this handle. Only an outermost draw prints.
        int draw_depth = 0;
        std::shared_ptr<DrawNames> draw_names = std::make_shared<DrawNames>();
        // This handle's printer onto its region of the case's document,
        // fetched at the first print.
        hegel_printer_t* printer = nullptr;
        // Set for an explicit test case, which has no engine handle.
        std::shared_ptr<ExplicitState> explicit_state;
    };

    // Reads the document of drawn values and notes that `data`'s test case
    // recorded, one entry per line. `data` must hold the case's root handle.
    std::vector<std::string> rendered_output(const TestCaseData& data);

} // namespace hegel::impl::test_case

/// @endcond
