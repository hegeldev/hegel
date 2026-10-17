use super::*;
use crate::native::draws::special::{MAX_YEAR, MIN_YEAR, Time};
use crate::tzdata::{Tag, Tags, Transition};
use jiff::Timestamp as JiffTimestamp;
use jiff::civil::date;
use jiff::tz::{AmbiguousOffset, TimeZone as JiffZone};
use std::boxed::Box;
use std::string::{String, ToString};
use std::vec;
use std::vec::Vec;

fn all_names() -> &'static [Name] {
    tzdata::names().unwrap()
}

fn named(name: &str) -> TimeZone {
    TimeZone::lookup(name)
        .unwrap()
        .unwrap_or_else(|| panic!("no zone {name}"))
}

fn primary_name(zone: TimeZone) -> &'static str {
    match zone {
        TimeZone::Unknown => UNKNOWN_NAME,
        TimeZone::Named(name) => &name.zone.name,
    }
}

fn zone_of(name: &str) -> &'static Zone {
    match named(name) {
        TimeZone::Named(name) => name.zone,
        TimeZone::Unknown => panic!("{name} is the unknown zone"),
    }
}

fn jiff_zone(name: &str) -> JiffZone {
    JiffZone::get(name).unwrap_or_else(|e| panic!("jiff has no zone {name}: {e}"))
}

fn at(second: i64) -> JiffTimestamp {
    JiffTimestamp::from_second(second).unwrap()
}

fn whole(second: i64) -> Timestamp {
    Timestamp {
        second,
        nanosecond: 0,
    }
}

fn instant_of(year: i16, month: i8, day: i8) -> i64 {
    date(year, month, day)
        .to_zoned(JiffZone::UTC)
        .unwrap()
        .timestamp()
        .as_second()
}

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> DateTime {
    DateTime {
        date: Date { year, month, day },
        time: Time {
            hour,
            minute,
            second,
            nanosecond: 0,
        },
    }
}

fn civil_of(wall: i64) -> DateTime {
    let date = civil_from_days(wall.div_euclid(DAY));
    let s = wall.rem_euclid(DAY);
    civil(
        date.year,
        date.month,
        date.day,
        (s / 3600) as u8,
        (s / 60 % 60) as u8,
        (s % 60) as u8,
    )
}

fn jiff_civil(dt: &DateTime) -> jiff::civil::DateTime {
    jiff::civil::DateTime::new(
        i16::try_from(dt.date.year).unwrap(),
        i8::try_from(dt.date.month).unwrap(),
        i8::try_from(dt.date.day).unwrap(),
        i8::try_from(dt.time.hour).unwrap(),
        i8::try_from(dt.time.minute).unwrap(),
        i8::try_from(dt.time.second).unwrap(),
        i32::try_from(dt.time.nanosecond).unwrap(),
    )
    .unwrap()
}

/// Every transition of `zone` up to 2201-01-01: the explicit ones and the
/// footer's from the last explicit one on.
fn transitions_to_2200(zone: &Zone) -> Vec<ZoneTransition> {
    transitions(zone, i64::MIN, instant_of(2201, 1, 1)).collect()
}

/// The table folds the Etc/GMT family into UTC, whose abbreviation jiff
/// spells "GMT" for those names.
fn same_abbreviation(zone: TimeZone, ours: &str, theirs: &str) -> bool {
    ours == theirs || (primary_name(zone) == "UTC" && ["UTC", "GMT"].contains(&theirs))
}

/// Drop the entries that change neither offset nor abbreviation, where
/// the main and rearguard data differ only in the DST flag.
fn offset_changes(list: Vec<(i64, i32, String)>) -> Vec<(i64, i32, String)> {
    let mut kept: Vec<(i64, i32, String)> = Vec::new();
    for (i, entry) in list.iter().enumerate() {
        if i == 0 || (list[i - 1].1, &list[i - 1].2) != (entry.1, &entry.2) {
            kept.push(entry.clone());
        }
    }
    kept
}

fn check_offset(name: &str, tz: &JiffZone, zone: TimeZone, second: i64) {
    let ours = zone.offset_at(whole(second)).unwrap();
    let theirs = tz.to_offset_info(at(second));
    let context = format!("{name} at {second}");
    assert_eq!(ours.seconds, theirs.offset().seconds(), "{context}");
    assert!(
        same_abbreviation(zone, ours.abbreviation, theirs.abbreviation()),
        "{context}: {:?} against {:?}",
        ours.abbreviation,
        theirs.abbreviation()
    );
    assert!(ours.known, "{context}");
}

#[test]
fn offsets_agree_with_jiff_around_every_transition() {
    for entry in all_names() {
        let name = entry.name.as_str();
        let tz = jiff_zone(name);
        let zone = TimeZone::Named(entry);
        let points = transitions_to_2200(entry.zone);
        assert!(
            points.len() >= entry.zone.transitions.len(),
            "{name} lost explicit transitions"
        );
        for t in &points {
            for second in [t.instant - 1, t.instant, t.instant + 1] {
                check_offset(name, &tz, zone, second);
            }
        }
        for second in [
            instant_of(1800, 1, 1),
            instant_of(1970, 1, 1),
            instant_of(2000, 1, 1),
            instant_of(2200, 7, 1),
        ] {
            check_offset(name, &tz, zone, second);
        }
    }
}

#[test]
fn transitions_agree_with_jiff() {
    let end = instant_of(2201, 1, 1);
    for entry in all_names() {
        let name = entry.name.as_str();
        let tz = jiff_zone(name);
        let zone = entry.zone;
        let from = zone
            .transitions
            .first()
            .map_or(instant_of(1900, 1, 1), |t| t.instant - 1);
        let ours: Vec<(i64, i32, String)> = transitions(zone, from, end)
            .map(|t| {
                (
                    t.instant,
                    t.after.utoff,
                    abbreviation(zone, t.after).unwrap().to_string(),
                )
            })
            .collect();
        let theirs: Vec<(i64, i32, String)> = tz
            .following(at(from))
            .take_while(|t| t.timestamp().as_second() < end)
            .map(|t| {
                (
                    t.timestamp().as_second(),
                    t.offset().seconds(),
                    t.abbreviation().to_string(),
                )
            })
            .collect();
        let explicit: Vec<i64> = zone.transitions.iter().map(|t| t.instant).collect();
        let ours = ours
            .into_iter()
            .map(|(instant, utoff, abbreviation)| {
                let abbreviation = if zone.name == "UTC" && abbreviation == "UTC" {
                    "GMT".to_string()
                } else {
                    abbreviation
                };
                (instant, utoff, abbreviation)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            offset_changes(ours.clone()),
            offset_changes(theirs),
            "{name}"
        );
        assert_eq!(
            &ours[..explicit.len()]
                .iter()
                .map(|t| t.0)
                .collect::<Vec<i64>>(),
            &explicit,
            "{name}"
        );
    }
}

/// Whether `wall` lies within two days of a last explicit transition that
/// changes nothing (zic's way of starting the footer rule at a chosen
/// instant, as in America/Nuuk 2023). jiff answers such wall times from
/// the footer rule alone and can report a fold or gap the explicit data
/// rules out, so the oracle is not consulted there.
fn near_a_dummy_last_transition(zone: &Zone, wall: i64) -> bool {
    let n = zone.transitions.len();
    zone.transitions.last().is_some_and(|last| {
        let before = if n >= 2 {
            zone.transitions[n - 2].kind
        } else {
            zone.initial
        };
        before.utoff == last.kind.utoff
            && (last.instant - WINDOW..=last.instant + WINDOW).contains(&wall)
    })
}

fn check_resolution(name: &str, tz: &JiffZone, zone: TimeZone, wall: i64) {
    let dt = civil_of(wall);
    if let TimeZone::Named(entry) = zone {
        if near_a_dummy_last_transition(entry.zone, wall) {
            return;
        }
    }
    let earlier = zone.resolve(&dt, false).unwrap();
    let later = zone.resolve(&dt, true).unwrap();
    let theirs = tz.to_ambiguous_timestamp(jiff_civil(&dt));
    let context = format!("{name} at wall {wall} ({dt:?})");
    let (kind, before, after) = match theirs.offset() {
        AmbiguousOffset::Unambiguous { offset } => (Kind::Unique, offset, offset),
        AmbiguousOffset::Gap { before, after } => (Kind::Gap, before, after),
        AmbiguousOffset::Fold { before, after } => (Kind::Fold, before, after),
    };
    assert_eq!(earlier.kind, kind, "{context}");
    assert_eq!(earlier.offset_before, before.seconds(), "{context}");
    assert_eq!(earlier.offset_after, after.seconds(), "{context}");
    assert!(earlier.offset_known, "{context}");
    assert_eq!(
        earlier.timestamp.second,
        theirs.compatible().unwrap().as_second(),
        "{context}"
    );
    assert_eq!(earlier.timestamp.nanosecond, 0, "{context}");
    assert_eq!(
        earlier.offset,
        tz.to_offset(at(earlier.timestamp.second)).seconds(),
        "{context}"
    );
    if kind == Kind::Fold {
        assert_eq!(
            later.timestamp.second,
            theirs.later().unwrap().as_second(),
            "{context}"
        );
        assert_eq!(
            later.offset,
            tz.to_offset(at(later.timestamp.second)).seconds(),
            "{context}"
        );
        assert_eq!(
            Resolution {
                timestamp: earlier.timestamp,
                offset: earlier.offset,
                ..later
            },
            earlier,
            "{context}"
        );
    } else {
        assert_eq!(later, earlier, "{context}");
    }
}

#[test]
fn resolutions_agree_with_jiff_around_every_transition() {
    for entry in all_names() {
        let name = entry.name.as_str();
        let tz = jiff_zone(name);
        let zone = TimeZone::Named(entry);
        for t in transitions_to_2200(entry.zone) {
            let (before, after) = (
                t.instant + i64::from(t.before.utoff),
                t.instant + i64::from(t.after.utoff),
            );
            let (low, high) = (before.min(after), before.max(after));
            for wall in [
                before - 1,
                before,
                before + 1,
                after - 1,
                after,
                after + 1,
                low + (high - low) / 2,
            ] {
                check_resolution(name, &tz, zone, wall);
            }
        }
        for wall in [
            instant_of(1800, 6, 1),
            instant_of(1970, 1, 1),
            instant_of(2000, 1, 1),
            instant_of(2200, 7, 1),
        ] {
            check_resolution(name, &tz, zone, wall);
        }
    }
}

#[test]
fn every_transition_is_consistent_with_type_at() {
    let end = instant_of(2401, 1, 1);
    for entry in all_names() {
        let zone = entry.zone;
        let mut previous: Option<ZoneTransition> = None;
        let mut count = 0;
        let last_explicit = zone.transitions.last().map_or(i64::MIN, |t| t.instant);
        for t in transitions(zone, i64::MIN, end) {
            let context = format!("{} at {}", entry.name, t.instant);
            if t.instant > last_explicit {
                assert_ne!(t.before, t.after, "{context}");
            }
            if let Some(p) = previous {
                assert!(p.instant < t.instant, "{context}");
                assert_eq!(p.after, t.before, "{context}");
            }
            assert_eq!(type_at(zone, t.instant - 1).unwrap(), t.before, "{context}");
            assert_eq!(type_at(zone, t.instant).unwrap(), t.after, "{context}");
            previous = Some(t);
            count += 1;
        }
        if zone.footer.unwrap().dst.is_some() {
            assert!(count >= 2 * (2400 - 2200), "{}", entry.name);
        }
    }
}

#[test]
fn pacific_apia_skipped_2011_12_30() {
    let apia = named("Pacific/Apia");
    let r = apia.resolve(&civil(2011, 12, 30, 12, 0, 0), false).unwrap();
    assert_eq!(r.kind, Kind::Gap);
    assert_eq!((r.offset_before, r.offset_after), (-36000, 50400));
    assert_eq!(r.offset, 50400);
    assert_eq!(r.timestamp, whole(1325239200 + 12 * 3600));
    assert_eq!(
        apia.resolve(&civil(2011, 12, 30, 12, 0, 0), true).unwrap(),
        r
    );
    let last_before = apia
        .resolve(&civil(2011, 12, 29, 23, 59, 59), false)
        .unwrap();
    assert_eq!(last_before.kind, Kind::Unique);
    assert_eq!(last_before.timestamp, whole(1325239200 - 1));
    let first_after = apia.resolve(&civil(2011, 12, 31, 0, 0, 0), false).unwrap();
    assert_eq!(first_after.kind, Kind::Unique);
    assert_eq!(first_after.timestamp, whole(1325239200));
    let before = apia.offset_at(whole(1325239200 - 1)).unwrap();
    let after = apia.offset_at(whole(1325239200)).unwrap();
    assert_eq!((before.seconds, before.abbreviation), (-36000, "-10"));
    assert_eq!((after.seconds, after.abbreviation), (50400, "+14"));
}

#[test]
fn europe_dublin_has_negative_dst() {
    let dublin = named("Europe/Dublin");
    let winter = dublin.offset_at(whole(instant_of(2024, 1, 15))).unwrap();
    let summer = dublin.offset_at(whole(instant_of(2024, 7, 15))).unwrap();
    assert_eq!((winter.seconds, winter.abbreviation), (0, "GMT"));
    assert_eq!((summer.seconds, summer.abbreviation), (3600, "IST"));
    assert!(winter.is_dst);
    assert!(!summer.is_dst);
    let footer = zone_of("Europe/Dublin").footer.unwrap();
    assert_eq!(footer.standard.utoff, 3600);
    assert_eq!(footer.dst.unwrap().kind.utoff, 0);
    let fold = dublin
        .resolve(&civil(2024, 10, 27, 1, 30, 0), false)
        .unwrap();
    assert_eq!(fold.kind, Kind::Fold);
    assert_eq!((fold.offset_before, fold.offset_after), (3600, 0));
    assert_eq!(fold.timestamp, whole(instant_of(2024, 10, 27) + 1800));
    let later = dublin
        .resolve(&civil(2024, 10, 27, 1, 30, 0), true)
        .unwrap();
    assert_eq!(later.timestamp, whole(instant_of(2024, 10, 27) + 5400));
    assert_eq!(later.offset, 0);
    let gap = dublin
        .resolve(&civil(2024, 3, 31, 1, 30, 0), false)
        .unwrap();
    assert_eq!(gap.kind, Kind::Gap);
    assert_eq!((gap.offset_before, gap.offset_after), (0, 3600));
    assert_eq!(gap.timestamp, whole(instant_of(2024, 3, 31) + 5400));
}

#[test]
fn africa_monrovia_dropped_its_seconds_in_1972() {
    let monrovia = named("Africa/Monrovia");
    let before = monrovia.offset_at(whole(63593070 - 1)).unwrap();
    let after = monrovia.offset_at(whole(63593070)).unwrap();
    assert_eq!((before.seconds, before.abbreviation), (-2670, "MMT"));
    assert_eq!((after.seconds, after.abbreviation), (0, "GMT"));
    let gap = monrovia
        .resolve(&civil(1972, 1, 7, 0, 20, 0), false)
        .unwrap();
    assert_eq!(gap.kind, Kind::Gap);
    assert_eq!((gap.offset_before, gap.offset_after), (-2670, 0));
    assert_eq!(gap.timestamp, whole(63593070 - 2670 + 1200 + 2670));
    let edge = monrovia
        .resolve(&civil(1972, 1, 7, 0, 44, 30), false)
        .unwrap();
    assert_eq!(edge.kind, Kind::Unique);
    assert_eq!(edge.timestamp, whole(63593070));
}

#[test]
fn europe_lisbon_1992_changed_only_the_abbreviation() {
    let lisbon = named("Europe/Lisbon");
    let before = lisbon.offset_at(whole(717555600 - 1)).unwrap();
    let after = lisbon.offset_at(whole(717555600)).unwrap();
    assert_eq!(
        (before.seconds, before.abbreviation, before.is_dst),
        (3600, "WEST", true)
    );
    assert_eq!(
        (after.seconds, after.abbreviation, after.is_dst),
        (3600, "CET", false)
    );
    let t = transitions(zone_of("Europe/Lisbon"), 717555600, 717555601)
        .next()
        .unwrap();
    assert_eq!(t.instant, 717555600);
    assert_eq!(t.before.utoff, t.after.utoff);
    assert_ne!(t.before, t.after);
    for dt in [
        civil(1992, 9, 27, 1, 59, 59),
        civil(1992, 9, 27, 2, 0, 0),
        civil(1992, 9, 27, 2, 0, 1),
    ] {
        let r = lisbon.resolve(&dt, false).unwrap();
        assert_eq!(r.kind, Kind::Unique, "{dt:?}");
        assert_eq!(r.offset, 3600, "{dt:?}");
    }
}

#[test]
fn asia_pyongyang_2015_fold_spans_midnight() {
    let pyongyang = named("Asia/Pyongyang");
    let fold = pyongyang
        .resolve(&civil(2015, 8, 14, 23, 45, 0), false)
        .unwrap();
    assert_eq!(fold.kind, Kind::Fold);
    assert_eq!((fold.offset_before, fold.offset_after), (32400, 30600));
    assert_eq!(fold.timestamp, whole(1439564400 - 900));
    let later = pyongyang
        .resolve(&civil(2015, 8, 14, 23, 45, 0), true)
        .unwrap();
    assert_eq!(later.timestamp, whole(1439564400 + 900));
    assert_eq!(
        pyongyang
            .resolve(&civil(2015, 8, 15, 0, 0, 0), false)
            .unwrap()
            .kind,
        Kind::Unique
    );
    let gap = pyongyang
        .resolve(&civil(2018, 5, 4, 23, 45, 0), false)
        .unwrap();
    assert_eq!(gap.kind, Kind::Gap);
    assert_eq!((gap.offset_before, gap.offset_after), (30600, 32400));
    assert_eq!(gap.timestamp, whole(1525446000 + 900));
    assert_eq!(gap.offset, 32400);
}

#[test]
fn the_etc_gmt_family_inverts_the_sign() {
    for (name, seconds, abbreviation) in [
        ("Etc/GMT+5", -18000, "-05"),
        ("Etc/GMT-14", 50400, "+14"),
        ("Etc/GMT+12", -43200, "-12"),
        ("Etc/GMT-0", 0, "UTC"),
    ] {
        let zone = named(name);
        for second in [
            MIN_INSTANT,
            instant_of(1800, 1, 1),
            0,
            instant_of(2400, 1, 1),
            MAX_INSTANT,
        ] {
            let offset = zone.offset_at(whole(second)).unwrap();
            assert_eq!(
                (offset.seconds, offset.abbreviation, offset.is_dst),
                (seconds, abbreviation, false),
                "{name} at {second}"
            );
        }
        let r = zone.resolve(&civil(2024, 6, 1, 12, 0, 0), false).unwrap();
        assert_eq!(r.kind, Kind::Unique, "{name}");
        assert_eq!(r.offset, seconds, "{name}");
        assert_eq!(
            r.timestamp,
            whole(instant_of(2024, 6, 1) + 12 * 3600 - i64::from(seconds))
        );
        assert_eq!(
            transitions(zone_of(name), i64::MIN, i64::MAX).count(),
            0,
            "{name}"
        );
        match zone {
            TimeZone::Named(entry) => assert!(
                entry.tags.contains(Tag::InvertedSign) || name == "Etc/GMT-0",
                "{name}"
            ),
            TimeZone::Unknown => panic!("{name}"),
        }
    }
}

#[test]
fn the_unknown_zone_has_utc_civil_time_and_no_offset() {
    let unknown = TimeZone::lookup(UNKNOWN_NAME).unwrap().unwrap();
    assert!(matches!(unknown, TimeZone::Unknown));
    assert_eq!(primary_name(unknown), "Etc/Unknown");
    let offset = unknown.offset_at(whole(instant_of(2024, 6, 1))).unwrap();
    assert_eq!(
        offset,
        Offset {
            seconds: 0,
            known: false,
            is_dst: false,
            abbreviation: "-00",
        }
    );
    let mut dt = civil(2024, 6, 1, 12, 34, 56);
    dt.time.nanosecond = 789;
    let r = unknown.resolve(&dt, true).unwrap();
    assert_eq!(
        r,
        Resolution {
            kind: Kind::Unique,
            offset_before: 0,
            offset_after: 0,
            offset: 0,
            offset_known: false,
            timestamp: Timestamp {
                second: instant_of(2024, 6, 1) + 12 * 3600 + 34 * 60 + 56,
                nanosecond: 789,
            },
        }
    );
    assert_eq!(UNKNOWN_NAME_CSTR.to_str().unwrap(), UNKNOWN_NAME);
    assert_eq!(
        UNKNOWN_ABBREVIATION_CSTR.to_str().unwrap(),
        UNKNOWN_ABBREVIATION
    );
}

#[test]
fn lookup_is_exact_and_links_keep_their_name() {
    assert!(TimeZone::lookup("Nowhere/Nope").unwrap().is_none());
    assert!(TimeZone::lookup("utc").unwrap().is_none());
    assert!(TimeZone::lookup("Etc/unknown").unwrap().is_none());
    let calcutta = named("Asia/Calcutta");
    assert_eq!(primary_name(calcutta), "Asia/Kolkata");
    match calcutta {
        TimeZone::Named(entry) => {
            assert_eq!(entry.name, "Asia/Calcutta");
            assert!(entry.is_link());
        }
        TimeZone::Unknown => panic!(),
    }
    assert_eq!(primary_name(named("UTC")), "UTC");
}

#[test]
fn invalid_inputs_are_argument_errors() {
    let zone = named("America/New_York");
    let bad_nanos = Timestamp {
        second: 0,
        nanosecond: 1_000_000_000,
    };
    let err = zone.offset_at(bad_nanos).unwrap_err();
    assert!(
        matches!(err, EngineError::InvalidArgument(ref m) if m.contains("nanosecond")),
        "{err:?}"
    );
    assert!(matches!(
        TimeZone::Unknown.offset_at(bad_nanos),
        Err(EngineError::InvalidArgument(_))
    ));
    for second in [MIN_INSTANT - 1, MAX_INSTANT + 1] {
        let err = zone.offset_at(whole(second)).unwrap_err();
        assert!(
            matches!(err, EngineError::InvalidArgument(ref m) if m.contains("timestamp.second")),
            "{err:?}"
        );
        assert!(matches!(
            TimeZone::Unknown.offset_at(whole(second)),
            Err(EngineError::InvalidArgument(_))
        ));
    }
    let err = zone
        .resolve(&civil(2024, 2, 30, 0, 0, 0), false)
        .unwrap_err();
    assert!(
        matches!(err, EngineError::InvalidArgument(ref m) if m.contains("civil.date")),
        "{err:?}"
    );
    let err = zone
        .resolve(&civil(2024, 2, 29, 24, 0, 0), false)
        .unwrap_err();
    assert!(
        matches!(err, EngineError::InvalidArgument(ref m) if m.contains("civil.time")),
        "{err:?}"
    );
    assert!(matches!(
        TimeZone::Unknown.resolve(&civil(2024, 13, 1, 0, 0, 0), false),
        Err(EngineError::InvalidArgument(_))
    ));
}

#[test]
fn nanoseconds_ride_along_and_never_change_the_offset() {
    let zone = named("America/New_York");
    let second = instant_of(2024, 3, 10) + 7 * 3600 - 1;
    let whole_second = zone.offset_at(whole(second)).unwrap();
    let late = zone
        .offset_at(Timestamp {
            second,
            nanosecond: 999_999_999,
        })
        .unwrap();
    assert_eq!(whole_second, late);
    assert_eq!(whole_second.seconds, -18000);
    assert_eq!(zone.offset_at(whole(second + 1)).unwrap().seconds, -14400);
    let mut dt = civil(2024, 3, 10, 2, 30, 0);
    dt.time.nanosecond = 123_456_789;
    let r = zone.resolve(&dt, false).unwrap();
    assert_eq!(r.kind, Kind::Gap);
    assert_eq!(r.timestamp.nanosecond, 123_456_789);
}

#[test]
fn extreme_instants_are_rejected_by_queries_and_clamped_by_type_at() {
    assert_eq!(
        MIN_INSTANT,
        days_from_civil(&Date {
            year: MIN_YEAR,
            month: 1,
            day: 1
        }) * DAY
    );
    assert_eq!(
        MAX_INSTANT,
        days_from_civil(&Date {
            year: MAX_YEAR,
            month: 12,
            day: 31
        }) * DAY
            + DAY
            - 1
    );
    for entry in all_names() {
        let zone = TimeZone::Named(entry);
        for second in [MIN_INSTANT, MAX_INSTANT] {
            let offset = zone.offset_at(whole(second)).unwrap();
            let kind = type_at(entry.zone, second).unwrap();
            assert_eq!(offset.seconds, kind.utoff, "{} at {second}", entry.name);
        }
        for second in [i64::MIN, MIN_INSTANT - 1, MAX_INSTANT + 1, i64::MAX] {
            let err = zone.offset_at(whole(second)).unwrap_err();
            assert!(
                matches!(err, EngineError::InvalidArgument(ref m) if m.contains("timestamp.second")),
                "{} at {second}: {err:?}",
                entry.name
            );
            let clamped = second.clamp(MIN_INSTANT, MAX_INSTANT);
            assert_eq!(
                type_at(entry.zone, second).unwrap(),
                type_at(entry.zone, clamped).unwrap(),
                "{} at {second}",
                entry.name
            );
        }
        let first = civil(MIN_YEAR, 1, 1, 0, 0, 0);
        let last = civil(MAX_YEAR, 12, 31, 23, 59, 59);
        let r = zone.resolve(&first, false).unwrap();
        assert_eq!(r.kind, Kind::Unique, "{}", entry.name);
        assert_eq!(r.offset, entry.zone.initial.utoff, "{}", entry.name);
        assert_eq!(r.timestamp.second, MIN_INSTANT - i64::from(r.offset));
        let r = zone.resolve(&last, false).unwrap();
        assert_eq!(r.timestamp.second, MAX_INSTANT - i64::from(r.offset));
        assert!(transitions(entry.zone, i64::MIN, i64::MAX).take(3).count() <= 3);
        assert_eq!(transitions(entry.zone, 0, 0).count(), 0);
        assert_eq!(transitions(entry.zone, 10, 0).count(), 0);
        assert_eq!(transitions(entry.zone, i64::MAX, i64::MAX).count(), 0);
        assert_eq!(transitions(entry.zone, i64::MIN, i64::MIN).count(), 0);
    }
}

#[test]
fn far_future_transitions_follow_the_footer_rule() {
    let zone = zone_of("America/New_York");
    let from = instant_of(9000, 1, 1);
    let ours: Vec<i64> = transitions(zone, from, instant_of(9002, 1, 1))
        .map(|t| t.instant)
        .collect();
    let tz = jiff_zone("America/New_York");
    let theirs: Vec<i64> = tz
        .following(at(from))
        .take(4)
        .map(|t| t.timestamp().as_second())
        .collect();
    assert_eq!(ours, theirs);
    let year_400000 = days_from_civil(&Date {
        year: 400_000,
        month: 1,
        day: 1,
    }) * DAY;
    let far: Vec<ZoneTransition> =
        transitions(zone, year_400000, year_400000 + 366 * DAY).collect();
    assert_eq!(far.len(), 2);
    assert_eq!(far[0].after.utoff, -14400);
    assert_eq!(far[1].after.utoff, -18000);
    assert_eq!(type_at(zone, far[0].instant).unwrap(), far[0].after);
}

fn ltt(utoff: i32, is_dst: bool, abbreviation: usize) -> LocalTimeType {
    LocalTimeType {
        utoff,
        is_dst,
        abbreviation,
    }
}

fn rule(day: RuleDay, time: i32) -> RuleDate {
    RuleDate { day, time }
}

fn posix_zone(name: &str, standard: LocalTimeType, dst: Option<DstRule>) -> Zone {
    Zone {
        name: name.to_string(),
        abbreviations: vec!["STD".to_string(), "DST".to_string()],
        initial: standard,
        transitions: Vec::new(),
        footer: Some(Footer { standard, dst }),
    }
}

fn est_edt(start: RuleDate, end: RuleDate) -> Zone {
    posix_zone(
        "synthetic",
        ltt(-18000, false, 0),
        Some(DstRule {
            kind: ltt(-14400, true, 1),
            start,
            end,
        }),
    )
}

/// Compare a synthetic footer-only zone with jiff's reading of the POSIX
/// string it came from, at and around every transition of 1970 to 2100.
fn check_against_posix(zone: &Zone, posix: &str) {
    let tz = JiffZone::posix(posix).unwrap();
    let (from, to) = (instant_of(1970, 1, 1), instant_of(2101, 1, 1));
    let ours: Vec<(i64, i32)> = transitions(zone, from, to)
        .map(|t| (t.instant, t.after.utoff))
        .collect();
    let theirs: Vec<(i64, i32)> = tz
        .following(at(from))
        .take_while(|t| t.timestamp().as_second() < to)
        .map(|t| (t.timestamp().as_second(), t.offset().seconds()))
        .collect();
    assert_eq!(ours, theirs, "{posix}");
    assert_eq!(ours.len(), 2 * 131, "{posix}");
    for (instant, _) in ours {
        for second in [instant - 1, instant, instant + 1] {
            assert_eq!(
                type_at(zone, second).unwrap().utoff,
                tz.to_offset(at(second)).seconds(),
                "{posix} at {second}"
            );
        }
        for wall in [instant - 18000, instant - 14400, instant - 16200] {
            let dt = civil_of(wall);
            let ours = resolve(zone, &dt, false).unwrap();
            let theirs = tz.to_ambiguous_timestamp(jiff_civil(&dt));
            let kind = match theirs.offset() {
                AmbiguousOffset::Unambiguous { .. } => Kind::Unique,
                AmbiguousOffset::Gap { .. } => Kind::Gap,
                AmbiguousOffset::Fold { .. } => Kind::Fold,
            };
            assert_eq!(ours.kind, kind, "{posix} at wall {wall}");
            assert_eq!(
                ours.timestamp.second,
                theirs.compatible().unwrap().as_second(),
                "{posix} at wall {wall}"
            );
        }
    }
}

#[test]
fn julian_day_rules_agree_with_jiff() {
    check_against_posix(
        &est_edt(
            rule(RuleDay::Julian(60), 7200),
            rule(RuleDay::Julian(300), 7200),
        ),
        "EST5EDT,J60/2,J300/2",
    );
    check_against_posix(
        &est_edt(
            rule(RuleDay::Julian(1), 6 * 3600),
            rule(RuleDay::Julian(365), 18 * 3600),
        ),
        "EST5EDT,J1/6,J365/18",
    );
}

#[test]
fn zero_based_day_rules_agree_with_jiff() {
    check_against_posix(
        &est_edt(
            rule(RuleDay::ZeroBased(59), 7200),
            rule(RuleDay::ZeroBased(299), 7200),
        ),
        "EST5EDT,59/2,299/2",
    );
    check_against_posix(
        &est_edt(
            rule(RuleDay::ZeroBased(0), 6 * 3600),
            rule(RuleDay::ZeroBased(364), 18 * 3600),
        ),
        "EST5EDT,0/6,364/18",
    );
}

#[test]
fn month_week_day_rules_agree_with_jiff() {
    check_against_posix(
        &est_edt(
            rule(
                RuleDay::MonthWeekDay {
                    month: 3,
                    week: 2,
                    weekday: 0,
                },
                7200,
            ),
            rule(
                RuleDay::MonthWeekDay {
                    month: 11,
                    week: 1,
                    weekday: 0,
                },
                7200,
            ),
        ),
        "EST5EDT,M3.2.0/2,M11.1.0/2",
    );
    check_against_posix(
        &est_edt(
            rule(
                RuleDay::MonthWeekDay {
                    month: 2,
                    week: 5,
                    weekday: 6,
                },
                86400,
            ),
            rule(
                RuleDay::MonthWeekDay {
                    month: 12,
                    week: 5,
                    weekday: 1,
                },
                0,
            ),
        ),
        "EST5EDT,M2.5.6/24,M12.5.1/0",
    );
}

#[test]
fn rule_times_beyond_a_day_land_on_the_following_days() {
    let week_2_sunday = RuleDay::MonthWeekDay {
        month: 3,
        week: 2,
        weekday: 0,
    };
    let november = rule(
        RuleDay::MonthWeekDay {
            month: 11,
            week: 1,
            weekday: 0,
        },
        7200,
    );
    let (from, to) = (instant_of(1970, 1, 1), instant_of(2101, 1, 1));
    let starts = |time: i32| -> Vec<i64> {
        transitions(&est_edt(rule(week_2_sunday, time), november), from, to)
            .filter(|t| t.after.utoff == -14400)
            .map(|t| t.instant)
            .collect()
    };
    let usual = starts(2 * 3600);
    assert_eq!(usual.len(), 131);
    for hours in [26, -22, 167, -167, 0, 24] {
        let shifted = starts(hours * 3600);
        let expected: Vec<i64> = usual
            .iter()
            .map(|t| t + i64::from(hours - 2) * 3600)
            .collect();
        assert_eq!(shifted, expected, "{hours} hours");
    }
    let tz = JiffZone::posix("EST5EDT,M3.2.1/2,M11.1.0/2").unwrap();
    let monday: Vec<i64> = tz
        .following(at(from))
        .take_while(|t| t.timestamp().as_second() < to)
        .filter(|t| t.offset().seconds() == -14400)
        .map(|t| t.timestamp().as_second())
        .collect();
    assert_ne!(starts(26 * 3600), monday);
}

#[test]
fn all_year_dst_has_no_transitions() {
    let zone = est_edt(
        rule(RuleDay::ZeroBased(0), 0),
        rule(RuleDay::Julian(365), 25 * 3600),
    );
    assert_eq!(transitions(&zone, i64::MIN, i64::MAX).count(), 0);
    for second in [
        i64::MIN,
        instant_of(1999, 12, 31) + 86399,
        instant_of(2000, 1, 1),
        instant_of(2000, 1, 1) + 3600 * 5,
        instant_of(2000, 7, 1),
        i64::MAX,
    ] {
        assert_eq!(
            type_at(&zone, second).unwrap(),
            zone.footer.unwrap().dst.unwrap().kind,
            "{second}"
        );
    }
    let r = resolve(&zone, &civil(2000, 1, 1, 0, 30, 0), false).unwrap();
    assert_eq!(r.kind, Kind::Unique);
    assert_eq!(r.offset, -14400);
}

#[test]
fn a_dst_rule_with_the_standard_offset_changes_only_the_type() {
    let zone = posix_zone(
        "same",
        ltt(3600, false, 0),
        Some(DstRule {
            kind: ltt(3600, true, 1),
            start: rule(RuleDay::Julian(100), 0),
            end: rule(RuleDay::Julian(200), 0),
        }),
    );
    let year: Vec<ZoneTransition> =
        transitions(&zone, instant_of(2024, 1, 1), instant_of(2025, 1, 1)).collect();
    assert_eq!(year.len(), 2);
    assert!(year.iter().all(|t| t.before.utoff == t.after.utoff));
    assert!(year[0].after.is_dst && !year[1].after.is_dst);
    for wall in [
        year[0].instant + 3600,
        year[1].instant + 3600,
        year[0].instant + 3600 - 1,
    ] {
        let r = resolve(&zone, &civil_of(wall), false).unwrap();
        assert_eq!(r.kind, Kind::Unique);
        assert_eq!(r.timestamp.second, wall - 3600);
    }
}

#[test]
fn rules_whose_years_overlap_stay_in_order() {
    let zone = est_edt(
        rule(
            RuleDay::MonthWeekDay {
                month: 1,
                week: 1,
                weekday: 0,
            },
            -167 * 3600,
        ),
        rule(
            RuleDay::MonthWeekDay {
                month: 12,
                week: 5,
                weekday: 0,
            },
            167 * 3600,
        ),
    );
    let all: Vec<ZoneTransition> =
        transitions(&zone, instant_of(1990, 1, 1), instant_of(2010, 1, 1)).collect();
    assert_eq!(all.len(), 40);
    for pair in all.windows(2) {
        assert!(pair[0].instant < pair[1].instant);
        assert_eq!(pair[0].after, pair[1].before);
    }
    for t in &all {
        assert_eq!(type_at(&zone, t.instant - 1).unwrap(), t.before);
        assert_eq!(type_at(&zone, t.instant).unwrap(), t.after);
        let dt = civil_of(t.instant + i64::from(t.before.utoff.min(t.after.utoff)) + 1800);
        let r = resolve(&zone, &dt, false).unwrap();
        assert_eq!(
            r.kind,
            if t.after.utoff > t.before.utoff {
                Kind::Gap
            } else {
                Kind::Fold
            }
        );
    }
}

#[test]
fn a_zone_without_a_footer_keeps_its_last_type() {
    let zone = Zone {
        name: "frozen".to_string(),
        abbreviations: vec!["A".to_string(), "B".to_string()],
        initial: ltt(0, false, 0),
        transitions: vec![
            Transition {
                instant: 100,
                kind: ltt(3600, false, 1),
            },
            Transition {
                instant: 200,
                kind: ltt(7200, false, 0),
            },
        ],
        footer: None,
    };
    for (second, utoff) in [
        (i64::MIN, 0),
        (99, 0),
        (100, 3600),
        (199, 3600),
        (200, 7200),
        (i64::MAX, 7200),
    ] {
        assert_eq!(type_at(&zone, second).unwrap().utoff, utoff, "{second}");
    }
    let all: Vec<ZoneTransition> = transitions(&zone, i64::MIN, i64::MAX).collect();
    assert_eq!(all.len(), 2);
    assert_eq!((all[0].before.utoff, all[0].after.utoff), (0, 3600));
    assert_eq!((all[1].before.utoff, all[1].after.utoff), (3600, 7200));
    assert_eq!(transitions(&zone, 101, 200).count(), 0);
    assert_eq!(transitions(&zone, 100, 201).count(), 2);
    let r = resolve(&zone, &civil_of(100 + 1800), false).unwrap();
    assert_eq!(r.kind, Kind::Gap);
    assert_eq!((r.offset_before, r.offset_after), (0, 3600));
    let empty = Zone {
        name: "empty".to_string(),
        abbreviations: vec!["Z".to_string()],
        initial: ltt(-30, true, 0),
        transitions: Vec::new(),
        footer: None,
    };
    assert_eq!(type_at(&empty, 0).unwrap(), empty.initial);
    assert_eq!(transitions(&empty, i64::MIN, i64::MAX).count(), 0);
    let r = resolve(&empty, &civil(2000, 1, 1, 0, 0, 0), true).unwrap();
    assert_eq!(
        (r.kind, r.offset, r.timestamp.second),
        (Kind::Unique, -30, instant_of(2000, 1, 1) + 30)
    );
}

#[test]
fn a_footer_without_dst_applies_from_the_last_transition() {
    let zone = Zone {
        name: "settled".to_string(),
        abbreviations: vec!["A".to_string(), "B".to_string()],
        initial: ltt(0, false, 0),
        transitions: vec![Transition {
            instant: 1000,
            kind: ltt(3600, true, 1),
        }],
        footer: Some(Footer {
            standard: ltt(3600, false, 0),
            dst: None,
        }),
    };
    assert_eq!(type_at(&zone, 999).unwrap(), zone.initial);
    assert_eq!(type_at(&zone, 1000).unwrap(), zone.footer.unwrap().standard);
    assert_eq!(transitions(&zone, i64::MIN, i64::MAX).count(), 1);
}

#[test]
fn abbreviation_indexes_are_checked() {
    let zone = zone_of("UTC");
    assert_eq!(abbreviation(zone, zone.initial).unwrap(), "UTC");
    let err = abbreviation(zone, ltt(0, false, 7)).unwrap_err();
    assert!(err.to_string().contains("no abbreviation 7"), "{err}");
}

#[test]
fn c_strings_mirror_the_table() {
    for entry in all_names() {
        let strings = c_strings_of(entry).unwrap();
        assert_eq!(strings.primary_name.to_str().unwrap(), entry.zone.name);
        let abbreviations: Vec<&str> = strings
            .abbreviations
            .iter()
            .map(|a| a.to_str().unwrap())
            .collect();
        assert_eq!(abbreviations, entry.zone.abbreviations);
    }
    let stray = Name {
        name: "Not/Listed".to_string(),
        rank: 0,
        tags: Tags::EMPTY,
        zone: Box::leak(Box::new(posix_zone("Not/Listed", ltt(0, false, 0), None))),
    };
    let err = c_strings_of(&stray).unwrap_err();
    assert!(err.to_string().contains("Not/Listed"), "{err}");
}

#[test]
fn rule_transitions_are_bounded_by_their_years() {
    let zone = est_edt(
        rule(RuleDay::Julian(60), 7200),
        rule(RuleDay::Julian(300), 7200),
    );
    let footer = zone.footer.unwrap();
    let mut rules = RuleTransitions::new(footer.standard, footer.dst.unwrap(), 2000, 2001);
    let years: Vec<i64> = rules.by_ref().map(|t| year_of(t.instant)).collect();
    assert_eq!(years, vec![2000, 2000, 2001, 2001]);
    assert!(rules.next().is_none());
    assert!(rules.next().is_none());
}

#[test]
fn the_c_abi_reports_every_tag_as_its_bit() {
    use crate::{
        hegel_context_free, hegel_context_new, hegel_timezone_info, hegel_timezone_info_t,
    };
    let expected = |tag: Tag| -> u32 {
        let position = Tag::ALL.iter().position(|&t| t == tag).unwrap();
        1 << position
    };
    let ctx = hegel_context_new();
    let mut seen = Tags::EMPTY;
    for entry in all_names() {
        let name = CString::new(entry.name.as_str()).unwrap();
        let mut info = hegel_timezone_info_t {
            known: false,
            primary_name: core::ptr::null(),
            is_link: false,
            rank: 0,
            tags: 0,
        };
        assert_eq!(
            unsafe { hegel_timezone_info(ctx, name.as_ptr(), &mut info) },
            crate::hegel_result_t::HEGEL_OK
        );
        assert!(info.known, "{}", entry.name);
        assert_eq!(info.rank, entry.rank, "{}", entry.name);
        assert_eq!(info.is_link, entry.is_link(), "{}", entry.name);
        assert_eq!(
            unsafe { CStr::from_ptr(info.primary_name) }
                .to_str()
                .unwrap(),
            entry.zone.name,
            "{}",
            entry.name
        );
        let bits = entry.tags.iter().fold(0, |mask, tag| mask | expected(tag));
        assert_eq!(info.tags, bits, "{}", entry.name);
        for tag in entry.tags.iter() {
            seen = seen.with(tag);
        }
    }
    assert_eq!(seen.len(), Tag::ALL.len());
    assert_eq!(
        unsafe { hegel_context_free(ctx) },
        crate::hegel_result_t::HEGEL_OK
    );
}
