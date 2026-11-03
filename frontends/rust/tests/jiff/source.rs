use crate::common::utils::assert_all_examples;
use hegel::extras::jiff as jiff_gs;
use hegel::generators as gs;
use hegel::generators::temporal::{
    Date, DateTime, Instant, Time, TransitionKind, UtcOffset, ZoneSource,
};
use jiff::Timestamp;
use jiff::tz::{AmbiguousOffset, TimeZone};

fn engine_civil(second: i64) -> DateTime {
    let dt = Timestamp::from_second(second)
        .unwrap()
        .to_zoned(TimeZone::UTC)
        .datetime();
    DateTime::new(
        Date::new(i32::from(dt.year()), dt.month() as u8, dt.day() as u8).unwrap(),
        Time::new(dt.hour() as u8, dt.minute() as u8, dt.second() as u8, 0).unwrap(),
    )
    .unwrap()
}

fn engine_offset(source: &ZoneSource, name: &str, second: i64) -> i32 {
    let instant = Instant::new(second, 0).unwrap();
    match source.offset_at(name, instant).unwrap().offset {
        UtcOffset::Known(seconds) => seconds,
        UtcOffset::Unknown => panic!("{name} has an unknown offset"),
    }
}

fn jiff_offset(zone: &TimeZone, second: i64) -> i32 {
    zone.to_offset(Timestamp::from_second(second).unwrap())
        .seconds()
}

/// Check that the engine reads every transition of `zone` from `start` to
/// `end` as jiff does: the offsets on either side, and the wall time
/// halfway through the change as the gap or fold jiff says it is.
/// Returns the number of offset changes checked.
fn check_transitions(
    source: &ZoneSource,
    name: &str,
    zone: &TimeZone,
    start: i64,
    end: i64,
) -> usize {
    let mut changes = 0;
    for transition in zone.following(Timestamp::from_second(start).unwrap()) {
        let t = transition.timestamp().as_second();
        if t > end {
            break;
        }
        let before = jiff_offset(zone, t - 1);
        let after = jiff_offset(zone, t);
        assert_eq!(
            engine_offset(source, name, t - 1),
            before,
            "{name} before {t}"
        );
        assert_eq!(engine_offset(source, name, t), after, "{name} at {t}");
        if before == after {
            continue;
        }
        changes += 1;
        let wall =
            t + i64::from(before.min(after)) + i64::from((before - after).unsigned_abs() / 2);
        let resolved = source.resolve(name, engine_civil(wall)).unwrap();
        let jiff_wall = Timestamp::from_second(wall)
            .unwrap()
            .to_zoned(TimeZone::UTC)
            .datetime();
        let expected = match zone.to_ambiguous_timestamp(jiff_wall).offset() {
            AmbiguousOffset::Unambiguous { .. } => TransitionKind::Unique,
            AmbiguousOffset::Gap { .. } => TransitionKind::Gap,
            AmbiguousOffset::Fold { .. } => TransitionKind::Fold,
        };
        assert_eq!(resolved.kind, expected, "{name} at wall time {wall}");
    }
    changes
}

fn year_start(year: i16) -> i64 {
    jiff::civil::date(year, 1, 1)
        .to_zoned(TimeZone::UTC)
        .unwrap()
        .timestamp()
        .as_second()
}

#[test]
fn test_jiff_zone_source_matches_jiff_for_every_zone() {
    let source = jiff_gs::zone_source();
    let ranges = [
        (year_start(1800), year_start(2101)),
        (year_start(2101), year_start(2300)),
        (year_start(9900), Timestamp::MAX.as_second() - 86_400 * 4),
    ];
    let mut changes = [0; 3];
    for name in jiff::tz::db().available() {
        let name = name.as_str();
        let zone = TimeZone::get(name).unwrap();
        for (count, (start, end)) in changes.iter_mut().zip(ranges) {
            *count += check_transitions(&source, name, &zone, start, end);
        }
    }
    assert!(changes.iter().all(|&count| count > 1_000), "{changes:?}");
}

#[test]
fn test_jiff_zone_source_knows_jiffs_names() {
    let source = jiff_gs::zone_source();
    for name in jiff::tz::db().available() {
        assert!(source.info(name.as_str()).unwrap().is_some(), "{name}");
    }
}

#[test]
fn test_jiff_zone_source_version() {
    let version = jiff_gs::zone_source().version();
    assert!(
        regex::Regex::new(r"^[0-9]{4}[a-z]$")
            .unwrap()
            .is_match(&version),
        "{version:?}"
    );
}

#[test]
fn test_jiff_zone_source_is_the_source_of_zoned_draws() {
    assert_all_examples(
        gs::temporal::zoned_datetimes()
            .zones(gs::temporal::zones().source(jiff_gs::zone_source()))
            .min_value(engine_civil(946_684_800))
            .max_value(engine_civil(4_102_444_800)),
        |z| {
            let zone = TimeZone::get(&z.zone.name).unwrap();
            let offset = jiff_offset(&zone, z.instant.second);
            z.kind == TransitionKind::Gap || z.offset == UtcOffset::Known(offset)
        },
    );
}
