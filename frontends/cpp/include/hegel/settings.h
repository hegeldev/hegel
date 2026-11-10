#pragma once

#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <initializer_list>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace hegel {
    /**
     * @brief Verbosity levels.
     */
    enum class Verbosity {
        Quiet,   ///< Minimal output (used by TUI)
        Normal,  ///< Default - standard test output
        Verbose, ///< More detailed output
        Debug    ///< Maximum verbosity + engine-side shrinker tracing
    };

    /**
     * @brief Convert Verbosity enum to command-line string.
     * @param v The verbosity level
     * @return The string representation for CLI argument
     */
    inline const char* verbosity_to_string(Verbosity v) {
        switch (v) {
        case Verbosity::Quiet:
            return "quiet";
        case Verbosity::Verbose:
            return "verbose";
        case Verbosity::Debug:
            return "debug";
        case Verbosity::Normal:
        default:
            return "normal";
        }
    }

    /**
     * @brief Health checks.
     */
    enum class HealthCheck {
        FilterTooMuch,        ///< Test filters out too many generated examples.
        TooSlow,              ///< Test cases are running too slowly.
        TestCasesTooLarge,    ///< Generated test cases are too large.
        LargeInitialTestCase, ///< First generated test case is unusually large.
    };

    /// @cond INTERNAL
    inline const char* health_check_to_string(HealthCheck c) {
        switch (c) {
        case HealthCheck::FilterTooMuch:
            return "filter_too_much";
        case HealthCheck::TooSlow:
            return "too_slow";
        case HealthCheck::TestCasesTooLarge:
            return "test_cases_too_large";
        case HealthCheck::LargeInitialTestCase:
            return "large_initial_test_case";
        }
        return "";
    }
    /// @endcond

    /**
     * @brief All health checks, suitable for full suppression.
     * @return A vector containing every HealthCheck variant.
     */
    inline std::vector<HealthCheck> all_health_checks() {
        return {HealthCheck::FilterTooMuch, HealthCheck::TooSlow,
                HealthCheck::TestCasesTooLarge,
                HealthCheck::LargeInitialTestCase};
    }

    /**
     * @brief Phases of a property-test run.
     */
    enum class Phase {
        Explicit, ///< Run hard-coded explicit examples.
        Reuse,    ///< Replay counterexamples persisted in the database.
        Generate, ///< Randomly generate fresh test cases.
        Target,   ///< Hill-climb toward observed target scores.
        Shrink,   ///< Shrink failing examples toward minimal counterexamples.
    };

    /**
     * @brief All phases.
     * @return A vector containing every Phase variant.
     */
    inline std::vector<Phase> all_phases() {
        return {Phase::Explicit, Phase::Reuse, Phase::Generate, Phase::Target,
                Phase::Shrink};
    }

    /**
     * @brief The phases of a run. See Settings::phases.
     *
     * Give the phases as a list: `.phases = {Phase::Generate}`. With an empty
     * list (`.phases = {}`), the run does nothing.
     */
    class Phases {
      public:
        /// Run only the given phases.
        /// @param phases The phases to run.
        Phases(std::initializer_list<Phase> phases)
            : set_(true), phases_(phases) {}

        /// Run only the given phases.
        /// @param phases The phases to run.
        Phases(std::vector<Phase> phases)
            : set_(true), phases_(std::move(phases)) {}

        /// Use the phases of the settings profile.
        /// @return An unset Phases.
        static Phases unset() { return Phases(UnsetTag{}); }

        /// @cond INTERNAL
        bool is_set() const { return set_; }
        const std::vector<Phase>& phases() const { return phases_; }
        /// @endcond

      private:
        struct UnsetTag {};
        explicit Phases(UnsetTag) {}
        bool set_ = false;
        std::vector<Phase> phases_;
    };

    /**
     * @brief Source of randomness the engine draws from.
     */
    enum class Backend {
        Auto,    ///< Leave the choice to the engine's settings profile (the
                 ///< default): Urandom inside Antithesis, Default otherwise.
        Default, ///< Expand a single seeded PRNG. Runs are reproducible from
                 ///< the seed and shrinking / replay work as usual.
        Urandom, ///< Read fresh entropy from `/dev/urandom` on every draw.
                 ///< Intended for running under Antithesis; you almost
                 ///< certainly don't want it otherwise.
    };

    /**
     * @brief How a run reacts when it detects nondeterministic test behavior:
     * a test whose structure or outcome changes when the same choices are
     * replayed.
     */
    enum class NondeterminismStrictness {
        Quiet, ///< Switch to nondeterministic handling silently: failures
               ///< are confirmed by repeated replay before they are shrunk
               ///< or reported, and the report says how reliably each one
               ///< reproduced. The default.
        Warn,  ///< Switch as under Quiet, printing a one-line notice once
               ///< per run.
        Error, ///< Fail the run with a flaky-test error, for suites that use
               ///< determinism as a lint.
    };

    /**
     * @brief Configure the Hegel database.
     */
    class Database {
      public:
        /// The configuration of the database. See the methods below.
        enum class Kind {
            Unset, ///< Default behavior. By default, Hegel places the database
                   ///< at `.hegel`.
            Disabled, ///< Disable the database.
            Path,     ///< Use the given directory as the database.
        };

        /// Default behavior. By default, Hegel places the database at `.hegel`.
        /// @return A database with Database.Kind.Unset.
        static Database unset() { return {Kind::Unset, {}}; }

        /// Disable the database.
        /// @return A database with Database.Kind.Disabled.
        static Database disabled() { return {Kind::Disabled, {}}; }

        /// Use the given directory as the database.
        /// @param path The directory path for the database.
        /// @return A database with Database.Kind.Path.
        static Database from_path(std::string path) {
            return {Kind::Path, std::move(path)};
        }

        /// @cond INTERNAL
        Kind kind() const { return kind_; }
        const std::string& path() const { return path_; }
        /// @endcond

      private:
        Database(Kind k, std::string p) : kind_(k), path_(std::move(p)) {}
        Kind kind_;
        std::string path_;
    };

    /// @cond INTERNAL
    namespace internal {
        // True if a CI environment is detected, by probing the variables the
        // common CI providers set. Used as the default for
        // Settings::database, which is disabled under CI.
        inline bool in_ci() {
            // expected == nullptr means "any value present satisfies it".
            struct CiVar {
                const char* name;
                const char* expected;
            };
            static constexpr CiVar ci_vars[] = {
                {"CI", nullptr},
                {"TF_BUILD", "true"},
                {"BUILDKITE", "true"},
                {"CIRCLECI", "true"},
                {"CIRRUS_CI", "true"},
                {"CODEBUILD_BUILD_ID", nullptr},
                {"GITHUB_ACTIONS", "true"},
                {"GITLAB_CI", nullptr},
                {"HEROKU_TEST_RUN_ID", nullptr},
                {"TEAMCITY_VERSION", nullptr},
            };
            for (const CiVar& v : ci_vars) {
                const char* value = std::getenv(v.name);
                if (value == nullptr) {
                    continue;
                }
                if (v.expected == nullptr ||
                    std::strcmp(value, v.expected) == 0) {
                    return true;
                }
            }
            return false;
        }
    } // namespace internal
    /// @endcond

    /**
     * @brief Configuration options for hegel::test().
     *
     * A field that you do not set gets its value from the settings profile.
     * The engine selects the profile from `hegel_set_default_profile`, the
     * `HEGEL_DEFAULT_PROFILE` environment variable, or a `hegel.toml` in the
     * working directory or a parent directory. If none of these selects a
     * profile, the engine uses `workload` in Antithesis, `ci` on a CI server,
     * and `development` in all other environments. The `HEGEL_*` settings
     * environment variables change the profile values. A field that you set
     * overrides the profile.
     */
    struct Settings {
        /// Number of test cases to run. Profile default: 100.
        std::optional<uint64_t> test_cases;

        /// Verbosity level. Profile default: Verbosity::Normal.
        std::optional<Verbosity> verbosity;

        /// Seed for the test. Profile default: no seed.
        std::optional<uint64_t> seed;

        /// If true, use a deterministic RNG. The test then generates the
        /// same data each time it runs. Profile default: true on a CI
        /// server, false in other environments.
        std::optional<bool> derandomize;

        /// If true, continue to generate after the first failure to find
        /// more distinct bugs, and report all of them. If false, stop the run
        /// at the first failing example. Profile default: false.
        std::optional<bool> report_multiple_failures;

        /// If true, stop generation when a failure occurs, then shrink and
        /// report the failure. If false, run all test_cases before the
        /// report. In both cases, the database keeps each distinct failure.
        /// Profile default: true.
        std::optional<bool> stop_on_first_failure;

        /// What the run does when a replay of the same data gives a
        /// different outcome or different draws. Profile default:
        /// NondeterminismStrictness::Quiet.
        std::optional<NondeterminismStrictness> nondeterminism_strictness;

        /// If true, a failure report ends with `rerun with:` and the base64
        /// blob that replays the failure. Profile default: true.
        std::optional<bool> print_blob;

        /// Configure the Hegel database. See Database. Defaults to a database
        /// at `.hegel`, or disabled when a CI environment is detected.
        Database database =
            internal::in_ci() ? Database::disabled() : Database::unset();

        /// Key scoping the examples stored in and replayed from the database.
        /// Unset (the default) disables persistence and Reuse-phase replay.
        std::optional<std::string> database_key;

        /// Health checks to suppress for this test. Empty (the default) uses
        /// the engine profile's set: none locally, `TooSlow` on a CI server.
        std::vector<HealthCheck> suppress_health_check;

        /// Phases to run. With an empty list, the run does nothing. Profile
        /// default: all phases.
        Phases phases = Phases::unset();

        /// Randomness backend. Defaults to Backend::Auto; picking an explicit
        /// backend overrides the engine profile's choice.
        Backend backend = Backend::Auto;

        /// The maximum number of steps a stateful test case attempts. Each
        /// case runs at least one step and at most this many. Must be at
        /// least 1; hegel::test() rejects other values with
        /// std::invalid_argument.
        int64_t stateful_step_count = 50;
    };
} // namespace hegel
