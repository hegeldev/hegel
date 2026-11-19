use crate::common::utils::{
    assert_all_examples, check_can_generate_examples, expect_panic, find_any, minimal,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use hegel::Generator;
use hegel::extras::chrono as chrono_gs;
use hegel::generators as gs;
use hegel::generators::temporal::{Ambiguous, Instant, Precision, Tag, UtcOffset, info};

fn new_york(date: (i32, u32, u32), time: (u32, u32, u32)) -> NaiveDateTime {
    NaiveDateTime::new(
        NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
        NaiveTime::from_hms_opt(time.0, time.1, time.2).unwrap(),
    )
}

fn resolves_to_itself(dt: &DateTime<Tz>) -> bool {
    dt.timezone().from_utc_datetime(&dt.naive_utc()) == *dt
}

#[test]
fn test_time_zones_default() {
    assert_all_examples(chrono_gs::zones(), |tz| {
        info(tz.name())
            .unwrap()
            .is_none_or(|info| !info.tags.contains(&Tag::Legacy))
    });
}

#[test]
fn test_time_zones_names() {
    assert_all_examples(
        chrono_gs::zones().names([Tz::Europe__London, Tz::Asia__Tokyo]),
        |tz| *tz == Tz::Europe__London || *tz == Tz::Asia__Tokyo,
    );
}

#[test]
fn test_time_zones_names_reach_every_name() {
    find_any(
        chrono_gs::zones().names([Tz::Europe__London, Tz::Asia__Tokyo]),
        |tz| *tz == Tz::Asia__Tokyo,
    );
}

#[test]
fn test_time_zones_links() {
    find_any(chrono_gs::zones(), |tz| {
        info(tz.name()).unwrap().is_some_and(|info| info.is_link)
    });
    assert_all_examples(chrono_gs::zones().allow_links(false), |tz| {
        info(tz.name()).unwrap().is_none_or(|info| !info.is_link)
    });
}

#[test]
fn test_time_zones_legacy() {
    find_any(chrono_gs::zones().allow_legacy(true), |tz| {
        info(tz.name())
            .unwrap()
            .is_some_and(|info| info.tags.contains(&Tag::Legacy))
    });
}

#[test]
fn test_time_zones_in_vec() {
    assert_all_examples(
        gs::vecs(chrono_gs::zones().names([Tz::UTC, Tz::Europe__Paris])).max_size(5),
        |v| {
            v.iter()
                .all(|tz| *tz == Tz::UTC || *tz == Tz::Europe__Paris)
        },
    );
}

#[test]
fn test_time_zone_default_generator() {
    check_can_generate_examples(gs::default::<Tz>());
}

#[test]
fn test_time_zones_shrink_to_utc() {
    let tz: Tz = minimal(chrono_gs::zones(), |_| true);
    assert_eq!(tz, Tz::UTC);
}

#[test]
fn test_time_zones_empty_names_is_a_usage_error() {
    expect_panic(
        || {
            hegel::Hegel::new(|tc| {
                let _ = tc.draw(chrono_gs::zones().names([]));
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "at least one allowed name",
    );
}

#[test]
fn test_datetimes_tz_default() {
    assert_all_examples(
        chrono_gs::datetimes().zones(chrono_gs::zones()),
        resolves_to_itself,
    );
}

#[test]
fn test_datetime_tz_default_generator() {
    assert_all_examples(gs::default::<DateTime<Tz>>(), resolves_to_itself);
}

#[test]
fn test_datetimes_tz_in_vec() {
    assert_all_examples(
        gs::vecs(chrono_gs::datetimes().zones(chrono_gs::zones())).max_size(3),
        |v| v.iter().all(resolves_to_itself),
    );
}

#[test]
fn test_datetimes_tz_wall_clock_bounds() {
    let min = new_york((2020, 1, 1), (0, 0, 0));
    let max = new_york((2024, 12, 31), (23, 59, 59));
    assert_all_examples(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones())
            .min_value(min)
            .max_value(max),
        move |dt| dt.naive_local() >= min && dt.naive_local() <= max,
    );
}

#[test]
fn test_datetimes_tz_instant_bounds() {
    let min = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let max = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
    assert_all_examples(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones())
            .min_instant(min)
            .max_instant(max),
        move |dt| *dt >= min && *dt <= max,
    );
}

#[test]
fn test_datetimes_tz_precision() {
    assert_all_examples(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones())
            .precision(Precision::Seconds),
        |dt| dt.nanosecond() == 0,
    );
}

#[test]
fn test_datetimes_tz_avoids_gaps_by_default() {
    assert_all_examples(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones().names([Tz::America__New_York]))
            .min_value(new_york((2024, 3, 10), (0, 0, 0)))
            .max_value(new_york((2024, 3, 10), (23, 59, 59))),
        |dt| {
            Tz::America__New_York
                .from_local_datetime(&dt.naive_local())
                .single()
                .is_some()
        },
    );
}

#[test]
fn test_datetimes_tz_allows_gaps_as_the_shifted_instant() {
    let max = new_york((2024, 3, 10), (2, 59, 59));
    assert_all_examples(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones().names([Tz::America__New_York]))
            .min_value(new_york((2024, 3, 10), (2, 0, 0)))
            .max_value(max)
            .allow_imaginary(true),
        move |dt| {
            dt.naive_local() > max
                && dt.naive_local() < new_york((2024, 3, 10), (4, 0, 0))
                && dt.offset().fix().local_minus_utc() == -4 * 3600
        },
    );
}

#[test]
fn test_datetimes_tz_draws_both_repeats_of_a_fold() {
    let generator = || {
        chrono_gs::datetimes()
            .zones(chrono_gs::zones().names([Tz::America__New_York]))
            .min_value(new_york((2024, 11, 3), (1, 0, 0)))
            .max_value(new_york((2024, 11, 3), (1, 59, 59)))
    };
    find_any(generator(), |dt| {
        dt.offset().fix().local_minus_utc() == -4 * 3600
    });
    find_any(generator(), |dt| {
        dt.offset().fix().local_minus_utc() == -5 * 3600
    });
}

#[test]
fn test_datetimes_tz_ambiguous_earlier() {
    assert_all_examples(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones().names([Tz::America__New_York]))
            .min_value(new_york((2024, 11, 3), (1, 0, 0)))
            .max_value(new_york((2024, 11, 3), (1, 59, 59)))
            .ambiguous(Ambiguous::Earlier),
        |dt| dt.offset().fix().local_minus_utc() == -4 * 3600,
    );
}

#[test]
fn test_datetimes_tz_ambiguous_later() {
    assert_all_examples(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones().names([Tz::America__New_York]))
            .min_value(new_york((2024, 11, 3), (1, 0, 0)))
            .max_value(new_york((2024, 11, 3), (1, 59, 59)))
            .ambiguous(Ambiguous::Later),
        |dt| dt.offset().fix().local_minus_utc() == -5 * 3600,
    );
}

#[test]
fn test_datetimes_tz_unreachable_instant_is_rejected() {
    let max = DateTime::<Utc>::MAX_UTC.naive_utc();
    let min = max - chrono::TimeDelta::hours(1);
    expect_panic(
        || {
            hegel::Hegel::new(move |tc| {
                let _ = tc.draw(
                    chrono_gs::datetimes()
                        .zones(chrono_gs::zones().names([Tz::America__New_York]))
                        .min_value(min)
                        .max_value(max),
                );
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "Unsatisfiable|FilterTooMuch",
    );
}

#[test]
fn test_datetimes_tz_shrinks_to_2000_at_utc() {
    let dt: DateTime<Tz> = minimal(chrono_gs::datetimes().zones(chrono_gs::zones()), |_| true);
    assert_eq!(dt.timezone(), Tz::UTC);
    assert_eq!(dt.naive_local(), new_york((2000, 1, 1), (0, 0, 0)));
}

#[hegel::test]
fn test_datetimes_tz_property(tc: hegel::TestCase) {
    let lo = tc.draw(
        gs::integers::<i64>()
            .min_value(-4_000_000_000)
            .max_value(4_000_000_000),
    );
    let hi = tc.draw(gs::integers::<i64>().min_value(lo).max_value(4_000_000_000));
    let min = DateTime::<Utc>::from_timestamp(lo, 0).unwrap();
    let max = DateTime::<Utc>::from_timestamp(hi, 999_999_999).unwrap();
    let dt = tc.draw(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones())
            .min_instant(min)
            .max_instant(max),
    );
    assert!(dt >= min && dt <= max);
    assert!(resolves_to_itself(&dt));
}

#[hegel::test]
fn test_datetimes_tz_rebuilt_after_a_draw_uses_the_new_bounds(tc: hegel::TestCase) {
    let max = new_york((2000, 1, 1), (0, 0, 0));
    let g = chrono_gs::datetimes().zones(chrono_gs::zones().names([Tz::UTC]));
    let _ = tc.draw(&g);
    let dt = tc.draw(g.max_value(max));
    assert!(dt.naive_local() <= max);
}

#[test]
fn test_datetimes_tz_follow_chrono_tz_where_its_database_differs_from_the_bundled_one() {
    let min = new_york((2027, 3, 14), (2, 0, 0));
    let max = new_york((2027, 3, 14), (3, 59, 59));
    assert_all_examples(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones().names([Tz::America__Edmonton]))
            .min_value(min)
            .max_value(max),
        move |dt| dt.naive_local() >= min && dt.naive_local() <= max && resolves_to_itself(dt),
    );
}

#[test]
fn test_datetimes_tz_draw_the_one_instant_bounds_allow_where_chrono_tz_has_no_daylight_saving() {
    let instant = Utc.with_ymd_and_hms(2100, 3, 15, 5, 0, 0).unwrap();
    assert_all_examples(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones().names([Tz::America__New_York]))
            .min_instant(instant)
            .max_instant(instant),
        move |dt| dt.to_utc() == instant && dt.offset().fix().local_minus_utc() == -5 * 3600,
    );
}

#[test]
fn test_datetimes_tz_hold_standard_time_after_chrono_tz_table_ends() {
    let min = Utc.with_ymd_and_hms(2100, 3, 15, 0, 0, 0).unwrap();
    let max = min + chrono::TimeDelta::hours(2);
    assert_all_examples(
        chrono_gs::datetimes()
            .zones(chrono_gs::zones().names([Tz::America__New_York]))
            .min_instant(min)
            .max_instant(max),
        move |dt| {
            dt.to_utc() >= min
                && dt.to_utc() <= max
                && dt.offset().fix().local_minus_utc() == -5 * 3600
        },
    );
}

#[hegel::test]
fn test_chrono_tz_zone_source_agrees_with_chrono_tz(tc: hegel::TestCase) {
    let tz = tc.draw(chrono_gs::zones().allow_legacy(true));
    let second = tc.draw(
        gs::integers::<i64>()
            .min_value(-7_000_000_000)
            .max_value(7_000_000_000),
    );
    let offset = chrono_gs::zone_source()
        .offset_at(tz.name(), Instant::new(second, 0))
        .unwrap();
    let expected =
        tz.offset_from_utc_datetime(&DateTime::from_timestamp(second, 0).unwrap().naive_utc());
    assert_eq!(
        offset.offset,
        UtcOffset::Known(expected.fix().local_minus_utc())
    );
    assert_eq!(offset.abbreviation, expected.to_string());
}

#[test]
fn test_chrono_tz_zone_source_loads_every_zone() {
    let source = chrono_gs::zone_source();
    for tz in chrono_tz::TZ_VARIANTS {
        let offset = source
            .offset_at(tz.name(), Instant::new(1_700_000_000, 0))
            .unwrap();
        assert_eq!(
            offset.offset,
            UtcOffset::Known(
                tz.offset_from_utc_datetime(
                    &DateTime::from_timestamp(1_700_000_000, 0)
                        .unwrap()
                        .naive_utc()
                )
                .fix()
                .local_minus_utc()
            )
        );
    }
}

#[test]
fn test_chrono_tz_zone_source_names_and_version() {
    let source = chrono_gs::zone_source();
    assert_eq!(source.version(), chrono_tz::IANA_TZDB_VERSION);
    assert!(source.info("Europe/London").unwrap().is_some());
    assert_eq!(source.info("Mars/Olympus_Mons").unwrap(), None);
}

#[test]
fn test_chrono_tz_tables_hold_their_last_offset_after_2100() {
    let start = Utc
        .with_ymd_and_hms(2101, 1, 1, 0, 0, 0)
        .unwrap()
        .timestamp();
    for tz in chrono_tz::TZ_VARIANTS {
        let offset_at = |second: i64| {
            tz.offset_from_utc_datetime(&DateTime::from_timestamp(second, 0).unwrap().naive_utc())
                .fix()
        };
        let held = offset_at(start);
        for day in (0..365 * 100).step_by(5) {
            assert_eq!(offset_at(start + day * 86_400), held, "{}", tz.name());
        }
    }
}

#[test]
fn test_datetimes_tz_from_any_generator_of_zones() {
    let zones = [Tz::America__New_York, Tz::Asia__Kolkata];
    assert_all_examples(
        chrono_gs::datetimes().zones(gs::sampled_from(zones.to_vec())),
        move |dt| zones.contains(&dt.timezone()) && resolves_to_itself(dt),
    );
    for zone in zones {
        find_any(
            chrono_gs::datetimes().zones(gs::sampled_from(zones.to_vec())),
            move |dt| dt.timezone() == zone,
        );
    }
    assert_all_examples(
        chrono_gs::datetimes().zones(chrono_gs::zones().names([Tz::Europe__London]).map(|tz| tz)),
        |dt| dt.timezone() == Tz::Europe__London && resolves_to_itself(dt),
    );
}

#[test]
fn test_datetimes_tz_from_any_generator_of_zones_apply_the_options() {
    let in_new_york = || chrono_gs::datetimes().zones(gs::just(Tz::America__New_York));
    let min = new_york((2024, 11, 3), (0, 0, 0));
    let max = new_york((2024, 11, 3), (23, 59, 59));
    let fold = new_york((2024, 11, 3), (1, 30, 0));
    assert_all_examples(in_new_york().min_value(min).max_value(max), move |dt| {
        dt.naive_local() >= min && dt.naive_local() <= max
    });
    for offset in [-4 * 3600, -5 * 3600] {
        find_any(in_new_york().min_value(fold).max_value(fold), move |dt| {
            dt.offset().fix().local_minus_utc() == offset
        });
    }
    assert_all_examples(
        in_new_york()
            .min_value(fold)
            .max_value(fold)
            .ambiguous(Ambiguous::Later),
        |dt| dt.offset().fix().local_minus_utc() == -5 * 3600,
    );
    let lo = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    let hi = DateTime::<Utc>::from_timestamp(86_400, 0).unwrap();
    assert_all_examples(in_new_york().min_instant(lo).max_instant(hi), move |dt| {
        *dt >= lo && *dt <= hi
    });
    assert_all_examples(in_new_york().precision(Precision::Minutes), |dt| {
        dt.second() == 0 && dt.nanosecond() == 0
    });
}

#[test]
fn test_datetimes_tz_wall_clock_and_instant_bounds_cannot_be_combined() {
    let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    expect_panic(
        move || {
            hegel::Hegel::new(move |tc| {
                tc.draw(
                    chrono_gs::datetimes()
                        .zones(chrono_gs::zones())
                        .max_value(epoch.naive_utc())
                        .min_instant(epoch),
                );
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "Cannot combine the wall-clock bounds min_value and max_value with the instant bounds \
         min_instant and max_instant",
    );
}
