#pragma once

/**
 * @cond INTERNAL
 *
 * Thin helpers over the libhegel C ABI shared by the run loop (hegel.cpp)
 * and the draw primitives (engine.cpp, generators.cpp).
 */

#include <array>
#include <cstddef>
#include <cstdint>
#include <hegel.h>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
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

    /// The span label for a generator kind: a stable hash of `name`.
    uint64_t label_from_name(hegel_context_t* ctx, std::string_view name);

    hegel_run_t* run_start(hegel_context_t* ctx, hegel_settings_t* s);
    hegel_test_case_t* test_case_from_blob(hegel_context_t* ctx,
                                           hegel_settings_t* s,
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
    hegel_run_result_t* run_result(hegel_context_t* ctx, hegel_run_t* run);
    hegel_run_status_t run_result_status(hegel_context_t* ctx,
                                         const hegel_run_result_t* result);
    std::optional<std::string>
    run_result_error(hegel_context_t* ctx, const hegel_run_result_t* result);
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

    // Date and time option handles. Each is built once at generator
    // construction from parameters the engine validates, throwing
    // std::invalid_argument (with the engine's diagnostic) on rejection.
    // The returned handle is immutable, shareable across test cases and
    // threads, and must be released with the matching *_free.

    hegel_date_options_t* date_options_new(uint32_t weekday_mask,
                                           uint32_t month_mask);
    void date_options_free(hegel_date_options_t* options);

    hegel_timezone_spec_t*
    timezone_spec_new(const std::optional<std::vector<std::string>>& names,
                      bool links, bool legacy, bool unknown, bool opaque);
    void timezone_spec_free(hegel_timezone_spec_t* spec);

    struct ZonedSpecParams {
        uint32_t precision = HEGEL_PRECISION_NANOSECONDS;
        std::optional<std::pair<hegel_datetime_t, hegel_datetime_t>>
            civil_bounds;
        std::optional<std::pair<hegel_timestamp_t, hegel_timestamp_t>>
            instant_bounds;
        uint32_t imaginary = HEGEL_IMAGINARY_ALLOW;
        uint32_t ambiguous = HEGEL_AMBIGUOUS_BOTH;
    };
    hegel_zoned_spec_t* zoned_spec_new(const ZonedSpecParams& params);
    void zoned_spec_free(hegel_zoned_spec_t* spec);

    struct SpanUnitBounds {
        int64_t min_value;
        int64_t max_value;
    };
    /// `bounds` is indexed by hegel_span_unit_t; a unit without bounds is
    /// not enabled.
    hegel_span_spec_t*
    span_spec_new(const std::array<std::optional<SpanUnitBounds>, 10>& bounds,
                  uint32_t sign);
    void span_spec_free(hegel_span_spec_t* spec);

    // Queries of the bundled time zone database. A rejected argument
    // throws std::invalid_argument with the engine's diagnostic.

    std::string tzdb_version();
    hegel_timezone_info_t timezone_info(std::string_view name);
    hegel_timezone_resolution_t timezone_resolve(std::string_view name,
                                                 hegel_datetime_t civil);
    hegel_timezone_offset_t timezone_offset_at(std::string_view name,
                                               hegel_timestamp_t timestamp);

    // Draw primitives used by the non-template generators in
    // generators.cpp and generators_datetime.cpp. (The template-visible
    // ones live in hegel::internal — see include/hegel/internal.h.)

    void target(const TestCase& tc, double score, std::string_view label);

    std::string draw_string(const TestCase& tc,
                            const hegel_string_generator_t* generator);
    std::vector<uint8_t> draw_bytes(const TestCase& tc, uint64_t min_size,
                                    uint64_t max_size);
    hegel_date_t draw_date(const TestCase& tc, hegel_date_t min_value,
                           hegel_date_t max_value,
                           const hegel_date_options_t* options);
    hegel_time_t draw_time(const TestCase& tc, hegel_time_t min_value,
                           hegel_time_t max_value, uint32_t precision);
    hegel_datetime_t draw_datetime(const TestCase& tc,
                                   hegel_datetime_t min_value,
                                   hegel_datetime_t max_value,
                                   uint32_t precision);
    hegel_timestamp_t draw_timestamp(const TestCase& tc,
                                     hegel_timestamp_t min_value,
                                     hegel_timestamp_t max_value,
                                     uint32_t precision);
    hegel_offset_t draw_offset(const TestCase& tc, int32_t min_seconds,
                               int32_t max_seconds, uint32_t granularity,
                               bool allow_unknown);
    hegel_duration_t draw_duration(const TestCase& tc,
                                   hegel_duration_t min_value,
                                   hegel_duration_t max_value,
                                   uint32_t precision);
    std::string draw_timezone(const TestCase& tc,
                              const hegel_timezone_spec_t* spec);
    struct ZonedDraw {
        hegel_zoned_datetime_t value;
        std::string zone;
    };
    ZonedDraw draw_zoned_datetime(const TestCase& tc,
                                  const hegel_timezone_spec_t* tz_spec,
                                  const hegel_zoned_spec_t* zoned_spec);
    hegel_span_t draw_span(const TestCase& tc, const hegel_span_spec_t* spec);
    std::string draw_uuid(const TestCase& tc, std::optional<uint8_t> version);
    /// Dotted-quad (v4) / RFC 5952 colon-hex (v6) text form.
    std::string draw_ipv4(const TestCase& tc);
    std::string draw_ipv6(const TestCase& tc);

} // namespace hegel::impl

/// @endcond
