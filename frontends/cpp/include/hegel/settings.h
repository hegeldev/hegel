#pragma once

#include <cstdint>
#include <initializer_list>
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
            Unset,    ///< Use the settings profile's database:
                      ///< `.hegel/examples` by default, disabled by the
                      ///< `ci` and `workload` profiles.
            Default,  ///< Use the database at `.hegel/examples`, whatever
                      ///< the profile says.
            Disabled, ///< Disable the database.
            Path,     ///< Use the given directory as the database.
        };

        /// Use the settings profile's database. This is the default.
        /// @return A database with Database::Kind::Unset.
        static Database unset() { return {Kind::Unset, {}}; }

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

    /**
     * @brief A list setting that is either unset or holds a list of values.
     *
     * An unset list keeps the value of the settings profile. A list given
     * with braces, also an empty one, replaces it:
     *
     * @code{.cpp}
     * hegel::Settings settings{
     *     .suppress_health_check = {hegel::HealthCheck::TooSlow},
     *     .phases = {hegel::Phase::Generate, hegel::Phase::Shrink},
     * };
     * hegel::Settings none_suppressed{.suppress_health_check = {}};
     * @endcode
     *
     * @tparam T The type of the values.
     */
    template <typename T> class ListSetting {
      public:
        /// A list with the given values.
        /// @param values The values.
        ListSetting(std::initializer_list<T> values) : values_(values) {}

        /// A list with the given values.
        /// @param values The values.
        ListSetting(std::vector<T> values) : values_(std::move(values)) {}

        /// The unset list, which keeps the value of the settings profile.
        /// @return An unset ListSetting.
        static ListSetting unset() { return ListSetting(Unset{}); }

        /// Whether the list is set.
        /// @return True if the list holds values, also an empty list.
        bool is_set() const { return values_.has_value(); }

        /// The values of a set list.
        /// @return The values. Empty for an unset list.
        const std::vector<T>& values() const {
            static const std::vector<T> empty;
            return values_.has_value() ? *values_ : empty;
        }

      private:
        struct Unset {};
        explicit ListSetting(Unset /*unused*/) {}
        std::optional<std::vector<T>> values_;
    };

    /**
     * @brief Configuration options for hegel::test().
     *
     * The run starts from the settings of an engine settings profile:
     * Settings::profile when it is set, else the `default` profile. The
     * engine resolves the `default` profile from hegel::set_default_profile(),
     * the `HEGEL_DEFAULT_PROFILE` environment variable, and a `hegel.toml`
     * in the working directory or an ancestor, else from the detected
     * environment (`workload` inside Antithesis, `ci` on a CI server,
     * `development` otherwise). The `HEGEL_*` settings environment variables
     * then apply over the profile.
     *
     * Each field that you set replaces the value of the profile. Each field
     * that you do not set keeps it.
     */
    struct Settings {
        /// Number of test cases to run. The base value is 100.
        std::optional<uint64_t> test_cases;

        /// Verbosity level. The base value is Verbosity::Normal.
        std::optional<Verbosity> verbosity;

        /// Explicit seed for the test. Unset uses the seed of the profile,
        /// which has none by default: each run then picks a fresh seed.
        std::optional<uint64_t> seed;

        /// If true, derive the seed from the database key, so the test is
        /// deterministic across executions. The base value is false; the
        /// `ci` profile sets it to true.
        std::optional<bool> derandomize;

        /// If true, keep generating after the first failure to surface
        /// additional distinct bugs, and report all of them. If false, stop
        /// the run at the first failing example. The base value is false.
        std::optional<bool> report_multiple_failures;

        /// If true, stop generating once a failure is found and move on to
        /// shrinking and reporting it. If false, the run uses its whole
        /// test_cases budget before reporting, whatever it finds along the
        /// way; every distinct failure found is persisted to the database
        /// either way. The base value is true.
        std::optional<bool> stop_on_first_failure;

        /// How the run reacts to a test whose outcome or draws change when
        /// the same generated data is replayed. The base value is
        /// NondeterminismStrictness::Quiet.
        std::optional<NondeterminismStrictness> nondeterminism_strictness;

        /// If true, the report of each failure ends with the line that
        /// replays it from its base64 reproduce blob, at every verbosity.
        /// The base value is true.
        std::optional<bool> print_blob;

        /// If true, the run prints statistics at its end: for each label
        /// recorded with an event, the fraction of test cases it occurred
        /// in. The base value is false.
        std::optional<bool> show_statistics;

        /// If true, a test case may make any number of choices. By default
        /// the engine stops a test case after 2^20 choices. A long-running
        /// test case needs this. The base value is false.
        std::optional<bool> unbounded_choices;

        /// Configure the Hegel database. See Database. The default,
        /// Database::unset(), keeps the database of the profile:
        /// `.hegel/examples`, or disabled under the `ci` and `workload`
        /// profiles.
        Database database = Database::unset();

        /// Key scoping the examples stored in and replayed from the
        /// database. A derandomized run also derives its seed from it.
        /// Unset, @ref HEGEL_TEST uses `"<file>::<name>"` of the test, and
        /// hegel::test() derives a key as it documents. A run with no key
        /// does not store or replay examples.
        std::optional<std::string> database_key;

        /// Health checks to suppress for this test. Unset keeps the set of
        /// the profile: none locally, `TooSlow` on a CI server, all of them
        /// inside Antithesis. An empty list (`{}`) suppresses none.
        ListSetting<HealthCheck> suppress_health_check =
            ListSetting<HealthCheck>::unset();

        /// Phases to run. Unset keeps the phases of the profile, which are
        /// all phases by default. Phases left out of a list are disabled
        /// (e.g. drop Phase::Shrink to keep failing examples unshrunk). An
        /// empty list (`{}`) produces a run that does nothing.
        ListSetting<Phase> phases = ListSetting<Phase>::unset();

        /// Randomness backend. Backend::Auto (the default) keeps the backend
        /// of the profile; picking an explicit backend overrides it.
        Backend backend = Backend::Auto;

        /// The settings profile to start from. Unset uses the `default`
        /// profile. A name can be reserved (`base`, `default`), shipped
        /// (`development`, `ci`, `workload`), defined in `hegel.toml`, or
        /// registered with hegel::register_profile(). An unknown name makes
        /// hegel::test() throw std::invalid_argument.
        std::optional<std::string> profile;
    };

    /**
     * @brief Register settings as a named settings profile.
     *
     * The profile holds a complete snapshot of @p settings, resolved as
     * hegel::test() resolves them: the profile they start from, with the
     * fields they set applied over it. Settings::database_key is not part of
     * the snapshot.
     * Registration is process-wide and replaces an earlier registration of
     * the same name. It is not retroactive, so register a profile before
     * the tests that use it run. A `hegel.toml` section for @p name still
     * applies over the snapshot.
     *
     * @code{.cpp}
     * hegel::register_profile("nightly",
     *                         {.test_cases = 10000, .profile = "ci"});
     * hegel::test(body, {.profile = "nightly"});
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
     * retroactive. The profile does not have to exist yet.
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
