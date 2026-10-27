#pragma once

/**
 * @cond INTERNAL
 *
 * Thin helpers over the libhegel C ABI shared by the run loop (hegel.cpp)
 * and the draw primitives (engine.cpp, generators.cpp).
 */

#include <cstddef>
#include <cstdint>
#include <functional>
#include <hegel.h>
#include <hegel/settings.h>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace hegel {
    class TestCase;
}

namespace hegel::impl {

    /// This thread's libhegel error-reporting context, created on first use
    /// and freed when the thread exits.
    hegel_context_t* thread_context();

    /// Most recent error message recorded on `ctx` (empty string if none).
    std::string last_error(hegel_context_t* ctx);

    /// The readable form of a typeid name.
    std::string demangle(const char* name);

    // Safe wrappers over the libhegel C ABI: each forwards to its `hegel_*`
    // entry point and routes the return code through an internal check that
    // throws std::runtime_error (with the context diagnostic) on failure.
    // Out-parameter calls return the produced value directly.

    void settings_set_test_cases(hegel_context_t* ctx, hegel_settings_t* s,
                                 uint64_t test_cases);
    void settings_set_verbosity(hegel_context_t* ctx, hegel_settings_t* s,
                                hegel_verbosity_t verbosity);
    void settings_set_seed(hegel_context_t* ctx, hegel_settings_t* s,
                           uint64_t seed, bool has_seed);
    void settings_set_derandomize(hegel_context_t* ctx, hegel_settings_t* s,
                                  bool derandomize);
    void settings_set_report_multiple_failures(hegel_context_t* ctx,
                                               hegel_settings_t* s, bool yes);
    void settings_set_stop_on_first_failure(hegel_context_t* ctx,
                                            hegel_settings_t* s, bool yes);
    void settings_set_nondeterminism_strictness(hegel_context_t* ctx,
                                                hegel_settings_t* s,
                                                uint32_t strictness);
    void settings_set_database(hegel_context_t* ctx, hegel_settings_t* s,
                               std::string_view path);
    void settings_set_database_key(hegel_context_t* ctx, hegel_settings_t* s,
                                   std::string_view key);
    void settings_set_suppress_health_check(hegel_context_t* ctx,
                                            hegel_settings_t* s, uint32_t mask);
    void settings_set_phases(hegel_context_t* ctx, hegel_settings_t* s,
                             uint32_t phases);
    void settings_set_backend(hegel_context_t* ctx, hegel_settings_t* s,
                              hegel_backend_t backend);
    /// Where the test lives. Inside Antithesis the engine reports the run's
    /// verdict under `<class_name>::<function>`; elsewhere it is unused.
    void settings_set_test_location(hegel_context_t* ctx, hegel_settings_t* s,
                                    std::string_view file, uint32_t begin_line,
                                    std::string_view class_name,
                                    std::string_view function);
    /// A handle initialized from the engine's default settings profile.
    hegel_settings_t* settings_new(hegel_context_t* ctx);
    /// A handle initialized from the named settings profile. An unknown
    /// profile throws std::invalid_argument.
    hegel_settings_t* settings_new_for_profile(hegel_context_t* ctx,
                                               std::string_view name);
    void settings_register_profile(hegel_context_t* ctx, std::string_view name,
                                   const hegel_settings_t* s);
    void set_default_profile(hegel_context_t* ctx, std::string_view name);
    std::string engine_version(hegel_context_t* ctx);
    void settings_set_show_statistics(hegel_context_t* ctx, hegel_settings_t* s,
                                      bool yes);
    void settings_set_unbounded_choices(hegel_context_t* ctx,
                                        hegel_settings_t* s, bool yes);
    void settings_set_print_blob(hegel_context_t* ctx, hegel_settings_t* s,
                                 bool yes);
    /// Selects the engine's default database location.
    void settings_set_default_database(hegel_context_t* ctx,
                                       hegel_settings_t* s);
    hegel_verbosity_t settings_get_verbosity(hegel_context_t* ctx,
                                             const hegel_settings_t* s);
    bool settings_get_print_blob(hegel_context_t* ctx,
                                 const hegel_settings_t* s);
    bool settings_get_report_multiple_failures(hegel_context_t* ctx,
                                               const hegel_settings_t* s);
    /// The enabled phases, as a bitwise OR of hegel_phase_t values.
    uint32_t settings_get_phases(hegel_context_t* ctx,
                                 const hegel_settings_t* s);

    /// The span label for a generator kind: a stable hash of `name`.
    uint64_t label_from_name(hegel_context_t* ctx, std::string_view name);
    uint64_t label_combine(hegel_context_t* ctx,
                           const std::vector<uint64_t>& labels);

    hegel_run_t* run_start(hegel_context_t* ctx, hegel_settings_t* s);
    /// A run that replays `blob`. An invalid blob throws
    /// std::invalid_argument.
    hegel_run_t* run_start_blob(hegel_context_t* ctx, hegel_settings_t* s,
                                std::string_view blob);
    /// NULL once the engine has no more cases to hand out. The returned
    /// handle is caller-owned; release it with hegel_test_case_free.
    hegel_test_case_t* next_test_case(hegel_context_t* ctx, hegel_run_t* run);

    /// Fork an independent draw stream of `tc` (shared outcome/budget). The
    /// returned handle is caller-owned; release it with hegel_test_case_free.
    hegel_test_case_t* test_case_clone(hegel_context_t* ctx,
                                       const hegel_test_case_t* tc);
    /// Whether the engine stamped `tc` for capture: its output and, if it
    /// fails, its diagnostic are the material for that origin's report.
    bool test_case_should_capture(hegel_context_t* ctx,
                                  const hegel_test_case_t* tc);
    void mark_complete(hegel_context_t* ctx, hegel_test_case_t* tc,
                       hegel_status_t status,
                       const std::optional<std::string>& origin);

    /// Caller-owned result snapshot; release with hegel_run_result_free.
    hegel_test_case_t* test_case_block(hegel_context_t* ctx,
                                       const hegel_test_case_t* tc,
                                       uint64_t indent);
    void test_case_set_worker(hegel_context_t* ctx, const hegel_test_case_t* tc,
                              int64_t worker_index);
    hegel_printer_t* test_case_printer(hegel_context_t* ctx,
                                       hegel_test_case_t* tc);
    // Appends a note to `tc`'s region of its document. A note on a region
    // whose document was already read is dropped.
    void note(hegel_context_t* ctx, hegel_test_case_t* tc,
              std::string_view text);
    void printer_free(hegel_context_t* ctx, hegel_printer_t* printer);
    // Lays out and returns the document `root` (a document's root handle)
    // belongs to. Seals the document.
    std::string read_document(hegel_context_t* ctx, hegel_printer_t* root);
    hegel_run_result_t* run_result(hegel_context_t* ctx, hegel_run_t* run);
    hegel_run_status_t run_result_status(hegel_context_t* ctx,
                                         const hegel_run_result_t* result);
    std::optional<std::string>
    run_result_error(hegel_context_t* ctx, const hegel_run_result_t* result);
    struct RunErrorKind {
        hegel_run_error_kind_t kind;
        /// The hegel_health_check_t of a failed health check, else 0.
        uint32_t health_check;
    };
    RunErrorKind run_result_error_kind(hegel_context_t* ctx,
                                       const hegel_run_result_t* result);
    /// The HealthCheck for one hegel_health_check_t flag, or nullopt for
    /// any other value.
    std::optional<HealthCheck> health_check_from_flag(uint32_t flag);
    /// Throws the RunError subclass for `kind`, with `message`.
    [[noreturn]] void raise_run_error(const std::string& message,
                                      RunErrorKind kind);
    size_t run_result_failure_count(hegel_context_t* ctx,
                                    const hegel_run_result_t* result);
    /// Caller-owned failure snapshot; release with hegel_failure_free.
    hegel_failure_t* run_result_failure(hegel_context_t* ctx,
                                        const hegel_run_result_t* result,
                                        size_t index);
    /// The base64 blob that replays the failure, or nullopt when the engine
    /// produced none for it.
    std::optional<std::string>
    failure_reproduction_blob(hegel_context_t* ctx,
                              const hegel_failure_t* failure);
    /// The origin string the engine grouped this failure's probes under.
    std::string failure_origin(hegel_context_t* ctx,
                               const hegel_failure_t* failure);
    /// The failure's standing under the run's nondeterministic handling, or
    /// nullopt for a deterministic failure.
    std::optional<std::string> failure_caveat(hegel_context_t* ctx,
                                              const hegel_failure_t* failure);

    // String-generator construction. Each validates its parameters
    // immediately and throws std::invalid_argument (with the engine's
    // diagnostic) on rejection. The returned handle is immutable, shareable
    // across test cases, and must be released with string_generator_free.

    struct TextGeneratorParams {
        uint64_t min_size = 0;
        uint64_t max_size = 0;
        const char* codec = nullptr; // nullptr = all of Unicode
        uint32_t min_codepoint = 0;
        uint32_t max_codepoint = UINT32_MAX;
        const std::vector<std::string>* categories = nullptr;
        const std::vector<std::string>* exclude_categories = nullptr;
        const std::string* include_characters = nullptr;
        const std::string* exclude_characters = nullptr;
    };

    hegel_string_generator_t*
    string_generator_text(const TextGeneratorParams& params);
    hegel_string_generator_t* string_generator_regex(std::string_view pattern,
                                                     bool fullmatch);
    hegel_string_generator_t* string_generator_email();
    hegel_string_generator_t* string_generator_url();
    hegel_string_generator_t* string_generator_domain(uint64_t max_length);
    void string_generator_free(hegel_string_generator_t* generator);

    // Draw primitives used by the non-template generators in
    // generators.cpp. (The template-visible ones live in
    // hegel::internal — see include/hegel/internal.h.)

    void target(const TestCase& tc, double score, std::string_view label);
    void event(const TestCase& tc, std::string_view label);
    void event_value(const TestCase& tc, std::string_view label, double value);

    std::string draw_string(const TestCase& tc,
                            const hegel_string_generator_t* generator);
    std::vector<uint8_t> draw_bytes(const TestCase& tc, uint64_t min_size,
                                    uint64_t max_size);
    hegel_date_t draw_date(const TestCase& tc);
    hegel_time_t draw_time(const TestCase& tc);
    hegel_datetime_t draw_datetime(const TestCase& tc);
    std::string draw_uuid(const TestCase& tc, std::optional<uint8_t> version);
    /// Dotted-quad (v4) / RFC 5952 colon-hex (v6) text form.
    std::string draw_ipv4(const TestCase& tc);
    std::string draw_ipv6(const TestCase& tc);

    // Grammar construction and drawing.

    struct GrammarParams {
        std::string_view text;
        std::optional<std::string> start;
        uint64_t max_size = 0;  // 0 = the engine's default
        uint32_t max_depth = 0; // 0 = the engine's default
        const hegel_string_generator_t* alphabet = nullptr;
        std::vector<std::string> explicit_names;
        // The directory %import-ed grammar files are read from; with none,
        // every imported file is missing.
        std::optional<std::string> import_dir;
    };

    /// Compiles a grammar. A grammar the engine rejects, or an imported file
    /// that exists but cannot be read, throws std::invalid_argument. The
    /// returned handle is immutable and shareable across test cases; release
    /// it with grammar_free.
    hegel_grammar_t* grammar_new(const GrammarParams& params);
    void grammar_free(hegel_grammar_t* grammar);
    /// Draws a string of `grammar`, with the value of each occurrence of an
    /// explicit terminal from `value`, called with the terminal's index in
    /// GrammarParams::explicit_names.
    std::string draw_grammar(const TestCase& tc, const hegel_grammar_t* grammar,
                             const std::function<std::string(size_t)>& value);

} // namespace hegel::impl

/// @endcond
