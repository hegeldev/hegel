#include <engine.h>

#include <hegel/errors.h>
#include <hegel/internal.h>
#include <hegel/printer.h>
#include <hegel/settings.h>
#include <hegel/test_case.h>

#include <hegel.h>
#include <test_case.h>

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <functional>
#include <ios>
#include <iterator>
#include <limits>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <system_error>
#include <utility>
#include <vector>

#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>

namespace hegel::impl {

    hegel_context_t* thread_context() {
        struct ThreadContext {
            hegel_context_t* ctx = hegel_context_new();
            ~ThreadContext() { hegel_context_free(ctx); }
        };
        thread_local ThreadContext context;
        return context.ctx;
    }

    namespace {

        // A borrowed hegel_string_t over `text`, valid while `text` lives.
        hegel_string_t borrow(std::string_view text) {
            return hegel_string_t{text.data(), text.size()};
        }

        // The C++ copy of an engine-owned string, or nullopt for NULL.
        std::optional<std::string> copy_optional(const hegel_string_t* text) {
            if (text == nullptr) {
                return std::nullopt;
            }
            return std::string(text->data, text->len);
        }

    } // namespace

    // GCOVR_EXCL_START

    std::string last_error(hegel_context_t* ctx) {
        hegel_string_t msg = hegel_context_last_error(ctx);
        return std::string(msg.data, msg.len);
    }

    namespace {

        const char* rc_label(hegel_result_t rc) {
            switch (rc) {
            case HEGEL_E_STOP_TEST:
                return "engine stopped test";
            case HEGEL_E_ASSUME:
                return "assumption rejected";
            case HEGEL_E_BACKEND:
                return "backend error";
            case HEGEL_E_INVALID_HANDLE:
                return "invalid handle";
            case HEGEL_E_INVALID_ARG:
                return "invalid argument";
            case HEGEL_E_ALREADY_COMPLETE:
                return "test case already complete";
            case HEGEL_E_NOT_COMPLETE:
                return "previous test case not complete";
            case HEGEL_E_INTERNAL:
                return "internal error";
            case HEGEL_E_CONCURRENT_USE:
                return "concurrent use of test case handle";
            default:
                return "unknown error";
            }
        }

        // Translate a libhegel return code into success or a thrown
        // diagnostic, deriving the label from the code and appending the
        // context's last-error message.
        void check_rc(hegel_context_t* ctx, hegel_result_t rc) {
            if (rc == HEGEL_OK) {
                return;
            }
            std::string msg = last_error(ctx);
            throw std::runtime_error(std::string(rc_label(rc)) +
                                     (msg.empty() ? "" : ": " + msg));
        }

    } // namespace

    void settings_set_test_cases(hegel_context_t* ctx, hegel_settings_t* s,
                                 uint64_t test_cases) {
        check_rc(ctx, hegel_settings_set_test_cases(ctx, s, test_cases));
    }

    void settings_set_verbosity(hegel_context_t* ctx, hegel_settings_t* s,
                                hegel_verbosity_t verbosity) {
        check_rc(ctx, hegel_settings_set_verbosity(ctx, s, verbosity));
    }

    void settings_set_seed(hegel_context_t* ctx, hegel_settings_t* s,
                           uint64_t seed, bool has_seed) {
        check_rc(ctx, hegel_settings_set_seed(ctx, s, seed, has_seed));
    }

    void settings_set_derandomize(hegel_context_t* ctx, hegel_settings_t* s,
                                  bool derandomize) {
        check_rc(ctx, hegel_settings_set_derandomize(ctx, s, derandomize));
    }

    void settings_set_report_multiple_failures(hegel_context_t* ctx,
                                               hegel_settings_t* s, bool yes) {
        check_rc(ctx, hegel_settings_set_report_multiple_failures(ctx, s, yes));
    }

    void settings_set_suppress_health_check(hegel_context_t* ctx,
                                            hegel_settings_t* s,
                                            uint32_t mask) {
        check_rc(ctx, hegel_settings_set_suppress_health_check(ctx, s, mask));
    }

    void settings_set_phases(hegel_context_t* ctx, hegel_settings_t* s,
                             uint32_t phases) {
        check_rc(ctx, hegel_settings_set_phases(ctx, s, phases));
    }

    void settings_set_backend(hegel_context_t* ctx, hegel_settings_t* s,
                              hegel_backend_t backend) {
        check_rc(ctx, hegel_settings_set_backend(ctx, s, backend));
    }

    hegel_run_t* run_start(hegel_context_t* ctx, hegel_settings_t* s) {
        hegel_run_t* run = nullptr;
        // A NULL output callback (and user data) keeps engine output on
        // stderr.
        check_rc(ctx, hegel_run_start(ctx, s, nullptr, nullptr, &run));
        return run;
    }

    hegel_test_case_t* next_test_case(hegel_context_t* ctx, hegel_run_t* run) {
        hegel_test_case_t* tc = nullptr;
        check_rc(ctx, hegel_next_test_case(ctx, run, &tc));
        return tc;
    }

    hegel_test_case_t* test_case_clone(hegel_context_t* ctx,
                                       const hegel_test_case_t* tc) {
        hegel_test_case_t* clone = nullptr;
        check_rc(ctx, hegel_test_case_clone(ctx, tc, &clone));
        return clone;
    }

    hegel_run_result_t* run_result(hegel_context_t* ctx, hegel_run_t* run) {
        hegel_run_result_t* result = nullptr;
        check_rc(ctx, hegel_run_result(ctx, run, &result));
        return result;
    }

    hegel_run_status_t run_result_status(hegel_context_t* ctx,
                                         const hegel_run_result_t* result) {
        hegel_run_status_t status = HEGEL_RUN_STATUS_PASSED;
        check_rc(ctx, hegel_run_result_status(ctx, result, &status));
        return status;
    }

    size_t run_result_failure_count(hegel_context_t* ctx,
                                    const hegel_run_result_t* result) {
        size_t count = 0;
        check_rc(ctx, hegel_run_result_failure_count(ctx, result, &count));
        return count;
    }

    hegel_failure_t* run_result_failure(hegel_context_t* ctx,
                                        const hegel_run_result_t* result,
                                        size_t index) {
        hegel_failure_t* failure = nullptr;
        check_rc(ctx, hegel_run_result_failure(ctx, result, index, &failure));
        return failure;
    }

    // GCOVR_EXCL_STOP

    namespace {

        // As check_rc, but ignores a write to a region whose document was
        // already read: a straggler's output has nowhere left to go.
        void tolerate_dead_region(hegel_context_t* ctx, hegel_result_t rc) {
            if (rc == HEGEL_E_INVALID_HANDLE) {
                return;
            }
            check_rc(ctx, rc);
        }

    } // namespace

    hegel_test_case_t* test_case_block(hegel_context_t* ctx,
                                       const hegel_test_case_t* tc,
                                       uint64_t indent) {
        hegel_test_case_t* block = nullptr;
        check_rc(ctx, hegel_test_case_block(ctx, tc, indent, &block));
        return block;
    }

    void test_case_set_worker(hegel_context_t* ctx, const hegel_test_case_t* tc,
                              int64_t worker_index) {
        check_rc(ctx, hegel_test_case_set_worker(ctx, tc, worker_index));
    }

    hegel_printer_t* test_case_printer(hegel_context_t* ctx,
                                       hegel_test_case_t* tc) {
        hegel_printer_t* printer = nullptr;
        check_rc(ctx, hegel_test_case_printer(ctx, tc, nullptr, &printer));
        return printer;
    }

    void note(hegel_context_t* ctx, hegel_test_case_t* tc,
              std::string_view text) {
        tolerate_dead_region(ctx, hegel_note(ctx, tc, borrow(text)));
    }

    void printer_free(hegel_context_t* ctx, hegel_printer_t* printer) {
        hegel_printer_free(ctx, printer);
    }

    std::string read_document(hegel_context_t* ctx, hegel_printer_t* root) {
        // Fails only when no deferred region is outstanding, which leaves
        // nothing to splice in.
        hegel_printer_resolve(ctx, root);
        hegel_printer_value_result_t result{nullptr, 0};
        check_rc(ctx, hegel_printer_value(ctx, root, &result));
        std::string document(result.data, result.len);
        hegel_printer_value_result_free(ctx, &result);
        return document;
    }

    namespace {

        // As check_rc, but a rejected argument throws std::invalid_argument
        // with the engine's diagnostic.
        void check_arg(hegel_context_t* ctx, hegel_result_t rc) {
            if (rc == HEGEL_E_INVALID_ARG) {
                throw std::invalid_argument(last_error(ctx));
            }
            check_rc(ctx, rc);
        }

    } // namespace

    hegel_settings_t* settings_new(hegel_context_t* ctx) {
        hegel_settings_t* s = nullptr;
        check_arg(ctx, hegel_settings_new(ctx, &s));
        return s;
    }

    hegel_settings_t* settings_new_for_profile(hegel_context_t* ctx,
                                               std::string_view name) {
        hegel_settings_t* s = nullptr;
        check_arg(ctx, hegel_settings_new_for_profile(ctx, borrow(name), &s));
        return s;
    }

    void settings_register_profile(hegel_context_t* ctx, std::string_view name,
                                   const hegel_settings_t* s) {
        check_arg(ctx, hegel_settings_register_profile(ctx, borrow(name), s));
    }

    void set_default_profile(hegel_context_t* ctx, std::string_view name) {
        check_arg(ctx, hegel_set_default_profile(ctx, borrow(name)));
    }

    std::string engine_version(hegel_context_t* ctx) {
        hegel_string_t version{nullptr, 0};
        check_rc(ctx, hegel_version(ctx, &version));
        return std::string(version.data, version.len);
    }

    void settings_set_show_statistics(hegel_context_t* ctx, hegel_settings_t* s,
                                      bool yes) {
        check_rc(ctx, hegel_settings_set_show_statistics(ctx, s, yes));
    }

    void settings_set_unbounded_choices(hegel_context_t* ctx,
                                        hegel_settings_t* s, bool yes) {
        check_rc(ctx, hegel_settings_set_unbounded_choices(ctx, s, yes));
    }

    void settings_set_print_blob(hegel_context_t* ctx, hegel_settings_t* s,
                                 bool yes) {
        check_rc(ctx, hegel_settings_set_print_blob(ctx, s, yes));
    }

    void settings_set_default_database(hegel_context_t* ctx,
                                       hegel_settings_t* s) {
        check_rc(ctx, hegel_settings_set_database(ctx, s, nullptr));
    }

    hegel_verbosity_t settings_get_verbosity(hegel_context_t* ctx,
                                             const hegel_settings_t* s) {
        hegel_verbosity_t verbosity = HEGEL_VERBOSITY_NORMAL;
        check_rc(ctx, hegel_settings_get_verbosity(ctx, s, &verbosity));
        return verbosity;
    }

    bool settings_get_print_blob(hegel_context_t* ctx,
                                 const hegel_settings_t* s) {
        bool yes = true;
        check_rc(ctx, hegel_settings_get_print_blob(ctx, s, &yes));
        return yes;
    }

    bool settings_get_report_multiple_failures(hegel_context_t* ctx,
                                               const hegel_settings_t* s) {
        bool yes = false;
        check_rc(ctx,
                 hegel_settings_get_report_multiple_failures(ctx, s, &yes));
        return yes;
    }

    uint32_t settings_get_phases(hegel_context_t* ctx,
                                 const hegel_settings_t* s) {
        uint32_t phases = 0;
        check_rc(ctx, hegel_settings_get_phases(ctx, s, &phases));
        return phases;
    }

    uint64_t settings_get_test_cases(hegel_context_t* ctx,
                                     const hegel_settings_t* s) {
        uint64_t test_cases = 0;
        check_rc(ctx, hegel_settings_get_test_cases(ctx, s, &test_cases));
        return test_cases;
    }

    std::optional<uint64_t> settings_get_seed(hegel_context_t* ctx,
                                              const hegel_settings_t* s) {
        uint64_t seed = 0;
        bool has_seed = false;
        check_rc(ctx, hegel_settings_get_seed(ctx, s, &seed, &has_seed));
        if (!has_seed) {
            return std::nullopt;
        }
        return seed;
    }

    bool settings_get_derandomize(hegel_context_t* ctx,
                                  const hegel_settings_t* s) {
        bool yes = false;
        check_rc(ctx, hegel_settings_get_derandomize(ctx, s, &yes));
        return yes;
    }

    std::optional<std::string>
    settings_get_database(hegel_context_t* ctx, const hegel_settings_t* s) {
        const hegel_string_t* database = nullptr;
        check_rc(ctx, hegel_settings_get_database(ctx, s, &database));
        return copy_optional(database);
    }

    uint32_t settings_get_suppress_health_check(hegel_context_t* ctx,
                                                const hegel_settings_t* s) {
        uint32_t mask = 0;
        check_rc(ctx, hegel_settings_get_suppress_health_check(ctx, s, &mask));
        return mask;
    }

    bool settings_get_report_multiple_failures(hegel_context_t* ctx,
                                               const hegel_settings_t* s) {
        bool yes = false;
        check_rc(ctx,
                 hegel_settings_get_report_multiple_failures(ctx, s, &yes));
        return yes;
    }

    bool settings_get_stop_on_first_failure(hegel_context_t* ctx,
                                            const hegel_settings_t* s) {
        bool yes = false;
        check_rc(ctx, hegel_settings_get_stop_on_first_failure(ctx, s, &yes));
        return yes;
    }

    bool settings_get_show_statistics(hegel_context_t* ctx,
                                      const hegel_settings_t* s) {
        bool yes = false;
        check_rc(ctx, hegel_settings_get_show_statistics(ctx, s, &yes));
        return yes;
    }

    bool settings_get_unbounded_choices(hegel_context_t* ctx,
                                        const hegel_settings_t* s) {
        bool yes = false;
        check_rc(ctx, hegel_settings_get_unbounded_choices(ctx, s, &yes));
        return yes;
    }

    hegel_backend_t settings_get_backend(hegel_context_t* ctx,
                                         const hegel_settings_t* s) {
        hegel_backend_t backend = HEGEL_BACKEND_DEFAULT;
        check_rc(ctx, hegel_settings_get_backend(ctx, s, &backend));
        return backend;
    }

    hegel_nondeterminism_strictness_t
    settings_get_nondeterminism_strictness(hegel_context_t* ctx,
                                           const hegel_settings_t* s) {
        hegel_nondeterminism_strictness_t strictness =
            HEGEL_NONDETERMINISM_QUIET;
        check_rc(ctx, hegel_settings_get_nondeterminism_strictness(
                          ctx, s, &strictness));
        return strictness;
    }

    hegel_run_t* run_start_blob(hegel_context_t* ctx, hegel_settings_t* s,
                                std::string_view blob) {
        hegel_run_t* run = nullptr;
        check_arg(ctx, hegel_run_start_blob(ctx, s, borrow(blob), nullptr,
                                            nullptr, &run));
        return run;
    }

    std::optional<HealthCheck> health_check_from_flag(uint32_t flag) {
        switch (flag) {
        case HEGEL_HC_FILTER_TOO_MUCH:
            return HealthCheck::FilterTooMuch;
        case HEGEL_HC_TOO_SLOW:
            return HealthCheck::TooSlow;
        case HEGEL_HC_TEST_CASES_TOO_LARGE:
            return HealthCheck::TestCasesTooLarge;
        case HEGEL_HC_LARGE_INITIAL_TEST_CASE:
            return HealthCheck::LargeInitialTestCase;
        default:
            return std::nullopt;
        }
    }

    void raise_run_error(const std::string& message, RunErrorKind kind) {
        switch (kind.kind) {
        case HEGEL_RUN_ERROR_HEALTH_CHECK:
            throw FailedHealthCheck(message,
                                    health_check_from_flag(kind.health_check));
        case HEGEL_RUN_ERROR_FLAKY:
        case HEGEL_RUN_ERROR_NONDETERMINISTIC:
            throw Flaky(message);
        case HEGEL_RUN_ERROR_UNSATISFIABLE:
            throw Unsatisfiable(message);
        default:
            throw RunError(message);
        }
    }

    RunErrorKind run_result_error_kind(hegel_context_t* ctx,
                                       const hegel_run_result_t* result) {
        RunErrorKind kind{HEGEL_RUN_ERROR_NONE, 0};
        check_rc(ctx, hegel_run_result_error_kind(ctx, result, &kind.kind,
                                                  &kind.health_check));
        return kind;
    }

    void settings_set_stop_on_first_failure(hegel_context_t* ctx,
                                            hegel_settings_t* s, bool yes) {
        check_rc(ctx, hegel_settings_set_stop_on_first_failure(ctx, s, yes));
    }

    void settings_set_nondeterminism_strictness(hegel_context_t* ctx,
                                                hegel_settings_t* s,
                                                uint32_t strictness) {
        check_rc(ctx, hegel_settings_set_nondeterminism_strictness(ctx, s,
                                                                   strictness));
    }

    void settings_set_database(hegel_context_t* ctx, hegel_settings_t* s,
                               std::string_view path) {
        hegel_string_t database = borrow(path);
        check_rc(ctx, hegel_settings_set_database(ctx, s, &database));
    }

    void settings_set_database_key(hegel_context_t* ctx, hegel_settings_t* s,
                                   std::string_view key) {
        hegel_string_t database_key = borrow(key);
        check_rc(ctx, hegel_settings_set_database_key(ctx, s, &database_key));
    }

    bool test_case_should_capture(hegel_context_t* ctx,
                                  const hegel_test_case_t* tc) {
        bool should_capture = false;
        check_rc(ctx, hegel_test_case_should_capture(ctx, tc, &should_capture));
        return should_capture;
    }

    void mark_complete(hegel_context_t* ctx, hegel_test_case_t* tc,
                       hegel_status_t status,
                       const std::optional<std::string>& origin) {
        hegel_string_t origin_string{nullptr, 0};
        if (origin.has_value()) {
            origin_string = borrow(*origin);
        }
        check_rc(ctx, hegel_mark_complete(ctx, tc, status,
                                          origin.has_value() ? &origin_string
                                                             : nullptr));
    }

    std::optional<std::string>
    run_result_error(hegel_context_t* ctx, const hegel_run_result_t* result) {
        const hegel_string_t* err = nullptr;
        check_rc(ctx, hegel_run_result_error(ctx, result, &err));
        return copy_optional(err);
    }

    std::optional<std::string>
    failure_reproduction_blob(hegel_context_t* ctx,
                              const hegel_failure_t* failure) {
        const hegel_string_t* blob = nullptr;
        check_rc(ctx, hegel_failure_reproduction_blob(ctx, failure, &blob));
        return copy_optional(blob);
    }

    std::string failure_origin(hegel_context_t* ctx,
                               const hegel_failure_t* failure) {
        hegel_string_t origin{nullptr, 0};
        check_rc(ctx, hegel_failure_origin(ctx, failure, &origin));
        return std::string(origin.data, origin.len);
    }

    std::optional<std::string> failure_caveat(hegel_context_t* ctx,
                                              const hegel_failure_t* failure) {
        const hegel_string_t* caveat = nullptr;
        check_rc(ctx, hegel_failure_caveat(ctx, failure, &caveat));
        return copy_optional(caveat);
    }

    void settings_set_test_location(hegel_context_t* ctx, hegel_settings_t* s,
                                    std::string_view file, uint32_t begin_line,
                                    std::string_view class_name,
                                    std::string_view function) {
        check_rc(ctx, hegel_settings_set_test_location(
                          ctx, s, borrow(file), begin_line, borrow(class_name),
                          borrow(function)));
    }

    uint64_t label_from_name(hegel_context_t* ctx, std::string_view name) {
        uint64_t label = 0;
        check_rc(ctx, hegel_label_from_name(ctx, borrow(name), &label));
        return label;
    }

    uint64_t label_combine(hegel_context_t* ctx,
                           const std::vector<uint64_t>& labels) {
        uint64_t label = 0;
        check_rc(ctx, hegel_label_combine(ctx, labels.data(), labels.size(),
                                          &label));
        return label;
    }

    namespace {
        struct DrawScope {
            hegel_context_t* ctx;
            hegel_test_case_t* tc;

            explicit DrawScope(const TestCase& obj) {
                if (obj.data()->explicit_state) {
                    throw internal::UsageError(
                        "This operation needs generated data, which an "
                        "explicit test case does not have: it gives values "
                        "only to TestCase::draw().");
                }
                ctx = thread_context();
                tc = obj.data()->tc;
            }

            // Route a libhegel return code to success or the matching
            // exception.
            void raise_for_rc(hegel_result_t rc, const char* what) const {
                if (rc == HEGEL_OK) {
                    return;
                }
                if (rc == HEGEL_E_STOP_TEST) {
                    throw internal::HegelStopTest();
                }
                if (rc == HEGEL_E_ASSUME) {
                    throw internal::HegelReject();
                }
                if (rc == HEGEL_E_INVALID_ARG) {
                    throw std::invalid_argument(last_error(ctx));
                }
                // Engine-level failure; not reachable without fault
                // injection into the C ABI.
                // GCOVR_EXCL_START
                std::string msg = last_error(ctx);
                throw std::runtime_error(std::string(what) + " failed (" +
                                         rc_label(rc) +
                                         (msg.empty() ? ")" : "): " + msg));
                // GCOVR_EXCL_STOP
            }
        };

        // RAII for the engine-allocated draw buffers.
        struct BytesResultGuard {
            hegel_context_t* ctx;
            hegel_generate_bytes_result_t result{nullptr, 0};
            ~BytesResultGuard() {
                hegel_generate_bytes_result_free(ctx, &result);
            }
        };

        struct StringResultGuard {
            hegel_context_t* ctx;
            hegel_owned_string_t result{nullptr, 0};
            ~StringResultGuard() { hegel_owned_string_free(ctx, &result); }
        };

        // Construct a string generator, converting an engine rejection into
        // std::invalid_argument at generator-construction time.
        template <typename F>
        hegel_string_generator_t* make_string_generator(const char* what,
                                                        F&& build) {
            hegel_context_t* ctx = thread_context();
            hegel_string_generator_t* generator = nullptr;
            hegel_result_t rc = build(ctx, &generator);
            if (rc != HEGEL_OK) {
                std::string msg = last_error(ctx);
                throw std::invalid_argument(
                    std::string(what) +
                    (msg.empty() ? ": invalid arguments" : ": " + msg));
            }
            return generator;
        }

    } // namespace

    hegel_string_generator_t*
    string_generator_text(const TextGeneratorParams& params) {
        auto borrow_all = [](const std::vector<std::string>* names) {
            std::vector<hegel_string_t> strings;
            if (names != nullptr) {
                for (const std::string& name : *names) {
                    strings.push_back(borrow(name));
                }
            }
            return strings;
        };
        auto borrow_chars = [](const std::string* chars) {
            return chars != nullptr ? borrow(*chars)
                                    : hegel_string_t{nullptr, 0};
        };
        std::vector<hegel_string_t> categories = borrow_all(params.categories);
        std::vector<hegel_string_t> exclude_categories =
            borrow_all(params.exclude_categories);
        hegel_string_t codec{nullptr, 0};
        if (params.codec != nullptr) {
            codec = borrow(params.codec);
        }
        // A given list, even an empty one, needs a non-NULL pointer: NULL
        // means no list.
        static const hegel_string_t no_names{nullptr, 0};
        auto list = [](const std::vector<std::string>* given,
                       const std::vector<hegel_string_t>& strings) {
            if (given == nullptr) {
                return static_cast<const hegel_string_t*>(nullptr);
            }
            return strings.empty() ? &no_names : strings.data();
        };
        return make_string_generator(
            "text", [&](hegel_context_t* ctx, hegel_string_generator_t** out) {
                return hegel_string_generator_text(
                    ctx, params.min_size, params.max_size,
                    params.codec != nullptr ? &codec : nullptr,
                    params.min_codepoint, params.max_codepoint,
                    list(params.categories, categories), categories.size(),
                    list(params.exclude_categories, exclude_categories),
                    exclude_categories.size(),
                    borrow_chars(params.include_characters),
                    borrow_chars(params.exclude_characters), out);
            });
    }

    hegel_string_generator_t*
    string_generator_regex(std::string_view pattern, bool fullmatch,
                           const hegel_string_generator_t* alphabet) {
        return make_string_generator(
            "from_regex",
            [&](hegel_context_t* ctx, hegel_string_generator_t** out) {
                // A NULL alphabet lets padding and wildcards draw from all
                // of Unicode.
                return hegel_string_generator_regex(ctx, borrow(pattern),
                                                    fullmatch, alphabet, out);
            });
    }

    hegel_string_generator_t* string_generator_email() {
        return make_string_generator(
            "emails", [](hegel_context_t* ctx, hegel_string_generator_t** out) {
                return hegel_string_generator_email(ctx, out);
            });
    }

    hegel_string_generator_t* string_generator_url() {
        return make_string_generator(
            "urls", [](hegel_context_t* ctx, hegel_string_generator_t** out) {
                return hegel_string_generator_url(ctx, out);
            });
    }

    hegel_string_generator_t* string_generator_domain(uint64_t max_length) {
        return make_string_generator(
            "domains",
            [&](hegel_context_t* ctx, hegel_string_generator_t** out) {
                return hegel_string_generator_domain(ctx, max_length, out);
            });
    }

    void string_generator_free(hegel_string_generator_t* generator) {
        hegel_string_generator_free(thread_context(), generator);
    }

    std::string draw_string(const TestCase& tc,
                            const hegel_string_generator_t* generator) {
        DrawScope scope(tc);
        StringResultGuard guard{scope.ctx};
        scope.raise_for_rc(hegel_generate_string(scope.ctx, scope.tc, generator,
                                                 &guard.result),
                           "hegel_generate_string");
        std::string value(guard.result.data, guard.result.len);
        return value;
    }

    std::vector<uint8_t> draw_bytes(const TestCase& tc, uint64_t min_size,
                                    uint64_t max_size) {
        DrawScope scope(tc);
        BytesResultGuard guard{scope.ctx};
        scope.raise_for_rc(hegel_generate_bytes(scope.ctx, scope.tc, min_size,
                                                max_size, &guard.result),
                           "hegel_generate_bytes");
        std::vector<uint8_t> value(guard.result.data,
                                   guard.result.data + guard.result.len);
        return value;
    }

    hegel_date_t draw_date(const TestCase& tc) {
        DrawScope scope(tc);
        hegel_date_t value{};
        scope.raise_for_rc(
            hegel_generate_date(scope.ctx, scope.tc, hegel_date_t{1, 1, 1},
                                hegel_date_t{9999, 12, 31}, &value),
            "hegel_generate_date");
        return value;
    }

    hegel_time_t draw_time(const TestCase& tc) {
        DrawScope scope(tc);
        hegel_time_t value{};
        scope.raise_for_rc(
            hegel_generate_time(scope.ctx, scope.tc, hegel_time_t{0, 0, 0, 0},
                                hegel_time_t{23, 59, 59, 999999999}, &value),
            "hegel_generate_time");
        return value;
    }

    hegel_datetime_t draw_datetime(const TestCase& tc) {
        DrawScope scope(tc);
        hegel_datetime_t value{};
        hegel_datetime_t min_value{{1, 1, 1}, {0, 0, 0, 0}};
        hegel_datetime_t max_value{{9999, 12, 31}, {23, 59, 59, 999999999}};
        scope.raise_for_rc(hegel_generate_datetime(scope.ctx, scope.tc,
                                                   min_value, max_value,
                                                   &value),
                           "hegel_generate_datetime");
        return value;
    }

    std::string draw_ipv4(const TestCase& tc) {
        DrawScope scope(tc);
        std::array<uint8_t, 4> bytes{};
        scope.raise_for_rc(
            hegel_generate_ipv4(scope.ctx, scope.tc, bytes.data()),
            "hegel_generate_ipv4");
        char buf[INET_ADDRSTRLEN];
        if (inet_ntop(AF_INET, bytes.data(), buf, sizeof(buf)) == nullptr) {
            // 4 bytes always format; not reachable.
            // GCOVR_EXCL_START
            throw std::runtime_error("failed to format IPv4 address");
            // GCOVR_EXCL_STOP
        }
        return buf;
    }

    std::string draw_ipv6(const TestCase& tc) {
        DrawScope scope(tc);
        std::array<uint8_t, 16> bytes{};
        scope.raise_for_rc(
            hegel_generate_ipv6(scope.ctx, scope.tc, bytes.data()),
            "hegel_generate_ipv6");
        char buf[INET6_ADDRSTRLEN];
        if (inet_ntop(AF_INET6, bytes.data(), buf, sizeof(buf)) == nullptr) {
            // 16 bytes always format; not reachable.
            // GCOVR_EXCL_START
            throw std::runtime_error("failed to format IPv6 address");
            // GCOVR_EXCL_STOP
        }
        return buf;
    }

    std::string draw_uuid(const TestCase& tc, std::optional<uint8_t> version) {
        DrawScope scope(tc);
        std::array<uint8_t, 16> bytes{};
        scope.raise_for_rc(
            hegel_generate_uuid(scope.ctx, scope.tc, version.value_or(0),
                                version.has_value(), bytes.data()),
            "hegel_generate_uuid");
        // Canonical 8-4-4-4-12 hex, with hyphens before bytes 4, 6, 8, 10.
        static constexpr char kHex[] = "0123456789abcdef";
        std::string value;
        value.reserve(36);
        for (size_t i = 0; i < bytes.size(); ++i) {
            if (i == 4 || i == 6 || i == 8 || i == 10) {
                value.push_back('-');
            }
            value.push_back(kHex[bytes[i] >> 4]);
            value.push_back(kHex[bytes[i] & 0x0F]);
        }
        return value;
    }

    void target(const TestCase& tc, double score, std::string_view label) {
        DrawScope scope(tc);
        scope.raise_for_rc(
            hegel_target(scope.ctx, scope.tc, score, borrow(label)),
            "hegel_target");
    }

    void event(const TestCase& tc, std::string_view label) {
        DrawScope scope(tc);
        scope.raise_for_rc(hegel_event(scope.ctx, scope.tc, borrow(label)),
                           "hegel_event");
    }

    void event_value(const TestCase& tc, std::string_view label, double value) {
        DrawScope scope(tc);
        scope.raise_for_rc(
            hegel_event_value(scope.ctx, scope.tc, value, borrow(label)),
            "hegel_event_value");
    }

    namespace {

        struct GrammarBuilderGuard {
            hegel_context_t* ctx;
            hegel_grammar_builder_t* builder = nullptr;
            ~GrammarBuilderGuard() { hegel_grammar_builder_free(ctx, builder); }
        };

        struct GrammarDrawGuard {
            hegel_context_t* ctx;
            hegel_grammar_draw_t* draw = nullptr;
            ~GrammarDrawGuard() { hegel_grammar_draw_free(ctx, draw); }
        };

        // The text of the grammar file `path` under `dir`, or nullopt when
        // there is no directory or no such file.
        std::optional<std::string>
        read_grammar_file(const std::optional<std::string>& dir,
                          std::string_view path) {
            if (!dir) {
                return std::nullopt;
            }
            std::filesystem::path file =
                std::filesystem::path(*dir) / std::filesystem::path(path);
            std::error_code ec;
            std::filesystem::file_type type =
                std::filesystem::status(file, ec).type();
            if (type == std::filesystem::file_type::not_found) {
                return std::nullopt;
            }
            std::ifstream in(file, std::ios::binary);
            if (type != std::filesystem::file_type::regular || !in) {
                throw std::invalid_argument("Could not read grammar '" +
                                            file.string() + "'");
            }
            return std::string(std::istreambuf_iterator<char>(in),
                               std::istreambuf_iterator<char>());
        }

    } // namespace

    hegel_grammar_t* grammar_new(const GrammarParams& params) {
        hegel_context_t* ctx = thread_context();
        GrammarBuilderGuard guard{ctx};
        check_arg(ctx, hegel_grammar_builder_new(ctx, borrow(params.text),
                                                 &guard.builder));
        for (size_t index = 0;; ++index) {
            hegel_string_t path{nullptr, 0};
            if (hegel_grammar_builder_file(ctx, guard.builder, index, &path) ==
                HEGEL_E_NOT_FOUND) {
                break;
            }
            std::optional<std::string> text = read_grammar_file(
                params.import_dir, std::string_view(path.data, path.len));
            hegel_string_t contents =
                text ? borrow(*text) : hegel_string_t{nullptr, 0};
            check_arg(
                ctx, hegel_grammar_builder_provide(ctx, guard.builder, index,
                                                   text ? &contents : nullptr));
        }
        hegel_string_t start =
            params.start ? borrow(*params.start) : hegel_string_t{nullptr, 0};
        std::vector<hegel_string_t> names;
        names.reserve(params.explicit_names.size());
        for (const std::string& name : params.explicit_names) {
            names.push_back(borrow(name));
        }
        hegel_grammar_t* grammar = nullptr;
        check_arg(ctx, hegel_grammar_new(
                           ctx, guard.builder, params.start ? &start : nullptr,
                           params.max_size, params.max_depth, params.alphabet,
                           names.data(), names.size(), &grammar));
        return grammar;
    }

    void grammar_free(hegel_grammar_t* grammar) {
        hegel_grammar_free(thread_context(), grammar);
    }

    std::string draw_grammar(const TestCase& tc, const hegel_grammar_t* grammar,
                             const std::function<std::string(size_t)>& value) {
        DrawScope scope(tc);
        GrammarDrawGuard guard{scope.ctx};
        check_rc(scope.ctx,
                 hegel_grammar_draw_new(scope.ctx, grammar, 0, &guard.draw));
        size_t terminal = 0;
        hegel_result_t rc = HEGEL_OK;
        while ((rc = hegel_grammar_draw_next(scope.ctx, scope.tc, guard.draw,
                                             &terminal)) == HEGEL_OK) {
            std::string drawn = value(terminal);
            check_arg(scope.ctx, hegel_grammar_draw_provide(
                                     scope.ctx, guard.draw, borrow(drawn)));
        }
        if (rc != HEGEL_E_NOT_FOUND) {
            scope.raise_for_rc(rc, "hegel_grammar_draw_next");
        }
        StringResultGuard result{scope.ctx};
        check_rc(scope.ctx, hegel_grammar_draw_result(scope.ctx, guard.draw,
                                                      &result.result));
        return std::string(result.result.data, result.result.len);
    }

} // namespace hegel::impl

namespace hegel::internal {

    namespace {

        static_assert(state_machine_done == HEGEL_STATE_MACHINE_DONE);
        static_assert(filter_attempts == HEGEL_FILTER_ATTEMPTS);

        // The name each LabelName derives its engine label from, in
        // enumerator order.
        constexpr std::array<const char*, label_name_count> label_names = {
            "hegel.generator",
            "hegel.composite",
            "hegel.map",
            "hegel.flat_map",
            "hegel.filter",
            "hegel.filter.attempt",
            "hegel.just",
            "hegel.sampled_from",
            "hegel.one_of",
            "hegel.optional",
            "hegel.deferred",
            "hegel.integers",
            "hegel.floats",
            "hegel.booleans",
            "hegel.text",
            "hegel.characters",
            "hegel.from_regex",
            "hegel.from_lark",
            "hegel.binary",
            "hegel.emails",
            "hegel.urls",
            "hegel.domains",
            "hegel.ip_addresses.v4",
            "hegel.ip_addresses.v6",
            "hegel.uuids",
            "hegel.lists",
            "hegel.sets",
            "hegel.dicts",
            "hegel.tuples",
            "hegel.array",
            "hegel.permutation",
            "hegel.subsequence",
            "hegel.sample",
            "hegel.recursive",
            "hegel.stateful.rule",
            "hegel.pool.values_reusable",
            "hegel.pool.values_consumed",
        };
        static_assert(label_names[static_cast<size_t>(LabelName::Generator)] ==
                      std::string_view("hegel.generator"));
        static_assert(
            label_names[static_cast<size_t>(LabelName::PoolValuesConsumed)] ==
            std::string_view("hegel.pool.values_consumed"));

        // Two's-complement little-endian encoding of a uint64_t for the
        // engine's big-integer draw: 9 bytes so the top bit is never read
        // as a sign.
        constexpr size_t big_u64_len = 9;

        void encode_u64_le(uint64_t value, uint8_t* out) {
            for (size_t i = 0; i < 8; ++i) {
                out[i] = static_cast<uint8_t>(value >> (8 * i));
            }
            out[8] = 0;
        }

        uint64_t decode_u64_le(const uint8_t* in) {
            uint64_t value = 0;
            for (size_t i = 0; i < 8; ++i) {
                value |= static_cast<uint64_t>(in[i]) << (8 * i);
            }
            return value;
        }

    } // namespace

    uint64_t label(LabelName name) {
        static const std::array<uint64_t, label_name_count> labels = [] {
            std::array<uint64_t, label_name_count> values{};
            hegel_context_t* ctx = impl::thread_context();
            for (size_t i = 0; i < values.size(); ++i) {
                values[i] = impl::label_from_name(ctx, label_names[i]);
            }
            return values;
        }();
        return labels[static_cast<size_t>(name)];
    }

    uint64_t combine_labels(const std::vector<uint64_t>& labels) {
        return impl::label_combine(impl::thread_context(), labels);
    }

    void start_span(const TestCase& tc, uint64_t label) {
        impl::DrawScope scope(tc);
        scope.raise_for_rc(hegel_start_span(scope.ctx, scope.tc, label),
                           "hegel_start_span");
    }

    void stop_span(const TestCase& tc, bool discard) {
        impl::DrawScope scope(tc);
        scope.raise_for_rc(hegel_stop_span(scope.ctx, scope.tc, discard),
                           "hegel_stop_span");
    }

    int64_t draw_integer(const TestCase& tc, int64_t min_value,
                         int64_t max_value) {
        impl::DrawScope scope(tc);
        int64_t value = 0;
        scope.raise_for_rc(hegel_generate_integer(scope.ctx, scope.tc,
                                                  min_value, max_value, &value),
                           "hegel_generate_integer");
        return value;
    }

    uint64_t draw_integer_unsigned(const TestCase& tc, uint64_t min_value,
                                   uint64_t max_value) {
        if (max_value <=
            static_cast<uint64_t>(std::numeric_limits<int64_t>::max())) {
            return static_cast<uint64_t>(
                draw_integer(tc, static_cast<int64_t>(min_value),
                             static_cast<int64_t>(max_value)));
        }
        uint8_t min_bytes[big_u64_len];
        uint8_t max_bytes[big_u64_len];
        uint8_t out_bytes[big_u64_len];
        encode_u64_le(min_value, min_bytes);
        encode_u64_le(max_value, max_bytes);
        draw_integer_bytes(tc, min_bytes, max_bytes, big_u64_len, out_bytes);
        return decode_u64_le(out_bytes);
    }

    void draw_integer_bytes(const TestCase& tc, const uint8_t* min_value,
                            const uint8_t* max_value, size_t len,
                            uint8_t* out) {
        impl::DrawScope scope(tc);
        size_t out_len = 0;
        scope.raise_for_rc(hegel_generate_integer_big(scope.ctx, scope.tc,
                                                      min_value, len, max_value,
                                                      len, out, len, &out_len),
                           "hegel_generate_integer_big");
    }

    bool draw_boolean(const TestCase& tc, double p,
                      std::optional<bool> forced) {
        impl::DrawScope scope(tc);
        bool value = false;
        scope.raise_for_rc(
            hegel_generate_boolean(scope.ctx, scope.tc, p,
                                   /*forced=*/forced.value_or(false),
                                   /*has_forced=*/forced.has_value(), &value),
            "hegel_generate_boolean");
        return value;
    }

    double draw_float(const TestCase& tc, uint32_t width, double min_value,
                      double max_value, bool allow_nan, bool allow_infinity,
                      bool exclude_min, bool exclude_max,
                      double smallest_nonzero_magnitude) {
        impl::DrawScope scope(tc);
        double value = 0.0;
        scope.raise_for_rc(hegel_generate_float(
                               scope.ctx, scope.tc, width, min_value, max_value,
                               allow_nan, allow_infinity, exclude_min,
                               exclude_max, smallest_nonzero_magnitude, &value),
                           "hegel_generate_float");
        return value;
    }

    CollectionHandle::CollectionHandle(const TestCase& tc, uint64_t min_size,
                                       uint64_t max_size) {
        impl::DrawScope scope(tc);
        scope.raise_for_rc(hegel_new_collection(scope.ctx, scope.tc, min_size,
                                                max_size, &handle_),
                           "hegel_new_collection");
    }

    CollectionHandle::~CollectionHandle() {
        hegel_collection_free(impl::thread_context(), handle_);
    }

    bool CollectionHandle::more(const TestCase& tc) {
        impl::DrawScope scope(tc);
        bool more = false;
        scope.raise_for_rc(
            hegel_collection_more(scope.ctx, scope.tc, handle_, &more),
            "hegel_collection_more");
        return more;
    }

    void CollectionHandle::reject(const TestCase& tc, const char* why) {
        impl::DrawScope scope(tc);
        hegel_string_t reason{why, why != nullptr ? std::strlen(why) : 0};
        scope.raise_for_rc(
            hegel_collection_reject(scope.ctx, scope.tc, handle_,
                                    why != nullptr ? &reason : nullptr),
            "hegel_collection_reject");
    }

    RepeatHandle::RepeatHandle(const TestCase& tc) {
        impl::DrawScope scope(tc);
        scope.raise_for_rc(hegel_new_repeat(scope.ctx, scope.tc, &handle_),
                           "hegel_new_repeat");
    }

    RepeatHandle::~RepeatHandle() {
        hegel_repeat_free(impl::thread_context(), handle_);
    }

    bool RepeatHandle::more(const TestCase& tc) {
        impl::DrawScope scope(tc);
        bool more = false;
        scope.raise_for_rc(
            hegel_repeat_more(scope.ctx, scope.tc, handle_, &more),
            "hegel_repeat_more");
        return more;
    }

    void RepeatHandle::failed(const TestCase& tc) {
        impl::DrawScope scope(tc);
        scope.raise_for_rc(hegel_repeat_failed(scope.ctx, scope.tc, handle_),
                           "hegel_repeat_failed");
    }

    PoolHandle::PoolHandle(const TestCase& tc) {
        impl::DrawScope scope(tc);
        scope.raise_for_rc(hegel_new_pool(scope.ctx, scope.tc, &handle_),
                           "hegel_new_pool");
    }

    PoolHandle::~PoolHandle() {
        hegel_pool_free(impl::thread_context(), handle_);
    }

    int64_t PoolHandle::add(const TestCase& tc) {
        impl::DrawScope scope(tc);
        int64_t var_id = 0;
        scope.raise_for_rc(
            hegel_pool_add(scope.ctx, scope.tc, handle_, &var_id),
            "hegel_pool_add");
        return var_id;
    }

    int64_t PoolHandle::draw_variable(const TestCase& tc, bool consume) {
        impl::DrawScope scope(tc);
        int64_t var_id = 0;
        scope.raise_for_rc(
            hegel_pool_generate(scope.ctx, scope.tc, handle_, consume, &var_id),
            "hegel_pool_generate");
        return var_id;
    }

    void PoolHandle::restore(const TestCase& tc, int64_t variable) {
        impl::DrawScope scope(tc);
        scope.raise_for_rc(
            hegel_pool_restore(scope.ctx, scope.tc, handle_, variable),
            "hegel_pool_restore");
    }

    void PoolHandle::restore_unchecked(const TestCase& tc,
                                       int64_t variable) noexcept {
        hegel_pool_restore(impl::thread_context(), tc.data()->tc, handle_,
                           variable);
    }

    RecursionHandle::RecursionHandle(const TestCase& tc, uint64_t max_depth,
                                     uint64_t max_leaves) {
        impl::DrawScope scope(tc);
        scope.raise_for_rc(hegel_new_recursion(scope.ctx, scope.tc, max_depth,
                                               max_leaves, &handle_),
                           "hegel_new_recursion");
    }

    RecursionHandle::~RecursionHandle() {
        hegel_recursion_free(impl::thread_context(), handle_);
    }

    bool RecursionHandle::branch(const TestCase& tc, uint64_t depth) {
        impl::DrawScope scope(tc);
        bool branch = false;
        scope.raise_for_rc(hegel_recursion_branch(scope.ctx, scope.tc, handle_,
                                                  depth, &branch),
                           "hegel_recursion_branch");
        return branch;
    }

    void RecursionHandle::leaf(const TestCase& tc) {
        impl::DrawScope scope(tc);
        hegel_result_t rc = hegel_recursion_leaf(scope.ctx, scope.tc, handle_);
        if (rc == HEGEL_E_RETRY) {
            throw LeafBudgetExceeded();
        }
        scope.raise_for_rc(rc, "hegel_recursion_leaf");
    }

    void RecursionHandle::retry(const TestCase& tc) {
        impl::DrawScope scope(tc);
        scope.raise_for_rc(hegel_recursion_retry(scope.ctx, scope.tc, handle_),
                           "hegel_recursion_retry");
    }

    void RecursionHandle::finish(const TestCase& tc) {
        impl::DrawScope scope(tc);
        hegel_result_t rc =
            hegel_recursion_finish(scope.ctx, scope.tc, handle_);
        if (rc == HEGEL_E_RETRY) {
            throw AttemptMispriced();
        }
        scope.raise_for_rc(rc, "hegel_recursion_finish");
    }

    StateMachineHandle::StateMachineHandle(
        const TestCase& tc, const std::vector<std::string>& rule_names,
        const std::vector<int64_t>& rule_groups,
        const std::vector<double>& rule_weights,
        const std::vector<std::string>& invariant_names,
        const std::vector<bool>& invariant_always_check,
        int64_t min_concurrency, int64_t max_concurrency, int64_t step_count) {
        impl::DrawScope scope(tc);

        auto to_strings = [](const std::vector<std::string>& names) {
            std::vector<hegel_string_t> strings;
            strings.reserve(names.size());
            for (const std::string& name : names) {
                strings.push_back(hegel_string_t{name.data(), name.size()});
            }
            return strings;
        };

        std::vector<hegel_string_t> rule_name_strings = to_strings(rule_names);
        std::vector<hegel_string_t> invariant_name_strings =
            to_strings(invariant_names);

        auto always_check =
            std::make_unique<bool[]>(invariant_always_check.size());
        std::copy(invariant_always_check.begin(), invariant_always_check.end(),
                  always_check.get());

        scope.raise_for_rc(hegel_new_state_machine(
                               scope.ctx, scope.tc, rule_name_strings.data(),
                               rule_groups.data(), rule_weights.data(),
                               rule_names.size(), invariant_name_strings.data(),
                               always_check.get(), invariant_names.size(),
                               min_concurrency, max_concurrency, step_count,
                               &handle_, &concurrency_),
                           "hegel_new_state_machine");
    }

    StateMachineHandle::~StateMachineHandle() {
        hegel_state_machine_free(impl::thread_context(), handle_);
    }

    int64_t StateMachineHandle::next_group(const TestCase& tc) {
        impl::DrawScope scope(tc);
        int64_t group_id;

        scope.raise_for_rc(hegel_state_machine_next_group(scope.ctx, scope.tc,
                                                          handle_, &group_id),
                           "hegel_state_machine_next_group");
        return group_id;
    }

    int64_t StateMachineHandle::next_rule(const TestCase& tc,
                                          int64_t worker_index) {
        impl::DrawScope scope(tc);
        int64_t rule_idx;

        scope.raise_for_rc(hegel_state_machine_next_rule(scope.ctx, scope.tc,
                                                         handle_, worker_index,
                                                         &rule_idx),
                           "hegel_state_machine_next_rule");
        return rule_idx;
    }

    void StateMachineHandle::rule_rejected(const TestCase& tc,
                                           int64_t worker_index) {
        impl::DrawScope scope(tc);
        scope.raise_for_rc(hegel_state_machine_rule_rejected(
                               scope.ctx, scope.tc, handle_, worker_index),
                           "hegel_state_machine_rule_rejected");
    }

    bool StateMachineHandle::should_check_invariant(const TestCase& tc,
                                                    int64_t invariant_index) {
        impl::DrawScope scope(tc);
        bool should_check = false;
        scope.raise_for_rc(
            hegel_state_machine_should_check_invariant(
                scope.ctx, scope.tc, handle_, invariant_index, &should_check),
            "hegel_state_machine_should_check_invariant");
        return should_check;
    }

} // namespace hegel::internal

namespace hegel {

    namespace {

        void printer_call(hegel_context_t* ctx, hegel_result_t rc) {
            if (rc == HEGEL_E_INVALID_ARG) {
                throw std::logic_error(impl::last_error(ctx));
            }
            impl::tolerate_dead_region(ctx, rc);
        }

    } // namespace

    Printer Printer::noop() { return Printer(nullptr, false); }

    Printer::Printer(Printer&& other) noexcept
        : handle_(std::exchange(other.handle_, nullptr)),
          owned_(std::exchange(other.owned_, false)) {}

    Printer& Printer::operator=(Printer&& other) noexcept {
        std::swap(handle_, other.handle_);
        std::swap(owned_, other.owned_);
        return *this;
    }

    Printer::~Printer() {
        if (owned_) {
            hegel_printer_free(impl::thread_context(), handle_);
        }
    }

    bool Printer::should_print() const {
        if (handle_ == nullptr) {
            return false;
        }
        bool live = false;
        hegel_printer_is_live(impl::thread_context(), handle_, &live);
        return live;
    }

    void Printer::text(std::string_view text) {
        if (handle_ == nullptr) {
            return;
        }
        hegel_context_t* ctx = impl::thread_context();
        std::size_t start = 0;
        while (true) {
            std::size_t end = text.find('\n', start);
            std::string_view segment = text.substr(start, end - start);
            if (!segment.empty()) {
                printer_call(ctx, hegel_printer_text(ctx, handle_,
                                                     impl::borrow(segment)));
            }
            if (end == std::string_view::npos) {
                return;
            }
            printer_call(ctx, hegel_printer_hard_break(ctx, handle_));
            start = end + 1;
        }
    }

    void Printer::reflow(std::string_view text) {
        if (handle_ == nullptr) {
            return;
        }
        hegel_context_t* ctx = impl::thread_context();
        printer_call(ctx, hegel_printer_reflow(ctx, handle_, nullptr,
                                               impl::borrow(text)));
    }

    void Printer::breakable(std::string_view separator) {
        if (handle_ == nullptr) {
            return;
        }
        hegel_context_t* ctx = impl::thread_context();
        printer_call(ctx, hegel_printer_breakable(ctx, handle_,
                                                  impl::borrow(separator)));
    }

    void Printer::if_break(std::string_view text) {
        if (handle_ == nullptr) {
            return;
        }
        hegel_context_t* ctx = impl::thread_context();
        printer_call(ctx,
                     hegel_printer_if_break(ctx, handle_, impl::borrow(text)));
    }

    void Printer::hard_break() {
        if (handle_ == nullptr) {
            return;
        }
        hegel_context_t* ctx = impl::thread_context();
        printer_call(ctx, hegel_printer_hard_break(ctx, handle_));
    }

    void Printer::begin_group(std::size_t indent, std::string_view open) {
        if (handle_ == nullptr) {
            return;
        }
        hegel_context_t* ctx = impl::thread_context();
        printer_call(ctx, hegel_printer_begin_group(ctx, handle_, indent,
                                                    impl::borrow(open)));
    }

    void Printer::end_group(std::string_view close) {
        if (handle_ == nullptr) {
            return;
        }
        hegel_context_t* ctx = impl::thread_context();
        printer_call(
            ctx, hegel_printer_end_group(ctx, handle_, impl::borrow(close)));
    }

    void Printer::separator() {
        text(",");
        breakable(" ");
    }

    void Printer::shift_indent(std::ptrdiff_t delta) {
        if (handle_ == nullptr) {
            return;
        }
        hegel_context_t* ctx = impl::thread_context();
        printer_call(ctx, hegel_printer_shift_indent(
                              ctx, handle_, static_cast<int64_t>(delta)));
    }

    void Printer::comment(std::string_view text) {
        if (handle_ == nullptr) {
            return;
        }
        hegel_context_t* ctx = impl::thread_context();
        std::string rendered = "  // " + std::string(text);
        printer_call(
            ctx, hegel_printer_comment(ctx, handle_, impl::borrow(rendered)));
    }

    void Printer::begin_block(std::string_view open) {
        begin_group(4, open);
        breakable("");
    }

    void Printer::end_block(std::string_view close) {
        shift_indent(-4);
        breakable("");
        shift_indent(4);
        end_group(close);
    }

    Speculation Printer::speculate() {
        if (handle_ != nullptr) {
            hegel_context_t* ctx = impl::thread_context();
            printer_call(ctx, hegel_printer_begin_speculative(ctx, handle_));
        }
        return Speculation(handle_);
    }

    Printer Printer::deferred() {
        if (handle_ == nullptr) {
            return noop();
        }
        hegel_printer_t* child = nullptr;
        hegel_context_t* ctx = impl::thread_context();
        printer_call(ctx, hegel_printer_deferred(ctx, handle_, &child));
        return Printer(child, child != nullptr);
    }

    Speculation::Speculation(Speculation&& other) noexcept
        : handle_(other.handle_),
          resolved_(std::exchange(other.resolved_, true)) {}

    Speculation::~Speculation() {
        if (!resolved_ && handle_ != nullptr) {
            hegel_printer_abort_speculative(impl::thread_context(), handle_);
        }
    }

    void Speculation::commit() { close(true); }

    void Speculation::abort() { close(false); }

    void Speculation::close(bool keep) {
        bool was_resolved = std::exchange(resolved_, true);
        if (was_resolved || handle_ == nullptr) {
            return;
        }
        hegel_context_t* ctx = impl::thread_context();
        printer_call(ctx, keep ? hegel_printer_commit_speculative(ctx, handle_)
                               : hegel_printer_abort_speculative(ctx, handle_));
    }

    Document::Document(std::size_t max_width) : printer_(Printer::noop()) {
        hegel_context_t* ctx = impl::thread_context();
        hegel_printer_options_t* options = nullptr;
        impl::check_rc(ctx, hegel_printer_options_new(ctx, &options));
        hegel_result_t rc =
            hegel_printer_options_set_max_width(ctx, options, max_width);
        hegel_printer_t* handle = nullptr;
        if (rc == HEGEL_OK) {
            rc = hegel_printer_new(ctx, options, &handle);
        }
        hegel_printer_options_free(ctx, options);
        impl::check_rc(ctx, rc);
        printer_ = Printer(handle, true);
    }

    std::string Document::finish() && {
        return impl::read_document(impl::thread_context(), printer_.handle());
    }

} // namespace hegel
