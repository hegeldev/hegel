/*
 * temporal.c — the temporal part of the API from C.
 *
 * Builds a zone source holding one zone given as fields and one name whose
 * data a loader supplies, draws zoned datetimes from it both ways (a zone
 * the engine draws, and a zone the caller names), checks each against
 * hegel_timezone_resolve_in, and prints values with the hegel_format_*
 * functions. It passes the temporal structs by value and by pointer from a
 * C compiler, so a layout the header and the library disagree on shows up
 * here as a wrong value.
 *
 * Build (from this directory, after `cargo build -p hegeltest-c --release`):
 *
 *   cc -o temporal temporal.c \
 *      -I../include \
 *      ../../target/release/libhegel_c.a -lpthread -ldl -lm -lrt
 *
 * Run:
 *   ./temporal
 */

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "hegel.h"
#include "hegel_check.h"

static int failures = 0;

static void expect_text(hegel_context_t *ctx, const char *what, hegel_owned_string_t *text, const char *expected) {
    if (text->len != strlen(expected) || memcmp(text->data, expected, text->len) != 0) {
        fprintf(stderr, "%s: expected \"%s\", got \"%.*s\"\n", what, expected, (int)text->len, text->data);
        failures++;
    }
    HEGEL_CHECK(hegel_owned_string_free, ctx, text);
}

static void expect(int condition, const char *what) {
    if (!condition) {
        fprintf(stderr, "expected %s\n", what);
        failures++;
    }
}

typedef struct {
    int loads;
    int releases;
} loader_state_t;

/* The loader: every name it is asked for is a link to the zone the source
   already holds. A nonzero return would fail the draw that needed the name. */
static int32_t load_zone(void *user_data, hegel_context_t *ctx, hegel_zone_source_t *source, hegel_string_t name) {
    loader_state_t *state = user_data;
    state->loads++;
    return hegel_zone_source_add_link(ctx, source, name, STRING("Test/Step")) == HEGEL_OK ? 0 : 1;
}

static void release_loader(void *user_data) {
    loader_state_t *state = user_data;
    state->releases++;
}

static void check_against_resolve(hegel_context_t *ctx, const hegel_zone_source_t *source, hegel_string_t name,
                                  const hegel_zoned_datetime_t *value) {
    hegel_timezone_resolution_t resolved;
    HEGEL_CHECK(hegel_timezone_resolve_in, ctx, source, name, value->civil, &resolved);
    expect(resolved.kind == value->kind, "the draw and the query to agree on the kind");
    expect(resolved.offset_before == value->offset_before, "the same offset before");
    expect(resolved.offset_after == value->offset_after, "the same offset after");
    expect(value->offset_known, "a known offset");
    expect(value->civil.date.year == 2001 && value->civil.date.month == 9, "a wall time within the bounds");
    if (!value->fold) {
        expect(resolved.timestamp.second == value->timestamp.second, "the same instant");
    }
}

int main(void) {
    hegel_context_t *ctx = hegel_context_new();
    hegel_owned_string_t text;

    HEGEL_CHECK(hegel_format_date, ctx, (hegel_date_t){2024, 3, 10}, &text);
    expect_text(ctx, "date", &text, "2024-03-10");
    HEGEL_CHECK(hegel_format_time, ctx, (hegel_time_t){2, 30, 0, 500000000}, &text);
    expect_text(ctx, "time", &text, "02:30:00.5");
    HEGEL_CHECK(hegel_format_datetime, ctx, (hegel_datetime_t){{-1, 12, 31}, {23, 59, 59, 0}}, &text);
    expect_text(ctx, "datetime", &text, "-000001-12-31T23:59:59");
    HEGEL_CHECK(hegel_format_timestamp, ctx, (hegel_timestamp_t){-2, 500000000}, &text);
    expect_text(ctx, "timestamp", &text, "1969-12-31T23:59:58.5Z");
    HEGEL_CHECK(hegel_format_offset, ctx, (hegel_offset_t){-2670, true}, &text);
    expect_text(ctx, "offset", &text, "-00:44:30");
    HEGEL_CHECK(hegel_format_offset, ctx, (hegel_offset_t){0, false}, &text);
    expect_text(ctx, "unknown offset", &text, "-00:00");
    HEGEL_CHECK(hegel_format_duration, ctx, (hegel_duration_t){5405, 500000000}, &text);
    expect_text(ctx, "duration", &text, "PT1H30M5.5S");
    hegel_span_t span = {0};
    span.days = -3;
    span.hours = 4;
    HEGEL_CHECK(hegel_format_span, ctx, &span, &text);
    expect_text(ctx, "span", &text, "-3d +4h");

    /* The arguments of a draw can be checked where a generator is built. */
    hegel_datetime_t earliest = {{2001, 9, 1}, {0, 0, 0, 0}};
    hegel_datetime_t latest = {{2001, 9, 30}, {23, 59, 59, 0}};
    HEGEL_CHECK(hegel_validate_datetime, ctx, earliest, latest, HEGEL_PRECISION_SECONDS);
    expect(hegel_validate_datetime(ctx, latest, earliest, HEGEL_PRECISION_SECONDS) == HEGEL_E_INVALID_ARG,
           "inverted bounds to be rejected");
    expect(hegel_validate_offset(ctx, -3600, 3600, HEGEL_OFFSET_GRANULARITY_HOURS, true) == HEGEL_OK,
           "whole-hour offsets to be accepted");

    /* A zone at UT until 2001-09-09T01:46:40Z and an hour ahead from then. */
    hegel_zone_transition_t transitions[] = {
        {1000000000, {3600, false, STRING("BBB")}},
    };
    hegel_zone_data_t data = {0};
    data.initial = (hegel_zone_type_t){0, false, STRING("AAA")};
    data.transitions = transitions;
    data.transition_count = 1;
    data.after = HEGEL_ZONE_AFTER_HOLD;

    hegel_zone_source_t *source;
    HEGEL_CHECK(hegel_zone_source_new, ctx, &source);
    HEGEL_CHECK(hegel_zone_source_add_zone, ctx, source, STRING("Test/Step"), &data);
    hegel_string_t declared[] = {STRING("Test/Lazy")};
    HEGEL_CHECK(hegel_zone_source_declare, ctx, source, declared, 1);
    loader_state_t state = {0, 0};
    HEGEL_CHECK(hegel_zone_source_set_loader, ctx, source, load_zone, &state, release_loader);

    hegel_timezone_spec_t *zones;
    HEGEL_CHECK(hegel_timezone_spec_new, ctx, &zones);
    HEGEL_CHECK(hegel_timezone_spec_set_source, ctx, zones, source);
    HEGEL_CHECK(hegel_timezone_spec_validate, ctx, zones);

    hegel_zoned_spec_t *spec;
    HEGEL_CHECK(hegel_zoned_spec_new, ctx, &spec);
    HEGEL_CHECK(hegel_zoned_spec_set_precision, ctx, spec, HEGEL_PRECISION_SECONDS);
    HEGEL_CHECK(hegel_zoned_spec_set_civil_bounds, ctx, spec, earliest, latest);
    HEGEL_CHECK(hegel_zoned_spec_validate, ctx, spec);

    hegel_settings_t *s;
    HEGEL_CHECK(hegel_settings_new, ctx, &s);
    HEGEL_CHECK(hegel_settings_set_test_cases, ctx, s, 50);
    HEGEL_CHECK(hegel_settings_set_database, ctx, s, &STRING(""));
    HEGEL_CHECK(hegel_settings_set_derandomize, ctx, s, true);
    hegel_run_t *run;
    HEGEL_CHECK(hegel_run_start, ctx, s, NULL, NULL, &run);

    int cases = 0;
    for (;;) {
        hegel_test_case_t *tc;
        HEGEL_CHECK(hegel_next_test_case, ctx, run, &tc);
        if (!tc) {
            break;
        }
        cases++;

        hegel_zoned_datetime_t value;
        hegel_owned_string_t zone;
        hegel_result_t rc = hegel_generate_zoned_datetime(ctx, tc, zones, spec, &value, &zone);
        if (rc == HEGEL_OK) {
            hegel_string_t name = {zone.data, zone.len};
            check_against_resolve(ctx, source, name, &value);
            HEGEL_CHECK(hegel_format_zoned_datetime, ctx, &value, name, &text);
            if (cases == 1) {
                printf("drew %.*s\n", (int)text.len, text.data);
            }
            HEGEL_CHECK(hegel_owned_string_free, ctx, &text);
            HEGEL_CHECK(hegel_owned_string_free, ctx, &zone);
            rc = hegel_generate_zoned_datetime_for_zone(ctx, tc, source, STRING("Test/Step"), spec, &value);
        }
        if (rc == HEGEL_OK) {
            check_against_resolve(ctx, source, STRING("Test/Step"), &value);
        } else if (rc != HEGEL_E_STOP_TEST) {
            hegel_string_t err = hegel_context_last_error(ctx);
            fprintf(stderr, "a zoned draw failed: rc=%d %.*s\n", (int)rc, (int)err.len, err.data);
            failures++;
        }
        HEGEL_CHECK(hegel_mark_complete, ctx, tc, rc == HEGEL_OK ? HEGEL_STATUS_VALID : HEGEL_STATUS_OVERRUN, NULL);
        HEGEL_CHECK(hegel_test_case_free, ctx, tc);
    }
    expect(cases > 0, "at least one test case");
    expect(state.loads == 1, "the lazy name to load once");

    hegel_zoned_datetime_t gap = {0};
    gap.civil = (hegel_datetime_t){{2001, 9, 9}, {2, 0, 0, 0}};
    gap.offset_seconds = 3600;
    gap.offset_known = true;
    gap.kind = HEGEL_TIMEZONE_KIND_GAP;
    HEGEL_CHECK(hegel_format_zoned_datetime, ctx, &gap, STRING("Test/Step"), &text);
    expect_text(ctx, "zoned datetime", &text, "2001-09-09T02:00:00+01:00[Test/Step][gap]");

    HEGEL_CHECK(hegel_run_free, ctx, run);
    HEGEL_CHECK(hegel_settings_free, ctx, s);
    HEGEL_CHECK(hegel_zoned_spec_free, ctx, spec);
    HEGEL_CHECK(hegel_zone_source_free, ctx, source);
    expect(state.releases == 0, "the loader to live while a spec uses its source");
    HEGEL_CHECK(hegel_timezone_spec_free, ctx, zones);
    expect(state.releases == 1, "the loader to be released once with its source");
    HEGEL_CHECK(hegel_context_free, ctx);

    if (failures) {
        return 1;
    }
    printf("temporal: %d test cases\n", cases);
    return 0;
}
