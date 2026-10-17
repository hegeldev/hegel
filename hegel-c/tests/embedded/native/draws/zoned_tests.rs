//! Tests for `src/native/draws/zoned.rs`: every drawn value round-trips
//! through jiff with the reported kind, offsets and instant; the targeted
//! node fires at its rate and reaches transitions; the policies, bounds,
//! precision and printing behave as `hegel-c/docs/datetime.md` §6.2 and
//! §6.5 say; and a failing value shrinks down the zone ladder and toward
//! the origin.

use super::*;
use crate::exchange::drive_no_yield;
use crate::native::bignum::BigInt;
use crate::native::core::{ChoiceNode, ChoiceValue, NativeTestCase};
use crate::native::draws::timezone::TimeZoneSpec;
use crate::native::rng::EngineRng;
use crate::native::shrinker::{ShrinkRun, Shrinker};
use crate::native::timezone::{MAX_INSTANT, MIN_INSTANT};
use crate::tzdata::{self, Tag};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;
use jiff::tz::{AmbiguousOffset, TimeZone as JiffZone};

fn fresh_ntc(seed: u64) -> NativeTestCase {
    NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap()
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

fn years(min: i32, max: i32) -> ZonedBounds {
    ZonedBounds::Civil {
        min: civil(min, 1, 1, 0, 0, 0),
        max: civil(max, 12, 31, 23, 59, 59),
    }
}

fn spec_with(bounds: ZonedBounds) -> ZonedSpec {
    ZonedSpec {
        bounds,
        ..ZonedSpec::default()
    }
}

fn zones(names: &[&str]) -> TimeZoneSpec {
    let mut spec = TimeZoneSpec::new().unwrap();
    spec.set_names(names.iter().map(|s| s.to_string()).collect())
        .unwrap();
    spec
}

fn at(second: i64) -> Timestamp {
    Timestamp {
        second,
        nanosecond: 0,
    }
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

/// Whether `z`'s wall time lies within two days of a last explicit
/// transition that changes nothing (America/Nuuk 2023), where jiff
/// answers from the footer rule alone; see the evaluator's tests.
fn near_a_dummy_last_transition(z: &ZonedDateTime) -> bool {
    let zone = tzdata::lookup(&z.zone).unwrap().unwrap().zone;
    let n = zone.transitions.len();
    zone.transitions.last().is_some_and(|last| {
        let before = if n >= 2 {
            zone.transitions[n - 2].kind
        } else {
            zone.initial
        };
        let wall = wall_ns(&z.civil) / NS;
        before.utoff == last.kind.utoff
            && (i128::from(last.instant - 2 * DAY)..=i128::from(last.instant + 2 * DAY))
                .contains(&wall)
    })
}

fn check_with_jiff(z: &ZonedDateTime) {
    if near_a_dummy_last_transition(z) {
        return;
    }
    let tz = JiffZone::get(&z.zone).unwrap();
    let theirs = tz.to_ambiguous_timestamp(jiff_civil(&z.civil));
    let (kind, before, after) = match theirs.offset() {
        AmbiguousOffset::Unambiguous { offset } => (Kind::Unique, offset, offset),
        AmbiguousOffset::Gap { before, after } => (Kind::Gap, before, after),
        AmbiguousOffset::Fold { before, after } => (Kind::Fold, before, after),
    };
    let context = format!("{z}");
    assert_eq!(z.kind, kind, "{context}");
    assert_eq!(z.offset_before, before.seconds(), "{context}");
    assert_eq!(z.offset_after, after.seconds(), "{context}");
    assert!(z.offset_known, "{context}");
    let expected = if z.fold {
        assert_eq!(z.kind, Kind::Fold, "{context}");
        theirs.later().unwrap()
    } else {
        theirs.compatible().unwrap()
    };
    assert_eq!(
        instant_ns(&z.timestamp),
        expected.as_nanosecond(),
        "{context}"
    );
    assert_eq!(
        z.offset_seconds,
        tz.to_offset(expected).seconds(),
        "{context}"
    );
}

#[test]
fn every_value_round_trips_through_jiff() {
    let tz = TimeZoneSpec::new().unwrap();
    let spec = spec_with(years(1850, 2100));
    let mut kinds = [0usize; 3];
    for seed in 0..3000 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert!((1850..=2100).contains(&z.civil.date.year), "{z}");
        check_with_jiff(&z);
        kinds[z.kind as usize] += 1;
    }
    assert!(kinds.iter().all(|&n| n > 0), "{kinds:?}");
}

#[test]
fn targeted_draws_fire_at_their_rate_and_reach_transitions() {
    let tz = zones(&["America/New_York"]);
    let spec = spec_with(years(2000, 2030));
    let (mut targeted, mut hits, draws) = (0, 0, 4000);
    for seed in 0..draws {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        check_with_jiff(&z);
        let ChoiceValue::Boolean(t) = ntc.nodes[1].value() else {
            panic!("{:?}", ntc.nodes[1]);
        };
        targeted += usize::from(t);
        hits += usize::from(z.kind != Kind::Unique);
        assert_eq!(
            ntc.nodes.len(),
            if t { 5 } else { 4 } + usize::from(z.kind == Kind::Fold)
        );
    }
    let rate = targeted as f64 / draws as f64;
    assert!((0.03..=0.07).contains(&rate), "{rate}");
    assert!((20..=200).contains(&hits), "{hits}");
}

#[test]
fn a_zone_without_transitions_draws_no_targeted_node() {
    let tz = zones(&["UTC"]);
    let spec = spec_with(years(2000, 2030));
    for seed in 0..50 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 3);
        assert_eq!(z.kind, Kind::Unique);
        assert_eq!(
            (z.offset_seconds, z.offset_before, z.offset_after),
            (0, 0, 0)
        );
        assert_eq!(z.timestamp.second, wall_ns(&z.civil).div_euclid(NS) as i64);
    }
}

fn run_once(
    tz: &TimeZoneSpec,
    spec: &ZonedSpec,
    ntc: &mut NativeTestCase,
) -> Result<ZonedDateTime, EngineError> {
    let result = generate_zoned_datetime(ntc, tz, spec);
    ntc.freeze();
    result
}

fn shrinker_for(
    tz: Arc<TimeZoneSpec>,
    spec: ZonedSpec,
    interesting: impl Fn(&ZonedDateTime) -> bool + Send + Clone + 'static,
) -> (Shrinker<'static>, ZonedDateTime) {
    let (initial, nodes, spans) = (0..)
        .find_map(|seed| {
            let mut ntc = fresh_ntc(seed);
            let z = run_once(&tz, &spec, &mut ntc).ok()?;
            interesting(&z).then(|| (z, ntc.nodes.clone(), ntc.spans.clone()))
        })
        .unwrap();
    let probe = move |run: ShrinkRun<'_>| {
        let mut ntc = match run {
            ShrinkRun::Full(nodes) => {
                let values: Vec<ChoiceValue> = nodes.iter().map(|n| n.value()).collect();
                NativeTestCase::for_choices(&values, None, None)
            }
            ShrinkRun::Probe { prefix, max_size } => {
                NativeTestCase::for_probe(prefix, EngineRng::seeded(0), max_size).unwrap()
            }
        };
        let ok = matches!(run_once(&tz, &spec, &mut ntc), Ok(z) if interesting(&z));
        (ok, ntc.nodes.clone(), ntc.spans.clone())
    };
    (Shrinker::with_probe(Box::new(probe), nodes, spans), initial)
}

fn replay(tz: &TimeZoneSpec, spec: &ZonedSpec, nodes: &[ChoiceNode]) -> ZonedDateTime {
    let values: Vec<ChoiceValue> = nodes.iter().map(|n| n.value()).collect();
    let mut ntc = NativeTestCase::for_choices(&values, None, None);
    generate_zoned_datetime(&mut ntc, tz, spec).unwrap()
}

#[test]
fn a_failure_in_every_value_shrinks_to_utc_at_the_origin() {
    let tz = Arc::new(TimeZoneSpec::new().unwrap());
    let spec = spec_with(years(1900, 2100));
    let (mut shrinker, _) = shrinker_for(Arc::clone(&tz), spec, |_| true);
    drive_no_yield(shrinker.shrink()).unwrap();
    let z = replay(&tz, &spec, &shrinker.current_nodes);
    assert_eq!(z.zone, "UTC");
    assert_eq!(z.civil, civil(2000, 1, 1, 0, 0, 0));
    assert_eq!(z.timestamp, at(EPOCH_2000));
    assert_eq!(shrinker.current_nodes.len(), 3);
}

#[test]
fn a_failure_in_zones_with_daylight_saving_shrinks_to_the_lowest_ranked_one() {
    let has_dst = |z: &ZonedDateTime| {
        tzdata::lookup(&z.zone)
            .unwrap()
            .unwrap()
            .tags
            .contains(Tag::Dst)
    };
    let tz = Arc::new(TimeZoneSpec::new().unwrap());
    let spec = spec_with(years(1990, 2030));
    let (mut shrinker, initial) = shrinker_for(Arc::clone(&tz), spec, has_dst);
    drive_no_yield(shrinker.shrink()).unwrap();
    let z = replay(&tz, &spec, &shrinker.current_nodes);
    let lowest = tzdata::names()
        .unwrap()
        .iter()
        .filter(|n| n.tags.contains(Tag::Dst) && !n.tags.contains(Tag::Legacy))
        .min_by_key(|n| n.rank)
        .unwrap();
    assert_eq!(z.zone, lowest.name, "from {initial}");
    assert_eq!(z.civil, civil(2000, 1, 1, 0, 0, 0));
    assert_eq!(z.kind, Kind::Unique);
}

#[test]
fn a_stored_zone_the_spec_no_longer_allows_is_punned_on_replay() {
    let wide = zones(&["UTC", "Europe/London", "America/New_York"]);
    let spec = spec_with(years(2000, 2030));
    let (nodes, original) = (0..)
        .find_map(|seed| {
            let mut ntc = fresh_ntc(seed);
            let z = generate_zoned_datetime(&mut ntc, &wide, &spec).unwrap();
            (z.zone == "America/New_York").then_some((ntc.nodes.clone(), z))
        })
        .unwrap();
    assert_eq!(replay(&wide, &spec, &nodes), original);
    let narrow = zones(&["UTC", "Europe/London"]);
    let ChoiceValue::String(stored) = nodes[0].value() else {
        unreachable!()
    };
    assert!(!narrow.kind().validate(&stored));
    let punned = replay(&narrow, &spec, &nodes);
    assert_eq!(punned.zone, "UTC");
    assert_eq!(punned.offset_seconds, 0);
}

#[test]
fn avoid_moves_a_gap_time_to_the_first_wall_time_after_the_gap() {
    let tz = zones(&["America/New_York", "Asia/Kathmandu", "Pacific/Apia"]);
    let allow = spec_with(years(1980, 2030));
    let avoid = ZonedSpec {
        imaginary: Imaginary::Avoid,
        ..allow
    };
    let mut gaps = 0;
    for seed in 0..3000 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &avoid).unwrap();
        assert_ne!(z.kind, Kind::Gap, "{z}");
        check_with_jiff(&z);
        let under_allow = replay(&tz, &allow, &ntc.nodes);
        if under_allow.kind == Kind::Gap {
            gaps += 1;
            let moved = replay(&tz, &avoid, &ntc.nodes);
            assert_eq!(moved, z);
            assert!(
                z.timestamp <= under_allow.timestamp,
                "{under_allow} moved to {z}"
            );
            assert!(wall_ns(&z.civil) > wall_ns(&under_allow.civil));
            let tz_jiff = JiffZone::get(&z.zone).unwrap();
            let one_unit_before = jiff_civil(&z.civil)
                .checked_sub(jiff::Span::new().nanoseconds(1))
                .unwrap();
            assert!(
                matches!(
                    tz_jiff.to_ambiguous_timestamp(one_unit_before).offset(),
                    AmbiguousOffset::Gap { .. }
                ),
                "{under_allow} moved to {z}, which is not the first wall time after the gap"
            );
        } else {
            assert_eq!(under_allow, z);
        }
    }
    assert!(gaps > 0);
}

#[test]
fn avoid_rejects_a_value_moved_past_the_window() {
    let tz = zones(&["America/New_York"]);
    let spec = ZonedSpec {
        bounds: ZonedBounds::Civil {
            min: civil(2024, 3, 10, 2, 0, 0),
            max: civil(2024, 3, 10, 2, 30, 0),
        },
        imaginary: Imaginary::Avoid,
        ..ZonedSpec::default()
    };
    for seed in 0..20 {
        let mut ntc = fresh_ntc(seed);
        let err = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap_err();
        assert!(matches!(err, EngineError::InvalidTestCase), "{err:?}");
        assert_eq!(ntc.status(), Some(Status::Invalid));
    }
    let allow = ZonedSpec {
        imaginary: Imaginary::Allow,
        ..spec
    };
    let mut ntc = fresh_ntc(0);
    let z = generate_zoned_datetime(&mut ntc, &tz, &allow).unwrap();
    assert_eq!(z.kind, Kind::Gap);
    assert_eq!(
        z.civil.date,
        Date {
            year: 2024,
            month: 3,
            day: 10
        }
    );
    assert_eq!(z.civil.time.hour, 2);
}

#[test]
fn earlier_and_later_draw_no_fold_node() {
    let tz = zones(&["America/New_York"]);
    let both = spec_with(years(2000, 2030));
    let (nodes, fold_value) = (0..)
        .find_map(|seed| {
            let mut ntc = fresh_ntc(seed);
            let z = generate_zoned_datetime(&mut ntc, &tz, &both).unwrap();
            (z.kind == Kind::Fold).then_some((ntc.nodes.clone(), z))
        })
        .unwrap();
    assert!(matches!(
        nodes.last().unwrap().value(),
        ChoiceValue::Boolean(_)
    ));
    let values: Vec<ChoiceValue> = nodes.iter().map(|n| n.value()).collect();
    for (policy, fold) in [(Ambiguous::Earlier, false), (Ambiguous::Later, true)] {
        let spec = ZonedSpec {
            ambiguous: policy,
            ..both
        };
        let mut ntc = NativeTestCase::for_choices(&values, None, None);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), nodes.len() - 1);
        assert_eq!(z.kind, Kind::Fold);
        assert_eq!(z.fold, fold);
        assert_eq!(z.civil, fold_value.civil);
        check_with_jiff(&z);
        let later = fold_value.timestamp.second
            + i64::from(fold_value.offset_before - fold_value.offset_after)
            - if fold_value.fold {
                i64::from(fold_value.offset_before - fold_value.offset_after)
            } else {
                0
            };
        let earlier = later - i64::from(fold_value.offset_before - fold_value.offset_after);
        assert_eq!(z.timestamp.second, if fold { later } else { earlier });
    }
}

#[test]
fn a_fold_on_the_wrong_side_of_an_instant_bound_is_rejected() {
    let tz = zones(&["America/New_York"]);
    let max = 1_730_611_800;
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(max - 2 * DAY),
            max: at(max),
        },
        ..ZonedSpec::default()
    };
    let date_offset = days_from_civil(&Date {
        year: 2024,
        month: 11,
        day: 3,
    }) - days_from_civil(&Date {
        year: 2024,
        month: 11,
        day: 1,
    });
    let values = |fold: bool| {
        vec![
            ChoiceValue::String("America/New_York".chars().map(u32::from).collect()),
            ChoiceValue::Boolean(false),
            ChoiceValue::Integer(BigInt::from(date_offset)),
            ChoiceValue::Integer(BigInt::from(5_400_000_000_000i64)),
            ChoiceValue::Boolean(fold),
        ]
    };
    let mut ntc = NativeTestCase::for_choices(&values(false), None, None);
    let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
    assert_eq!(z.civil, civil(2024, 11, 3, 1, 30, 0));
    assert_eq!(z.kind, Kind::Fold);
    assert_eq!(z.timestamp.second, max);
    let mut ntc = NativeTestCase::for_choices(&values(true), None, None);
    let err = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap_err();
    assert!(matches!(err, EngineError::InvalidTestCase), "{err:?}");
    assert_eq!(ntc.status(), Some(Status::Invalid));
}

#[test]
fn a_target_window_past_the_bounds_widens_to_the_bounds() {
    let tz = zones(&["America/New_York"]);
    let spec = ZonedSpec {
        bounds: ZonedBounds::Civil {
            min: civil(2024, 3, 10, 2, 0, 0),
            max: civil(2024, 3, 10, 2, 30, 0),
        },
        precision: Precision::Seconds,
        ..ZonedSpec::default()
    };
    let values = |offset: i64| {
        vec![
            ChoiceValue::String("America/New_York".chars().map(u32::from).collect()),
            ChoiceValue::Boolean(true),
            ChoiceValue::Integer(BigInt::from(0)),
            ChoiceValue::Integer(BigInt::from(0)),
            ChoiceValue::Integer(BigInt::from(offset)),
        ]
    };
    let mut ntc = NativeTestCase::for_choices(&values(0), None, None);
    let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
    assert_eq!(ntc.nodes.len(), 5);
    assert_eq!(z.civil, civil(2024, 3, 10, 2, 30, 0));
    assert_eq!(z.kind, Kind::Gap);
    check_with_jiff(&z);
    let mut ntc = NativeTestCase::for_choices(&values(-1800), None, None);
    let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
    assert_eq!(z.civil, civil(2024, 3, 10, 2, 0, 0), "{:?}", ntc.nodes);
    check_with_jiff(&z);
}

#[test]
fn instant_bounds_hold_through_the_wall_clock_window() {
    let tz = zones(&["America/New_York", "Pacific/Apia", "UTC"]);
    let (min, max) = (1_704_067_200, 1_735_689_599);
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(min),
            max: at(max),
        },
        ..ZonedSpec::default()
    };
    let mut kinds = [0usize; 3];
    for seed in 0..2000 {
        let mut ntc = fresh_ntc(seed);
        let Ok(z) = generate_zoned_datetime(&mut ntc, &tz, &spec) else {
            assert_eq!(ntc.status(), Some(Status::Invalid));
            continue;
        };
        assert!((min..=max).contains(&z.timestamp.second), "{z}");
        check_with_jiff(&z);
        kinds[z.kind as usize] += 1;
    }
    assert!(kinds.iter().all(|&n| n > 0), "{kinds:?}");
}

#[test]
fn a_short_window_near_a_transition_draws_the_instant() {
    let tz = zones(&["America/New_York"]);
    let transition = 1_710_054_000;
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(transition - 3600),
            max: at(transition + 3600),
        },
        precision: Precision::Seconds,
        ..ZonedSpec::default()
    };
    let (mut before, mut after) = (0, 0);
    for seed in 0..300 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 2);
        assert!((transition - 3600..=transition + 3600).contains(&z.timestamp.second));
        assert_eq!(z.timestamp.nanosecond, 0);
        assert_eq!(z.kind, Kind::Unique);
        check_with_jiff(&z);
        if z.timestamp.second < transition {
            before += 1;
        } else {
            after += 1;
        }
    }
    assert!(before > 0 && after > 0, "{before} {after}");
    let fold = 1_730_613_600;
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(fold - 3600),
            max: at(fold + 3599),
        },
        ..ZonedSpec::default()
    };
    let mut folds = [0usize; 2];
    for seed in 0..300 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 2);
        assert_eq!(z.kind, Kind::Fold, "{z}");
        folds[usize::from(z.fold)] += 1;
        check_with_jiff(&z);
    }
    assert!(folds.iter().all(|&n| n > 0), "{folds:?}");
    let mut ntc = NativeTestCase::for_choices(
        &[
            ChoiceValue::String("America/New_York".chars().map(u32::from).collect()),
            ChoiceValue::Integer(BigInt::from(0)),
        ],
        None,
        None,
    );
    let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
    assert_eq!(z.timestamp, at(fold - 3600));
}

#[test]
fn an_instant_window_with_no_aligned_wall_time_draws_the_instant() {
    let tz = zones(&["Asia/Kathmandu"]);
    let noon = 1_720_094_400;
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(noon - 300),
            max: at(noon + 300),
        },
        precision: Precision::Hours,
        ..ZonedSpec::default()
    };
    let mut ntc = fresh_ntc(1);
    let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
    assert_eq!(ntc.nodes.len(), 2);
    assert_eq!(z.timestamp, at(noon));
    assert_eq!(z.civil, civil(2024, 7, 4, 17, 45, 0));
    check_with_jiff(&z);
}

#[test]
fn instant_bounds_in_the_unknown_zone_read_as_utc() {
    let mut tz = zones(&["Etc/Unknown"]);
    tz.set_unknown(true).unwrap();
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(0),
            max: at(DAY),
        },
        ..ZonedSpec::default()
    };
    for seed in 0..20 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 3);
        assert!((0..=DAY).contains(&z.timestamp.second));
        assert!(!z.offset_known);
        assert_eq!(z.civil.date.year, 1970);
    }
}

#[test]
fn opaque_and_unknown_zones_read_civil_time_as_utc() {
    let mut tz = TimeZoneSpec::new().unwrap();
    tz.set_opaque(true).unwrap();
    tz.set_names(vec!["Mars/Olympus_Mons".to_string()]).unwrap();
    tz.set_unknown(true).unwrap();
    let spec = spec_with(years(2000, 2030));
    let mut seen = [false; 2];
    for seed in 0..40 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 3);
        assert!(!z.offset_known);
        assert_eq!(z.kind, Kind::Unique);
        assert_eq!(
            (z.offset_seconds, z.offset_before, z.offset_after),
            (0, 0, 0)
        );
        assert_eq!(z.timestamp.second, wall_ns(&z.civil).div_euclid(NS) as i64);
        assert_eq!(z.timestamp.nanosecond, z.civil.time.nanosecond);
        seen[usize::from(z.zone == UNKNOWN_NAME)] = true;
        let printed = format!("{z}");
        if z.zone == UNKNOWN_NAME {
            assert!(printed.ends_with("Z[Etc/Unknown]"), "{printed}");
        } else {
            assert!(printed.ends_with("-00:00[Mars/Olympus_Mons]"), "{printed}");
        }
    }
    assert_eq!(seen, [true, true]);
}

#[test]
fn the_time_part_is_a_whole_number_of_units() {
    let tz = zones(&["America/New_York", "Asia/Kolkata"]);
    for precision in [
        Precision::Nanoseconds,
        Precision::Microseconds,
        Precision::Milliseconds,
        Precision::Seconds,
        Precision::Minutes,
        Precision::Hours,
    ] {
        let spec = ZonedSpec {
            bounds: ZonedBounds::Civil {
                min: DateTime {
                    time: Time {
                        hour: 6,
                        minute: 30,
                        second: 15,
                        nanosecond: 500,
                    },
                    ..civil(2024, 3, 9, 0, 0, 0)
                },
                max: DateTime {
                    time: Time {
                        hour: 18,
                        minute: 45,
                        second: 1,
                        nanosecond: 999,
                    },
                    ..civil(2024, 3, 11, 0, 0, 0)
                },
            },
            precision,
            ..ZonedSpec::default()
        };
        let unit = precision.unit();
        let (mut first_day, mut last_day) = (false, false);
        for seed in 0..300 {
            let mut ntc = fresh_ntc(seed);
            let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
            assert_eq!(time_to_ns(&z.civil.time) % unit, 0, "{z} at {precision:?}");
            let ZonedBounds::Civil { min, max } = spec.bounds else {
                unreachable!()
            };
            assert!(
                (wall_ns(&min)..=wall_ns(&max)).contains(&wall_ns(&z.civil)),
                "{z}"
            );
            first_day |= z.civil.date.day == 9;
            last_day |= z.civil.date.day == 11;
            check_with_jiff(&z);
        }
        assert!(first_day && last_day, "{precision:?}");
    }
}

#[test]
fn the_default_spec_spans_the_civil_envelope() {
    let tz = zones(&["UTC", "Europe/London"]);
    let spec = ZonedSpec::default();
    assert_eq!(spec.bounds, FULL_CIVIL_BOUNDS);
    let (mut early, mut late) = (false, false);
    for seed in 0..300 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert!((MIN_YEAR..=MAX_YEAR).contains(&z.civil.date.year));
        assert!((MIN_INSTANT - 2 * DAY..=MAX_INSTANT + 2 * DAY).contains(&z.timestamp.second));
        early |= z.civil.date.year < -100_000;
        late |= z.civil.date.year > 100_000;
    }
    assert!(early && late);
}

#[test]
fn bad_bounds_are_rejected() {
    let tz = zones(&["UTC"]);
    let reject = |spec: ZonedSpec, needle: &str| {
        let mut ntc = fresh_ntc(0);
        let err = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap_err();
        assert!(
            matches!(&err, EngineError::InvalidArgument(msg) if msg.contains(needle)),
            "{err:?} lacks {needle:?}"
        );
        assert!(ntc.nodes.is_empty());
    };
    reject(spec_with(years(2001, 2000)), "min_value <= max_value");
    reject(
        spec_with(ZonedBounds::Civil {
            min: civil(2000, 2, 30, 0, 0, 0),
            max: civil(2000, 3, 1, 0, 0, 0),
        }),
        "min_value.date",
    );
    reject(
        spec_with(ZonedBounds::Civil {
            min: civil(2000, 1, 1, 0, 0, 0),
            max: civil(2000, 13, 1, 0, 0, 0),
        }),
        "max_value.date",
    );
    reject(
        spec_with(ZonedBounds::Civil {
            min: civil(2000, 1, 1, 24, 0, 0),
            max: civil(2000, 3, 1, 0, 0, 0),
        }),
        "min_value.time",
    );
    reject(
        spec_with(ZonedBounds::Civil {
            min: civil(2000, 1, 1, 0, 0, 0),
            max: civil(2000, 3, 1, 0, 60, 0),
        }),
        "max_value.time",
    );
    let half = DateTime {
        time: Time {
            hour: 12,
            minute: 0,
            second: 0,
            nanosecond: 500_000_000,
        },
        ..civil(2000, 1, 1, 0, 0, 0)
    };
    reject(
        ZonedSpec {
            bounds: ZonedBounds::Civil {
                min: half,
                max: half,
            },
            precision: Precision::Seconds,
            ..ZonedSpec::default()
        },
        "no value at precision",
    );
    reject(
        spec_with(ZonedBounds::Instant {
            min: at(1),
            max: at(0),
        }),
        "min_value <= max_value",
    );
    reject(
        spec_with(ZonedBounds::Instant {
            min: at(MIN_INSTANT - 1),
            max: at(0),
        }),
        "timestamp.second",
    );
    reject(
        spec_with(ZonedBounds::Instant {
            min: at(0),
            max: Timestamp {
                second: 0,
                nanosecond: 1_000_000_000,
            },
        }),
        "timestamp.nanosecond",
    );
    reject(
        ZonedSpec {
            bounds: ZonedBounds::Instant {
                min: at(1),
                max: at(59),
            },
            precision: Precision::Minutes,
            ..ZonedSpec::default()
        },
        "no value at precision",
    );
}

#[test]
fn values_print_as_rfc_9557() {
    let base = ZonedDateTime {
        zone: "America/New_York".to_string(),
        civil: civil(2024, 3, 10, 2, 30, 0),
        offset_seconds: -14400,
        offset_known: true,
        fold: false,
        kind: Kind::Gap,
        offset_before: -18000,
        offset_after: -14400,
        timestamp: at(1_710_055_800),
    };
    assert_eq!(
        base.to_string(),
        "2024-03-10T02:30:00-04:00[America/New_York][gap]"
    );
    let unique = ZonedDateTime {
        kind: Kind::Unique,
        offset_seconds: -18000,
        civil: DateTime {
            time: Time {
                nanosecond: 120_000_000,
                ..base.civil.time
            },
            ..base.civil
        },
        ..base.clone()
    };
    assert_eq!(
        unique.to_string(),
        "2024-03-10T02:30:00.12-05:00[America/New_York]"
    );
    let fold = ZonedDateTime {
        kind: Kind::Fold,
        fold: true,
        civil: DateTime {
            time: Time {
                nanosecond: 1,
                ..base.civil.time
            },
            ..base.civil
        },
        ..base.clone()
    };
    assert_eq!(
        fold.to_string(),
        "2024-03-10T02:30:00.000000001-04:00[America/New_York][fold=1]"
    );
    let earlier = ZonedDateTime {
        kind: Kind::Fold,
        ..base.clone()
    };
    assert!(earlier.to_string().ends_with("[fold=0]"));
    let lmt = ZonedDateTime {
        zone: "Europe/Dublin".to_string(),
        civil: civil(1800, 1, 1, 0, 0, 0),
        offset_seconds: -1521,
        kind: Kind::Unique,
        ..base.clone()
    };
    assert_eq!(
        lmt.to_string(),
        "1800-01-01T00:00:00-00:25:21[Europe/Dublin]"
    );
    let far = ZonedDateTime {
        zone: "UTC".to_string(),
        civil: civil(-12345, 6, 7, 8, 9, 10),
        offset_seconds: 0,
        kind: Kind::Unique,
        ..base.clone()
    };
    assert_eq!(far.to_string(), "-012345-06-07T08:09:10+00:00[UTC]");
    let later = ZonedDateTime {
        civil: civil(123_456, 6, 7, 8, 9, 10),
        ..far.clone()
    };
    assert!(later.to_string().starts_with("+123456-06-07"), "{later}");
    let unknown = ZonedDateTime {
        zone: UNKNOWN_NAME.to_string(),
        offset_known: false,
        ..far.clone()
    };
    assert_eq!(unknown.to_string(), "-012345-06-07T08:09:10Z[Etc/Unknown]");
    let opaque = ZonedDateTime {
        zone: "Mars/Olympus_Mons".to_string(),
        offset_known: false,
        ..far
    };
    assert_eq!(
        opaque.to_string(),
        "-012345-06-07T08:09:10-00:00[Mars/Olympus_Mons]"
    );
}

#[test]
fn the_unknown_zone_has_no_gap_to_move_past() {
    let resolution = Resolution {
        kind: Kind::Gap,
        offset_before: 0,
        offset_after: 3600,
        offset: 3600,
        offset_known: true,
        timestamp: at(0),
    };
    let err = after_gap(TimeZone::Unknown, &resolution, 1).unwrap_err();
    assert!(matches!(err, EngineError::Internal(_)), "{err:?}");
}

#[test]
fn wall_arithmetic_round_trips() {
    for seed in 0..200 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &zones(&["UTC"]), &ZonedSpec::default()).unwrap();
        assert_eq!(datetime_of_wall(wall_ns(&z.civil)), z.civil);
        assert_eq!(timestamp_of(instant_ns(&z.timestamp)), z.timestamp);
    }
    assert_eq!(align_up(-7, 5), -5);
    assert_eq!(align_up(-10, 5), -10);
    assert_eq!(align_down(-7, 5), -10);
    assert_eq!(align_down(7, 5), 5);
}
