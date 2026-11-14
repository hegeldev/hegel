mod common;

use common::utils::expect_panic;
use hegel::generators::{self as gs, Generator};
use hegel::{Hegel, Settings};

fn expect_draw_panic<T, G>(generator: G, pattern: &str)
where
    G: Generator<T> + 'static,
    T: std::fmt::Debug + Send + 'static,
{
    let generator = std::panic::AssertUnwindSafe(generator);
    expect_panic(
        move || {
            let generator = generator;
            Hegel::new(move |tc| {
                tc.draw_silent(&generator.0);
            })
            .settings(Settings::new().test_cases(1).database(None))
            .run();
        },
        pattern,
    );
}

fn check_draws<T, G>(generator: G)
where
    G: Generator<T> + 'static,
    T: std::fmt::Debug + Send + 'static,
{
    Hegel::new(move |tc| {
        tc.draw_silent(&generator);
    })
    .settings(Settings::new().test_cases(5).database(None))
    .run();
}

#[test]
fn test_integers_min_greater_than_max() {
    expect_draw_panic(
        gs::integers::<i32>().min_value(10).max_value(5),
        "max_value < min_value",
    );
}

#[test]
fn test_floats_pos_zero_min_neg_zero_max() {
    expect_draw_panic(
        gs::floats::<f64>().min_value(0.0).max_value(-0.0),
        "InvalidArgument",
    );
}

#[test]
fn test_floats_pos_zero_min_neg_zero_max_f32() {
    expect_draw_panic(
        gs::floats::<f32>().min_value(0.0).max_value(-0.0),
        "InvalidArgument",
    );
}

#[test]
fn test_text_min_greater_than_max() {
    expect_draw_panic(gs::text().min_size(5).max_size(3), "max_size < min_size");
}

#[test]
fn test_text_character_params_draw() {
    check_draws(gs::text().codec("ascii"));
    check_draws(gs::text().min_codepoint(0x20).max_codepoint(0x7E));
    check_draws(gs::text().categories(&["L", "Nd"]));
    check_draws(gs::text().exclude_categories(&["Cc"]));
    check_draws(gs::text().include_characters("abc"));
    check_draws(gs::text().exclude_characters("xyz"));
}

#[test]
fn test_text_categories_including_cs_panics() {
    expect_draw_panic(
        gs::text().categories(&["L", "Cs"]),
        "\"Cs\" includes surrogate codepoints",
    );
}

#[test]
fn test_text_categories_including_cs_supercat_panics() {
    expect_draw_panic(
        gs::text().categories(&["C"]),
        "\"C\" includes surrogate codepoints",
    );
}

#[test]
fn test_characters_draws() {
    check_draws(gs::characters());
}

#[test]
fn test_characters_params_draw() {
    check_draws(gs::characters().codec("ascii"));
    check_draws(gs::characters().min_codepoint(0x20).max_codepoint(0x7E));
    check_draws(gs::characters().categories(&["L", "Nd"]));
    check_draws(gs::characters().exclude_categories(&["Cc"]));
    check_draws(gs::characters().include_characters("abc"));
    check_draws(gs::characters().exclude_characters("xyz"));
}

#[test]
fn test_characters_categories_including_cs_panics() {
    expect_draw_panic(
        gs::characters().categories(&["Cs"]),
        "\"Cs\" includes surrogate codepoints",
    );
}

#[test]
fn test_characters_categories_including_cs_supercat_panics() {
    expect_draw_panic(
        gs::characters().categories(&["C"]),
        "\"C\" includes surrogate codepoints",
    );
}

/// Unknown codec names are rejected eagerly, when `.codec(...)` is called,
/// rather than on first draw.
#[test]
fn test_text_unknown_codec_rejected_eagerly() {
    expect_panic(
        || {
            let _ = gs::text().codec("not-a-real-codec");
        },
        "invalid codec: not-a-real-codec",
    );
}

/// Same eager rejection for `characters()`.
#[test]
fn test_characters_unknown_codec_rejected_eagerly() {
    expect_panic(
        || {
            let _ = gs::characters().codec("not-a-real-codec");
        },
        "invalid codec: not-a-real-codec",
    );
}

/// All engine-supported codec names pass the client-side check.
#[test]
fn test_supported_codecs_accepted() {
    for codec in ["ascii", "latin-1", "iso-8859-1", "utf-8"] {
        check_draws(gs::text().codec(codec));
        check_draws(gs::characters().codec(codec));
    }
}

#[test]
fn test_text_alphabet_with_codec() {
    expect_draw_panic(gs::text().alphabet("abc").codec("ascii"), "Cannot combine");
}

#[test]
fn test_text_codec_with_alphabet() {
    expect_draw_panic(gs::text().codec("ascii").alphabet("abc"), "Cannot combine");
}

#[test]
fn test_text_alphabet_with_categories() {
    expect_draw_panic(
        gs::text().alphabet("abc").categories(&["Lu"]),
        "Cannot combine",
    );
}

#[test]
fn test_binary_min_greater_than_max() {
    expect_draw_panic(gs::binary().min_size(5).max_size(3), "max_size < min_size");
}

#[test]
fn test_vecs_min_greater_than_max() {
    expect_draw_panic(
        gs::vecs(gs::booleans()).min_size(5).max_size(3),
        "max_size < min_size",
    );
}

#[test]
fn test_hashsets_min_greater_than_max() {
    expect_draw_panic(
        gs::hashsets(gs::booleans()).min_size(5).max_size(3),
        "max_size < min_size",
    );
}

#[test]
fn test_hashmaps_min_greater_than_max() {
    expect_draw_panic(
        gs::hashmaps(gs::text(), gs::booleans())
            .min_size(5)
            .max_size(3),
        "max_size < min_size",
    );
}

#[test]
fn test_btree_sets_min_greater_than_max() {
    expect_draw_panic(
        gs::btree_sets(gs::booleans()).min_size(5).max_size(3),
        "max_size < min_size",
    );
}

#[test]
fn test_btree_maps_min_greater_than_max() {
    expect_draw_panic(
        gs::btree_maps(gs::text(), gs::booleans())
            .min_size(5)
            .max_size(3),
        "max_size < min_size",
    );
}

#[test]
fn test_samples_min_greater_than_max() {
    expect_draw_panic(
        gs::samples(vec![1, 2, 3]).min_size(3).max_size(2),
        "max_size < min_size",
    );
}

#[test]
fn test_samples_nonempty_from_empty_sequence() {
    expect_draw_panic(
        gs::samples(Vec::<i32>::new()).min_size(1),
        "non-empty sample from an empty sequence",
    );
}

#[test]
fn test_samples_without_replacement_min_size_too_large() {
    expect_draw_panic(
        gs::samples(vec![1, 2, 3]).without_replacement().min_size(4),
        "min_size 4 is larger than the 3 elements",
    );
}

#[test]
fn test_subsequences_min_greater_than_max() {
    expect_draw_panic(
        gs::subsequences(vec![1, 2, 3]).min_size(3).max_size(2),
        "max_size < min_size",
    );
}

#[test]
fn test_subsequences_min_size_too_large() {
    expect_draw_panic(
        gs::subsequences(vec![1, 2, 3]).min_size(4),
        "min_size 4 is larger than the 3 elements",
    );
}

#[test]
fn test_domains_max_length_too_small() {
    expect_draw_panic(
        gs::domains().max_length(2),
        "domain max_length=2 leaves no eligible TLDs",
    );
}

#[test]
#[should_panic(expected = "sampled_from cannot be empty")]
fn test_sampled_from_empty() {
    let _g = gs::sampled_from(Vec::<i32>::new());
}

#[test]
#[should_panic(expected = "one_of requires at least one generator")]
fn test_one_of_empty() {
    let _g = gs::one_of(Vec::<hegel::generators::BoxedGenerator<'_, i32>>::new());
}

#[hegel::test]
#[should_panic(expected = "InvalidArgument")]
fn test_surrogate_only_character_range_is_invalid_argument(tc: hegel::TestCase) {
    let _: char = tc.draw(gs::characters().min_codepoint(0xD800).max_codepoint(0xD800));
}

mod validation {
    use super::common::utils::{check_can_generate_examples, expect_panic};
    use hegel::generators::{self as gs, Generator};
    use hegel::{Hegel, Settings};

    fn expect_draw_panic<T, G>(generator: G, pattern: &str)
    where
        G: Generator<T> + 'static,
        T: std::fmt::Debug + Send + 'static,
    {
        let generator = std::panic::AssertUnwindSafe(generator);
        expect_panic(
            move || {
                let generator = generator;
                Hegel::new(move |tc| {
                    tc.draw_silent(&generator.0);
                })
                .settings(Settings::new().test_cases(1).database(None))
                .run();
            },
            pattern,
        );
    }

    #[test]
    fn test_float_ranges() {
        expect_draw_panic(
            gs::floats::<f64>().min_value(f64::NAN).max_value(0.0),
            "max_value < min_value",
        );
        expect_draw_panic(
            gs::floats::<f64>().min_value(1.0).max_value(-1.0),
            "max_value < min_value",
        );
    }

    #[test]
    fn test_float_range_and_allow_nan_cannot_both_be_enabled() {
        expect_draw_panic(
            gs::floats::<f64>().min_value(1.0).allow_nan(true),
            "allow_nan=true with min_value or max_value",
        );
        expect_draw_panic(
            gs::floats::<f64>().max_value(1.0).allow_nan(true),
            "allow_nan=true with min_value or max_value",
        );
    }

    #[test]
    fn test_float_finite_range_and_allow_infinity_cannot_both_be_enabled() {
        expect_draw_panic(
            gs::floats::<f64>()
                .min_value(0.0)
                .max_value(1.0)
                .allow_infinity(true),
            "allow_infinity=true with both min_value and max_value",
        );
    }

    #[test]
    fn test_does_not_error_if_min_size_is_bigger_than_default_size() {
        check_can_generate_examples(gs::vecs(gs::integers::<i64>()).min_size(50));
        check_can_generate_examples(gs::hashsets(gs::integers::<i64>()).min_size(50));
        check_can_generate_examples(gs::vecs(gs::integers::<i64>()).min_size(50).unique(true));
    }

    #[test]
    fn test_filter_validates() {
        expect_draw_panic(
            gs::integers::<i64>()
                .min_value(1)
                .max_value(0)
                .filter(|x: &i64| *x != 0),
            "max_value < min_value",
        );
    }

    #[test]
    fn test_validation_happens_on_draw() {
        expect_draw_panic(
            gs::integers::<i64>().flat_map(|_| gs::integers::<i64>().min_value(1).max_value(0)),
            "max_value < min_value",
        );
    }
}

mod given_error_conditions {
    use hegel::generators as gs;
    use hegel::{Hegel, Settings, TestCase};

    #[test]
    fn test_does_not_raise_unsatisfiable_if_some_false_in_finite_set() {
        Hegel::new(|tc: TestCase| {
            let x: bool = tc.draw(gs::booleans());
            tc.assume(x);
        })
        .settings(Settings::new().database(None))
        .run();
    }
}

#[test]
fn test_from_lark_rejects_grammar_errors() {
    expect_draw_panic(gs::from_lark("start: )"), "line 1");
    expect_draw_panic(gs::from_lark("start: missing"), "missing");
    expect_draw_panic(gs::from_lark("rule: \"a\""), "start");
    expect_draw_panic(
        gs::from_lark("start: \"a\"").explicit("B", gs::text()),
        "does not use explicit terminal 'B'",
    );
}

#[test]
fn test_from_lark_rejects_impossible_caps() {
    expect_draw_panic(
        gs::from_lark("start: \"a\"").max_size(0),
        "max_size must be at least 1",
    );
    expect_draw_panic(
        gs::from_lark("start: \"a\"").max_depth(0),
        "max_depth must be at least 1",
    );
    expect_draw_panic(gs::from_lark("start: \"ab\"").max_size(1), "max_size is 1");
    expect_draw_panic(
        gs::from_lark("start: \"a\"*").max_size(usize::MAX),
        "may be at most 1048576",
    );
    expect_draw_panic(
        gs::from_lark("start: x\nx: \"a\"").max_depth(1),
        "max_depth is 1",
    );
    expect_draw_panic(
        gs::from_lark("start: \"a\"").max_depth(u32::MAX),
        "may be at most 500",
    );
}

mod datetime_validation {
    use super::expect_draw_panic;
    use hegel::generators as gs;
    use hegel::generators::temporal::{
        Date, DateTime, Error, Instant, Precision, SignedDuration, SpanSign, SpanUnit, Time,
        TimeZone, UtcOffset, Weekday, ZoneSource,
    };
    use std::time::{Duration, UNIX_EPOCH};

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::new(year, month, day)
    }

    fn time(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Time {
        Time::new(hour, minute, second, nanosecond)
    }

    fn datetime(year: i32, month: u8, day: u8) -> DateTime {
        DateTime::new(date(year, month, day), time(0, 0, 0, 0))
    }

    #[test]
    fn test_dates_min_greater_than_max() {
        expect_draw_panic(
            gs::temporal::dates()
                .min_value(date(2024, 1, 2))
                .max_value(date(2024, 1, 1)),
            "min_value <= max_value",
        );
    }

    #[test]
    fn test_dates_invalid_bound() {
        let bad = Date {
            year: 2024,
            month: 2,
            day: 30,
        };
        expect_draw_panic(gs::temporal::dates().min_value(bad), "not a valid date");
    }

    #[test]
    fn test_dates_empty_weekdays() {
        expect_draw_panic(gs::temporal::dates().weekdays([]), "weekdays mask");
    }

    #[test]
    fn test_dates_empty_months() {
        expect_draw_panic(gs::temporal::dates().months([]), "months mask");
    }

    #[test]
    fn test_dates_no_allowed_day_in_bounds() {
        expect_draw_panic(
            gs::temporal::dates()
                .min_value(date(2024, 1, 1))
                .max_value(date(2024, 1, 5))
                .weekdays([Weekday::Sunday]),
            "no date in",
        );
    }

    #[test]
    fn test_times_min_greater_than_max() {
        expect_draw_panic(
            gs::temporal::times()
                .min_value(time(1, 0, 0, 0))
                .max_value(time(0, 0, 0, 0)),
            "min_value <= max_value",
        );
    }

    #[test]
    fn test_times_no_value_at_precision() {
        expect_draw_panic(
            gs::temporal::times()
                .min_value(time(0, 0, 0, 1))
                .max_value(time(0, 0, 0, 2))
                .precision(Precision::Seconds),
            "no value at precision",
        );
    }

    #[test]
    fn test_datetimes_min_greater_than_max() {
        expect_draw_panic(
            gs::temporal::datetimes()
                .min_value(datetime(2024, 1, 2))
                .max_value(datetime(2024, 1, 1)),
            "min_value <= max_value",
        );
    }

    #[test]
    fn test_instants_min_greater_than_max() {
        expect_draw_panic(
            gs::temporal::instants()
                .min_value(Instant::new(1, 0))
                .max_value(Instant::new(0, 0)),
            "min_value <= max_value",
        );
    }

    #[test]
    fn test_instants_invalid_bound() {
        let bad = Instant {
            second: 0,
            nanosecond: 1_000_000_000,
        };
        expect_draw_panic(
            gs::temporal::instants().max_value(bad),
            "nanosecond must be at most",
        );
    }

    #[test]
    fn test_offsets_min_greater_than_max() {
        expect_draw_panic(
            gs::temporal::offsets()
                .min_value(UtcOffset::Known(1))
                .max_value(UtcOffset::Known(0)),
            "max_value < min_value",
        );
    }

    #[test]
    fn test_offsets_unknown_bound() {
        expect_draw_panic(
            gs::temporal::offsets().min_value(UtcOffset::Unknown),
            "min_value must be a known offset",
        );
        expect_draw_panic(
            gs::temporal::offsets().max_value(UtcOffset::Unknown),
            "max_value must be a known offset",
        );
    }

    #[test]
    fn test_offsets_out_of_range_bound() {
        expect_draw_panic(
            gs::temporal::offsets().max_value(UtcOffset::Known(100_000)),
            "max_value must be within ±25:59:59 \\(93599 seconds\\), got 100000 seconds",
        );
    }

    #[test]
    fn test_zones_empty_names() {
        expect_draw_panic(
            gs::temporal::zones().names(Vec::<String>::new()),
            "at least one allowed name",
        );
    }

    #[test]
    fn test_zones_unknown_name() {
        expect_draw_panic(
            gs::temporal::zones().names(["Mars/Olympus"]),
            "unknown time zone \"Mars/Olympus\"",
        );
    }

    #[test]
    fn test_zoned_datetimes_min_greater_than_max() {
        expect_draw_panic(
            gs::temporal::zoned_datetimes()
                .min_value(datetime(2024, 1, 2))
                .max_value(datetime(2024, 1, 1)),
            "min_value <= max_value",
        );
    }

    #[test]
    fn test_zoned_datetimes_min_instant_greater_than_max_instant() {
        expect_draw_panic(
            gs::temporal::zoned_datetimes()
                .min_instant(Instant::new(1, 0))
                .max_instant(Instant::new(0, 0)),
            "max_instant < min_instant",
        );
    }

    #[test]
    fn test_zoned_datetimes_invalid_civil_bound() {
        let bad = DateTime {
            date: Date {
                year: 2024,
                month: 2,
                day: 30,
            },
            time: time(0, 0, 0, 0),
        };
        expect_draw_panic(
            gs::temporal::zoned_datetimes().max_value(bad),
            "not a valid date",
        );
    }

    #[test]
    fn test_zoned_datetimes_invalid_instant_bound() {
        let bad = Instant {
            second: 0,
            nanosecond: 1_000_000_000,
        };
        expect_draw_panic(
            gs::temporal::zoned_datetimes().min_instant(bad),
            "nanosecond must be at most",
        );
    }

    #[test]
    fn test_zoned_datetimes_zones_are_validated() {
        expect_draw_panic(
            gs::temporal::zoned_datetimes().zones(gs::temporal::zones().names(["Mars/Olympus"])),
            "unknown time zone",
        );
    }

    #[test]
    fn test_zoned_datetimes_wall_clock_and_instant_bounds() {
        let message = "Cannot combine the wall-clock bounds min_value and max_value with the \
                       instant bounds min_instant and max_instant";
        expect_draw_panic(
            gs::temporal::zoned_datetimes()
                .min_value(datetime(2024, 1, 1))
                .max_instant(Instant::new(0, 0)),
            message,
        );
        expect_draw_panic(
            gs::temporal::zoned_datetimes()
                .min_instant(Instant::new(0, 0))
                .max_value(datetime(2024, 1, 1)),
            message,
        );
        expect_draw_panic(
            gs::temporal::zoned_datetimes()
                .zones(gs::just(TimeZone::new("Europe/London").unwrap()))
                .max_instant(Instant::new(0, 0))
                .min_value(datetime(2024, 1, 1)),
            message,
        );
    }

    fn unresolved(name: &str) -> TimeZone {
        TimeZone {
            name: name.to_string(),
            primary_name: name.to_string(),
        }
    }

    #[test]
    fn test_zoned_datetimes_zone_the_source_does_not_know() {
        expect_draw_panic(
            gs::temporal::zoned_datetimes().zones(gs::just(unresolved("Mars/Olympus"))),
            "zoned_datetimes\\(\\): the zones generator drew the time zone \"Mars/Olympus\", \
             which the default zone source does not know\\. Pass the zone source that has it \
             to source\\(\\)\\.",
        );
        expect_draw_panic(
            gs::temporal::zoned_datetimes()
                .zones(gs::just(unresolved("Mars/Olympus")))
                .source(ZoneSource::bundled()),
            "the time zone \"Mars/Olympus\", which the zone source given to source\\(\\) does \
             not know",
        );
    }

    #[test]
    fn test_zoned_datetimes_zone_the_source_fails_to_load() {
        let source = ZoneSource::builder()
            .declare(["Test/Broken"])
            .loader(|_, name| Err(Error::from(format!("cannot load {name}"))))
            .build()
            .unwrap();
        expect_draw_panic(
            gs::temporal::zoned_datetimes()
                .zones(gs::just(unresolved("Test/Broken")))
                .source(source),
            "the zone source's loader for \"Test/Broken\" failed: cannot load Test/Broken",
        );
    }

    #[test]
    fn test_zoned_datetimes_source_with_a_zones_generator() {
        expect_draw_panic(
            gs::temporal::zoned_datetimes()
                .zones(gs::temporal::zones())
                .source(ZoneSource::bundled()),
            "source\\(\\) cannot be combined with a zones\\(\\) generator, which has a zone \
             source of its own\\. Pass the source to zones\\(\\)\\.source\\(\\) instead\\.",
        );
    }

    #[test]
    fn test_durations_min_greater_than_max() {
        expect_draw_panic(
            gs::temporal::durations()
                .min_value(SignedDuration::new(1, 0))
                .max_value(SignedDuration::new(0, 0)),
            "min_value <= max_value",
        );
    }

    #[test]
    fn test_durations_invalid_bound() {
        let bad = SignedDuration {
            seconds: 1,
            nanoseconds: -1,
        };
        expect_draw_panic(
            gs::temporal::durations().max_value(bad),
            "not a valid duration",
        );
    }

    #[test]
    fn test_spans_inverted_unit_bounds() {
        expect_draw_panic(
            gs::temporal::spans().unit_bounds(SpanUnit::Days, 5, 3),
            "min_value <= max_value is required for days",
        );
    }

    #[test]
    fn test_spans_negative_bound_under_nonnegative_sign() {
        expect_draw_panic(
            gs::temporal::spans()
                .unit_bounds(SpanUnit::Days, -5, 3)
                .sign(SpanSign::NonNegative),
            "nonnegative sign policy requires min >= 0",
        );
    }

    #[test]
    fn test_spans_uniform_sign_with_no_common_sign() {
        expect_draw_panic(
            gs::temporal::spans()
                .unit_bounds(SpanUnit::Days, 1, 3)
                .unit_bounds(SpanUnit::Hours, -3, -1),
            "admit none in common",
        );
    }

    #[test]
    fn test_std_durations_min_greater_than_max() {
        expect_draw_panic(
            gs::temporal::std_durations()
                .min_value(Duration::from_secs(2))
                .max_value(Duration::from_secs(1)),
            "min_value <= max_value",
        );
    }

    #[test]
    fn test_std_durations_min_unrepresentable() {
        expect_draw_panic(
            gs::temporal::std_durations().min_value(Duration::from_secs(u64::MAX)),
            "largest generatable Duration",
        );
    }

    #[test]
    fn test_system_times_min_greater_than_max() {
        expect_draw_panic(
            gs::temporal::system_times()
                .min_value(UNIX_EPOCH + Duration::from_secs(1))
                .max_value(UNIX_EPOCH),
            "min_value <= max_value",
        );
    }
}
