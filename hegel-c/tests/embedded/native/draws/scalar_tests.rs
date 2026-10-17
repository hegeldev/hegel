//! Tests for `src/native/draws/scalar.rs`: every drawn timestamp, offset,
//! duration and span lies in its bounds at its precision and agrees with
//! jiff's own types; the samplers reach the catalogues of Appendix C; the
//! node shapes are as `hegel-c/docs/datetime.md` §6.3 says; invalid
//! arguments are rejected; and a failing value shrinks to the origin.

use super::*;
use crate::exchange::drive_no_yield;
use crate::native::bignum::BigInt;
use crate::native::core::choices::BooleanChoice;
use crate::native::core::{ChoiceKind, ChoiceNode, ChoiceValue, NativeTestCase};
use crate::native::rng::EngineRng;
use crate::native::shrinker::{ShrinkRun, Shrinker};
use crate::native::timezone::{MAX_INSTANT, MIN_INSTANT};
use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;

fn fresh_ntc(seed: u64) -> NativeTestCase {
    NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap()
}

fn at(second: i64) -> Timestamp {
    Timestamp {
        second,
        nanosecond: 0,
    }
}

fn duration(seconds: i64, nanoseconds: i32) -> Duration {
    Duration {
        seconds,
        nanoseconds,
    }
}

fn replay<T>(
    nodes: &[ChoiceNode],
    draw: impl Fn(&mut NativeTestCase) -> Result<T, EngineError>,
) -> T {
    let values: Vec<ChoiceValue> = nodes.iter().map(|n| n.value()).collect();
    let mut ntc = NativeTestCase::for_choices(&values, None, None);
    draw(&mut ntc).unwrap()
}

fn simplest<T>(draw: impl Fn(&mut NativeTestCase) -> Result<T, EngineError>) -> T {
    let mut ntc = NativeTestCase::for_simplest(1000).unwrap();
    draw(&mut ntc).unwrap()
}

/// Shrink the first seeded draw `interesting` accepts and replay the
/// result.
fn shrunk<T: 'static>(
    draw: impl Fn(&mut NativeTestCase) -> Result<T, EngineError> + Send + Sync + 'static,
    interesting: impl Fn(&T) -> bool + Send + Sync + 'static,
) -> T {
    let draw = Arc::new(draw);
    let interesting = Arc::new(interesting);
    let run_once = {
        let (draw, interesting) = (Arc::clone(&draw), Arc::clone(&interesting));
        move |ntc: &mut NativeTestCase| {
            let result = draw(ntc);
            ntc.freeze();
            matches!(result, Ok(v) if interesting(&v))
        }
    };
    let (nodes, spans) = (0..)
        .find_map(|seed| {
            let mut ntc = fresh_ntc(seed);
            run_once(&mut ntc).then(|| (ntc.nodes.clone(), ntc.spans.clone()))
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
        let ok = run_once(&mut ntc);
        (ok, ntc.nodes.clone(), ntc.spans.clone())
    };
    let mut shrinker = Shrinker::with_probe(Box::new(probe), nodes, spans);
    drive_no_yield(shrinker.shrink()).unwrap();
    replay(&shrinker.current_nodes, |ntc| draw(ntc))
}

fn integer_bounds(node: &ChoiceNode) -> (BigInt, BigInt) {
    match node.kind() {
        ChoiceKind::Integer(ic) => (ic.min_value(), ic.max_value()),
        other => panic!("{other:?}"),
    }
}

fn jiff_timestamp(t: &Timestamp) -> jiff::Timestamp {
    jiff::Timestamp::new(t.second, i32::try_from(t.nanosecond).unwrap()).unwrap()
}

#[test]
fn timestamps_lie_in_bounds_at_the_precision_and_agree_with_jiff() {
    let (min, max) = (
        Timestamp {
            second: -1_000_000_000,
            nanosecond: 123,
        },
        Timestamp {
            second: 4_000_000_000,
            nanosecond: 999_999_999,
        },
    );
    for precision in [
        Precision::Nanoseconds,
        Precision::Milliseconds,
        Precision::Seconds,
        Precision::Hours,
    ] {
        let unit = i128::from(precision.unit());
        let mut seen_fraction = false;
        for seed in 0..300 {
            let mut ntc = fresh_ntc(seed);
            let t = generate_timestamp(&mut ntc, min, max, precision).unwrap();
            assert!(min <= t && t <= max, "{t:?} outside [{min:?}, {max:?}]");
            assert_eq!(instant_ns(&t) % unit, 0, "{t:?} at {precision:?}");
            assert_eq!(ntc.nodes.len(), 1);
            let (lo, hi) = integer_bounds(&ntc.nodes[0]);
            let values =
                (align_down(instant_ns(&max), unit) - align_up(instant_ns(&min), unit)) / unit;
            assert_eq!(hi - lo, BigInt::from(values));
            assert_eq!(jiff_timestamp(&t).as_nanosecond(), instant_ns(&t));
            seen_fraction |= t.nanosecond != 0;
        }
        assert_eq!(
            seen_fraction,
            precision != Precision::Seconds && precision != Precision::Hours
        );
    }
}

#[test]
fn timestamps_shrink_to_2000_clamped() {
    let origin = at(EPOCH_2000);
    assert_eq!(jiff_timestamp(&origin).to_string(), "2000-01-01T00:00:00Z");
    let full = (at(MIN_INSTANT), at(MAX_INSTANT));
    assert_eq!(
        simplest(|ntc| generate_timestamp(ntc, full.0, full.1, Precision::Nanoseconds)),
        origin
    );
    let later = (at(EPOCH_2000 + 7), at(MAX_INSTANT));
    assert_eq!(
        simplest(|ntc| generate_timestamp(ntc, later.0, later.1, Precision::Minutes)),
        at(EPOCH_2000 + 60)
    );
    let earlier = (at(MIN_INSTANT), at(EPOCH_2000 - 1));
    assert_eq!(
        simplest(|ntc| generate_timestamp(ntc, earlier.0, earlier.1, Precision::Seconds)),
        at(EPOCH_2000 - 1)
    );
    let z = shrunk(
        move |ntc| generate_timestamp(ntc, full.0, full.1, Precision::Nanoseconds),
        |t| t.second > EPOCH_2000 + 86_400 || t.second < EPOCH_2000 - 86_400,
    );
    assert_eq!(
        z,
        Timestamp {
            second: EPOCH_2000 - 86_401,
            nanosecond: 999_999_999
        }
    );
}

#[test]
fn timestamps_target_the_interesting_instants_and_the_transitions() {
    let catalogue = TRANSITION_CATALOGUE.as_ref().unwrap();
    assert!(catalogue.len() > 5_000, "{}", catalogue.len());
    assert!(catalogue.windows(2).all(|w| w[0] < w[1]));
    let london = jiff::tz::TimeZone::get("Europe/London").unwrap();
    let march_2024 = jiff::civil::date(2024, 3, 31)
        .at(1, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .unwrap()
        .timestamp();
    assert_eq!(
        london.to_offset(march_2024).seconds(),
        3_600,
        "the 2024 spring transition of London is at {march_2024}"
    );
    assert!(catalogue.binary_search(&march_2024.as_second()).is_ok());
    let interesting: BTreeSet<i128> = interesting_instant_ns().collect();
    assert_eq!(interesting.len(), 44, "{interesting:?}");
    assert!(interesting.contains(&(i128::from(i32::MAX) * NS)));
    assert_eq!(
        jiff_timestamp(&at(i64::from(i32::MAX))).to_string(),
        "2038-01-19T03:14:07Z"
    );
    let (min, max) = (at(-3_000_000_000), at(5_000_000_000));
    let (mut hits, mut transitions) = (BTreeSet::new(), 0);
    for seed in 0..4000 {
        let mut ntc = fresh_ntc(seed);
        let t = generate_timestamp(&mut ntc, min, max, Precision::Nanoseconds).unwrap();
        let ns = instant_ns(&t);
        if interesting.contains(&ns) {
            hits.insert(ns);
        } else if t.nanosecond == 0 && catalogue.binary_search(&t.second).is_ok() {
            transitions += 1;
        }
    }
    assert!(hits.len() >= 10, "{hits:?}");
    assert!((20..=300).contains(&transitions), "{transitions}");
}

#[test]
fn timestamps_reject_invalid_arguments() {
    let mut ntc = fresh_ntc(0);
    let bad = [
        (at(MIN_INSTANT - 1), at(0), Precision::Seconds),
        (at(0), at(MAX_INSTANT + 1), Precision::Seconds),
        (at(1), at(0), Precision::Seconds),
        (
            Timestamp {
                second: 0,
                nanosecond: 1,
            },
            Timestamp {
                second: 0,
                nanosecond: 999_999_999,
            },
            Precision::Seconds,
        ),
    ];
    for (min, max, precision) in bad {
        let err = generate_timestamp(&mut ntc, min, max, precision).unwrap_err();
        assert!(
            matches!(err, EngineError::InvalidArgument(_)),
            "{min:?}..{max:?}: {err}"
        );
    }
    assert!(ntc.nodes.is_empty());
}

fn jiff_offset(o: &UtcOffset) -> jiff::tz::Offset {
    jiff::tz::Offset::from_seconds(o.seconds).unwrap()
}

#[test]
fn offsets_lie_in_bounds_at_the_granularity() {
    for granularity in [
        Granularity::Hours,
        Granularity::QuarterHours,
        Granularity::Minutes,
        Granularity::Seconds,
    ] {
        let unit = granularity.unit();
        let mut seen = BTreeSet::new();
        for seed in 0..500 {
            let mut ntc = fresh_ntc(seed);
            let o = generate_offset(&mut ntc, -MAX_OFFSET, MAX_OFFSET, granularity, false).unwrap();
            assert!(o.known);
            assert!((-MAX_OFFSET..=MAX_OFFSET).contains(&o.seconds), "{o:?}");
            assert_eq!(o.seconds % unit, 0, "{o:?} at {granularity:?}");
            assert_eq!(jiff_offset(&o).seconds(), o.seconds);
            assert_eq!(ntc.nodes.len(), 1);
            let (lo, hi) = integer_bounds(&ntc.nodes[0]);
            assert_eq!(hi - lo, BigInt::from(2 * (MAX_OFFSET / unit)));
            seen.insert(o.seconds);
        }
        assert!(seen.len() > 50, "{granularity:?}: {}", seen.len());
        assert!(seen.iter().any(|&s| s < 0) && seen.iter().any(|&s| s > 0));
    }
    for seed in 0..200 {
        let mut ntc = fresh_ntc(seed);
        let o = generate_offset(&mut ntc, -1_799, 5_400, Granularity::Hours, false).unwrap();
        assert!([0, 3_600].contains(&o.seconds), "{o:?}");
    }
}

#[test]
fn offsets_target_the_offsets_zones_have_used() {
    let catalogue = OFFSET_CATALOGUE.as_ref().unwrap();
    assert!(catalogue.windows(2).all(|w| w[0] < w[1]));
    assert!(
        catalogue
            .iter()
            .all(|&s| (-MAX_OFFSET..=MAX_OFFSET).contains(&s))
    );
    let kathmandu = jiff::tz::TimeZone::get("Asia/Kathmandu").unwrap();
    assert_eq!(
        kathmandu.to_offset(jiff::Timestamp::UNIX_EPOCH).seconds(),
        19_800
    );
    for expected in [
        19_800,
        20_700,
        45_900,
        49_500,
        31_500,
        -12_600,
        12_600,
        16_200,
        -16_200,
        30_600,
        -34_200,
        37_800,
        50_400,
        -43_200,
        -2_670,
        -1_521,
        11_212,
        -57_368,
        64_800,
        -64_800,
        86_399,
        -86_399,
        MAX_OFFSET,
        -MAX_OFFSET,
        1,
        -1,
        0,
    ] {
        assert!(
            catalogue.binary_search(&expected).is_ok(),
            "{expected} in {catalogue:?}"
        );
    }
    let mut hits = BTreeSet::new();
    for seed in 0..3000 {
        let mut ntc = fresh_ntc(seed);
        let o = generate_offset(
            &mut ntc,
            -MAX_OFFSET,
            MAX_OFFSET,
            Granularity::Seconds,
            false,
        )
        .unwrap();
        if o.seconds % 900 != 0 && catalogue.binary_search(&o.seconds).is_ok() {
            hits.insert(o.seconds);
        }
    }
    assert!(hits.len() >= 10, "{hits:?}");
}

#[test]
fn offsets_shrink_to_known_zero_then_the_offset() {
    let draw = |ntc: &mut NativeTestCase| {
        generate_offset(ntc, -MAX_OFFSET, MAX_OFFSET, Granularity::Minutes, true)
    };
    assert_eq!(
        simplest(draw),
        UtcOffset {
            seconds: 0,
            known: true
        }
    );
    let unknown = replay(
        &[ChoiceNode::boolean(
            BooleanChoice {
                p: UNKNOWN_PROBABILITY,
            },
            true,
            false,
        )],
        draw,
    );
    assert_eq!(
        unknown,
        UtcOffset {
            seconds: 0,
            known: false
        }
    );
    let (mut unknowns, draws) = (0, 2000);
    for seed in 0..draws {
        let mut ntc = fresh_ntc(seed);
        let o = draw(&mut ntc).unwrap();
        assert_eq!(ntc.nodes.len(), if o.known { 2 } else { 1 });
        unknowns += usize::from(!o.known);
    }
    let rate = unknowns as f64 / draws as f64;
    assert!((0.06..=0.14).contains(&rate), "{rate}");
    assert_eq!(
        shrunk(draw, |o| o.known && o.seconds.unsigned_abs() >= 3_600),
        UtcOffset {
            seconds: 3_600,
            known: true
        }
    );
    assert_eq!(
        shrunk(draw, |o| !o.known || o.seconds.unsigned_abs() >= 3_600),
        UtcOffset {
            seconds: 0,
            known: false
        }
    );
    assert_eq!(
        simplest(|ntc| generate_offset(ntc, 3_601, 7_200, Granularity::Hours, false)),
        UtcOffset {
            seconds: 7_200,
            known: true
        }
    );
    assert_eq!(
        simplest(|ntc| generate_offset(ntc, -7_200, -3_601, Granularity::Hours, false)),
        UtcOffset {
            seconds: -7_200,
            known: true
        }
    );
}

#[test]
fn offsets_reject_invalid_arguments() {
    let mut ntc = fresh_ntc(0);
    for (min, max, granularity) in [
        (-MAX_OFFSET - 1, 0, Granularity::Seconds),
        (0, MAX_OFFSET + 1, Granularity::Seconds),
        (1, 0, Granularity::Seconds),
        (1, 3_599, Granularity::Hours),
        (-3_599, -1, Granularity::Hours),
    ] {
        let err = generate_offset(&mut ntc, min, max, granularity, true).unwrap_err();
        assert!(
            matches!(err, EngineError::InvalidArgument(_)),
            "{min}..{max}: {err}"
        );
    }
    assert!(ntc.nodes.is_empty());
}

#[test]
fn durations_lie_in_bounds_at_the_precision_and_agree_with_jiff() {
    let (min, max) = (
        duration(-100_000, -500_000_001),
        duration(100_000, 999_999_999),
    );
    for precision in [
        Precision::Nanoseconds,
        Precision::Microseconds,
        Precision::Seconds,
        Precision::Minutes,
    ] {
        let unit = i128::from(precision.unit());
        let (mut negatives, mut fractions) = (0, 0);
        for seed in 0..300 {
            let mut ntc = fresh_ntc(seed);
            let d = generate_duration(&mut ntc, min, max, precision).unwrap();
            let ns = duration_ns(&d);
            assert!(
                (duration_ns(&min)..=duration_ns(&max)).contains(&ns),
                "{d:?}"
            );
            assert_eq!(ns % unit, 0, "{d:?} at {precision:?}");
            validate_duration("drawn", &d).unwrap();
            let theirs = jiff::SignedDuration::new(d.seconds, d.nanoseconds);
            assert_eq!(theirs.as_nanos(), ns);
            assert_eq!(i128::from(theirs.as_secs()), i128::from(d.seconds));
            assert_eq!(theirs.subsec_nanos(), d.nanoseconds);
            assert_eq!(ntc.nodes.len(), 1);
            negatives += usize::from(ns < 0);
            fractions += usize::from(d.nanoseconds != 0);
        }
        assert!(negatives > 50, "{precision:?}: {negatives}");
        assert_eq!(
            fractions > 0,
            matches!(precision, Precision::Nanoseconds | Precision::Microseconds),
            "{precision:?}: {fractions}"
        );
    }
}

#[test]
fn durations_shrink_to_zero_clamped_and_target_the_catalogue() {
    let (min, max) = (duration(i64::MIN, 0), duration(i64::MAX, 0));
    assert_eq!(
        simplest(|ntc| generate_duration(ntc, min, max, Precision::Nanoseconds)),
        duration(0, 0)
    );
    assert_eq!(
        simplest(|ntc| generate_duration(ntc, duration(0, 1), max, Precision::Milliseconds)),
        duration(0, 1_000_000)
    );
    assert_eq!(
        simplest(|ntc| generate_duration(ntc, min, duration(0, -1), Precision::Milliseconds)),
        duration(0, -1_000_000)
    );
    assert_eq!(
        shrunk(
            move |ntc| generate_duration(ntc, min, max, Precision::Nanoseconds),
            |d| d.seconds <= -86_400 || d.seconds >= 86_400
        ),
        duration(86_400, 0)
    );
    let interesting: BTreeSet<i128> = interesting_duration_ns().collect();
    assert_eq!(interesting.len(), 24);
    let mut hits = BTreeSet::new();
    for seed in 0..3000 {
        let mut ntc = fresh_ntc(seed);
        let d = generate_duration(&mut ntc, min, max, Precision::Nanoseconds).unwrap();
        let ns = duration_ns(&d);
        if interesting.contains(&ns) && ns.unsigned_abs() > 1 {
            hits.insert(ns);
        }
    }
    assert!(hits.len() >= 8, "{hits:?}");
}

#[test]
fn durations_reject_invalid_arguments() {
    let mut ntc = fresh_ntc(0);
    for (min, max, precision) in [
        (
            duration(0, 1_000_000_000),
            duration(1, 0),
            Precision::Seconds,
        ),
        (duration(0, 0), duration(1, -1), Precision::Seconds),
        (duration(-1, 1), duration(1, 0), Precision::Seconds),
        (duration(1, 0), duration(0, 0), Precision::Seconds),
        (duration(0, 1), duration(0, 999_999_999), Precision::Seconds),
        (
            duration(-5, -999_999_999),
            duration(-5, -1),
            Precision::Seconds,
        ),
    ] {
        let err = generate_duration(&mut ntc, min, max, precision).unwrap_err();
        assert!(
            matches!(err, EngineError::InvalidArgument(_)),
            "{min:?}..{max:?}: {err}"
        );
    }
    assert!(ntc.nodes.is_empty());
}

fn span_spec(sign: SpanSign, bounds: &[(SpanUnit, i64, i64)]) -> SpanSpec {
    let mut spec = SpanSpec {
        sign,
        ..SpanSpec::default()
    };
    for &(unit, min, max) in bounds {
        spec.set_unit_bounds(unit, min, max);
    }
    spec
}

fn jiff_span(s: &Span) -> jiff::Span {
    let [
        years,
        months,
        weeks,
        days,
        hours,
        minutes,
        seconds,
        milliseconds,
        microseconds,
        nanoseconds,
    ] = s.units;
    jiff::Span::new()
        .try_years(years)
        .unwrap()
        .try_months(months)
        .unwrap()
        .try_weeks(weeks)
        .unwrap()
        .try_days(days)
        .unwrap()
        .try_hours(hours)
        .unwrap()
        .try_minutes(minutes)
        .unwrap()
        .try_seconds(seconds)
        .unwrap()
        .try_milliseconds(milliseconds)
        .unwrap()
        .try_microseconds(microseconds)
        .unwrap()
        .try_nanoseconds(nanoseconds)
        .unwrap()
}

#[test]
fn uniform_spans_share_one_sign_and_agree_with_jiff() {
    let spec = span_spec(
        SpanSign::Uniform,
        &[
            (SpanUnit::Years, -10, 10),
            (SpanUnit::Days, -400, 400),
            (SpanUnit::Nanoseconds, 0, 999_999_999),
        ],
    );
    let (mut negatives, mut positives) = (0, 0);
    for seed in 0..500 {
        let mut ntc = fresh_ntc(seed);
        let s = generate_span(&mut ntc, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 4, "{:?}", ntc.nodes);
        assert!(matches!(ntc.nodes[0].kind(), ChoiceKind::Boolean(_)));
        let signs: BTreeSet<i64> = s
            .units
            .iter()
            .map(|v| v.signum())
            .filter(|&s| s != 0)
            .collect();
        assert!(signs.len() <= 1, "{s:?}");
        assert!((-10..=10).contains(&s.units[0]), "{s:?}");
        assert!((-400..=400).contains(&s.units[3]), "{s:?}");
        assert!((-999_999_999..=999_999_999).contains(&s.units[9]), "{s:?}");
        assert!(
            [1, 2, 4, 5, 6, 7, 8].iter().all(|&i| s.units[i] == 0),
            "{s:?}"
        );
        let theirs = jiff_span(&s);
        assert_eq!(i64::from(theirs.get_years()), s.units[0]);
        assert_eq!(i64::from(theirs.get_days()), s.units[3]);
        assert_eq!(theirs.get_nanoseconds(), s.units[9]);
        negatives += usize::from(signs.contains(&-1));
        positives += usize::from(signs.contains(&1));
    }
    assert!(
        negatives > 100 && positives > 100,
        "{negatives} {positives}"
    );
    let negative = span_spec(SpanSign::Uniform, &[(SpanUnit::Hours, -48, -1)]);
    let positive = span_spec(
        SpanSign::Uniform,
        &[(SpanUnit::Hours, 1, 48), (SpanUnit::Minutes, -59, 59)],
    );
    for seed in 0..100 {
        let mut ntc = fresh_ntc(seed);
        let s = generate_span(&mut ntc, &negative).unwrap();
        assert_eq!(ntc.nodes.len(), 1);
        assert!((-48..=-1).contains(&s.units[4]), "{s:?}");
        let mut ntc = fresh_ntc(seed);
        let s = generate_span(&mut ntc, &positive).unwrap();
        assert_eq!(ntc.nodes.len(), 2);
        assert!(
            (1..=48).contains(&s.units[4]) && (0..=59).contains(&s.units[5]),
            "{s:?}"
        );
    }
}

#[test]
fn mixed_and_nonnegative_spans_draw_each_unit_within_its_bounds() {
    let mixed = span_spec(
        SpanSign::Mixed,
        &[
            (SpanUnit::Hours, -1, 1),
            (SpanUnit::Minutes, -30, 30),
            (SpanUnit::Seconds, i64::MIN, i64::MAX),
        ],
    );
    let mut mixed_signs = false;
    for seed in 0..500 {
        let mut ntc = fresh_ntc(seed);
        let s = generate_span(&mut ntc, &mixed).unwrap();
        assert_eq!(ntc.nodes.len(), 3);
        assert!(
            (-1..=1).contains(&s.units[4]) && (-30..=30).contains(&s.units[5]),
            "{s:?}"
        );
        mixed_signs |= s.units[4].signum() * s.units[5].signum() < 0;
    }
    assert!(mixed_signs);
    let nonnegative = span_spec(
        SpanSign::NonNegative,
        &[(SpanUnit::Months, 0, 11), (SpanUnit::Weeks, 2, 4)],
    );
    for seed in 0..200 {
        let mut ntc = fresh_ntc(seed);
        let s = generate_span(&mut ntc, &nonnegative).unwrap();
        assert_eq!(ntc.nodes.len(), 2);
        assert!(
            (0..=11).contains(&s.units[1]) && (2..=4).contains(&s.units[2]),
            "{s:?}"
        );
    }
}

#[test]
fn spans_shrink_to_zero_clamped() {
    let everything: Vec<(SpanUnit, i64, i64)> = SpanUnit::ALL
        .into_iter()
        .map(|unit| (unit, i64::MIN, i64::MAX))
        .collect();
    for sign in [SpanSign::Uniform, SpanSign::Mixed] {
        let spec = span_spec(sign, &everything);
        assert_eq!(
            simplest(move |ntc| generate_span(ntc, &spec)),
            Span::default()
        );
        assert_eq!(
            shrunk(
                move |ntc| generate_span(ntc, &spec),
                |s| s.units.iter().filter(|&&v| v != 0).count() >= 2
            )
            .units,
            [0, 0, 0, 0, 0, 0, 0, 0, 1, 1]
        );
    }
    let extremes = span_spec(
        SpanSign::Uniform,
        &[
            (SpanUnit::Years, i64::MIN, i64::MIN),
            (SpanUnit::Days, -5, 5),
        ],
    );
    assert_eq!(
        simplest(move |ntc| generate_span(ntc, &extremes)).units,
        [i64::MIN, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    let clamped = span_spec(SpanSign::NonNegative, &[(SpanUnit::Days, 3, 5)]);
    assert_eq!(
        simplest(move |ntc| generate_span(ntc, &clamped)).units[3],
        3
    );
}

#[test]
fn spans_reject_invalid_specs() {
    let mut ntc = fresh_ntc(0);
    let bad = [
        span_spec(SpanSign::Uniform, &[]),
        span_spec(SpanSign::Uniform, &[(SpanUnit::Years, 2, 1)]),
        span_spec(
            SpanSign::Uniform,
            &[(SpanUnit::Years, 1, 2), (SpanUnit::Months, -2, -1)],
        ),
        span_spec(SpanSign::NonNegative, &[(SpanUnit::Years, -1, 2)]),
    ];
    for spec in &bad {
        let err = generate_span(&mut ntc, spec).unwrap_err();
        assert!(
            matches!(err, EngineError::InvalidArgument(_)),
            "{spec:?}: {err}"
        );
    }
    assert!(ntc.nodes.is_empty());
    assert!(bad[1].validate().is_err());
    assert!(bad[2].validate().is_err());
    assert!(bad[3].validate().is_err());
    assert!(bad[0].validate().is_ok());
    let mixed = span_spec(
        SpanSign::Mixed,
        &[(SpanUnit::Years, 1, 2), (SpanUnit::Months, -2, -1)],
    );
    assert!(mixed.validate().is_ok());
}

#[test]
fn spans_replay_their_nodes() {
    let spec = span_spec(
        SpanSign::Uniform,
        &[(SpanUnit::Years, -3, 3), (SpanUnit::Months, -11, 11)],
    );
    let nodes = vec![
        ChoiceValue::Boolean(true),
        ChoiceValue::Integer(BigInt::from(2)),
        ChoiceValue::Integer(BigInt::from(5)),
    ];
    let mut ntc = NativeTestCase::for_choices(&nodes, None, None);
    let s = generate_span(&mut ntc, &spec).unwrap();
    assert_eq!(s.units[..2], [-2, -5]);
    let mut ntc = NativeTestCase::for_choices(
        &[
            ChoiceValue::Boolean(false),
            ChoiceValue::Integer(BigInt::from(2)),
            ChoiceValue::Integer(BigInt::from(5)),
        ],
        None,
        None,
    );
    let s = generate_span(&mut ntc, &spec).unwrap();
    assert_eq!(s.units[..2], [2, 5]);
}
