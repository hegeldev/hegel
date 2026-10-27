#pragma once

#include <cstdint>
#include <optional>
#include <string>
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
     * @brief Health checks.
     */
    enum class HealthCheck {
        FilterTooMuch,        ///< Test filters out too many generated examples.
        TooSlow,              ///< Test cases are running too slowly.
        TestCasesTooLarge,    ///< Generated test cases are too large.
        LargeInitialTestCase, ///< First generated test case is unusually large.
    };

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
     * @brief All phases, the base value of Settings::phases.
     * @return A vector containing every Phase variant.
     */
    inline std::vector<Phase> all_phases() {
        return {Phase::Explicit, Phase::Reuse, Phase::Generate, Phase::Target,
                Phase::Shrink};
    }

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
            Default,  ///< Use the database at `.hegel/examples`.
            Disabled, ///< Disable the database.
            Path,     ///< Use the given directory as the database.
        };

        /// Use the database at `.hegel/examples`, also where the settings
        /// profile disables the database.
        /// @return A database with Database::Kind::Default.
        static Database default_location() { return {Kind::Default, {}}; }

        /// Disable the database.
        /// @return A database with Database::Kind::Disabled.
        static Database disabled() { return {Kind::Disabled, {}}; }

        /// Use the given directory as the database.
        /// @param path The directory path for the database.
        /// @return A database with Database::Kind::Path.
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
    namespace detail {
        /// The values of a settings profile, as the engine resolves them.
        struct ResolvedSettings {
            uint64_t test_cases = 100;
            Verbosity verbosity = Verbosity::Normal;
            std::optional<uint64_t> seed;
            bool derandomize = false;
            bool report_multiple_failures = false;
            bool stop_on_first_failure = true;
            NondeterminismStrictness nondeterminism_strictness =
                NondeterminismStrictness::Quiet;
            bool print_blob = true;
            bool show_statistics = false;
            bool unbounded_choices = false;
            Database database = Database::default_location();
            std::vector<HealthCheck> suppress_health_check;
            std::vector<Phase> phases;
            Backend backend = Backend::Default;
        };

        /// Resolve the named profile, or the `default` profile, now.
        ResolvedSettings
        resolve_settings(const std::optional<std::string>& profile);
    } // namespace detail
    /// @endcond

    /**
     * @brief Configuration options for hegel::test().
     *
     * Creating a Settings resolves an engine settings profile and copies
     * every value it resolves: the `default` profile, or the profile named
     * to Settings::from_profile(). The engine resolves the `default` profile
     * from hegel::set_default_profile(), the `HEGEL_DEFAULT_PROFILE`
     * environment variable, and a `hegel.toml` in the working directory or
     * an ancestor, else from the detected environment (`workload` inside
     * Antithesis, `ci` on a CI server, `development` otherwise). The
     * `HEGEL_*` settings environment variables then apply over the profile.
     *
     * Each field that you set replaces the resolved value. Each field that
     * you do not set keeps it:
     *
     * @code{.cpp}
     * hegel::Settings settings{.test_cases = 500, .seed = 42};
     * hegel::Settings nightly = hegel::Settings::from_profile("ci");
     * nightly.test_cases = 10000;
     * @endcode
     *
     * The values are fixed when the Settings is created: a profile that
     * hegel::register_profile() or hegel::set_default_profile() changes
     * afterwards does not change it. A malformed `hegel.toml` or `HEGEL_*`
     * variable throws std::invalid_argument from the creation.
     */
    struct Settings {
        /// @cond INTERNAL
        detail::ResolvedSettings resolved_ =
            detail::resolve_settings(std::nullopt);
        /// @endcond

        /// Settings resolved from the named profile instead of the `default`
        /// one. A name can be reserved (`base`, `default`), shipped
        /// (`development`, `ci`, `workload`), defined in `hegel.toml`, or
        /// registered with hegel::register_profile().
        /// @param name The profile name.
        /// @return The settings of that profile.
        /// @throws std::invalid_argument for an unknown name, or a malformed
        ///         `hegel.toml` or `HEGEL_*` variable.
        static Settings from_profile(const std::string& name);

        /// Number of test cases to run. The base value is 100.
        uint64_t test_cases = resolved_.test_cases;

        /// Verbosity level. The base value is Verbosity::Normal.
        Verbosity verbosity = resolved_.verbosity;

        /// Explicit seed for the test. The profiles have none by default:
        /// each run then picks a fresh seed.
        std::optional<uint64_t> seed = resolved_.seed;

        /// If true, derive the seed from the database key, so the test is
        /// deterministic across executions. The base value is false; the
        /// `ci` profile sets it to true.
        bool derandomize = resolved_.derandomize;

        /// If true, keep generating after the first failure to surface
        /// additional distinct bugs, and report all of them. If false, stop
        /// the run at the first failing example. The base value is false.
        bool report_multiple_failures = resolved_.report_multiple_failures;

        /// If true, stop generating once a failure is found and move on to
        /// shrinking and reporting it. If false, the run uses its whole
        /// test_cases budget before reporting, whatever it finds along the
        /// way; every distinct failure found is persisted to the database
        /// either way. The base value is true.
        bool stop_on_first_failure = resolved_.stop_on_first_failure;

        /// How the run reacts to a test whose outcome or draws change when
        /// the same generated data is replayed. The base value is
        /// NondeterminismStrictness::Quiet.
        NondeterminismStrictness nondeterminism_strictness =
            resolved_.nondeterminism_strictness;

        /// If true, the report of each failure ends with the line that
        /// replays it from its base64 reproduce blob, at every verbosity.
        /// The base value is true.
        bool print_blob = resolved_.print_blob;

        /// If true, the run prints statistics at its end: for each label
        /// recorded with an event, the fraction of test cases it occurred
        /// in. The base value is false.
        bool show_statistics = resolved_.show_statistics;

        /// If true, a test case may make any number of choices. By default
        /// the engine stops a test case after 2^20 choices. A long-running
        /// test case needs this. The base value is false.
        bool unbounded_choices = resolved_.unbounded_choices;

        /// Configure the Hegel database. See Database. The base value is
        /// Database::default_location(), `.hegel/examples`; the `ci` and
        /// `workload` profiles disable the database.
        Database database = resolved_.database;

        /// Key scoping the examples stored in and replayed from the
        /// database. A derandomized run also derives its seed from it.
        /// Unset, @ref HEGEL_TEST uses `"<file>::<name>"` of the test, and
        /// hegel::test() derives a key as it documents. A run with no key
        /// does not store or replay examples.
        std::optional<std::string> database_key;

        /// Health checks to suppress for this test. The profiles suppress
        /// none locally, `TooSlow` on a CI server and all of them inside
        /// Antithesis. An empty list (`{}`) suppresses none.
        std::vector<HealthCheck> suppress_health_check =
            resolved_.suppress_health_check;

        /// Phases to run, all of them by default. Phases left out of a list
        /// are disabled (e.g. drop Phase::Shrink to keep failing examples
        /// unshrunk). An empty list (`{}`) produces a run that does nothing.
        std::vector<Phase> phases = resolved_.phases;

        /// Randomness backend. The base value is Backend::Default; the
        /// `workload` profile selects Backend::Urandom.
        Backend backend = resolved_.backend;

        /// The maximum number of steps a stateful test case attempts. Each
        /// case runs at least one step and at most this many. Must be at
        /// least 1; hegel::test() rejects other values with
        /// std::invalid_argument. Hegel for C++ applies this setting
        /// itself, so it is not part of a settings profile.
        int64_t stateful_step_count = 50;
    };

    /**
     * @brief Register settings as a named settings profile.
     *
     * The profile holds a complete snapshot of @p settings.
     * Settings::database_key and Settings::stateful_step_count are not part
     * of the snapshot. Registration is process-wide and replaces an earlier
     * registration of the same name. It is not retroactive: a Settings
     * created before it keeps its values, so register a profile before
     * creating the Settings that use it. A `hegel.toml` section for @p name
     * still applies over the snapshot.
     *
     * @code{.cpp}
     * hegel::Settings nightly = hegel::Settings::from_profile("ci");
     * nightly.test_cases = 10000;
     * hegel::register_profile("nightly", nightly);
     * hegel::test(body, hegel::Settings::from_profile("nightly"));
     * @endcode
     *
     * @param name The profile name: ASCII letters, digits, `-` and `_`. The
     *             reserved names `base` and `default` are not permitted.
     * @param settings The settings to register.
     * @throws std::invalid_argument for an invalid or reserved name, or when
     *         @p settings name a profile that does not exist.
     */
    void register_profile(const std::string& name, const Settings& settings);

    /**
     * @brief Set the default settings profile for the whole process.
     *
     * The `default` profile then resolves to @p name. This takes precedence
     * over the `HEGEL_DEFAULT_PROFILE` environment variable, the `default`
     * entry in `hegel.toml`, and the detected environment. It is not
     * retroactive: a Settings created before it keeps its values. The
     * profile does not have to exist yet.
     *
     * @param name The profile name, or an empty string to remove an earlier
     *             call's selection.
     * @throws std::invalid_argument for an invalid name.
     */
    void set_default_profile(const std::string& name);

    /**
     * @brief The version of libhegel, the engine that Hegel for C++ drives.
     * @return The version, e.g. `"0.48.0"`.
     */
    std::string engine_version();
} // namespace hegel
