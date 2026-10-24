use crate::common::utils::{assert_all_examples, check_can_generate_examples, find_any, minimal};
use hegel::datetime::{Ambiguous, Granularity, Precision, Tag, UNKNOWN_ZONE, info};
use hegel::extras::jiff as jiff_gs;
use hegel::generators as gs;
use jiff::civil::DateTime;
use jiff::tz::{AmbiguousOffset, AmbiguousTimestamp, AmbiguousZoned, Offset, TimeZone};
use jiff::{Timestamp, Zoned};

#[test]
fn test_jiff_offsets_default() {
    assert_all_examples(jiff_gs::offsets(), |o| {
        (-93_599..=93_599).contains(&o.seconds())
    });
    find_any(jiff_gs::offsets(), |o| *o == Offset::MIN);
    find_any(jiff_gs::offsets(), |o| *o == Offset::MAX);
}

#[test]
fn test_jiff_offsets_min_value() {
    let min = Offset::from_seconds(-3600).unwrap();
    assert_all_examples(jiff_gs::offsets().min_value(min), move |o| o >= &min);
}

#[test]
fn test_jiff_offsets_max_value() {
    let max = Offset::from_seconds(3600).unwrap();
    assert_all_examples(jiff_gs::offsets().max_value(max), move |o| o <= &max);
}

#[test]
fn test_jiff_offsets_granularity() {
    assert_all_examples(
        jiff_gs::offsets().granularity(Granularity::QuarterHours),
        |o| o.seconds() % 900 == 0,
    );
    find_any(jiff_gs::offsets(), |o| o.seconds() % 60 != 0);
}

#[test]
fn test_jiff_offsets_in_vec() {
    let max = Offset::from_seconds(3600).unwrap();
    assert_all_examples(
        gs::vecs(jiff_gs::offsets().max_value(max)).max_size(5),
        move |v| v.iter().all(|o| o.seconds() <= 3600),
    );
}

#[test]
fn test_jiff_offset_default_generator() {
    check_can_generate_examples(gs::default::<Offset>());
}

#[hegel::test]
fn test_jiff_offsets_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::integers::<i32>().min_value(-93_599).max_value(93_599));
    let hi = tc.draw(gs::integers::<i32>().min_value(lo).max_value(93_599));
    let min = Offset::from_seconds(lo).unwrap();
    let max = Offset::from_seconds(hi).unwrap();
    let v = tc.draw(jiff_gs::offsets().min_value(min).max_value(max));
    assert!(v.seconds() >= lo && v.seconds() <= hi);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_jiff_offsets_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        jiff_gs::offsets()
            .min_value(Offset::from_seconds(10).unwrap())
            .max_value(Offset::from_seconds(5).unwrap()),
    );
}

fn iana_name(zone: &TimeZone) -> &str {
    zone.iana_name().unwrap()
}

#[test]
fn test_jiff_time_zones_default() {
    check_can_generate_examples(jiff_gs::timezones());
    assert_all_examples(jiff_gs::timezones(), |zone| {
        let info = info(iana_name(zone)).unwrap();
        !info.tags.contains(&Tag::Legacy) && *zone != TimeZone::unknown()
    });
}

#[test]
fn test_jiff_time_zones_resolve_through_jiff() {
    find_any(jiff_gs::timezones(), |zone| {
        iana_name(zone) == "Asia/Calcutta"
    });
    assert_all_examples(jiff_gs::timezones(), |zone| {
        jiff::tz::db()
            .available()
            .any(|name| name.as_str() == iana_name(zone))
            && TimeZone::get(iana_name(zone)).unwrap() == *zone
    });
}

#[test]
fn test_jiff_time_zones_names() {
    let names = ["Europe/London", "Asia/Calcutta"];
    assert_all_examples(jiff_gs::timezones().names(names), move |zone| {
        names.contains(&iana_name(zone))
    });
    find_any(jiff_gs::timezones().names(names), |zone| {
        iana_name(zone) == "Asia/Calcutta"
    });
}

#[test]
fn test_jiff_time_zones_names_accept_the_unknown_zone() {
    assert_all_examples(jiff_gs::timezones().names([UNKNOWN_ZONE]), |zone| {
        *zone == TimeZone::unknown()
    });
}

#[test]
fn test_jiff_time_zones_links() {
    assert_all_examples(jiff_gs::timezones().links(false), |zone| {
        !info(iana_name(zone)).unwrap().is_link
    });
    find_any(jiff_gs::timezones(), |zone| {
        info(iana_name(zone)).unwrap().is_link
    });
}

#[test]
fn test_jiff_time_zones_legacy() {
    find_any(jiff_gs::timezones().legacy(true), |zone| {
        info(iana_name(zone)).unwrap().tags.contains(&Tag::Legacy)
    });
}

#[test]
fn test_jiff_time_zones_unknown() {
    find_any(jiff_gs::timezones().unknown(true), |zone| {
        *zone == TimeZone::unknown()
    });
}

#[test]
fn test_jiff_time_zones_in_vec() {
    assert_all_examples(gs::vecs(jiff_gs::timezones()).max_size(5), |v| {
        v.iter().all(|zone| zone.iana_name().is_some())
    });
}

#[test]
fn test_jiff_time_zones_shrink_to_utc() {
    assert_eq!(minimal(jiff_gs::timezones(), |_| true), TimeZone::UTC);
}

#[hegel::test]
#[should_panic(expected = "jiff's time zone database does not know \"Mars/Olympus_Mons\"")]
fn test_jiff_time_zones_reject_a_name_jiff_lacks(tc: hegel::TestCase) {
    tc.draw(jiff_gs::timezones().names(["Europe/London", "Mars/Olympus_Mons"]));
}

#[test]
fn test_jiff_timezone_default_generator() {
    check_can_generate_examples(gs::default::<TimeZone>());
    find_any(gs::default::<TimeZone>(), |zone| zone.iana_name().is_some());
    find_any(gs::default::<TimeZone>(), |zone| {
        zone.iana_name().is_none() && *zone != TimeZone::unknown()
    });
}

#[test]
fn test_jiff_timezones_in_vec() {
    assert_all_examples(gs::vecs(gs::default::<TimeZone>()).max_size(5), |v| {
        v.iter()
            .all(|tz: &TimeZone| !tz.iana_name().unwrap_or("").contains('\0'))
    });
}

fn wall_time(z: &Zoned) -> DateTime {
    let second = z.timestamp().as_nanosecond().div_euclid(1_000_000_000) as i64;
    let offset = z
        .time_zone()
        .to_offset(Timestamp::from_second(second).unwrap());
    z.timestamp().to_zoned(TimeZone::fixed(offset)).datetime()
}

fn resolution(z: &Zoned) -> AmbiguousTimestamp {
    z.time_zone().to_ambiguous_timestamp(wall_time(z))
}

fn resolved_as_jiff_would(z: &Zoned) -> bool {
    let ambiguous = resolution(z);
    match ambiguous.offset() {
        AmbiguousOffset::Unambiguous { .. } => ambiguous.unambiguous().unwrap() == z.timestamp(),
        AmbiguousOffset::Fold { .. } => {
            ambiguous.earlier().unwrap() == z.timestamp()
                || ambiguous.later().unwrap() == z.timestamp()
        }
        AmbiguousOffset::Gap { .. } => false,
    }
}

fn is_fold(z: &Zoned, later: bool) -> bool {
    let ambiguous = resolution(z);
    match ambiguous.offset() {
        AmbiguousOffset::Fold { .. } => {
            let expected = if later {
                ambiguous.later()
            } else {
                ambiguous.earlier()
            };
            expected.unwrap() == z.timestamp()
        }
        _ => false,
    }
}

#[test]
fn test_jiff_zoneds_default() {
    check_can_generate_examples(jiff_gs::zoneds());
    assert_all_examples(jiff_gs::zoneds(), |z| {
        z.datetime() >= jiff_gs::MIN_CIVIL
            && z.datetime() <= jiff_gs::MAX_CIVIL
            && resolved_as_jiff_would(z)
    });
}

#[test]
fn test_jiff_zoneds_reach_both_folds() {
    find_any(jiff_gs::zoneds(), |z| is_fold(z, false));
    find_any(jiff_gs::zoneds(), |z| is_fold(z, true));
}

#[test]
fn test_jiff_zoneds_in_vec() {
    assert_all_examples(gs::vecs(jiff_gs::zoneds()).max_size(3), |v| {
        v.iter().all(resolved_as_jiff_would)
    });
}

#[test]
fn test_jiff_zoned_default_generator() {
    check_can_generate_examples(gs::default::<Zoned>());
}

#[test]
fn test_jiff_zoneds_time_zones() {
    assert_all_examples(
        jiff_gs::zoneds().timezones(jiff_gs::timezones().names(["Europe/London"])),
        |z| iana_name(z.time_zone()) == "Europe/London" && resolved_as_jiff_would(z),
    );
    assert_all_examples(
        jiff_gs::zoneds().timezones(jiff_gs::timezones().names([UNKNOWN_ZONE])),
        |z| *z.time_zone() == TimeZone::unknown() && z.offset() == Offset::UTC,
    );
}

#[test]
fn test_jiff_zoneds_min_value() {
    let min = DateTime::constant(2024, 1, 1, 0, 0, 0, 0);
    assert_all_examples(jiff_gs::zoneds().min_value(min), move |z| {
        z.datetime() >= min
    });
}

#[test]
fn test_jiff_zoneds_max_value() {
    let max = DateTime::constant(1999, 12, 31, 0, 0, 0, 0);
    assert_all_examples(jiff_gs::zoneds().max_value(max), move |z| {
        z.datetime() <= max
    });
}

#[test]
fn test_jiff_zoneds_civil_bounds_at_the_ends_of_the_range() {
    find_any(
        jiff_gs::zoneds().max_value(DateTime::constant(-9999, 1, 4, 0, 0, 0, 0)),
        |z| z.datetime() == jiff_gs::MIN_CIVIL,
    );
    find_any(
        jiff_gs::zoneds().min_value(DateTime::constant(9999, 12, 29, 0, 0, 0, 0)),
        |z| z.datetime() == jiff_gs::MAX_CIVIL,
    );
}

#[test]
fn test_jiff_zoneds_min_instant() {
    let min = Timestamp::UNIX_EPOCH;
    assert_all_examples(jiff_gs::zoneds().min_instant(min), move |z| {
        z.timestamp() >= min
    });
}

#[test]
fn test_jiff_zoneds_max_instant() {
    let max = Timestamp::UNIX_EPOCH;
    assert_all_examples(jiff_gs::zoneds().max_instant(max), move |z| {
        z.timestamp() <= max
    });
}

#[test]
fn test_jiff_zoneds_instant_bounds_reach_the_ends_of_the_range() {
    find_any(jiff_gs::zoneds().max_instant(Timestamp::MIN), |z| {
        z.timestamp() == Timestamp::MIN
    });
    find_any(jiff_gs::zoneds().min_instant(Timestamp::MAX), |z| {
        z.timestamp() == Timestamp::MAX
    });
}

#[test]
fn test_jiff_zoneds_instant_bounds_replace_civil_bounds_and_back() {
    let min = Timestamp::UNIX_EPOCH;
    let max = Timestamp::from_second(86_400).unwrap();
    assert_all_examples(
        jiff_gs::zoneds()
            .min_value(DateTime::constant(1999, 1, 1, 0, 0, 0, 0))
            .max_instant(max)
            .min_instant(min)
            .min_value(DateTime::constant(2000, 1, 1, 0, 0, 0, 0))
            .max_value(DateTime::constant(2000, 12, 31, 0, 0, 0, 0)),
        |z| z.year() == 2000,
    );
    assert_all_examples(
        jiff_gs::zoneds()
            .min_instant(min)
            .max_value(DateTime::constant(1999, 12, 31, 0, 0, 0, 0)),
        |z| z.year() <= 1999,
    );
}

#[test]
fn test_jiff_zoneds_precision() {
    assert_all_examples(jiff_gs::zoneds().precision(Precision::Minutes), |z| {
        z.second() == 0 && z.subsec_nanosecond() == 0
    });
}

#[test]
fn test_jiff_zoneds_imaginary() {
    let london = || jiff_gs::timezones().names(["Europe/London"]);
    assert_all_examples(
        jiff_gs::zoneds().timezones(london()).allow_imaginary(true),
        resolved_as_jiff_would,
    );
    assert_all_examples(
        jiff_gs::zoneds().timezones(london()).allow_imaginary(false),
        resolved_as_jiff_would,
    );
}

#[test]
fn test_jiff_zoneds_imaginary_allow_shifts_a_gap_forward() {
    let apia = jiff_gs::timezones().names(["Pacific/Apia"]);
    assert_all_examples(
        jiff_gs::zoneds()
            .timezones(apia)
            .min_value(DateTime::constant(2011, 12, 30, 0, 0, 0, 0))
            .max_value(DateTime::constant(2011, 12, 30, 23, 59, 59, 999_999_999))
            .allow_imaginary(true),
        |z| {
            z.date() == jiff::civil::Date::constant(2011, 12, 31)
                && z.offset() == Offset::from_hours(14).unwrap()
        },
    );
}

#[test]
fn test_jiff_zoneds_ambiguous() {
    assert_all_examples(jiff_gs::zoneds().ambiguous(Ambiguous::Earlier), |z| {
        !is_fold(z, true) && resolved_as_jiff_would(z)
    });
    assert_all_examples(jiff_gs::zoneds().ambiguous(Ambiguous::Later), |z| {
        !is_fold(z, false) && resolved_as_jiff_would(z)
    });
    find_any(jiff_gs::zoneds().ambiguous(Ambiguous::Later), |z| {
        is_fold(z, true)
    });
}

#[hegel::test]
fn test_jiff_zoneds_property(tc: hegel::TestCase) {
    let lo = tc.draw(jiff_gs::timestamps());
    let hi = tc.draw(jiff_gs::timestamps().min_value(lo));
    let z = tc.draw(jiff_gs::zoneds().min_instant(lo).max_instant(hi));
    assert!(z.timestamp() >= lo && z.timestamp() <= hi);
}

#[test]
fn test_jiff_zoneds_stay_in_instant_bounds_where_jiff_reports_a_fold_the_engine_does_not() {
    let dummy = Timestamp::from_second(1_698_541_200).unwrap();
    let min = dummy - jiff::SignedDuration::from_hours(1);
    let max = dummy + jiff::SignedDuration::from_hours(1);
    assert_all_examples(
        jiff_gs::zoneds()
            .timezones(jiff_gs::timezones().names(["America/Nuuk"]))
            .min_instant(min)
            .max_instant(max),
        move |z| z.timestamp() >= min && z.timestamp() <= max,
    );
}

#[test]
fn test_jiff_zoneds_shrink_to_utc_at_the_millennium() {
    let z = minimal(jiff_gs::zoneds(), |_| true);
    assert_eq!(z.to_string(), "2000-01-01T00:00:00+00:00[UTC]");
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_jiff_zoneds_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        jiff_gs::zoneds()
            .min_value(DateTime::constant(2025, 1, 1, 0, 0, 0, 0))
            .max_value(DateTime::constant(2024, 1, 1, 0, 0, 0, 0)),
    );
}

#[hegel::test]
#[should_panic(expected = "max_instant < min_instant")]
fn test_jiff_zoneds_min_instant_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        jiff_gs::zoneds()
            .min_instant(Timestamp::from_second(10).unwrap())
            .max_instant(Timestamp::from_second(5).unwrap()),
    );
}

#[hegel::test]
#[should_panic(expected = "min_value must be at or after -009999-01-03T04:00:00")]
fn test_jiff_zoneds_min_value_before_the_convertible_range(tc: hegel::TestCase) {
    tc.draw(jiff_gs::zoneds().min_value(DateTime::MIN));
}

#[hegel::test]
#[should_panic(expected = "max_value must be at or before 9999-12-29T20:00:00")]
fn test_jiff_zoneds_max_value_after_the_convertible_range(tc: hegel::TestCase) {
    tc.draw(jiff_gs::zoneds().max_value(DateTime::MAX));
}

#[test]
fn test_jiff_ambiguous_offset_default_generator() {
    check_can_generate_examples(gs::default::<AmbiguousOffset>());
    find_any(gs::default::<AmbiguousOffset>(), |o| {
        matches!(o, AmbiguousOffset::Gap { .. })
    });
    find_any(gs::default::<AmbiguousOffset>(), |o| {
        matches!(o, AmbiguousOffset::Fold { .. })
    });
}

#[test]
fn test_jiff_ambiguous_offsets_in_vec() {
    assert_all_examples(
        gs::vecs(gs::default::<AmbiguousOffset>()).max_size(5),
        |v| {
            v.iter().all(|o| {
                matches!(
                    o,
                    AmbiguousOffset::Unambiguous { .. }
                        | AmbiguousOffset::Gap { .. }
                        | AmbiguousOffset::Fold { .. }
                )
            })
        },
    );
}

#[test]
fn test_jiff_ambiguous_zoned_default_generator() {
    check_can_generate_examples(gs::default::<AmbiguousZoned>());
    find_any(gs::default::<AmbiguousZoned>(), |z| {
        matches!(z.offset(), AmbiguousOffset::Gap { .. })
    });
    assert_all_examples(gs::default::<AmbiguousZoned>(), |z| {
        z.time_zone().iana_name().is_some() && z.clone().compatible().is_ok()
    });
}

#[test]
fn test_jiff_ambiguous_timestamp_default_generator() {
    check_can_generate_examples(gs::default::<AmbiguousTimestamp>());
    assert_all_examples(gs::default::<AmbiguousTimestamp>(), |t| {
        t.compatible().is_ok()
    });
}

#[test]
fn test_jiff_zoneds_stay_in_bounds_where_jiff_misreads_the_last_second_before_a_gap() {
    let min = DateTime::constant(1946, 5, 31, 0, 0, 0, 0);
    let max = DateTime::constant(1946, 5, 31, 23, 59, 59, 999_999_999);
    assert_all_examples(
        jiff_gs::zoneds()
            .timezones(jiff_gs::timezones().names(["Europe/Istanbul"]))
            .min_value(min)
            .max_value(max),
        move |z| z.datetime() >= min && z.datetime() <= max && resolved_as_jiff_would(z),
    );
}

#[test]
fn test_jiff_zoneds_stay_in_bounds_where_jiff_reports_a_fold_the_engine_does_not() {
    let min = DateTime::constant(2023, 10, 28, 23, 0, 0, 0);
    let max = DateTime::constant(2023, 10, 29, 23, 59, 59, 0);
    assert_all_examples(
        jiff_gs::zoneds()
            .timezones(jiff_gs::timezones().names(["America/Nuuk"]))
            .min_value(min)
            .max_value(max),
        move |z| z.datetime() >= min && z.datetime() <= max,
    );
}
