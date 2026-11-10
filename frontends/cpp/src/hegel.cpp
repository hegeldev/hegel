/*
 * hegel.cpp - The hegel::test() entry point.
 *
 * Drives Hegel's native engine (libhegel) in-process through its C ABI:
 * start a run, pull test cases, run the user body, mark each complete, then
 * inspect the aggregate result and replay any counterexamples.
 */

#include <hegel/hegel.h>
#include <hegel/internal.h>
#include <hegel/settings.h>
#include <hegel/test_case.h>

#include <engine.h>
#include <test_case.h>

#include <hegel.h>

#include <algorithm>
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
#include <string_view>
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
        // names a test defined by the HEGEL_TEST macro. The option form is
        // the command-line option of a program defined by HEGEL_MAIN.
        enum class RerunForm { Call, Annotation, Option };

        // Set when a run throws because the property failed, after the
        // failure was reported. Any other exception from a run is an error
        // that nothing has reported yet.
        thread_local bool failure_reported = false;

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

        // Scopes the example database to one test. The name is the framework
        // test that runs now, or the function that called hegel::test()
        Settings with_default_database_key(Settings settings, const char* file,
                                           const std::string& name) {
            if (settings.database_key.has_value() || name.empty() ||
                settings.database.kind() == Database::Kind::Disabled) {
                return settings;
            }
            settings.database_key =
                std::string(file == nullptr ? "" : file) + "::" + name;
            return settings;
        }

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
            std::vector<impl::test_case::WorkerOutputLine> lines;
        };
        using Captures = std::map<std::string, CapturedFailure>;

        void
        store_capture(Captures& captures, const BodyOutcome& outcome,
                      std::vector<impl::test_case::WorkerOutputLine> lines) {
            auto existing = captures.find(outcome.origin);
            if (existing != captures.end() && !existing->second.lines.empty() &&
                lines.empty()) {
                return;
            }
            captures[outcome.origin] =
                CapturedFailure{outcome, std::move(lines)};
        }

        std::vector<std::string> group_concurrent_output(
            const std::vector<impl::test_case::WorkerOutputLine>& records) {
            std::vector<std::string> lines;
            std::vector<impl::test_case::WorkerOutputLine> round;
            auto append_grouped_worker_lines = [&] {
                std::stable_sort(round.begin(), round.end(),
                                 [](const auto& left, const auto& right) {
                                     return *left.worker < *right.worker;
                                 });
                for (const auto& line : round) {
                    lines.push_back(line.text);
                }
                round.clear();
            };
            for (const auto& record : records) {
                if (record.worker.has_value()) {
                    round.push_back(record);
                } else {
                    append_grouped_worker_lines();
                    lines.push_back(
                        record.text); // text from main thread has null worker
                }
            }
            append_grouped_worker_lines();
            return lines;
        }

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

        // Replays one counterexample. `in_report` wraps the replay's output in
        // a framed report's body: it opens with a blank line and is indented.
        BodyOutcome replay_failure(hegel_context_t* ctx, hegel_settings_t* s,
                                   const std::string& blob,
                                   const Settings& settings, bool in_report,
                                   const std::function<void(TestCase&)>& fn,
                                   bool* printed_output = nullptr) {
            hegel_test_case_t* handle = impl::test_case_from_blob(ctx, s, blob);
            // GCOVR_EXCL_START
            if (handle == nullptr) {
                // should be handled by check_rc, possible bug in Hegel if hit
                throw std::runtime_error(
                    "Unreachable: failure blob produced a null test case.");
            }
            // GCOVR_EXCL_STOP
            // Positional init (fields: tc, is_final, verbosity) so this
            // TU stays clean under a C++17 (HEGEL_REFLECTION=OFF) build.
            TestCase tc_obj(std::unique_ptr<impl::test_case::TestCaseData>(
                new impl::test_case::TestCaseData{
                    handle, /*is_final=*/true,
                    settings.verbosity.value_or(Verbosity::Normal),
                    /*note_indent=*/in_report ? 1 : 0, in_report}));
            tc_obj.data()->stateful_step_count = settings.stateful_step_count;
            BodyOutcome outcome = run_body(fn, tc_obj);
            if (printed_output != nullptr) {
                *printed_output = *tc_obj.data()->printed_output;
            }
            mark_complete(ctx, tc_obj.data()->tc, outcome);
            return outcome;
        }

        // Width the framed report's header rule is padded to.
        constexpr size_t frame_width = 72;

        void print_failure_header(const std::optional<TestLocation>& location,
                                  uint64_t cases_run,
                                  uint64_t cases_discarded) {
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
            std::fprintf(stderr,
                         "Falsified after %llu test case%s (%llu discarded):\n",
                         static_cast<unsigned long long>(cases_run),
                         cases_run == 1 ? "" : "s",
                         static_cast<unsigned long long>(cases_discarded));
        }

        // Closes one failure section: the exception it raised, the
        // engine's caveat on how reliably it reproduced, then the line that
        // replays it.
        void print_failure_body(const Settings& settings,
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
            if (!settings.print_blob.value_or(true) || !blob.has_value()) {
                return;
            }
            if (rerun_form == RerunForm::Annotation && location.has_value()) {
                std::fprintf(
                    stderr, "rerun with: HEGEL_REPRODUCE_FAILURE(%s, \"%s\")\n",
                    location->name.c_str(), blob->c_str());
            } else if (rerun_form == RerunForm::Option) {
                std::fprintf(stderr, "rerun with: --failure-blob %s\n",
                             blob->c_str());
            } else {
                std::fprintf(
                    stderr,
                    "rerun with: hegel::test(test_fn, settings, {\"%s\"})\n",
                    blob->c_str());
            }
        }

        // Prints the output a stamped test case buffered, as the body of a
        // failure section. Lines from concurrent workers are grouped by
        // worker within each round.
        void print_captured_output(const CapturedFailure& captured) {
            std::vector<std::string> lines =
                group_concurrent_output(captured.lines);
            if (lines.empty()) {
                return;
            }
            std::fprintf(stderr, "\n");
            for (const std::string& line : lines) {
                std::fprintf(stderr, "%s\n", line.c_str());
            }
        }

        hegel_verbosity_t verbosity_to_engine(Verbosity verbosity) {
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

        Verbosity verbosity_from_engine(hegel_verbosity_t verbosity) {
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
        strictness_to_engine(NondeterminismStrictness strictness) {
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

        uint32_t phases_to_engine(const std::vector<Phase>& phases) {
            uint32_t bits = 0;
            for (Phase p : phases) {
                switch (p) {
                case Phase::Explicit:
                    bits |= HEGEL_PHASE_EXPLICIT;
                    break;
                case Phase::Reuse:
                    bits |= HEGEL_PHASE_REUSE;
                    break;
                case Phase::Generate:
                    bits |= HEGEL_PHASE_GENERATE;
                    break;
                case Phase::Target:
                    bits |= HEGEL_PHASE_TARGET;
                    break;
                case Phase::Shrink:
                    bits |= HEGEL_PHASE_SHRINK;
                    break;
                }
            }
            return bits;
        }

        // Translate hegel::Settings onto a fresh hegel_settings_t handle. A
        // field left unset keeps the value the engine's settings profile
        // gave the handle.
        void apply_settings(hegel_context_t* ctx, hegel_settings_t* s,
                            const Settings& settings) {
            if (settings.stateful_step_count < 1) {
                throw std::invalid_argument(
                    "Settings::stateful_step_count must be at least 1");
            }
            if (settings.test_cases.has_value()) {
                impl::settings_set_test_cases(ctx, s, *settings.test_cases);
            }

            if (settings.verbosity.has_value()) {
                impl::settings_set_verbosity(
                    ctx, s, verbosity_to_engine(*settings.verbosity));
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
                    ctx, s,
                    strictness_to_engine(*settings.nondeterminism_strictness));
            }
            if (settings.print_blob.has_value()) {
                impl::settings_set_print_blob(ctx, s, *settings.print_blob);
            }

            switch (settings.database.kind()) {
            case Database::Kind::Unset:
                break;
            case Database::Kind::Disabled:
                impl::settings_set_database(ctx, s, "");
                break;
            case Database::Kind::Path:
                impl::settings_set_database(ctx, s, settings.database.path());
                break;
            }

            if (settings.database_key.has_value()) {
                impl::settings_set_database_key(ctx, s, *settings.database_key);
            }

            uint32_t suppress = 0;
            for (HealthCheck c : settings.suppress_health_check) {
                switch (c) {
                case HealthCheck::FilterTooMuch:
                    suppress |= HEGEL_HC_FILTER_TOO_MUCH;
                    break;
                case HealthCheck::TooSlow:
                    suppress |= HEGEL_HC_TOO_SLOW;
                    break;
                case HealthCheck::TestCasesTooLarge:
                    suppress |= HEGEL_HC_TEST_CASES_TOO_LARGE;
                    break;
                case HealthCheck::LargeInitialTestCase:
                    suppress |= HEGEL_HC_LARGE_INITIAL_TEST_CASE;
                    break;
                }
            }
            if (suppress != 0) {
                impl::settings_set_suppress_health_check(ctx, s, suppress);
            }

            if (settings.phases.is_set()) {
                impl::settings_set_phases(
                    ctx, s, phases_to_engine(settings.phases.phases()));
            }

            switch (settings.backend) {
            case Backend::Auto:
                // The settings profile picks the backend.
                break;
            case Backend::Default:
                impl::settings_set_backend(ctx, s, HEGEL_BACKEND_DEFAULT);
                break;
            case Backend::Urandom:
                impl::settings_set_backend(ctx, s, HEGEL_BACKEND_URANDOM);
                break;
            }
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
        // failure), or whose replay does not fail, is reported from the
        // output the engine had the run capture for its origin.
        BodyOutcome handle_failure(
            const std::function<void(TestCase&)>& test_fn, hegel_context_t* ctx,
            hegel_settings_t* s, const Settings& settings,
            const std::optional<TestLocation>& location, RerunForm rerun_form,
            const hegel_failure_t* failure, const Captures& captures) {
            std::optional<std::string> blob =
                impl::failure_reproduction_blob(ctx, failure);
            std::optional<std::string> caveat =
                impl::failure_caveat(ctx, failure);
            bool quiet = settings.verbosity == Verbosity::Quiet;
            if (blob.has_value()) {
                bool printed_output = false;
                BodyOutcome outcome = replay_failure(ctx, s, *blob, settings,
                                                     /*in_report=*/!quiet,
                                                     test_fn, &printed_output);
                if (outcome.status == HEGEL_STATUS_INTERESTING) {
                    if (!quiet) {
                        print_failure_body(settings, location, rerun_form,
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
                print_captured_output(captured->second);
                print_failure_body(settings, location, rerun_form,
                                   captured->second.outcome, blob, caveat,
                                   !captured->second.lines.empty());
            }
            return captured->second.outcome;
        }

        void run_from_blob(const std::function<void(TestCase&)>& test_fn,
                           hegel_context_t* ctx, hegel_settings_t* s,
                           const Settings& settings, RerunForm rerun_form,
                           const std::vector<std::string>& failure_blobs) {
            // multiple blobs are accepted for bookkeeping, but only the first
            // one is run like in the other Hegel libraries
            BodyOutcome outcome =
                replay_failure(ctx, s, failure_blobs.front(), settings,
                               /*in_report=*/false, test_fn);

            if (outcome.exception == nullptr) {
                throw std::runtime_error(
                    "The failure blob did not reproduce an error");
            }
            if (settings.verbosity != Verbosity::Quiet) {
                std::fprintf(stderr, "Failure blob %s reproduced an error\n",
                             failure_blobs.front().c_str());
                // A program has no caller to show the exception to.
                if (rerun_form == RerunForm::Option) {
                    std::fprintf(stderr, "Exception: %s%s\n",
                                 outcome.type_name.c_str(),
                                 outcome.message.empty()
                                     ? ""
                                     : (": " + outcome.message).c_str());
                }
            }

            failure_reported = true;
            std::rethrow_exception(outcome.exception);
        }

        void run_from_engine(const std::function<void(TestCase&)>& test_fn,
                             hegel_context_t* ctx, hegel_settings_t* s,
                             const Settings& settings,
                             const std::optional<TestLocation>& location,
                             RerunForm rerun_form) {
            RunGuard run_guard{ctx};
            run_guard.run = impl::run_start(ctx, s);
            hegel_run_t* run = run_guard.run;

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
                auto data = std::unique_ptr<impl::test_case::TestCaseData>(
                    new impl::test_case::TestCaseData{
                        handle, /*is_final=*/false,
                        settings.verbosity.value_or(Verbosity::Normal)});
                bool stamped = impl::test_case_should_capture(ctx, handle);
                data->buffer_output = stamped;
                data->stateful_step_count = settings.stateful_step_count;
                TestCase tc_obj(std::move(data));
                BodyOutcome outcome = run_body(test_fn, tc_obj);
                if (outcome.status == HEGEL_STATUS_INTERESTING) {
                    std::vector<impl::test_case::WorkerOutputLine> lines;
                    if (stamped) {
                        lines = *tc_obj.data()->output_lines;
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
                return;
            }

            if (run_status == HEGEL_RUN_STATUS_ERROR) {
                // The run itself failed (health check, nondeterminism, engine
                // panic) and produced no verdict on the property.
                std::optional<std::string> run_err =
                    impl::run_result_error(ctx, result);
                throw std::runtime_error("Hegel run error: " +
                                         run_err.value_or("unknown error"));
            }

            bool quiet = settings.verbosity == Verbosity::Quiet;
            // Failed: one framed report, holding one section per distinct
            // counterexample.
            size_t failure_count = impl::run_result_failure_count(ctx, result);
            if (!quiet) {
                print_failure_header(location, cases_run, cases_discarded);
            }

            if (failure_count == 1) {
                FailureGuard failure_guard{ctx};
                failure_guard.failure =
                    impl::run_result_failure(ctx, result, 0);
                std::exception_ptr exception =
                    handle_failure(test_fn, ctx, s, settings, location,
                                   rerun_form, failure_guard.failure, captures)
                        .exception;
                failure_reported = true;
                std::rethrow_exception(exception);
            }

            for (size_t i = 0; i < failure_count; i++) {
                FailureGuard failure_guard{ctx};
                failure_guard.failure =
                    impl::run_result_failure(ctx, result, i);
                if (!quiet) {
                    std::fprintf(stderr, "\nFailure %zu of %zu:\n", i + 1,
                                 failure_count);
                }
                handle_failure(test_fn, ctx, s, settings, location, rerun_form,
                               failure_guard.failure, captures);
            }
            failure_reported = true;
            throw std::runtime_error("Hegel test failed with " +
                                     std::to_string(failure_count) +
                                     " distinct failures");
        }
    } // namespace

    namespace internal {
        namespace {
            // Function-local static so registrations from other translation
            // units' static initializers always find a constructed registry.
            std::map<std::string, std::string>& blob_registry() {
                static std::map<std::string, std::string> registry;
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
            blob_registry().insert({name, blobs.front()});
            return true;
        }

        std::vector<std::string> reproduce_blobs_for(const char* name) {
            auto blob = blob_registry().find(name);
            if (blob == blob_registry().end()) {
                return {};
            }
            return {blob->second};
        }
    } // namespace internal

    namespace {
        void run_with_handle(const std::function<void(TestCase&)>& test_fn,
                             hegel_context_t* ctx, hegel_settings_t* s,
                             const Settings& settings,
                             const std::optional<TestLocation>& location,
                             RerunForm rerun_form,
                             const std::vector<std::string>& failure_blobs) {
            Settings resolved = settings;
            resolved.verbosity =
                verbosity_from_engine(impl::settings_get_verbosity(ctx, s));
            resolved.print_blob = impl::settings_get_print_blob(ctx, s);
            if (failure_blobs.empty()) {
                run_from_engine(test_fn, ctx, s, resolved, location,
                                rerun_form);
            } else {
                run_from_blob(test_fn, ctx, s, resolved, rerun_form,
                              failure_blobs);
            }
        }

        void run_test(const std::function<void(TestCase&)>& test_fn,
                      const std::optional<TestLocation>& location,
                      RerunForm rerun_form, const Settings& settings,
                      const std::vector<std::string>& failure_blobs) {
            hegel_context_t* ctx = impl::thread_context();

            SettingsGuard settings_guard{ctx};
            settings_guard.s = impl::settings_new(ctx);
            hegel_settings_t* s = settings_guard.s;
            apply_settings(ctx, s, settings);
            if (location.has_value()) {
                apply_test_location(ctx, s, *location);
            }
            run_with_handle(test_fn, ctx, s, settings, location, rerun_form,
                            failure_blobs);
        }
    } // namespace

    void test(const std::function<void(TestCase&)>& test_fn,
              const Settings& settings,
              const std::vector<std::string>& failure_blobs,
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
        run_test(test_fn, location, RerunForm::Call,
                 with_default_database_key(settings, caller_file, key_name),
                 failure_blobs);
    }

    void test(const std::function<void(TestCase&)>& test_fn,
              const TestLocation& location, const Settings& settings,
              const std::vector<std::string>& failure_blobs) {
        run_test(test_fn, location, RerunForm::Call,
                 with_default_database_key(settings, location.file.c_str(),
                                           location.name),
                 failure_blobs);
    }

    namespace {
        // One `--SETTING` option of a HEGEL_MAIN program.
        struct SettingOption {
            std::string flag;
            std::string key;
            std::optional<std::string> value;
        };

        struct MainArgs {
            bool help = false;
            std::optional<std::string> profile;
            std::optional<std::string> failure_blob;
            std::vector<SettingOption> settings;
        };

        bool starts_with(std::string_view text, std::string_view prefix) {
            return text.substr(0, prefix.size()) == prefix;
        }

        std::string main_usage(const char* program) {
            return std::string("Usage: ") + program + R"( [options]

Hegel property-based testing binary. Runs exactly one test case.

Options:
  --profile <NAME>                       Set the default settings profile, like
                                         the HEGEL_DEFAULT_PROFILE environment
                                         variable; other flags apply on top
  --failure-blob <BLOB>                  Replay the failure that BLOB encodes
  --seed <N|none>                        Seed for randomisation ('none' for unset)
  --derandomize <BOOL>                   Use a deterministic derived seed
  --database <PATH|disabled|default>     Database path for failing-example storage
  --database-key <KEY>                   Key the failing examples are stored under
  --verbosity <LEVEL>                    quiet | normal | verbose | debug
  --phases <NAMES>                       Comma-separated phases to run: explicit,
                                         reuse, generate, target, shrink
  --suppress-health-check <NAMES>        Comma-separated health check names
                                         (filter_too_much, too_slow,
                                         test_cases_too_large,
                                         large_initial_test_case), or 'all'
  --report-multiple-failures <BOOL>      Report every distinct failure found
  --stop-on-first-failure <BOOL>         End generation once a failure is found
  --show-statistics <BOOL>               Print the run's statistics
  --print-blob <BOOL>                    Print the failure's reproduce blob
  --backend <default|urandom>            Randomness source (urandom: the Antithesis
                                         fuzzer, else /dev/urandom)
  --nondeterminism-strictness <LEVEL>    quiet | warn | error
  --unbounded-choices <BOOL>             Remove the limit on choices per test case
  -h, --help                             Show this help and exit

Every flag also takes the form --flag=VALUE. A BOOL is true, false, 1, 0,
yes or no. Values use the vocabulary of the matching hegel.toml key.
)";
        }

        // Reads the command line into `args`. Returns the problem with it,
        // if there is one.
        std::optional<std::string> parse_main_args(int argc, char** argv,
                                                   MainArgs& args) {
            for (int i = 1; i < argc; i++) {
                std::string_view arg = argv[i];
                if (arg == "-h" || arg == "--help") {
                    args.help = true;
                    return std::nullopt;
                }
                if (!starts_with(arg, "--") || arg == "--") {
                    return "unknown argument: " + std::string(arg);
                }
                std::string_view option = arg.substr(2);
                std::string name;
                std::optional<std::string> value;
                size_t equals = option.find('=');
                if (equals != std::string_view::npos) {
                    name = option.substr(0, equals);
                    value = option.substr(equals + 1);
                } else {
                    name = option;
                    if (i + 1 < argc && !starts_with(argv[i + 1], "--")) {
                        value = argv[++i];
                    }
                }
                std::string flag = "--" + name;
                if (name.find('_') != std::string::npos) {
                    return "unknown argument: " + flag;
                }
                if (name == "test-cases") {
                    return "--test-cases cannot be set: a HEGEL_MAIN program "
                           "runs one test case";
                }
                if (name == "profile" || name == "failure-blob") {
                    std::optional<std::string>& target =
                        name == "profile" ? args.profile : args.failure_blob;
                    if (!value.has_value() || value->empty()) {
                        return flag + " requires a value";
                    }
                    if (target.has_value()) {
                        return flag + " can be given only once";
                    }
                    target = std::move(value);
                    continue;
                }
                std::string key = name;
                std::replace(key.begin(), key.end(), '-', '_');
                args.settings.push_back({flag, key, std::move(value)});
            }
            return std::nullopt;
        }

        // Sets one `--SETTING` option on `s`. Returns the problem with it,
        // if there is one.
        std::optional<std::string>
        apply_setting_option(hegel_context_t* ctx, hegel_settings_t* s,
                             const SettingOption& option) {
            std::optional<std::string> error = impl::settings_set_from_string(
                ctx, s, option.key, option.value.value_or(""));
            if (error.has_value() && starts_with(*error, "unknown setting")) {
                return "unknown argument: " + option.flag;
            }
            if (!option.value.has_value()) {
                return option.flag + " requires a value";
            }
            if (option.key == "database_key" && option.value->empty()) {
                return "--database-key expects a non-empty key";
            }
            if (!error.has_value()) {
                return std::nullopt;
            }
            // The engine's message starts with the key in backticks.
            std::string message = *error;
            std::string key_prefix = "`" + option.key + "` ";
            if (starts_with(message, key_prefix)) {
                message.erase(0, key_prefix.size());
            }
            return option.flag + " " + message;
        }
    } // namespace

    namespace internal {
        int main_from_macro(int argc, char** argv,
                            const std::function<void(TestCase&)>& test_fn,
                            const TestLocation& location,
                            const Settings& settings,
                            const std::vector<std::string>& failure_blobs) {
            const char* program =
                argc > 0 && argv[0] != nullptr ? argv[0] : "program";
            auto usage_error = [&](const std::string& message) {
                std::fprintf(stderr, "error: %s\n\n%s", message.c_str(),
                             main_usage(program).c_str());
                return 2;
            };

            MainArgs args;
            if (std::optional<std::string> error =
                    parse_main_args(argc, argv, args)) {
                return usage_error(*error);
            }
            if (args.help) {
                std::fputs(main_usage(program).c_str(), stdout);
                return 0;
            }

            hegel_context_t* ctx = impl::thread_context();
            if (args.profile.has_value()) {
                if (std::optional<std::string> error =
                        impl::set_default_profile(ctx, *args.profile)) {
                    return usage_error("--profile: " + *error);
                }
            }

            SettingsGuard settings_guard{ctx};
            std::vector<std::string> blobs = failure_blobs;
            try {
                settings_guard.s = impl::settings_new(ctx);
                hegel_settings_t* s = settings_guard.s;
                apply_settings(ctx, s, settings);
                apply_test_location(ctx, s, location);
                for (const SettingOption& option : args.settings) {
                    if (std::optional<std::string> error =
                            apply_setting_option(ctx, s, option)) {
                        return usage_error(*error);
                    }
                }
                // Each run of the program is one test case. Both health
                // checks judge how a run accumulates test cases.
                impl::settings_set_test_cases(ctx, s, 1);
                impl::settings_set_suppress_health_check(
                    ctx, s,
                    impl::settings_get_suppress_health_check(ctx, s) |
                        HEGEL_HC_TOO_SLOW | HEGEL_HC_TEST_CASES_TOO_LARGE);
            } catch (const std::exception& e) {
                std::fprintf(stderr, "error: %s\n", e.what());
                return 1;
            }
            if (args.failure_blob.has_value()) {
                blobs.insert(blobs.begin(), *args.failure_blob);
            }

            failure_reported = false;
            try {
                run_with_handle(test_fn, ctx, settings_guard.s, settings,
                                location, RerunForm::Option, blobs);
            } catch (const std::exception& e) {
                if (!failure_reported) {
                    std::fprintf(stderr, "error: %s\n", e.what());
                }
                return 1;
            } catch (...) {
                // Only a test case throws an exception of another type, and
                // its failure is already reported.
                return 1;
            }
            return 0;
        }

        void test_from_macro(const std::function<void(TestCase&)>& test_fn,
                             const TestLocation& location,
                             const Settings& settings,
                             const std::vector<std::string>& failure_blobs) {
            run_test(test_fn, location, RerunForm::Annotation, settings,
                     failure_blobs);
        }
    } // namespace internal
} // namespace hegel
