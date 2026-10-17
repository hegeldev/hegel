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
use crate::native::draws::special::time_to_ns;
use crate::native::draws::timezone::TimeZoneSpec;
use crate::native::rng::EngineRng;
use crate::native::shrinker::{ShrinkRun, Shrinker};
use crate::native::timezone::{MAX_INSTANT, MIN_INSTANT};
use crate::tzdata::{self, Tag};
use alloc::boxed::Box;
use alloc::collections::BTreeSet;
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
    let at_second = jiff::Timestamp::from_second(z.timestamp.second).unwrap();
    assert_eq!(
        z.offset_seconds,
        tz.to_offset(at_second).seconds(),
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
            if t { 6 } else { 4 } + usize::from(z.kind == Kind::Fold)
        );
    }
    let rate = targeted as f64 / draws as f64;
    assert!((0.03..=0.07).contains(&rate), "{rate}");
    assert!((20..=200).contains(&hits), "{hits}");
}

#[test]
fn every_zone_targets_the_interesting_instants() {
    assert_eq!(JIFF_MIN_INSTANT, jiff::Timestamp::MIN.as_second());
    assert_eq!(JIFF_MAX_INSTANT, jiff::Timestamp::MAX.as_second());
    let boundary = |(year, month): (i32, u8)| {
        jiff::civil::date(
            i16::try_from(year).unwrap(),
            i8::try_from(month).unwrap(),
            1,
        )
        .to_zoned(JiffZone::UTC)
        .unwrap()
        .timestamp()
        .as_second()
    };
    let expected: Vec<i64> = [
        0,
        EPOCH_2000,
        i64::from(i32::MAX),
        i64::from(i32::MIN),
        1 << 32,
        1_000_000_000,
        JIFF_MIN_INSTANT,
        JIFF_MAX_INSTANT,
    ]
    .into_iter()
    .chain(LEAP_SECOND_BOUNDARIES.into_iter().map(boundary))
    .collect();
    assert_eq!(interesting_instants().collect::<Vec<_>>(), expected);
    assert_eq!(expected.len(), 35);
    let spec = ZonedSpec {
        bounds: years(1990, 2040),
        precision: Precision::Seconds,
        ..ZonedSpec::default()
    };
    let in_window: Vec<i64> = expected
        .iter()
        .copied()
        .filter(|&t| (boundary((1990, 1))..boundary((2041, 1))).contains(&t))
        .collect();
    assert_eq!(in_window.len(), 16);
    let mut unknown = zones(&["Etc/Unknown"]);
    unknown.set_unknown(true).unwrap();
    for (name, tz) in [("UTC", zones(&["UTC"])), (UNKNOWN_NAME, unknown)] {
        let prefix = |index: i64| {
            vec![
                ChoiceValue::String(name.chars().map(u32::from).collect()),
                ChoiceValue::Boolean(true),
                ChoiceValue::Integer(BigInt::from(index)),
                ChoiceValue::Integer(BigInt::from(0)),
            ]
        };
        let mut reached = BTreeSet::new();
        for index in 0..in_window.len() as i64 {
            let mut hits = BTreeSet::new();
            for seed in 0..60 {
                let mut ntc =
                    NativeTestCase::for_probe(&prefix(index), EngineRng::seeded(seed), 10).unwrap();
                let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
                assert_eq!(ntc.nodes.len(), 6);
                assert_eq!(z.zone, name);
                assert!(
                    in_window
                        .iter()
                        .any(|t| (t - 2..=t + 2).contains(&z.timestamp.second)),
                    "{z}"
                );
                if in_window.contains(&z.timestamp.second) {
                    hits.insert(z.timestamp.second);
                }
            }
            assert_eq!(hits.len(), 1, "{name} target {index}: {hits:?}");
            reached.extend(hits);
        }
        assert_eq!(reached.len(), in_window.len());
        let mut ntc = NativeTestCase::for_choices(
            &[
                prefix(0),
                vec![
                    ChoiceValue::Integer(BigInt::from(0)),
                    ChoiceValue::Integer(BigInt::from(0)),
                ],
            ]
            .concat(),
            None,
            None,
        );
        let origin = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(origin.timestamp, at(EPOCH_2000));
        let mut targeted = 0;
        for seed in 0..200 {
            let mut ntc = fresh_ntc(seed);
            generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
            targeted += usize::from(ntc.nodes.len() == 6);
        }
        assert!(targeted > 0, "{name}");
    }
}

#[test]
fn a_window_without_targets_draws_no_targeted_node() {
    let tz = zones(&["UTC"]);
    let spec = spec_with(years(2050, 2060));
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
    assert_eq!(shrinker.current_nodes.len(), 4);
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
fn avoid_redraws_a_gap_time_within_the_sampler() {
    let tz = zones(&["America/New_York"]);
    let allow = ZonedSpec {
        bounds: ZonedBounds::Civil {
            min: civil(2024, 3, 10, 1, 0, 0),
            max: civil(2024, 3, 10, 4, 0, 0),
        },
        precision: Precision::Seconds,
        ..ZonedSpec::default()
    };
    let avoid = ZonedSpec {
        imaginary: Imaginary::Avoid,
        ..allow
    };
    let (mut allowed_gaps, mut targeted) = (0, 0);
    for seed in 0..1000 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &avoid).unwrap();
        assert_ne!(z.kind, Kind::Gap, "{z}");
        check_with_jiff(&z);
        assert_eq!(replay(&tz, &allow, &ntc.nodes), z);
        targeted += usize::from(ntc.nodes.len() == 6);
        let mut ntc = fresh_ntc(seed);
        let fresh = generate_zoned_datetime(&mut ntc, &tz, &allow).unwrap();
        allowed_gaps += usize::from(fresh.kind == Kind::Gap);
    }
    assert!(allowed_gaps > 100, "{allowed_gaps}");
    assert!(targeted > 20, "{targeted}");
    let skipped_day = ZonedSpec {
        bounds: ZonedBounds::Civil {
            min: civil(2011, 12, 30, 0, 0, 0),
            max: civil(2011, 12, 31, 23, 59, 59),
        },
        ..avoid
    };
    let apia = zones(&["Pacific/Apia"]);
    let mut moved = 0;
    for seed in 0..100 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &apia, &skipped_day).unwrap();
        assert_eq!(z.kind, Kind::Unique, "{z}");
        assert_eq!(z.civil.date.day, 31, "{z}");
        check_with_jiff(&z);
        let under_allow = replay(
            &apia,
            &ZonedSpec {
                imaginary: Imaginary::Allow,
                ..skipped_day
            },
            &ntc.nodes,
        );
        if under_allow.kind == Kind::Gap {
            moved += 1;
            assert_eq!(
                z.civil,
                civil(2011, 12, 31, 0, 0, 0),
                "{under_allow} moved to {z}"
            );
        } else {
            assert_eq!(under_allow, z);
        }
    }
    assert!(moved > 0);
}

#[test]
fn a_replayed_gap_time_under_avoid_moves_past_the_gap() {
    let tz = zones(&["America/New_York"]);
    let allow = ZonedSpec {
        bounds: ZonedBounds::Civil {
            min: civil(2024, 3, 10, 1, 0, 0),
            max: civil(2024, 3, 10, 3, 30, 0),
        },
        precision: Precision::Seconds,
        ..ZonedSpec::default()
    };
    let avoid = ZonedSpec {
        imaginary: Imaginary::Avoid,
        ..allow
    };
    let values = |seconds: i64| {
        vec![
            ChoiceValue::String("America/New_York".chars().map(u32::from).collect()),
            ChoiceValue::Boolean(false),
            ChoiceValue::Integer(BigInt::from(0)),
            ChoiceValue::Integer(BigInt::from(seconds)),
        ]
    };
    let nodes = values(4500);
    let mut ntc = NativeTestCase::for_choices(&nodes, None, None);
    let gap = generate_zoned_datetime(&mut ntc, &tz, &allow).unwrap();
    assert_eq!(gap.civil, civil(2024, 3, 10, 2, 15, 0));
    assert_eq!(gap.kind, Kind::Gap);
    let mut ntc = NativeTestCase::for_choices(&nodes, None, None);
    let forward = generate_zoned_datetime(&mut ntc, &tz, &avoid).unwrap();
    assert_eq!(ntc.nodes.len(), 4);
    assert_eq!(forward.civil, civil(2024, 3, 10, 3, 0, 0));
    assert_eq!(forward.kind, Kind::Unique);
    assert!(forward.timestamp <= gap.timestamp);
    check_with_jiff(&forward);
    let short = ZonedSpec {
        bounds: ZonedBounds::Civil {
            min: civil(2024, 3, 10, 1, 0, 0),
            max: civil(2024, 3, 10, 2, 30, 0),
        },
        ..avoid
    };
    let mut ntc = NativeTestCase::for_choices(&nodes, None, None);
    let backward = generate_zoned_datetime(&mut ntc, &tz, &short).unwrap();
    assert_eq!(backward.civil, civil(2024, 3, 10, 1, 59, 59));
    assert_eq!(backward.kind, Kind::Unique);
    check_with_jiff(&backward);
    let mut ntc = NativeTestCase::for_choices(&values(0), None, None);
    let untouched = generate_zoned_datetime(&mut ntc, &tz, &short).unwrap();
    assert_eq!(untouched.civil, civil(2024, 3, 10, 1, 0, 0));
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
    let values = |seconds: i64| {
        vec![
            ChoiceValue::String("America/New_York".chars().map(u32::from).collect()),
            ChoiceValue::Boolean(true),
            ChoiceValue::Integer(BigInt::from(0)),
            ChoiceValue::Integer(BigInt::from(0)),
            ChoiceValue::Integer(BigInt::from(0)),
            ChoiceValue::Integer(BigInt::from(seconds)),
        ]
    };
    let mut ntc = NativeTestCase::for_choices(&values(0), None, None);
    let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
    assert_eq!(ntc.nodes.len(), 6);
    assert_eq!(z.civil, civil(2024, 3, 10, 2, 0, 0));
    assert_eq!(z.kind, Kind::Gap);
    check_with_jiff(&z);
    let mut ntc = NativeTestCase::for_choices(&values(1800), None, None);
    let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
    assert_eq!(z.civil, civil(2024, 3, 10, 2, 30, 0), "{:?}", ntc.nodes);
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
    let (mut before, mut after, mut gaps) = (0, 0, 0);
    for seed in 0..300 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 2);
        assert!((transition - 3600..=transition + 3600).contains(&z.timestamp.second));
        assert_eq!(z.timestamp.nanosecond, 0);
        assert_ne!(z.kind, Kind::Fold);
        check_with_jiff(&z);
        if z.timestamp.second < transition {
            before += 1;
        } else {
            after += 1;
        }
        gaps += usize::from(z.kind == Kind::Gap);
    }
    assert!(
        before > 0 && after > 0 && gaps > 0,
        "{before} {after} {gaps}"
    );
    let avoid = ZonedSpec {
        imaginary: Imaginary::Avoid,
        ..spec
    };
    for seed in 0..300 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &avoid).unwrap();
        assert_eq!(ntc.nodes.len(), 2);
        assert_eq!(z.kind, Kind::Unique, "{z}");
        check_with_jiff(&z);
    }
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
fn the_instant_fallback_honours_the_ambiguous_policy() {
    let tz = zones(&["America/New_York"]);
    let fold = 1_730_613_600;
    for (policy, expected) in [(Ambiguous::Earlier, false), (Ambiguous::Later, true)] {
        let spec = ZonedSpec {
            bounds: ZonedBounds::Instant {
                min: at(fold - 3600),
                max: at(fold + 3599),
            },
            ambiguous: policy,
            ..ZonedSpec::default()
        };
        for seed in 0..300 {
            let mut ntc = fresh_ntc(seed);
            let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
            assert_eq!(ntc.nodes.len(), 2);
            assert_eq!(z.kind, Kind::Fold, "{z}");
            assert_eq!(z.fold, expected, "{z}");
            assert!(
                (fold - 3600..=fold + 3599).contains(&z.timestamp.second),
                "{z}"
            );
            check_with_jiff(&z);
        }
    }
    let later = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(fold - 3600),
            max: at(fold - 1),
        },
        ambiguous: Ambiguous::Later,
        ..ZonedSpec::default()
    };
    let earlier = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(fold + 10),
            max: at(fold + 3000),
        },
        ambiguous: Ambiguous::Earlier,
        ..ZonedSpec::default()
    };
    for spec in [later, earlier] {
        let mut ntc = fresh_ntc(0);
        let err = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap_err();
        assert!(
            matches!(&err, EngineError::InvalidArgument(msg) if msg.contains("any allowed zone")),
            "{err:?}"
        );
        assert_eq!(ntc.nodes.len(), 1);
        let with_utc = zones(&["America/New_York", "UTC"]);
        let mut seen = [false; 2];
        for seed in 0..60 {
            let mut ntc = fresh_ntc(seed);
            match generate_zoned_datetime(&mut ntc, &with_utc, &spec) {
                Ok(z) => {
                    assert_eq!(z.zone, "UTC");
                    seen[0] = true;
                }
                Err(err) => {
                    assert!(matches!(err, EngineError::InvalidTestCase), "{err:?}");
                    assert_eq!(ntc.status(), Some(Status::Invalid));
                    seen[1] = true;
                }
            }
        }
        assert_eq!(seen, [true, true]);
    }
    let both = ZonedSpec {
        ambiguous: Ambiguous::Both,
        ..earlier
    };
    for seed in 0..100 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &both).unwrap();
        assert_eq!(ntc.nodes.len(), 2);
        assert_eq!((z.kind, z.fold), (Kind::Fold, true), "{z}");
        assert!(
            (fold + 10..=fold + 3000).contains(&z.timestamp.second),
            "{z}"
        );
        check_with_jiff(&z);
    }
}

#[test]
fn the_instant_fallback_aligns_the_wall_time_to_the_precision() {
    let tz = zones(&["Asia/Kathmandu"]);
    let transition = 504_901_800;
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(transition - 3600),
            max: at(transition + 3600),
        },
        precision: Precision::Hours,
        ..ZonedSpec::default()
    };
    let mut seen = BTreeSet::new();
    for seed in 0..300 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 2);
        assert_eq!(
            time_to_ns(&z.civil.time) % Precision::Hours.unit(),
            0,
            "{z}"
        );
        assert!(
            (transition - 3600..=transition + 3600).contains(&z.timestamp.second),
            "{z}"
        );
        check_with_jiff(&z);
        seen.insert((z.civil.time.hour, z.kind as u8));
    }
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        [
            (0, Kind::Gap as u8),
            (1, Kind::Unique as u8),
            (23, Kind::Unique as u8)
        ]
    );
    let noon = 1_720_094_400;
    let none = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(noon - 300),
            max: at(noon + 300),
        },
        ..spec
    };
    assert!(none.validate().is_ok());
    let mut ntc = fresh_ntc(0);
    let err = generate_zoned_datetime(&mut ntc, &tz, &none).unwrap_err();
    assert!(
        matches!(&err, EngineError::InvalidArgument(msg) if msg.contains("no value at precision")),
        "{err:?}"
    );
}

#[test]
fn instant_bounds_at_the_civil_envelope_clamp_the_wall_window() {
    let instants = |min: i64, max: i64| ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(min),
            max: at(max),
        },
        ..ZonedSpec::default()
    };
    let rejected = |tz: &TimeZoneSpec, spec: &ZonedSpec| {
        let mut ntc = fresh_ntc(0);
        let err = generate_zoned_datetime(&mut ntc, tz, spec).unwrap_err();
        assert!(
            matches!(&err, EngineError::InvalidArgument(msg) if msg.contains("any allowed zone")),
            "{err:?}"
        );
    };
    let drawn = |tz: &TimeZoneSpec, spec: &ZonedSpec| {
        let ZonedBounds::Instant { min, max } = spec.bounds else {
            unreachable!()
        };
        for seed in 0..50 {
            let mut ntc = fresh_ntc(seed);
            let z = generate_zoned_datetime(&mut ntc, tz, spec).unwrap();
            assert!((MIN_YEAR..=MAX_YEAR).contains(&z.civil.date.year), "{z}");
            assert!((min..=max).contains(&z.timestamp), "{z}");
            assert_eq!(
                instant_ns(&z.timestamp) + i128::from(z.offset_seconds) * NS,
                wall_ns(&z.civil),
                "{z}"
            );
        }
    };
    let tokyo = zones(&["Asia/Tokyo"]);
    drawn(&tokyo, &instants(MAX_INSTANT - 10 * DAY, MAX_INSTANT));
    rejected(&tokyo, &instants(MAX_INSTANT - 10, MAX_INSTANT));
    let new_york = zones(&["America/New_York"]);
    drawn(&new_york, &instants(MIN_INSTANT, MIN_INSTANT + 10 * DAY));
    rejected(&new_york, &instants(MIN_INSTANT, MIN_INSTANT + 10));
    let kiritimati = zones(&["Pacific/Kiritimati"]);
    drawn(&kiritimati, &instants(MIN_INSTANT, MAX_INSTANT));
}

#[test]
fn instant_bounds_in_the_unknown_zone_read_as_utc() {
    let mut tz = zones(&["Etc/Unknown"]);
    tz.set_unknown(true).unwrap();
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(DAY),
            max: at(2 * DAY),
        },
        ..ZonedSpec::default()
    };
    for seed in 0..20 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 3);
        assert!((DAY..=2 * DAY).contains(&z.timestamp.second));
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
    let spec = spec_with(years(2050, 2060));
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
    let tz = zones(&["UTC", "Europe/London", "Asia/Kolkata", "America/New_York"]);
    let spec = ZonedSpec::default();
    assert_eq!(spec.bounds, FULL_CIVIL_BOUNDS);
    let (mut early, mut late) = (false, false);
    for seed in 0..2000 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert!((MIN_YEAR..=MAX_YEAR).contains(&z.civil.date.year));
        assert!(
            (MIN_INSTANT..=MAX_INSTANT).contains(&z.timestamp.second),
            "{z}"
        );
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
    let no_minute = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(1),
            max: at(59),
        },
        precision: Precision::Minutes,
        ..ZonedSpec::default()
    };
    assert!(no_minute.validate().is_ok());
    let mut ntc = fresh_ntc(0);
    let err = generate_zoned_datetime(&mut ntc, &tz, &no_minute).unwrap_err();
    assert!(
        matches!(&err, EngineError::InvalidArgument(msg) if msg.contains("no value at precision")),
        "{err:?}"
    );
    assert_eq!(ntc.nodes.len(), 1);
}

#[test]
fn the_instant_fallback_targets_both_sides_of_a_transition() {
    let tz = zones(&["America/New_York"]);
    let fold = 1_730_613_600;
    let spec = spec_with(ZonedBounds::Instant {
        min: at(fold - 3600),
        max: at(fold + 3599),
    });
    let (mut last_before, mut first_after) = (0, 0);
    for seed in 0..2000 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 2);
        last_before += usize::from(
            z.timestamp
                == Timestamp {
                    second: fold - 1,
                    nanosecond: 999_999_999,
                },
        );
        first_after += usize::from(z.timestamp == at(fold));
    }
    assert!((15..=150).contains(&last_before), "{last_before}");
    assert!((15..=150).contains(&first_after), "{first_after}");
}

#[test]
fn the_fallback_values_index_the_instants_in_order() {
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(-3600),
            max: at(3599),
        },
        precision: Precision::Seconds,
        ..ZonedSpec::default()
    };
    let utc = TimeZone::lookup("UTC").unwrap().unwrap();
    for zone in [utc, TimeZone::Unknown] {
        let values = Values::of(zone, &at(-3600), &at(3599), &spec).unwrap();
        assert_eq!(values.total, 7200);
        assert_eq!(values.runs.len(), 1);
        assert_eq!(values.index_at(0), 3600);
        assert_eq!(values.index_at(-3600 * NS - 1), 0);
        assert_eq!(values.index_at(3600 * NS), 7199);
        assert_eq!(values.index_at(1), 3601);
        assert_eq!(values.targets(-3600 * NS, 3599 * NS), vec![3600]);
        assert_eq!(
            values.at(3600).unwrap(),
            (civil(1970, 1, 1, 0, 0, 0), false)
        );
        assert_eq!(
            values.at(7199).unwrap(),
            (civil(1970, 1, 1, 0, 59, 59), false)
        );
        let err = values.at(7200).unwrap_err();
        assert!(matches!(err, EngineError::Internal(_)), "{err:?}");
    }
}

#[test]
fn instant_bounds_narrower_than_a_unit_hold_a_value_in_a_fractional_zone() {
    let kolkata = zones(&["Asia/Kolkata"]);
    let midnight = 1_704_067_200;
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(midnight + 1800),
            max: at(midnight + 3599),
        },
        precision: Precision::Hours,
        ..ZonedSpec::default()
    };
    assert!(spec.validate().is_ok());
    for seed in 0..50 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &kolkata, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 3);
        assert_eq!(z.civil, civil(2024, 1, 1, 6, 0, 0));
        assert_eq!(z.timestamp, at(midnight + 1800));
        check_with_jiff(&z);
    }
    let none = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(midnight),
            max: at(midnight + 1799),
        },
        ..spec
    };
    assert!(none.validate().is_ok());
    let mut ntc = fresh_ntc(0);
    let err = generate_zoned_datetime(&mut ntc, &kolkata, &none).unwrap_err();
    assert!(
        matches!(&err, EngineError::InvalidArgument(msg) if msg.contains("no value at precision")),
        "{err:?}"
    );
    let with_utc = zones(&["Asia/Kolkata", "UTC"]);
    let mut seen = [false; 2];
    for seed in 0..60 {
        let mut ntc = fresh_ntc(seed);
        match generate_zoned_datetime(&mut ntc, &with_utc, &none) {
            Ok(z) => {
                assert_eq!((z.zone.as_str(), z.timestamp), ("UTC", at(midnight)));
                seen[0] = true;
            }
            Err(err) => {
                assert!(matches!(err, EngineError::InvalidTestCase), "{err:?}");
                assert_eq!(ntc.status(), Some(Status::Invalid));
                seen[1] = true;
            }
        }
    }
    assert_eq!(seen, [true, true]);
}

#[test]
fn civil_bounds_narrow_to_the_timestamp_range() {
    let name = |s: &str| ChoiceValue::String(s.chars().map(u32::from).collect());
    let new_york = zones(&["America/New_York"]);
    let last_day = spec_with(ZonedBounds::Civil {
        min: civil(MAX_YEAR, 12, 31, 0, 0, 0),
        max: MAX_CIVIL,
    });
    for seed in 0..300 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &new_york, &last_day).unwrap();
        assert_eq!(ntc.nodes.len(), 3);
        assert!(z.civil.time.hour <= 18, "{z}");
        assert!(z.timestamp <= MAX_TIMESTAMP, "{z}");
    }
    let mut ntc = NativeTestCase::for_choices(
        &[
            name("America/New_York"),
            ChoiceValue::Integer(BigInt::from(0)),
            ChoiceValue::Integer(BigInt::from(19 * 3600 * NS - 1)),
        ],
        None,
        None,
    );
    let z = generate_zoned_datetime(&mut ntc, &new_york, &last_day).unwrap();
    assert_eq!(z.timestamp, MAX_TIMESTAMP);
    assert_eq!(
        z.to_string(),
        "+999999-12-31T18:59:59.999999999-05:00[America/New_York]"
    );
    let kolkata = zones(&["Asia/Kolkata"]);
    let first_day = spec_with(ZonedBounds::Civil {
        min: MIN_CIVIL,
        max: civil(MIN_YEAR, 1, 1, 23, 59, 59),
    });
    for seed in 0..300 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &kolkata, &first_day).unwrap();
        assert!(z.timestamp >= MIN_TIMESTAMP, "{z}");
    }
    let mut ntc = NativeTestCase::for_choices(
        &[
            name("Asia/Kolkata"),
            ChoiceValue::Integer(BigInt::from(0)),
            ChoiceValue::Integer(BigInt::from(0)),
        ],
        None,
        None,
    );
    let z = generate_zoned_datetime(&mut ntc, &kolkata, &first_day).unwrap();
    assert_eq!(z.timestamp, MIN_TIMESTAMP);
    assert_eq!(
        z.to_string(),
        "-999999-01-01T05:53:28+05:53:28[Asia/Kolkata]"
    );
    let before_the_range = spec_with(ZonedBounds::Civil {
        min: MIN_CIVIL,
        max: civil(MIN_YEAR, 1, 1, 5, 0, 0),
    });
    for seed in 0..20 {
        let mut ntc = fresh_ntc(seed);
        let err = generate_zoned_datetime(&mut ntc, &kolkata, &before_the_range).unwrap_err();
        assert!(matches!(err, EngineError::InvalidTestCase), "{err:?}");
        assert_eq!(ntc.status(), Some(Status::Invalid));
    }
}

#[test]
fn the_instant_fallback_reaches_every_value_at_the_precision() {
    let tz = zones(&["America/New_York"]);
    let spring = 1_205_046_000;
    let spec = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(spring - 100),
            max: at(spring),
        },
        precision: Precision::Minutes,
        ..ZonedSpec::default()
    };
    let mut seen = BTreeSet::new();
    for seed in 0..200 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 2);
        assert!((spring - 100..=spring).contains(&z.timestamp.second), "{z}");
        check_with_jiff(&z);
        seen.insert(z.to_string());
    }
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        [
            "2008-03-09T01:59:00-05:00[America/New_York]",
            "2008-03-09T02:00:00-04:00[America/New_York][gap]",
            "2008-03-09T03:00:00-04:00[America/New_York]",
        ]
    );
    let mut ntc = NativeTestCase::for_choices(
        &[
            ChoiceValue::String("America/New_York".chars().map(u32::from).collect()),
            ChoiceValue::Integer(BigInt::from(0)),
        ],
        None,
        None,
    );
    let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
    assert_eq!(z.timestamp, at(spring - 60));
    let avoid = ZonedSpec {
        imaginary: Imaginary::Avoid,
        ..spec
    };
    let mut seen = BTreeSet::new();
    for seed in 0..200 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &tz, &avoid).unwrap();
        assert_eq!(z.kind, Kind::Unique, "{z}");
        seen.insert(z.timestamp.second);
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), [spring - 60, spring]);
}

#[test]
fn an_instant_bound_inside_a_fold_reaches_the_other_repeat() {
    let tz = zones(&["America/New_York"]);
    let fold = 1_730_613_600;
    let name = ChoiceValue::String("America/New_York".chars().map(u32::from).collect());
    let from_the_earlier_repeat = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(fold - 1800),
            max: at(fold + 10 * DAY),
        },
        precision: Precision::Seconds,
        ..ZonedSpec::default()
    };
    let (mut later_repeat, mut rejected) = (0, 0);
    for seed in 0..2000 {
        let mut ntc = fresh_ntc(seed);
        let Ok(z) = generate_zoned_datetime(&mut ntc, &tz, &from_the_earlier_repeat) else {
            assert_eq!(ntc.status(), Some(Status::Invalid));
            rejected += 1;
            continue;
        };
        assert!(
            (fold - 1800..=fold + 10 * DAY).contains(&z.timestamp.second),
            "{z}"
        );
        check_with_jiff(&z);
        later_repeat += usize::from((fold..fold + 1800).contains(&z.timestamp.second));
    }
    assert!(
        later_repeat > 0 && rejected > 0,
        "{later_repeat} {rejected}"
    );
    let values = |fold: bool| {
        vec![
            name.clone(),
            ChoiceValue::Boolean(false),
            ChoiceValue::Integer(BigInt::from(0)),
            ChoiceValue::Integer(BigInt::from(0)),
            ChoiceValue::Boolean(fold),
        ]
    };
    let mut ntc = NativeTestCase::for_choices(&values(true), None, None);
    let z = generate_zoned_datetime(&mut ntc, &tz, &from_the_earlier_repeat).unwrap();
    assert_eq!(
        z.to_string(),
        "2024-11-03T01:00:00-05:00[America/New_York][fold=1]"
    );
    assert_eq!(z.timestamp, at(fold));
    let mut ntc = NativeTestCase::for_choices(&values(false), None, None);
    let err = generate_zoned_datetime(&mut ntc, &tz, &from_the_earlier_repeat).unwrap_err();
    assert!(matches!(err, EngineError::InvalidTestCase), "{err:?}");
    let to_the_later_repeat = ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(fold - 10 * DAY),
            max: at(fold + 1800),
        },
        ..from_the_earlier_repeat
    };
    let mut ntc = NativeTestCase::for_choices(
        &[
            name.clone(),
            ChoiceValue::Boolean(false),
            ChoiceValue::Integer(BigInt::from(10)),
            ChoiceValue::Integer(BigInt::from(7199)),
            ChoiceValue::Boolean(false),
        ],
        None,
        None,
    );
    let z = generate_zoned_datetime(&mut ntc, &tz, &to_the_later_repeat).unwrap();
    assert_eq!(
        z.to_string(),
        "2024-11-03T01:59:59-04:00[America/New_York][fold=0]"
    );
    assert_eq!(z.timestamp, at(fold - 1));
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
    let err = past_gap(TimeZone::Unknown, &resolution, 1, &Window::envelope()).unwrap_err();
    assert!(matches!(err, EngineError::Internal(_)), "{err:?}");
}

#[test]
fn wall_arithmetic_round_trips() {
    for seed in 0..200 {
        let mut ntc = fresh_ntc(seed);
        let z = generate_zoned_datetime(&mut ntc, &zones(&["UTC"]), &ZonedSpec::default()).unwrap();
        assert_eq!(datetime_of_wall(wall_ns(&z.civil)), z.civil);
    }
    assert_eq!(align_up(-7, 5), -5);
    assert_eq!(align_up(-10, 5), -10);
    assert_eq!(align_down(-7, 5), -10);
    assert_eq!(align_down(7, 5), 5);
}

#[test]
fn a_bound_inside_a_gap_reaches_the_skipped_wall_times() {
    let tz = zones(&["America/New_York"]);
    let spring = 1_710_054_000;
    let zone = TimeZone::lookup("America/New_York").unwrap().unwrap();
    let instants = |min: i64, max: i64| ZonedSpec {
        bounds: ZonedBounds::Instant {
            min: at(min),
            max: at(max),
        },
        precision: Precision::Seconds,
        ..ZonedSpec::default()
    };
    let spec = instants(spring + 1800, spring + 7200);
    let values = Values::of(zone, &at(spring + 1800), &at(spring + 7200), &spec).unwrap();
    assert_eq!(values.total, 7201);
    assert_eq!(values.at(0).unwrap(), (civil(2024, 3, 10, 2, 30, 0), false));
    assert_eq!(
        values.at(1800).unwrap(),
        (civil(2024, 3, 10, 3, 30, 0), false)
    );
    let avoid = ZonedSpec {
        imaginary: Imaginary::Avoid,
        ..spec
    };
    let values = Values::of(zone, &at(spring + 1800), &at(spring + 7200), &avoid).unwrap();
    assert_eq!(values.total, 5401);
    let mut ntc = NativeTestCase::for_choices(
        &[
            ChoiceValue::String("America/New_York".chars().map(u32::from).collect()),
            ChoiceValue::Integer(BigInt::from(0)),
        ],
        None,
        None,
    );
    let z = generate_zoned_datetime(&mut ntc, &tz, &spec).unwrap();
    assert_eq!(
        z.to_string(),
        "2024-03-10T02:30:00-04:00[America/New_York][gap]"
    );
    assert_eq!(z.timestamp, at(spring + 1800));
    for (min, max, fallback) in [
        (spring, spring + 7200, true),
        (spring + 1800, spring + 7200, true),
        (spring, spring + 3 * DAY, false),
        (spring + 1800, spring + 3 * DAY, false),
    ] {
        let spec = instants(min, max);
        let (mut gaps, mut rejected) = (0, 0);
        for seed in 0..600 {
            let mut ntc = fresh_ntc(seed);
            match generate_zoned_datetime(&mut ntc, &tz, &spec) {
                Ok(z) => {
                    assert!((min..=max).contains(&z.timestamp.second), "{z}");
                    check_with_jiff(&z);
                    gaps += usize::from(z.kind == Kind::Gap);
                }
                Err(err) => {
                    assert!(matches!(err, EngineError::InvalidTestCase), "{err:?}");
                    assert!(!fallback, "{spec:?}");
                    rejected += 1;
                }
            }
        }
        assert!(gaps > 0, "{spec:?}: {gaps} gap draws, {rejected} rejected");
        let avoid = ZonedSpec {
            imaginary: Imaginary::Avoid,
            ..spec
        };
        for seed in 0..100 {
            let mut ntc = fresh_ntc(seed);
            if let Ok(z) = generate_zoned_datetime(&mut ntc, &tz, &avoid) {
                assert_eq!(z.kind, Kind::Unique, "{z}");
            }
        }
    }
}

#[test]
fn a_run_between_two_folds_ends_in_the_earlier_repeat() {
    let ltt = |utoff: i32| tzdata::LocalTimeType {
        utoff,
        is_dst: false,
        abbreviation: 0,
    };
    let t1 = 959_817_600;
    let t2 = t1 + 12 * 3600;
    let zone: &'static tzdata::Zone = Box::leak(Box::new(tzdata::Zone {
        name: "synthetic".to_string(),
        abbreviations: vec!["S".to_string()],
        initial: ltt(3600),
        transitions: vec![
            tzdata::Transition {
                instant: t1,
                kind: ltt(0),
            },
            tzdata::Transition {
                instant: t2,
                kind: ltt(-3600),
            },
        ],
        footer: None,
    }));
    let name: &'static tzdata::Name = Box::leak(Box::new(tzdata::Name {
        name: "synthetic".to_string(),
        rank: u16::MAX,
        tags: tzdata::Tags::EMPTY,
        zone,
    }));
    let zone = TimeZone::Named(name);
    let (min, max) = (at(t1 - 3600), at(t2 + 3600));
    for (policy, excluded) in [
        (Ambiguous::Both, vec![]),
        (Ambiguous::Earlier, vec![t1..t1 + 3600, t2..t2 + 3600]),
        (Ambiguous::Later, vec![t1 - 3600..t1, t2 - 3600..t2]),
    ] {
        let spec = ZonedSpec {
            bounds: ZonedBounds::Instant { min, max },
            precision: Precision::Minutes,
            ambiguous: policy,
            ..ZonedSpec::default()
        };
        let values = Values::of(zone, &min, &max, &spec).unwrap();
        let instants: Vec<i64> = (0..values.total)
            .map(|i| {
                let (civil, fold) = values.at(i).unwrap();
                let r = zone.resolve(&civil, fold).unwrap();
                assert!(!fold || r.kind == Kind::Fold, "{civil:?} {fold}");
                assert_eq!(r.timestamp.nanosecond, 0);
                r.timestamp.second
            })
            .collect();
        let expected: Vec<i64> = (t1 - 3600..=t2 + 3600)
            .step_by(60)
            .filter(|s| !excluded.iter().any(|r| r.contains(s)))
            .collect();
        assert_eq!(instants, expected, "{policy:?}");
    }
}
