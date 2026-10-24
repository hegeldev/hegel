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

        // How a report prints its "rerun with:" line. The annotation form
        // names a test defined by the HEGEL_TEST macro.
        enum class RerunForm { Call, Annotation };

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
            // Exception type, as the report's "Exception:" line names it.
            std::string type_name;
            std::string message;
            std::exception_ptr exception;
        };

        // The outcome of a failing test case and, when the engine stamped
        // the case for capture, its buffered output. A failure whose
        // reproduce blob cannot reproduce it is reported from this; every
        // interesting case stores one so the report always has an outcome
        // to re-raise, and a capture with output is never replaced by one
        // without (the case that first reveals nondeterminism is unstamped).
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
                return {HEGEL_STATUS_VALID, "", "", ""};
            } catch (const internal::HegelStopTest&) {
                return {HEGEL_STATUS_OVERRUN, "", "", ""};
            } catch (const internal::HegelReject&) {
                return {HEGEL_STATUS_INVALID, "", "", ""};
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
                return {HEGEL_STATUS_INTERESTING, origin, type, e.what(),
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
                return {HEGEL_STATUS_INTERESTING, origin, type, "", exception};
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

        // Prints the output a test case recorded as the body of a failure
        // section: a blank line, then each line indented.
        void print_report_body(const std::vector<std::string>& lines) {
            if (lines.empty()) {
                return;
            }
            std::fprintf(stderr, "\n");
            for (const std::string& line : lines) {
                std::fprintf(stderr, "  %s\n", line.c_str());
            }
        }

        // Replays one counterexample and prints what it recorded as the body
        // of a failure section.
        BodyOutcome replay_failure(hegel_context_t* ctx, hegel_settings_t* s,
                                   const std::string& blob,
                                   const RunConfig& config,
                                   const std::function<void(TestCase&)>& fn,
                                   bool* printed_output) {
            hegel_test_case_t* handle = impl::test_case_from_blob(ctx, s, blob);
            // GCOVR_EXCL_START
            if (handle == nullptr) {
                // should be handled by check_rc, possible bug in Hegel if hit
                throw std::runtime_error(
                    "Unreachable: failure blob produced a null test case.");
            }
            // GCOVR_EXCL_STOP
            TestCase tc_obj(std::make_unique<impl::test_case::TestCaseData>(
                handle, records_output(config.verbosity, /*shown=*/true),
                config.stateful_step_count));
            BodyOutcome outcome = run_body(fn, tc_obj);
            if (tc_obj.data()->emit) {
                std::vector<std::string> lines =
                    impl::test_case::rendered_output(*tc_obj.data());
                *printed_output = !lines.empty();
                if (outcome.status == HEGEL_STATUS_INTERESTING) {
                    print_report_body(lines);
                }
            }
            mark_complete(ctx, tc_obj.data()->tc, outcome);
            return outcome;
        }

        // Width the framed report's header rule is padded to.
        constexpr size_t frame_width = 72;

        // The rule that opens a framed report, naming the test.
        void print_failure_title(const std::optional<TestLocation>& location) {
            std::string title = "Failure";
            if (location.has_value()) {
                title += ": " + location->name + " (" + location->file + ":" +
                         std::to_string(location->line) + ")";
            }
            std::string prefix = "--- " + title + " ";
            size_t dashes = prefix.size() + 3 >= frame_width
                                ? 3
                                : frame_width - prefix.size();
            std::fprintf(stderr, "%s%s\n", prefix.c_str(),
                         std::string(dashes, '-').c_str());
        }

        void print_failure_header(const std::optional<TestLocation>& location,
                                  uint64_t cases_run,
                                  uint64_t cases_discarded) {
            print_failure_title(location);
            std::fprintf(stderr,
                         "Falsified after %llu test case%s (%llu discarded):\n",
                         static_cast<unsigned long long>(cases_run),
                         cases_run == 1 ? "" : "s",
                         static_cast<unsigned long long>(cases_discarded));
        }

        // Closes one failure section: the exception it raised, the
        // engine's caveat on how reliably it reproduced, then the line that
        // replays it.
        void print_failure_body(const RunConfig& config,
                                const std::optional<TestLocation>& location,
                                RerunForm rerun_form,
                                const BodyOutcome& outcome,
                                const std::optional<std::string>& blob,
                                const std::optional<std::string>& caveat,
                                bool printed_output) {
            if (printed_output) {
                std::fprintf(stderr, "\n");
            }
            std::fprintf(stderr, "Exception: %s%s\n", outcome.type_name.c_str(),
                         outcome.message.empty()
                             ? ""
                             : (": " + outcome.message).c_str());
            if (caveat.has_value()) {
                std::fprintf(stderr, "note: %s\n", caveat->c_str());
            }
            if (!config.print_blob || !blob.has_value()) {
                return;
            }
            if (rerun_form == RerunForm::Annotation && location.has_value()) {
                std::fprintf(
                    stderr, "rerun with: HEGEL_REPRODUCE_FAILURE(%s, \"%s\")\n",
                    location->name.c_str(), blob->c_str());
            } else {
                std::fprintf(
                    stderr,
                    "rerun with: hegel::test(test_fn, settings, {\"%s\"})\n",
                    blob->c_str());
            }
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

        uint32_t to_engine(const ListSetting<HealthCheck>& checks) {
            uint32_t mask = 0;
            for (HealthCheck c : checks.values()) {
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

        uint32_t to_engine(const ListSetting<Phase>& phases) {
            uint32_t mask = 0;
            for (Phase p : phases.values()) {
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

        // Set each field the caller gave onto a handle resolved from a
        // settings profile. Every other field keeps the profile's value.
        void apply_settings(hegel_context_t* ctx, hegel_settings_t* s,
                            const Settings& settings) {
            if (settings.test_cases.has_value()) {
                impl::settings_set_test_cases(ctx, s, *settings.test_cases);
            }
            if (settings.verbosity.has_value()) {
                impl::settings_set_verbosity(ctx, s,
                                             to_engine(*settings.verbosity));
            }
            if (settings.seed.has_value()) {
                impl::settings_set_seed(ctx, s, *settings.seed, true);
            }
            if (settings.derandomize.has_value()) {
                impl::settings_set_derandomize(ctx, s, *settings.derandomize);
            }
            if (settings.report_multiple_failures.has_value()) {
                impl::settings_set_report_multiple_failures(
                    ctx, s, *settings.report_multiple_failures);
            }
            if (settings.stop_on_first_failure.has_value()) {
                impl::settings_set_stop_on_first_failure(
                    ctx, s, *settings.stop_on_first_failure);
            }
            if (settings.nondeterminism_strictness.has_value()) {
                impl::settings_set_nondeterminism_strictness(
                    ctx, s, to_engine(*settings.nondeterminism_strictness));
            }
            if (settings.print_blob.has_value()) {
                impl::settings_set_print_blob(ctx, s, *settings.print_blob);
            }
            if (settings.show_statistics.has_value()) {
                impl::settings_set_show_statistics(ctx, s,
                                                   *settings.show_statistics);
            }
            if (settings.unbounded_choices.has_value()) {
                impl::settings_set_unbounded_choices(
                    ctx, s, *settings.unbounded_choices);
            }

            switch (settings.database.kind()) {
            case Database::Kind::Unset:
                break;
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

            if (settings.suppress_health_check.is_set()) {
                impl::settings_set_suppress_health_check(
                    ctx, s, to_engine(settings.suppress_health_check));
            }
            if (settings.phases.is_set()) {
                impl::settings_set_phases(ctx, s, to_engine(settings.phases));
            }

            switch (settings.backend) {
            case Backend::Auto:
                break;
            case Backend::Default:
                impl::settings_set_backend(ctx, s, HEGEL_BACKEND_DEFAULT);
                break;
            case Backend::Urandom:
                impl::settings_set_backend(ctx, s, HEGEL_BACKEND_URANDOM);
                break;
            }
        }

        // A handle resolved from the settings profile that `settings` name,
        // with the fields they set applied over it.
        void resolve_settings(hegel_context_t* ctx, SettingsGuard& guard,
                              const Settings& settings) {
            if (settings.stateful_step_count < 1) {
                throw std::invalid_argument(
                    "Settings::stateful_step_count must be at least 1");
            }
            guard.s =
                settings.profile.has_value()
                    ? impl::settings_new_for_profile(ctx, *settings.profile)
                    : impl::settings_new(ctx);
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

        // Reports one failure as the body of a failure section, closed with
        // its exception and rerun line. A failure with a reproduce blob is
        // replayed live; one without (an unconfirmed nondeterministic
        // failure, or one found by replaying a blob), or whose replay does
        // not fail, is reported from the output the engine had the run
        // capture for its origin.
        BodyOutcome handle_failure(
            const std::function<void(TestCase&)>& test_fn, hegel_context_t* ctx,
            hegel_settings_t* s, const RunConfig& config,
            const std::optional<TestLocation>& location, RerunForm rerun_form,
            const hegel_failure_t* failure, const Captures& captures) {
            std::optional<std::string> blob =
                impl::failure_reproduction_blob(ctx, failure);
            std::optional<std::string> caveat =
                impl::failure_caveat(ctx, failure);
            bool quiet = config.verbosity == Verbosity::Quiet;
            if (blob.has_value()) {
                bool printed_output = false;
                BodyOutcome outcome = replay_failure(ctx, s, *blob, config,
                                                     test_fn, &printed_output);
                if (outcome.status == HEGEL_STATUS_INTERESTING) {
                    if (!quiet) {
                        print_failure_body(config, location, rerun_form,
                                           outcome, blob, caveat,
                                           printed_output);
                    }
                    return outcome;
                }
            }
            auto captured = captures.find(impl::failure_origin(ctx, failure));
            if (captured == captures.end()) {
                // GCOVR_EXCL_START
                throw std::runtime_error(
                    "internal error: the engine reported a failure that "
                    "neither reproduced from its blob nor was captured");
                // GCOVR_EXCL_STOP
            }
            if (!quiet) {
                print_report_body(captured->second.lines);
                print_failure_body(config, location, rerun_form,
                                   captured->second.outcome, blob, caveat,
                                   !captured->second.lines.empty());
            }
            return captured->second.outcome;
        }

        // Pulls every test case off `run`, then acts on its verdict: returns
        // nothing for a pass, throws the RunError subclass for a run error,
        // and for a failure prints one framed report, holding one section
        // per distinct counterexample, and returns their outcomes.
        std::vector<BodyOutcome> drive_run(
            const std::function<void(TestCase&)>& test_fn, hegel_context_t* ctx,
            hegel_settings_t* s, hegel_run_t* run, const RunConfig& config,
            const std::optional<TestLocation>& location, RerunForm rerun_form) {
            bool verbose = config.verbosity == Verbosity::Verbose ||
                           config.verbosity == Verbosity::Debug;
            uint64_t cases_run = 0;
            uint64_t cases_discarded = 0;
            bool seen_interesting = false;
            Captures captures;

            // Generation loop: pull cases until the engine reports completion
            // (NULL test case), running, marking, and releasing each. A case
            // the engine stamped for capture buffers its output, which
            // reports its failure when the reproduce blob cannot.
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
                    for (const std::string& line : lines) {
                        std::fprintf(stderr, "%s\n", line.c_str());
                    }
                }
                if (outcome.status == HEGEL_STATUS_INTERESTING) {
                    if (!stamped) {
                        lines.clear();
                    }
                    store_capture(captures, outcome, std::move(lines));
                }
                if (!seen_interesting) {
                    if (outcome.status == HEGEL_STATUS_INTERESTING) {
                        cases_run++;
                        seen_interesting = true;
                    } else if (outcome.status == HEGEL_STATUS_VALID) {
                        cases_run++;
                    } else if (outcome.status == HEGEL_STATUS_INVALID) {
                        cases_discarded++;
                    }
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
                // The run itself failed (health check, nondeterminism, engine
                // error) and produced no verdict on the property.
                impl::raise_run_error(impl::run_result_error(ctx, result)
                                          .value_or("unknown error"),
                                      impl::run_result_error_kind(ctx, result));
            }

            bool quiet = config.verbosity == Verbosity::Quiet;
            size_t failure_count = impl::run_result_failure_count(ctx, result);
            if (!quiet) {
                print_failure_header(location, cases_run, cases_discarded);
            }

            std::vector<BodyOutcome> outcomes;
            for (size_t i = 0; i < failure_count; i++) {
                FailureGuard failure_guard{ctx};
                failure_guard.failure =
                    impl::run_result_failure(ctx, result, i);
                if (!quiet && failure_count > 1) {
                    std::fprintf(stderr, "\nFailure %zu of %zu:\n", i + 1,
                                 failure_count);
                }
                outcomes.push_back(handle_failure(
                    test_fn, ctx, s, config, location, rerun_form,
                    failure_guard.failure, captures));
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

        // Throws std::invalid_argument when the draws of an explicit test
        // case did not take all of its values.
        void check_fully_used(const impl::test_case::ExplicitState& state) {
            std::string unused;
            const std::vector<ExplicitTestCase::Value>& values =
                state.test_case.values();
            for (size_t i = 0; i < values.size(); ++i) {
                if (state.used[i]) {
                    continue;
                }
                unused += unused.empty() ? "" : ", ";
                unused +=
                    values[i].name ? *values[i].name : "a positional value";
            }
            if (!unused.empty()) {
                throw std::invalid_argument(
                    "The explicit test case " + describe(state.test_case) +
                    " gave values that no draw took: " + unused);
            }
        }

        // Runs each explicit test case once, in order. The first that fails
        // is reported, and its exception propagates.
        void run_explicit_cases(
            const std::function<void(TestCase&)>& test_fn,
            const RunConfig& config,
            const std::optional<TestLocation>& location,
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
                    for (const std::string& line : state->lines) {
                        std::fprintf(stderr, "%s\n", line.c_str());
                    }
                }
                if (outcome.status == HEGEL_STATUS_INVALID) {
                    throw std::invalid_argument(
                        "The explicit test case " + describe(test_case) +
                        " was rejected (TestCase::assume() or "
                        "TestCase::reject()). Explicit test cases must not "
                        "be rejected.");
                }
                if (outcome.status == HEGEL_STATUS_INTERESTING) {
                    if (!quiet) {
                        print_failure_title(location);
                        std::fprintf(stderr,
                                     "Falsified by explicit test case %s:\n",
                                     describe(test_case).c_str());
                        print_report_body(state->lines);
                        print_failure_body(config, location, RerunForm::Call,
                                           outcome, std::nullopt, std::nullopt,
                                           !state->lines.empty());
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
                     drive_run(test_fn, ctx, s, run_guard.run, config, location,
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
                test_fn, ctx, s, run_guard.run, config, location, rerun_form);
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
                    run_explicit_cases(test_fn, config, location,
                                       explicit_cases);
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
        run_test(test_fn, location, RerunForm::Call, settings,
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
