#pragma once

#include <cstddef>
#include <exception>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include "settings.h"

namespace hegel {

    /**
     * @brief The engine stopped the run without a verdict on the property.
     *
     * hegel::test() throws a RunError, or one of its subclasses, when the run
     * itself fails rather than the test: a failed health check
     * (FailedHealthCheck), nondeterminism under
     * NondeterminismStrictness::Error (Flaky), too few valid test cases
     * (Unsatisfiable), or another engine error. The message says which.
     */
    class RunError : public std::runtime_error {
      public:
        /// @param message The engine's diagnostic.
        explicit RunError(const std::string& message)
            : std::runtime_error(message) {}
    };

    /**
     * @brief A health check failed.
     *
     * The run was likely to test poorly or slowly, for example because it
     * rejected too many test cases. The message says which check failed and
     * how to suppress it with Settings::suppress_health_check if the
     * behavior is intentional.
     */
    class FailedHealthCheck : public RunError {
      public:
        /// @param message The engine's diagnostic.
        /// @param health_check The check that failed, if the engine names
        ///        it.
        FailedHealthCheck(const std::string& message,
                          std::optional<HealthCheck> health_check)
            : RunError(message), health_check_(health_check) {}

        /// The check that failed.
        /// @return The check, or std::nullopt if the engine did not name it.
        std::optional<HealthCheck> health_check() const {
            return health_check_;
        }

      private:
        std::optional<HealthCheck> health_check_;
    };

    /**
     * @brief The test behaved nondeterministically under
     * NondeterminismStrictness::Error.
     *
     * The test produced different outcomes, or drew different data, when
     * the engine replayed the same choices. Under the default strictness the
     * engine confirms such failures by replay and reports them with a caveat
     * instead.
     */
    class Flaky : public RunError {
      public:
        /// @param message The engine's diagnostic.
        explicit Flaky(const std::string& message) : RunError(message) {}
    };

    /**
     * @brief The engine could not find inputs that satisfy the test's
     * assumptions and filters.
     */
    class Unsatisfiable : public RunError {
      public:
        /// @param message The engine's diagnostic.
        explicit Unsatisfiable(const std::string& message)
            : RunError(message) {}
    };

    /**
     * @brief No failure blob given to hegel::test() or
     * @ref HEGEL_REPRODUCE_FAILURE made the test fail.
     *
     * The bug is fixed (remove the blobs), or the test changed so that the
     * blobs mean something else.
     */
    class DidNotReproduce : public std::runtime_error {
      public:
        /// @param message What did not reproduce.
        explicit DidNotReproduce(const std::string& message)
            : std::runtime_error(message) {}
    };

    /**
     * @brief The run found more than one distinct failure.
     *
     * hegel::test() throws this when Settings::report_multiple_failures is
     * true and the run found more than one failure. Each failure's own
     * exception is in failures(). The message says how many failures there
     * are, then gives the message of each on a line of its own.
     */
    class MultipleFailures : public std::runtime_error {
      public:
        /// @param failures The exception of each failure.
        explicit MultipleFailures(std::vector<std::exception_ptr> failures)
            : std::runtime_error(describe(failures)),
              failures_(std::move(failures)) {}

        /// The exception of each failure, in the order of the report.
        /// @return The exceptions.
        const std::vector<std::exception_ptr>& failures() const {
            return failures_;
        }

      private:
        static std::string
        describe(const std::vector<std::exception_ptr>& failures) {
            std::string out = "Property-based test failed with " +
                              std::to_string(failures.size()) +
                              " distinct failures.";
            for (const std::exception_ptr& failure : failures) {
                out += "\n";
                try {
                    std::rethrow_exception(failure);
                } catch (const std::exception& error) {
                    out += error.what();
                } catch (...) {
                    out += "(an exception that is not a std::exception)";
                }
            }
            return out;
        }

        std::vector<std::exception_ptr> failures_;
    };

} // namespace hegel
