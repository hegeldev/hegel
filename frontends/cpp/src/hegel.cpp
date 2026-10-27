/*
 * hegel.cpp - The hegel::test() entry point.
 *
 * Drives Hegel's native engine (libhegel) in-process through its C ABI:
 * start a run, pull test cases, run the user body, mark each complete, then
 * inspect the aggregate result and replay any counterexamples.
 */

#include <hegel/errors.h>
#include <hegel/explicit.h>
#include <hegel/hegel.h>
#include <hegel/internal.h>
#include <hegel/settings.h>
#include <hegel/test_case.h>

#include <engine.h>
#include <test_case.h>

#include <hegel.h>

#include <array>
#include <charconv>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cxxabi.h>
#include <exception>
#include <functional>
#include <map>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <typeinfo>
#include <utility>
#include <vector>

#if defined(__APPLE__) || defined(__ELF__)
#include <dlfcn.h>
#endif

// HEGEL_HAS_THROW_SITE — whether a failure report names the source position
// of the throw that failed a test case. The wrapper below records that
// position, then forwards to the real __cxa_throw through
// dlsym(RTLD_NEXT, ...). The forwarding needs the C++ runtime to be a shared
// library; a statically linked runtime also defines __cxa_throw in an object
// the link always pulls in, and the two definitions collide. The CMake build
// sets the macro to 0 for HEGEL_THROW_SITE=OFF; a report then names the
// exception type without the site. When unset, the macro follows the
// availability of RTLD_NEXT.
#ifndef HEGEL_HAS_THROW_SITE
#if defined(RTLD_NEXT)
#define HEGEL_HAS_THROW_SITE 1
#else
#define HEGEL_HAS_THROW_SITE 0
#endif
#endif

#if HEGEL_HAS_THROW_SITE && !defined(RTLD_NEXT)
#error "HEGEL_HAS_THROW_SITE=1 requires dlsym(RTLD_NEXT, ...)"
#endif

#if HEGEL_HAS_THROW_SITE

namespace {
    // The return address of the innermost throw this thread made. One per
    // thread, since clone() and spawn() run test-case bodies on their own.
    thread_local const void* last_throw_address = nullptr;

    // GCC declares `__cxa_throw` for its own throw statements with `void*` in
    // place of the ABI's `std::type_info*`. The definition below must use the
    // same type, or the two declarations conflict.
#if defined(__clang__)
    using ThrowTypeInfo = std::type_info;
#else
    using ThrowTypeInfo = void;
#endif
} // namespace

// The stack unwinds before run_body() catches, so a backtrace taken there names
// run_body() as the origin for every failure. `__cxa_throw` runs while the
// throwing frame is still live, so Hegel defines its own and forwards to the
// real one.
// NOLINTNEXTLINE(bugprone-reserved-identifier) - the ABI hook name is fixed
extern "C" void __cxa_throw(void* exception, ThrowTypeInfo* type_info,
                            void (*destructor)(void*)) {
    // store for determining origin later
    last_throw_address = __builtin_return_address(0);

    using ThrowFn = void (*)(void*, ThrowTypeInfo*, void (*)(void*));
    // find the real __cxa_throw and cache
    static ThrowFn forward =
        reinterpret_cast<ThrowFn>(dlsym(RTLD_NEXT, "__cxa_throw"));
    // call the real __cxa_throw
    forward(exception, type_info, destructor);
    __builtin_unreachable(); // forward() never returns
}
#endif

namespace hegel {

    namespace impl {
        // Demangle a typeid name, owning the malloc'd result. The fallback
        // covers unparseable input and the demangler's allocation failure.
        std::string demangle(const char* name) {
            int status = 0;
            char* demangled =
                abi::__cxa_demangle(name, nullptr, nullptr, &status);
            if (demangled == nullptr) {
                return name; // GCOVR_EXCL_LINE
            }
            std::string out = demangled;
            std::free(demangled);
            return out;
        }
    } // namespace impl

    namespace {
        /*
        Needed because concurrent execution crosses threads. The main thread
        cannot access the worker’s thread-local origin otherwise.
        */
        thread_local std::optional<std::string> concurrent_exception_origin;

        // RAII guards for the libhegel handles. Each `*_free` is a no-op on
        // NULL and never throws. (The error-reporting context is not guarded
        // here: impl::thread_context() owns one per thread.)
        struct SettingsGuard {
            hegel_context_t* ctx;
            hegel_settings_t* s = nullptr;
            ~SettingsGuard() { hegel_settings_free(ctx, s); }
        };

        struct RunGuard {
            hegel_context_t* ctx;
            hegel_run_t* run = nullptr;
            ~RunGuard() { hegel_run_free(ctx, run); }
        };

        // Caller-owned snapshot of a finished run's result.
        struct ResultGuard {
            hegel_context_t* ctx;
            hegel_run_result_t* result = nullptr;
            ~ResultGuard() { hegel_run_result_free(ctx, result); }
        };

        // Caller-owned snapshot of one distinct failure.
        struct FailureGuard {
            hegel_context_t* ctx;
            hegel_failure_t* failure = nullptr;
            ~FailureGuard() { hegel_failure_free(ctx, failure); }
        };

        // How a report tells the user to replay a failure: through the
        // hegel::test() overload the run came from, or, for a test the
        // HEGEL_TEST macro defined, with HEGEL_REPRODUCE_FAILURE.
        enum class RerunForm { Call, Location, Annotation };

        // The installed test framework integration
        internal::FrameworkHooks& framework_hooks() {
            static internal::FrameworkHooks hooks;
            return hooks;
        }

        // Names the framework test that runs now, or "" outside one.
        std::string framework_test_name() {
            if (framework_hooks().current_test_name == nullptr) {
                return {};
            }
            return framework_hooks().current_test_name();
        }

        // Runs one test-case body, through the framework integration when
        // there is one so it can raise the failures it recorded.
        void run_case(const std::function<void()>& body) {
            if (framework_hooks().run_case == nullptr) {
                body();
                return;
            }
            framework_hooks().run_case(body);
        }

        // What a run reads from its resolved engine settings, plus the
        // settings Hegel for C++ applies itself.
        struct RunConfig {
            Verbosity verbosity;
            bool print_blob;
            int64_t stateful_step_count;
        };

        struct BodyOutcome {
            hegel_status_t status;
            std::string origin;
            std::exception_ptr exception;
        };

        // The outcome of a failing test case and, when the engine stamped
        // the case for capture, its output. The report of each failure is
        // built from these: every interesting case stores one so the report
        // always has an outcome to re-raise, and a capture with output is
        // never replaced by one without (the case that first reveals
        // nondeterminism is unstamped).
        struct CapturedFailure {
            BodyOutcome outcome;
            std::vector<std::string> lines;
        };
        using Captures = std::map<std::string, CapturedFailure>;

        void store_capture(Captures& captures, const BodyOutcome& outcome,
                           std::vector<std::string> lines) {
            auto existing = captures.find(outcome.origin);
            if (existing != captures.end() && !existing->second.lines.empty() &&
                lines.empty()) {
                return;
            }
            captures[outcome.origin] =
                CapturedFailure{outcome, std::move(lines)};
        }

        using impl::demangle;

#if HEGEL_HAS_THROW_SITE
        std::string to_hex(uintptr_t value) {
            std::array<char, 2 * sizeof(uintptr_t)> buf;
            auto [ptr, ec] =
                std::to_chars(buf.data(), buf.data() + buf.size(), value, 16);
            return std::string(buf.data(), ptr);
        }
#endif

        // Return the most recent throw in a thread as "<binary>+0x<offset
        // from its load address>".
        std::string last_throw_site() {
#if HEGEL_HAS_THROW_SITE
            // cache sites to prevent repeated lookups during shrinking
            static thread_local std::map<const void*, std::string> sites;
            auto known = sites.find(last_throw_address);
            if (known != sites.end()) {
                return known->second;
            }

            std::string site;
            Dl_info info;
            // translate the throw address to global offset in the binary that
            // contains it
            if (dladdr(last_throw_address, &info) != 0 &&
                info.dli_fname != nullptr) {
                auto offset = static_cast<uintptr_t>(
                    static_cast<const char*>(last_throw_address) -
                    static_cast<const char*>(info.dli_fbase));
                site = std::string(info.dli_fname) + "+0x" + to_hex(offset);
            }
            sites.emplace(last_throw_address, site);
            return site;
#else
            return {};
#endif
        }

        std::string origin_of(const FailureOrigin* named,
                              const std::string& type) {
            if (named != nullptr) {
                return named->failure_origin();
            }
            std::string site = last_throw_site();
            return site.empty() ? type : type + " at " + site;
        }

        // Run the user's test body once and classify the outcome into the
        // libhegel status the caller passes to hegel_mark_complete. The
        // origin is demangled here, before it reaches the engine, so the
        // engine's failure origins are readable as reported.
        BodyOutcome run_body(const std::function<void(TestCase&)>& test_fn,
                             TestCase& tc) {
            try {
                run_case([&] { test_fn(tc); });
                return {HEGEL_STATUS_VALID, "", nullptr};
            } catch (const internal::UsageError&) {
                throw;
            } catch (const internal::HegelStopTest&) {
                return {HEGEL_STATUS_OVERRUN, "", nullptr};
            } catch (const internal::HegelReject&) {
                return {HEGEL_STATUS_INVALID, "", nullptr};
            } catch (const std::exception& e) {
                std::string type = demangle(typeid(e).name());
                std::string origin;
                if (concurrent_exception_origin.has_value()) {
                    origin = std::move(*concurrent_exception_origin);
                    concurrent_exception_origin.reset();
                } else {
                    origin =
                        origin_of(dynamic_cast<const FailureOrigin*>(&e), type);
                }
                return {HEGEL_STATUS_INTERESTING, origin,
                        std::current_exception()};
            } catch (...) {
                // Only user code runs inside the try, so anything caught
                // here is a test failure — including a foreign (non-C++)
                // exception, for which the ABI can supply neither a
                // type_info nor an exception_ptr. Substitute a described
                // exception so the re-raise path stays valid.
                std::string type = "unknown_exception";
                if (const std::type_info* tinfo =
                        abi::__cxa_current_exception_type()) {
                    type = demangle(tinfo->name());
                }
                std::string origin;
                if (concurrent_exception_origin.has_value()) {
                    origin = std::move(*concurrent_exception_origin);
                    concurrent_exception_origin.reset();
                } else {
                    origin = origin_of(nullptr, type);
                }
                std::exception_ptr exception = std::current_exception();
                if (exception == nullptr) {
                    // GCOVR_EXCL_START
                    exception = std::make_exception_ptr(std::runtime_error(
                        "test body raised a foreign (non-C++) exception"));
                    // GCOVR_EXCL_STOP
                }
                return {HEGEL_STATUS_INTERESTING, origin, exception};
            }
        }

        void mark_complete(hegel_context_t* ctx, hegel_test_case_t* tc,
                           const BodyOutcome& outcome) {
            std::optional<std::string> origin;
            if (!outcome.origin.empty()) {
                origin = outcome.origin;
            }
            impl::mark_complete(ctx, tc, outcome.status, origin);
        }

        // Whether a test case records its notes and drawn values: when it
        // is the final replay or the engine stamped it for capture, unless
        // quiet, and every case when verbose.
        bool records_output(Verbosity verbosity, bool shown) {
            switch (verbosity) {
            case Verbosity::Quiet:
                return false;
            case Verbosity::Normal:
                return shown;
            case Verbosity::Verbose:
            case Verbosity::Debug:
            default:
                return true;
            }
        }

        void print_lines(const std::vector<std::string>& lines) {
            for (const std::string& line : lines) {
                std::fprintf(stderr, "%s\n", line.c_str());
            }
        }

        void print_reproducer(RerunForm rerun_form,
                              const std::optional<TestLocation>& location,
                              const std::string& blob) {
            if (rerun_form == RerunForm::Annotation && location.has_value()) {
                std::fprintf(stderr,
                             "\nTo reproduce this failure, add this line above "
                             "HEGEL_TEST(%s):\n"
                             "    HEGEL_REPRODUCE_FAILURE(%s, \"%s\")\n",
                             location->name.c_str(), location->name.c_str(),
                             blob.c_str());
                return;
            }
            const char* arguments = rerun_form == RerunForm::Location
                                        ? "test_fn, location, settings"
                                        : "test_fn, settings";
            std::fprintf(stderr,
                         "\nTo reproduce this failure, pass its blob to "
                         "hegel::test():\n"
                         "    hegel::test(%s, {\"%s\"})\n",
                         arguments, blob.c_str());
        }

        hegel_verbosity_t to_engine(Verbosity verbosity) {
            switch (verbosity) {
            case Verbosity::Quiet:
                return HEGEL_VERBOSITY_QUIET;
            case Verbosity::Verbose:
                return HEGEL_VERBOSITY_VERBOSE;
            case Verbosity::Debug:
                return HEGEL_VERBOSITY_DEBUG;
            case Verbosity::Normal:
            default:
                return HEGEL_VERBOSITY_NORMAL;
            }
        }

        Verbosity from_engine(hegel_verbosity_t verbosity) {
            switch (verbosity) {
            case HEGEL_VERBOSITY_QUIET:
                return Verbosity::Quiet;
            case HEGEL_VERBOSITY_VERBOSE:
                return Verbosity::Verbose;
            case HEGEL_VERBOSITY_DEBUG:
                return Verbosity::Debug;
            case HEGEL_VERBOSITY_NORMAL:
            default:
                return Verbosity::Normal;
            }
        }

        hegel_nondeterminism_strictness_t
        to_engine(NondeterminismStrictness strictness) {
            switch (strictness) {
            case NondeterminismStrictness::Warn:
                return HEGEL_NONDETERMINISM_WARN;
            case NondeterminismStrictness::Error:
                return HEGEL_NONDETERMINISM_ERROR;
            case NondeterminismStrictness::Quiet:
            default:
                return HEGEL_NONDETERMINISM_QUIET;
            }
        }

        NondeterminismStrictness
        from_engine(hegel_nondeterminism_strictness_t strictness) {
            switch (strictness) {
            case HEGEL_NONDETERMINISM_WARN:
                return NondeterminismStrictness::Warn;
            case HEGEL_NONDETERMINISM_ERROR:
                return NondeterminismStrictness::Error;
            case HEGEL_NONDETERMINISM_QUIET:
            default:
                return NondeterminismStrictness::Quiet;
            }
        }

        Backend from_engine(hegel_backend_t backend) {
            return backend == HEGEL_BACKEND_URANDOM ? Backend::Urandom
                                                    : Backend::Default;
        }

        Database from_engine(const std::optional<std::string>& database) {
            if (!database.has_value()) {
                return Database::default_location();
            }
            if (database->empty()) {
                return Database::disabled();
            }
            return Database::from_path(*database);
        }

        uint32_t to_engine(const std::vector<HealthCheck>& checks) {
            uint32_t mask = 0;
            for (HealthCheck c : checks) {
                switch (c) {
                case HealthCheck::FilterTooMuch:
                    mask |= HEGEL_HC_FILTER_TOO_MUCH;
                    break;
                case HealthCheck::TooSlow:
                    mask |= HEGEL_HC_TOO_SLOW;
                    break;
                case HealthCheck::TestCasesTooLarge:
                    mask |= HEGEL_HC_TEST_CASES_TOO_LARGE;
                    break;
                case HealthCheck::LargeInitialTestCase:
                    mask |= HEGEL_HC_LARGE_INITIAL_TEST_CASE;
                    break;
                }
            }
            return mask;
        }

        std::vector<HealthCheck> health_checks_from_engine(uint32_t mask) {
            std::vector<HealthCheck> checks;
            if ((mask & HEGEL_HC_FILTER_TOO_MUCH) != 0) {
                checks.push_back(HealthCheck::FilterTooMuch);
            }
            if ((mask & HEGEL_HC_TOO_SLOW) != 0) {
                checks.push_back(HealthCheck::TooSlow);
            }
            if ((mask & HEGEL_HC_TEST_CASES_TOO_LARGE) != 0) {
                checks.push_back(HealthCheck::TestCasesTooLarge);
            }
            if ((mask & HEGEL_HC_LARGE_INITIAL_TEST_CASE) != 0) {
                checks.push_back(HealthCheck::LargeInitialTestCase);
            }
            return checks;
        }

        uint32_t to_engine(const std::vector<Phase>& phases) {
            uint32_t mask = 0;
            for (Phase p : phases) {
                switch (p) {
                case Phase::Explicit:
                    mask |= HEGEL_PHASE_EXPLICIT;
                    break;
                case Phase::Reuse:
                    mask |= HEGEL_PHASE_REUSE;
                    break;
                case Phase::Generate:
                    mask |= HEGEL_PHASE_GENERATE;
                    break;
                case Phase::Target:
                    mask |= HEGEL_PHASE_TARGET;
                    break;
                case Phase::Shrink:
                    mask |= HEGEL_PHASE_SHRINK;
                    break;
                }
            }
            return mask;
        }

        std::vector<Phase> phases_from_engine(uint32_t mask) {
            std::vector<Phase> phases;
            if ((mask & HEGEL_PHASE_EXPLICIT) != 0) {
                phases.push_back(Phase::Explicit);
            }
            if ((mask & HEGEL_PHASE_REUSE) != 0) {
                phases.push_back(Phase::Reuse);
            }
            if ((mask & HEGEL_PHASE_GENERATE) != 0) {
                phases.push_back(Phase::Generate);
            }
            if ((mask & HEGEL_PHASE_TARGET) != 0) {
                phases.push_back(Phase::Target);
            }
            if ((mask & HEGEL_PHASE_SHRINK) != 0) {
                phases.push_back(Phase::Shrink);
            }
            return phases;
        }

        // Set every field of `settings`, resolved when it was created, onto
        // a fresh handle.
        void apply_settings(hegel_context_t* ctx, hegel_settings_t* s,
                            const Settings& settings) {
            impl::settings_set_test_cases(ctx, s, settings.test_cases);
            impl::settings_set_verbosity(ctx, s, to_engine(settings.verbosity));
            impl::settings_set_seed(ctx, s, settings.seed.value_or(0),
                                    settings.seed.has_value());
            impl::settings_set_derandomize(ctx, s, settings.derandomize);
            impl::settings_set_report_multiple_failures(
                ctx, s, settings.report_multiple_failures);
            impl::settings_set_stop_on_first_failure(
                ctx, s, settings.stop_on_first_failure);
            impl::settings_set_nondeterminism_strictness(
                ctx, s, to_engine(settings.nondeterminism_strictness));
            impl::settings_set_print_blob(ctx, s, settings.print_blob);
            impl::settings_set_show_statistics(ctx, s,
                                               settings.show_statistics);
            impl::settings_set_unbounded_choices(ctx, s,
                                                 settings.unbounded_choices);

            switch (settings.database.kind()) {
            case Database::Kind::Default:
                impl::settings_set_default_database(ctx, s);
                break;
            case Database::Kind::Disabled:
                impl::settings_set_database(ctx, s, "");
                break;
            case Database::Kind::Path:
                impl::settings_set_database(ctx, s, settings.database.path());
                break;
            }

            impl::settings_set_suppress_health_check(
                ctx, s, to_engine(settings.suppress_health_check));
            impl::settings_set_phases(ctx, s, to_engine(settings.phases));
            impl::settings_set_backend(ctx, s,
                                       settings.backend == Backend::Urandom
                                           ? HEGEL_BACKEND_URANDOM
                                           : HEGEL_BACKEND_DEFAULT);
        }

        // A handle holding every field of `settings`.
        void resolve_settings(hegel_context_t* ctx, SettingsGuard& guard,
                              const Settings& settings) {
            if (settings.stateful_step_count < 1) {
                throw std::invalid_argument(
                    "Settings::stateful_step_count must be at least 1");
            }
            guard.s = impl::settings_new(ctx);
            apply_settings(ctx, guard.s, settings);
        }

        // Record where the test lives, so the engine can report the run's
        // verdict to Antithesis under `<class>::<function>`. A GoogleTest
        // name (`Suite.Name`) splits into the two; any other name is scoped
        // by its file, like the default database key.
        void apply_test_location(hegel_context_t* ctx, hegel_settings_t* s,
                                 const TestLocation& location) {
            std::string class_name = location.file;
            std::string function = location.name;
            size_t dot = location.name.find('.');
            if (dot != std::string::npos) {
                class_name = location.name.substr(0, dot);
                function = location.name.substr(dot + 1);
            }
            uint32_t line =
                location.line < 0 ? 0 : static_cast<uint32_t>(location.line);
            impl::settings_set_test_location(ctx, s, location.file, line,
                                             class_name, function);
        }

        // Pulls every test case off `run`, then acts on its verdict: returns
        // nothing for a pass, throws the RunError subclass for a run error,
        // and for a failure prints the report of each distinct failure from
        // what the run captured for its origin, and returns their outcomes.
        std::vector<BodyOutcome> drive_run(
            const std::function<void(TestCase&)>& test_fn, hegel_context_t* ctx,
            hegel_run_t* run, const RunConfig& config,
            const std::optional<TestLocation>& location, RerunForm rerun_form) {
            bool verbose = config.verbosity == Verbosity::Verbose ||
                           config.verbosity == Verbosity::Debug;
            Captures captures;

            while (true) {
                hegel_test_case_t* handle = impl::next_test_case(ctx, run);
                if (handle == nullptr) {
                    break;
                }
                bool stamped = impl::test_case_should_capture(ctx, handle);
                TestCase tc_obj(std::make_unique<impl::test_case::TestCaseData>(
                    handle, records_output(config.verbosity, stamped),
                    config.stateful_step_count));
                BodyOutcome outcome = run_body(test_fn, tc_obj);
                std::vector<std::string> lines;
                if (tc_obj.data()->emit) {
                    lines = impl::test_case::rendered_output(*tc_obj.data());
                }
                if (verbose) {
                    print_lines(lines);
                }
                if (outcome.status == HEGEL_STATUS_INTERESTING) {
                    if (!stamped) {
                        lines.clear();
                    }
                    store_capture(captures, outcome, std::move(lines));
                }
                mark_complete(ctx, tc_obj.data()->tc, outcome);
            }

            ResultGuard result_guard{ctx};
            result_guard.result = impl::run_result(ctx, run);
            hegel_run_result_t* result = result_guard.result;
            hegel_run_status_t run_status =
                impl::run_result_status(ctx, result);

            if (run_status == HEGEL_RUN_STATUS_PASSED) {
                return {};
            }

            if (run_status == HEGEL_RUN_STATUS_ERROR) {
                impl::raise_run_error(impl::run_result_error(ctx, result)
                                          .value_or("unknown error"),
                                      impl::run_result_error_kind(ctx, result));
            }

            bool quiet = config.verbosity == Verbosity::Quiet;
            size_t failure_count = impl::run_result_failure_count(ctx, result);
            bool multiple = failure_count > 1;
            if (multiple && !quiet) {
                std::fprintf(
                    stderr,
                    "Property-based test failed with %zu distinct failures.\n",
                    failure_count);
            }

            std::vector<BodyOutcome> outcomes;
            for (size_t i = 0; i < failure_count; i++) {
                FailureGuard failure_guard{ctx};
                failure_guard.failure =
                    impl::run_result_failure(ctx, result, i);
                const hegel_failure_t* failure = failure_guard.failure;
                const CapturedFailure& captured =
                    captures.at(impl::failure_origin(ctx, failure));
                if (multiple && !quiet) {
                    std::fprintf(stderr, "\n");
                }
                print_lines(captured.lines);
                std::optional<std::string> caveat =
                    impl::failure_caveat(ctx, failure);
                if (caveat.has_value() && !quiet) {
                    std::fprintf(stderr, "note: %s\n", caveat->c_str());
                }
                std::optional<std::string> blob =
                    impl::failure_reproduction_blob(ctx, failure);
                if (config.print_blob && blob.has_value()) {
                    print_reproducer(rerun_form, location, *blob);
                }
                outcomes.push_back(captured.outcome);
            }
            return outcomes;
        }

        // Re-raises the test's own exception for a single failure, and a
        // MultipleFailures holding each one's exception for several.
        [[noreturn]] void raise_failures(std::vector<BodyOutcome> outcomes) {
            if (outcomes.size() == 1) {
                std::rethrow_exception(outcomes.front().exception);
            }
            std::vector<std::exception_ptr> exceptions;
            exceptions.reserve(outcomes.size());
            for (BodyOutcome& outcome : outcomes) {
                exceptions.push_back(std::move(outcome.exception));
            }
            throw MultipleFailures(std::move(exceptions));
        }

        // How an explicit test case reads in a report: its values, in the
        // order it was given them.
        std::string describe(const ExplicitTestCase& test_case) {
            std::string out = "(";
            const char* separator = "";
            for (const ExplicitTestCase::Value& value : test_case.values()) {
                out += separator;
                if (value.name) {
                    out += *value.name + " = ";
                }
                out += value.render(value.value);
                separator = ", ";
            }
            return out + ")";
        }

        // Throws a usage error when the draws of an explicit test case did
        // not take all of its values.
        void check_fully_used(const impl::test_case::ExplicitState& state) {
            std::string unused;
            const std::vector<ExplicitTestCase::Value>& values =
                state.test_case.values();
            for (size_t i = 0; i < values.size(); ++i) {
                if (state.used[i]) {
                    continue;
                }
                unused += unused.empty() ? "" : ", ";
                unused += values[i].name.value_or("a positional value");
            }
            if (!unused.empty()) {
                throw internal::UsageError(
                    "The explicit test case " + describe(state.test_case) +
                    " gave values that no draw took: " + unused);
            }
        }

        // Runs each explicit test case once, in order. The first that fails
        // prints what it drew and noted, and its exception propagates. A
        // misuse of the test case (a rejection included) is a usage error,
        // which propagates as it is.
        void run_explicit_cases(
            const std::function<void(TestCase&)>& test_fn,
            const RunConfig& config,
            const std::vector<ExplicitTestCase>& explicit_cases) {
            bool verbose = config.verbosity == Verbosity::Verbose ||
                           config.verbosity == Verbosity::Debug;
            bool quiet = config.verbosity == Verbosity::Quiet;
            for (const ExplicitTestCase& test_case : explicit_cases) {
                auto data = std::make_unique<impl::test_case::TestCaseData>(
                    nullptr, false, config.stateful_step_count);
                auto state =
                    std::make_shared<impl::test_case::ExplicitState>(test_case);
                data->explicit_state = state;
                TestCase tc(std::move(data));
                if (verbose) {
                    std::fprintf(stderr, "Running explicit test case %s\n",
                                 describe(test_case).c_str());
                }
                BodyOutcome outcome = run_body(test_fn, tc);
                if (verbose) {
                    print_lines(state->lines);
                }
                if (outcome.status == HEGEL_STATUS_INVALID) {
                    throw internal::UsageError(
                        "Explicit test cases must not be rejected, but the "
                        "explicit test case " +
                        describe(test_case) +
                        " was rejected by TestCase::assume() or "
                        "TestCase::reject().");
                }
                if (outcome.status == HEGEL_STATUS_INTERESTING) {
                    if (!quiet && !verbose) {
                        print_lines(state->lines);
                    }
                    std::rethrow_exception(outcome.exception);
                }
                check_fully_used(*state);
            }
        }

        // Replays each blob in a run of its own, then re-raises what they
        // reproduced.
        void run_from_blobs(const std::function<void(TestCase&)>& test_fn,
                            hegel_context_t* ctx, hegel_settings_t* s,
                            const RunConfig& config,
                            const std::optional<TestLocation>& location,
                            RerunForm rerun_form,
                            const std::vector<std::string>& failure_blobs) {
            std::vector<BodyOutcome> outcomes;
            for (const std::string& blob : failure_blobs) {
                RunGuard run_guard{ctx};
                run_guard.run = impl::run_start_blob(ctx, s, blob);
                for (BodyOutcome& outcome :
                     drive_run(test_fn, ctx, run_guard.run, config, location,
                               rerun_form)) {
                    outcomes.push_back(std::move(outcome));
                }
            }
            if (outcomes.empty()) {
                throw DidNotReproduce(
                    "The failure blob did not reproduce a failure. The "
                    "failure may be fixed, or the test may have changed so "
                    "that the blob means something else.");
            }
            raise_failures(std::move(outcomes));
        }

        void run_from_engine(const std::function<void(TestCase&)>& test_fn,
                             hegel_context_t* ctx, hegel_settings_t* s,
                             const RunConfig& config,
                             const std::optional<TestLocation>& location,
                             RerunForm rerun_form) {
            RunGuard run_guard{ctx};
            run_guard.run = impl::run_start(ctx, s);
            std::vector<BodyOutcome> outcomes = drive_run(
                test_fn, ctx, run_guard.run, config, location, rerun_form);
            if (!outcomes.empty()) {
                raise_failures(std::move(outcomes));
            }
        }
    } // namespace

    namespace internal {
        namespace {
            // Function-local static so registrations from other translation
            // units' static initializers always find a constructed registry.
            std::map<std::string, std::vector<std::string>>& blob_registry() {
                static std::map<std::string, std::vector<std::string>> registry;
                return registry;
            }

            std::map<std::string, std::vector<ExplicitTestCase>>&
            explicit_registry() {
                static std::map<std::string, std::vector<ExplicitTestCase>>
                    registry;
                return registry;
            }
        } // namespace

        bool install_framework_hooks(const FrameworkHooks& hooks) {
            framework_hooks() = hooks;
            return true;
        }

        CapturedException capture_current_exception() {
            std::exception_ptr exception = std::current_exception();
            try {
                throw;
            } catch (const std::exception& error) {
                std::string type = demangle(typeid(error).name());
                return {exception,
                        origin_of(dynamic_cast<const FailureOrigin*>(&error),
                                  type)};
            } catch (...) {
                std::string type = "unknown_exception";
                if (const std::type_info* info =
                        abi::__cxa_current_exception_type()) {
                    type = demangle(info->name());
                }
                return {exception, origin_of(nullptr, type)};
            }
        }

        [[noreturn]] void CapturedException::rethrow() const {
            concurrent_exception_origin = origin;
            std::rethrow_exception(exception);
        }

        bool register_blob(const char* name, std::vector<const char*> blobs) {
            blob_registry().insert(
                {name, std::vector<std::string>(blobs.begin(), blobs.end())});
            return true;
        }

        std::vector<std::string> reproduce_blobs_for(const char* name) {
            auto blob = blob_registry().find(name);
            if (blob == blob_registry().end()) {
                return {};
            }
            return blob->second;
        }

        bool register_explicit_case(const char* name,
                                    ExplicitTestCase test_case) {
            explicit_registry()[name].push_back(std::move(test_case));
            return true;
        }

        std::vector<ExplicitTestCase> explicit_cases_for(const char* name) {
            auto cases = explicit_registry().find(name);
            if (cases == explicit_registry().end()) {
                return {};
            }
            return cases->second;
        }
    } // namespace internal

    namespace {
        // Runs the test under `settings`, scoping the example database to
        // `default_key` unless the settings name a key.
        void run_test(const std::function<void(TestCase&)>& test_fn,
                      const std::optional<TestLocation>& location,
                      RerunForm rerun_form, const Settings& settings,
                      const std::string& default_key,
                      const std::vector<std::string>& failure_blobs,
                      const std::vector<ExplicitTestCase>& explicit_cases) {
            hegel_context_t* ctx = impl::thread_context();

            SettingsGuard settings_guard{ctx};
            resolve_settings(ctx, settings_guard, settings);
            hegel_settings_t* s = settings_guard.s;
            std::string key = settings.database_key.value_or(default_key);
            if (!key.empty()) {
                impl::settings_set_database_key(ctx, s, key);
            }
            if (location.has_value()) {
                apply_test_location(ctx, s, *location);
            }
            RunConfig config{from_engine(impl::settings_get_verbosity(ctx, s)),
                             impl::settings_get_print_blob(ctx, s),
                             settings.stateful_step_count};

            if (failure_blobs.empty()) {
                if ((impl::settings_get_phases(ctx, s) &
                     HEGEL_PHASE_EXPLICIT) != 0) {
                    run_explicit_cases(test_fn, config, explicit_cases);
                }
                run_from_engine(test_fn, ctx, s, config, location, rerun_form);
            } else {
                run_from_blobs(test_fn, ctx, s, config, location, rerun_form,
                               failure_blobs);
            }
        }

        // The database key of a test: its file and its name.
        std::string database_key_for(const std::string& file,
                                     const std::string& name) {
            if (name.empty()) {
                return {};
            }
            return file + "::" + name;
        }
    } // namespace

    void test(const std::function<void(TestCase&)>& test_fn,
              const Settings& settings,
              const std::vector<std::string>& failure_blobs,
              const std::vector<ExplicitTestCase>& explicit_cases,
              const char* caller_file, const char* caller_function,
              int caller_line) {
        std::string file = caller_file == nullptr ? "" : caller_file;
        std::string framework_name = framework_test_name();
        std::optional<TestLocation> location;
        if (!framework_name.empty()) {
            location = TestLocation{framework_name, file, caller_line};
        }
        std::string key_name = framework_name;
        if (key_name.empty() && caller_function != nullptr) {
            key_name = caller_function;
        }
        run_test(test_fn, location, RerunForm::Call, settings,
                 database_key_for(file, key_name), failure_blobs,
                 explicit_cases);
    }

    void test(const std::function<void(TestCase&)>& test_fn,
              const TestLocation& location, const Settings& settings,
              const std::vector<std::string>& failure_blobs,
              const std::vector<ExplicitTestCase>& explicit_cases) {
        run_test(test_fn, location, RerunForm::Location, settings,
                 database_key_for(location.file, location.name), failure_blobs,
                 explicit_cases);
    }

    void register_profile(const std::string& name, const Settings& settings) {
        hegel_context_t* ctx = impl::thread_context();
        SettingsGuard settings_guard{ctx};
        resolve_settings(ctx, settings_guard, settings);
        impl::settings_register_profile(ctx, name, settings_guard.s);
    }

    void set_default_profile(const std::string& name) {
        impl::set_default_profile(impl::thread_context(), name);
    }

    std::string engine_version() {
        return impl::engine_version(impl::thread_context());
    }

    Settings Settings::from_profile(const std::string& name) {
        return Settings{detail::resolve_settings(name)};
    }

    namespace detail {
        ResolvedSettings
        resolve_settings(const std::optional<std::string>& profile) {
            hegel_context_t* ctx = impl::thread_context();
            SettingsGuard guard{ctx};
            guard.s = profile.has_value()
                          ? impl::settings_new_for_profile(ctx, *profile)
                          : impl::settings_new(ctx);
            const hegel_settings_t* s = guard.s;
            return ResolvedSettings{
                impl::settings_get_test_cases(ctx, s),
                from_engine(impl::settings_get_verbosity(ctx, s)),
                impl::settings_get_seed(ctx, s),
                impl::settings_get_derandomize(ctx, s),
                impl::settings_get_report_multiple_failures(ctx, s),
                impl::settings_get_stop_on_first_failure(ctx, s),
                from_engine(
                    impl::settings_get_nondeterminism_strictness(ctx, s)),
                impl::settings_get_print_blob(ctx, s),
                impl::settings_get_show_statistics(ctx, s),
                impl::settings_get_unbounded_choices(ctx, s),
                from_engine(impl::settings_get_database(ctx, s)),
                health_checks_from_engine(
                    impl::settings_get_suppress_health_check(ctx, s)),
                phases_from_engine(impl::settings_get_phases(ctx, s)),
                from_engine(impl::settings_get_backend(ctx, s)),
            };
        }
    } // namespace detail

    namespace internal {
        void
        test_from_macro(const std::function<void(TestCase&)>& test_fn,
                        const TestLocation& location, const Settings& settings,
                        const std::vector<std::string>& failure_blobs,
                        const std::vector<ExplicitTestCase>& explicit_cases) {
            run_test(test_fn, location, RerunForm::Annotation, settings,
                     database_key_for(location.file, location.name),
                     failure_blobs, explicit_cases);
        }
    } // namespace internal
} // namespace hegel
