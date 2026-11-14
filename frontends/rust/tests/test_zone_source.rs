mod common;

use common::utils::{assert_all_examples, expect_panic, find_any};
use hegel::generators as gs;
use hegel::generators::temporal::{
    AfterProbe, Date, DateTime, DstRule, Error, Instant, LocalTimeType, Month, RuleDate, RuleDay,
    Tag, Time, TransitionKind, UNKNOWN_ZONE, UtcOffset, Weekday, ZoneAfter, ZoneData, ZoneOracle,
    ZoneRule, ZoneSource, ZoneTransition, tzdb_version,
};
use hegel::{Generator, TestCase};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn datetime(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> DateTime {
    DateTime::new(Date::new(year, month, day), Time::new(hour, minute, 0, 0))
}

fn local(offset_hours: i32, is_dst: bool, abbreviation: &str) -> LocalTimeType {
    LocalTimeType {
        offset: offset_hours * 3600,
        is_dst,
        abbreviation: abbreviation.to_string(),
    }
}

fn kind(source: &ZoneSource, name: &str, civil: DateTime) -> TransitionKind {
    source.resolve(name, civil).unwrap().kind
}

fn offset(source: &ZoneSource, name: &str, second: i64) -> UtcOffset {
    source
        .offset_at(name, Instant::new(second, 0))
        .unwrap()
        .offset
}

fn hours(n: i32) -> UtcOffset {
    UtcOffset::Known(n * 3600)
}

fn tzif(name: &str) -> &'static [u8] {
    jiff_tzdb::get(name).unwrap().1
}

fn us_rule() -> ZoneRule {
    ZoneRule {
        standard: local(-5, false, "EST"),
        dst: Some(DstRule {
            local_type: local(-4, true, "EDT"),
            start: RuleDate {
                day: RuleDay::MonthWeekday {
                    month: Month::March,
                    week: 2,
                    weekday: Weekday::Sunday,
                },
                time: 7200,
            },
            end: RuleDate {
                day: RuleDay::MonthWeekday {
                    month: Month::November,
                    week: 1,
                    weekday: Weekday::Sunday,
                },
                time: 7200,
            },
        }),
    }
}

/// A New York whose clocks have always followed today's US rule.
fn rule_zone() -> ZoneData {
    ZoneData {
        initial: local(-5, false, "EST"),
        transitions: vec![],
        after: ZoneAfter::Rule(us_rule()),
    }
}

#[test]
fn test_bundled_source_is_the_engines_database() {
    let bundled = ZoneSource::bundled();
    assert_eq!(bundled.version(), tzdb_version());
    let info = bundled.info("Asia/Calcutta").unwrap().unwrap();
    assert_eq!(info.primary_name, "Asia/Kolkata");
    assert_eq!(bundled.info("Mars/Olympus_Mons").unwrap(), None);
    assert_eq!(
        kind(&bundled, "America/New_York", datetime(2024, 3, 10, 2, 30)),
        TransitionKind::Gap
    );
    assert_eq!(
        offset(&bundled, "America/New_York", 1_720_000_000),
        hours(-4)
    );
    let zone = bundled
        .resolve("Asia/Calcutta", datetime(2024, 1, 1, 0, 0))
        .unwrap()
        .zone;
    assert_eq!(zone.primary_name, "Asia/Kolkata");
}

#[test]
fn test_tzif_source() {
    let source = ZoneSource::builder()
        .tzif("America/New_York", tzif("America/New_York"))
        .tzif("Test/Fixed", tzif("Asia/Tokyo"))
        .build()
        .unwrap();
    assert_eq!(source.version(), "");
    assert_eq!(
        kind(&source, "America/New_York", datetime(2024, 3, 10, 2, 30)),
        TransitionKind::Gap
    );
    assert_eq!(
        kind(&source, "America/New_York", datetime(2024, 11, 3, 1, 30)),
        TransitionKind::Fold
    );
    assert_eq!(offset(&source, "Test/Fixed", 1_720_000_000), hours(9));
    let info = source.info("Test/Fixed").unwrap().unwrap();
    assert_eq!(info.primary_name, "Test/Fixed");
    assert!(info.tags.contains(&Tag::Fixed) || !info.tags.is_empty());
    assert_eq!(source.info("Europe/London").unwrap(), None);
}

#[test]
fn test_a_malformed_tzif_fails_the_build_and_skips_later_steps() {
    let error = ZoneSource::builder()
        .tzif("Test/Broken", b"not a TZif file")
        .tzif("Test/Other", b"also not one")
        .build()
        .unwrap_err();
    assert!(!error.to_string().is_empty());
}

#[test]
fn test_structured_zone_with_a_rule() {
    let source = ZoneSource::builder()
        .zone("Test/Eastern", &rule_zone())
        .build()
        .unwrap();
    assert_eq!(
        kind(&source, "Test/Eastern", datetime(2030, 3, 10, 2, 30)),
        TransitionKind::Gap
    );
    assert_eq!(
        kind(&source, "Test/Eastern", datetime(1900, 11, 4, 1, 30)),
        TransitionKind::Fold
    );
    assert_eq!(
        kind(&source, "Test/Eastern", datetime(1900, 7, 4, 12, 0)),
        TransitionKind::Unique
    );
    let summer = source
        .offset_at("Test/Eastern", Instant::new(1_720_000_000, 0))
        .unwrap();
    assert_eq!(summer.offset, hours(-4));
    assert!(summer.is_dst);
    assert_eq!(summer.abbreviation, "EDT");
}

#[test]
fn test_structured_zone_rule_days() {
    let julian = |day| RuleDate {
        day: RuleDay::Julian(day),
        time: 0,
    };
    let zero_based = |day| RuleDate {
        day: RuleDay::ZeroBased(day),
        time: 0,
    };
    let zone = |start: RuleDate, end: RuleDate| ZoneData {
        initial: local(0, false, "STD"),
        transitions: vec![],
        after: ZoneAfter::Rule(ZoneRule {
            standard: local(0, false, "STD"),
            dst: Some(DstRule {
                local_type: local(1, true, "DST"),
                start,
                end,
            }),
        }),
    };
    let source = ZoneSource::builder()
        .zone("Test/Julian", &zone(julian(60), julian(300)))
        .zone("Test/ZeroBased", &zone(zero_based(59), zero_based(299)))
        .build()
        .unwrap();
    assert_eq!(
        kind(&source, "Test/Julian", datetime(2024, 3, 1, 0, 30)),
        TransitionKind::Gap
    );
    assert_eq!(
        kind(&source, "Test/ZeroBased", datetime(2024, 2, 29, 0, 30)),
        TransitionKind::Gap
    );
}

#[test]
fn test_structured_zone_with_transitions_held_or_inferred() {
    let transitions: Vec<ZoneTransition> = [
        (1_710_054_000, -4, true, "EDT"),
        (1_730_613_600, -5, false, "EST"),
        (1_741_503_600, -4, true, "EDT"),
        (1_762_063_200, -5, false, "EST"),
    ]
    .into_iter()
    .map(|(instant, hours, is_dst, abbreviation)| ZoneTransition {
        instant,
        local_type: local(hours, is_dst, abbreviation),
    })
    .collect();
    let zone = |after| ZoneData {
        initial: local(-5, false, "EST"),
        transitions: transitions.clone(),
        after,
    };
    let source = ZoneSource::builder()
        .zone("Test/Held", &zone(ZoneAfter::Hold))
        .zone("Test/Inferred", &zone(ZoneAfter::Infer { last_year: 2025 }))
        .build()
        .unwrap();
    let summer_2030 = 1_909_000_000;
    assert_eq!(offset(&source, "Test/Held", summer_2030), hours(-5));
    assert_eq!(offset(&source, "Test/Inferred", summer_2030), hours(-4));
    assert_eq!(
        kind(&source, "Test/Inferred", datetime(2030, 3, 10, 2, 30)),
        TransitionKind::Gap
    );
}

#[test]
fn test_an_invalid_structured_zone_fails_the_build() {
    let error = ZoneSource::builder()
        .zone(
            "Test/Wide",
            &ZoneData {
                initial: local(72, false, "FAR"),
                transitions: vec![],
                after: ZoneAfter::Hold,
            },
        )
        .build()
        .unwrap_err();
    assert!(!error.to_string().is_empty());
}

/// The offset New York's current rule gives at a Unix second.
fn eastern_at(second: i64) -> LocalTimeType {
    let source = ZoneSource::builder()
        .zone("Test/Eastern", &rule_zone())
        .build()
        .unwrap();
    let o = source
        .offset_at("Test/Eastern", Instant::new(second, 0))
        .unwrap();
    LocalTimeType {
        offset: match o.offset {
            UtcOffset::Known(seconds) => seconds,
            UtcOffset::Unknown => unreachable!(),
        },
        is_dst: o.is_dst,
        abbreviation: o.abbreviation,
    }
}

#[test]
fn test_sampled_oracle_held_after_its_range() {
    let rule = ZoneSource::builder()
        .zone("Test/Eastern", &rule_zone())
        .build()
        .unwrap();
    let source = ZoneSource::builder()
        .oracle(
            "Test/Oracle",
            ZoneOracle::new(2020, 2025, AfterProbe::Hold, |second| {
                let o = rule
                    .offset_at("Test/Eastern", Instant::new(second, 0))
                    .unwrap();
                let UtcOffset::Known(offset) = o.offset else {
                    return Err(Error::from("unknown offset"));
                };
                Ok(LocalTimeType {
                    offset,
                    is_dst: o.is_dst,
                    abbreviation: o.abbreviation,
                })
            }),
        )
        .build()
        .unwrap();
    assert_eq!(
        kind(&source, "Test/Oracle", datetime(2024, 3, 10, 2, 30)),
        TransitionKind::Gap
    );
    assert_eq!(offset(&source, "Test/Oracle", 1_909_000_000), hours(-5));
}

#[test]
fn test_oracle_with_next_transition_inferred_after_its_range() {
    let transitions = [1_710_054_000, 1_730_613_600, 1_741_503_600, 1_762_063_200];
    let source = ZoneSource::builder()
        .oracle(
            "Test/Oracle",
            ZoneOracle::new(2023, 2025, AfterProbe::Infer, eastern_at_ok)
                .next_transition(move |after| Ok(transitions.into_iter().find(|&t| t > after))),
        )
        .build()
        .unwrap();
    assert_eq!(offset(&source, "Test/Oracle", 1_909_000_000), hours(-4));
}

fn eastern_at_ok(second: i64) -> Result<LocalTimeType, Error> {
    Ok(eastern_at(second))
}

#[test]
fn test_a_failing_oracle_fails_the_build() {
    let error = ZoneSource::builder()
        .oracle(
            "Test/Oracle",
            ZoneOracle::new(2020, 2021, AfterProbe::Hold, |_| {
                Err(Error::from("the oracle is closed".to_string()))
            }),
        )
        .build()
        .unwrap_err();
    assert!(
        error.to_string().contains("the oracle is closed"),
        "{error}"
    );
}

#[test]
fn test_a_panicking_oracle_fails_the_build() {
    let error = ZoneSource::builder()
        .oracle(
            "Test/Oracle",
            ZoneOracle::new(2020, 2021, AfterProbe::Hold, |_| Ok(local(0, false, "UTC")))
                .next_transition(|_| panic!("no transitions today")),
        )
        .build()
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("zone oracle panicked: no transitions today"),
        "{error}"
    );
}

#[test]
fn test_links() {
    let source = ZoneSource::builder()
        .zone("Test/Eastern", &rule_zone())
        .link("Test/Alias", "Test/Eastern")
        .build()
        .unwrap();
    assert_eq!(
        kind(&source, "Test/Alias", datetime(2030, 3, 10, 2, 30)),
        TransitionKind::Gap
    );
    let info = source.info("Test/Alias").unwrap().unwrap();
    assert!(info.is_link);
    assert_eq!(info.primary_name, "Test/Eastern");
    let error = ZoneSource::builder()
        .link("Test/Loop", "Test/Loop")
        .build()
        .unwrap_err();
    assert!(!error.to_string().is_empty());
}

fn loading_source(loads: Arc<AtomicUsize>) -> ZoneSource {
    ZoneSource::builder()
        .declare([
            "America/New_York",
            "Europe/London",
            "Test/Failing",
            "Test/Panicking",
        ])
        .loader(move |source, name| {
            loads.fetch_add(1, Ordering::SeqCst);
            match name {
                "Test/Failing" => Err(Error::from("no data for you")),
                "Test/Panicking" => panic!("the loader fell over"),
                "Test/Unknown" => Ok(()),
                name => source.tzif(name, tzif(name)),
            }
        })
        .version("test-1")
        .build()
        .unwrap()
}

#[test]
fn test_loader_loads_a_declared_name_once_when_first_needed() {
    let loads = Arc::new(AtomicUsize::new(0));
    let source = loading_source(Arc::clone(&loads));
    assert_eq!(source.version(), "test-1");
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    assert_eq!(
        kind(&source, "America/New_York", datetime(2024, 3, 10, 2, 30)),
        TransitionKind::Gap
    );
    assert_eq!(
        offset(&source, "America/New_York", 1_720_000_000),
        hours(-4)
    );
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(source.info("Test/Unknown").unwrap(), None);
}

#[test]
fn test_loader_failures_are_reported() {
    let source = loading_source(Arc::new(AtomicUsize::new(0)));
    let civil = datetime(2024, 1, 1, 0, 0);
    let error = source.resolve("Test/Failing", civil).unwrap_err();
    assert!(error.to_string().contains("no data for you"), "{error}");
    let error = source
        .offset_at("Test/Panicking", Instant::new(0, 0))
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("zone loader for \"Test/Panicking\" panicked: the loader fell over"),
        "{error}"
    );
}

#[test]
fn test_loader_failure_fails_a_draw_as_a_usage_error() {
    let source = loading_source(Arc::new(AtomicUsize::new(0)));
    expect_panic(
        move || {
            hegel::Hegel::new(move |tc| {
                tc.draw(
                    gs::temporal::zoned_datetimes().zones(
                        gs::temporal::zones()
                            .source(source.clone())
                            .names(["Test/Failing"]),
                    ),
                );
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "no data for you",
    );
}

#[test]
fn test_zones_draw_the_names_of_their_source() {
    let source = loading_source(Arc::new(AtomicUsize::new(0)));
    let names = ["America/New_York", "Europe/London"];
    assert_all_examples(
        gs::temporal::zones().source(source.clone()).names(names),
        move |zone| names.contains(&zone.name.as_str()),
    );
    let lazy = ZoneSource::builder()
        .declare(names)
        .loader(|source, name| source.tzif(name, tzif(name)))
        .build()
        .unwrap();
    assert_all_examples(gs::temporal::zones().source(lazy.clone()), move |zone| {
        names.contains(&zone.name.as_str())
    });
    find_any(gs::temporal::zones().source(lazy), |zone| {
        zone.name == "Europe/London"
    });
}

#[test]
fn test_zones_of_a_source_whose_loader_fails_at_spec_build_are_a_usage_error() {
    let source = loading_source(Arc::new(AtomicUsize::new(0)));
    expect_panic(
        move || {
            hegel::Hegel::new(move |tc| {
                tc.draw(gs::temporal::zones().source(source.clone()));
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "no data for you|the loader fell over",
    );
}

#[test]
fn test_zoned_datetimes_classify_against_their_zones_source() {
    let source = ZoneSource::builder()
        .zone(
            "America/New_York",
            &ZoneData {
                initial: local(-5, false, "EST"),
                transitions: vec![],
                after: ZoneAfter::Hold,
            },
        )
        .build()
        .unwrap();
    assert_all_examples(
        gs::temporal::zoned_datetimes()
            .zones(gs::temporal::zones().source(source))
            .min_value(datetime(2024, 3, 10, 0, 0))
            .max_value(datetime(2024, 11, 3, 23, 59)),
        |z| {
            z.kind == TransitionKind::Unique
                && z.offset == hours(-5)
                && z.zone.name == "America/New_York"
        },
    );
}

#[test]
fn test_zone_sources_are_shared_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ZoneSource>();
    let source = loading_source(Arc::new(AtomicUsize::new(0)));
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let source = source.clone();
            std::thread::spawn(move || offset(&source, "Europe/London", 1_720_000_000))
        })
        .collect();
    for thread in threads {
        assert_eq!(thread.join().unwrap(), hours(1));
    }
}

#[test]
fn test_zone_source_debug() {
    assert!(format!("{:?}", ZoneSource::bundled()).contains("ZoneSource"));
}

#[test]
fn test_zone_sources_compare_by_identity() {
    let source = loading_source(Arc::new(AtomicUsize::new(0)));
    assert_eq!(source, source.clone());
    assert_ne!(source, loading_source(Arc::new(AtomicUsize::new(0))));
    assert_eq!(ZoneSource::bundled(), ZoneSource::bundled());
    assert_ne!(source, ZoneSource::bundled());
}

#[test]
fn test_zone_names_a_zone_of_the_source() {
    let source = ZoneSource::builder()
        .tzif("Test/Tokyo", tzif("Asia/Tokyo"))
        .link("Test/Alias", "Test/Tokyo")
        .build()
        .unwrap();
    let zone = source.zone("Test/Alias").unwrap();
    assert_eq!(zone.name, "Test/Alias");
    assert_eq!(zone.primary_name, "Test/Tokyo");
    let error = source.zone("Europe/London").unwrap_err();
    assert_eq!(error.to_string(), "unknown time zone \"Europe/London\"");
    let failing = loading_source(Arc::new(AtomicUsize::new(0)));
    let error = failing.zone("Test/Failing").unwrap_err();
    assert!(error.to_string().contains("no data for you"), "{error}");
}

/// The zone data a loader reads, counting when it is dropped.
struct Tzifs {
    drops: Arc<AtomicUsize>,
}

impl Tzifs {
    fn get(&self, name: &str) -> &'static [u8] {
        tzif(name)
    }
}

impl Drop for Tzifs {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn test_a_loader_is_dropped_with_the_last_user_of_its_source() {
    let drops = Arc::new(AtomicUsize::new(0));
    let tzifs = Tzifs {
        drops: Arc::clone(&drops),
    };
    let source = ZoneSource::builder()
        .declare(["Europe/London"])
        .loader(move |source, name| source.tzif(name, tzifs.get(name)))
        .build()
        .unwrap();
    assert_eq!(offset(&source, "Europe/London", 1_720_000_000), hours(1));
    let zones = gs::temporal::zones().source(source.clone());
    let zoned = gs::temporal::zoned_datetimes().source(source.clone());
    assert_all_examples(gs::temporal::zones().source(source.clone()), |zone| {
        zone.name == "Europe/London"
    });
    drop(source);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(zones);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(zoned);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn test_a_replaced_loader_is_dropped() {
    let drops = Arc::new(AtomicUsize::new(0));
    let tzifs = Tzifs {
        drops: Arc::clone(&drops),
    };
    let builder = ZoneSource::builder()
        .declare(["Europe/London"])
        .loader(move |source, name| source.tzif(name, tzifs.get(name)));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let source = builder
        .loader(|source, name| source.tzif(name, tzif("Asia/Tokyo")))
        .build()
        .unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(offset(&source, "Europe/London", 1_720_000_000), hours(9));
}

/// A source whose New York never changes its clocks, with a zone the
/// bundled database lacks.
fn fixed_source() -> ZoneSource {
    let fixed = |offset_hours| ZoneData {
        initial: local(offset_hours, false, "FIX"),
        transitions: vec![],
        after: ZoneAfter::Hold,
    };
    ZoneSource::builder()
        .zone("America/New_York", &fixed(-5))
        .zone("Test/Fixed", &fixed(9))
        .build()
        .unwrap()
}

fn is_a_fixed_source_value(z: &hegel::generators::temporal::ZonedDateTime) -> bool {
    z.kind == TransitionKind::Unique
        && match z.zone.name.as_str() {
            "America/New_York" => z.offset == hours(-5),
            "Test/Fixed" => z.offset == hours(9),
            _ => false,
        }
}

fn spring_forward_day() -> gs::temporal::ZonedDateTimeGenerator {
    gs::temporal::zoned_datetimes()
        .min_value(datetime(2024, 3, 10, 0, 0))
        .max_value(datetime(2024, 3, 10, 23, 59))
}

#[test]
fn test_zoned_datetimes_source_draws_the_zones_of_the_source() {
    let source = fixed_source();
    assert_all_examples(
        spring_forward_day().source(source.clone()),
        is_a_fixed_source_value,
    );
    for name in ["America/New_York", "Test/Fixed"] {
        find_any(spring_forward_day().source(source.clone()), move |z| {
            z.zone.name == name
        });
    }
}

#[test]
fn test_zoned_datetimes_source_resolves_what_another_zones_generator_draws() {
    let source = fixed_source();
    let zones = || gs::temporal::zones().source(source.clone());
    let wrapped = [
        zones().boxed(),
        zones().map(|zone| zone).boxed(),
        zones().filter(|zone| zone.name != "UTC").boxed(),
        gs::one_of(vec![zones().boxed(), zones().names(["Test/Fixed"]).boxed()]).boxed(),
        gs::sampled_from(vec![
            source.zone("America/New_York").unwrap(),
            source.zone("Test/Fixed").unwrap(),
        ])
        .boxed(),
    ];
    for zones in wrapped {
        assert_all_examples(
            spring_forward_day().zones(zones).source(source.clone()),
            is_a_fixed_source_value,
        );
    }
}

#[test]
fn test_zoned_datetimes_without_a_source_resolve_another_generators_zones_in_the_default() {
    let new_york = fixed_source().zone("America/New_York").unwrap();
    find_any(spring_forward_day().zones(gs::just(new_york)), |z| {
        z.kind == TransitionKind::Gap
    });
}

/// The wall times New York's clocks skipped in March 2024, with skipped
/// wall times not allowed: a draw New York has no value for.
fn in_new_yorks_gap() -> gs::temporal::ZonedDateTimeGenerator {
    gs::temporal::zoned_datetimes()
        .min_value(datetime(2024, 3, 10, 2, 10))
        .max_value(datetime(2024, 3, 10, 2, 50))
        .allow_imaginary(false)
}

/// Run a test drawing from `generator`, returning how many test cases
/// began the draw and how many got a value from it.
fn draws_and_values(generator: gs::temporal::ZonedDateTimeGenerator) -> (usize, usize) {
    let draws = Arc::new(AtomicUsize::new(0));
    let values = Arc::new(AtomicUsize::new(0));
    let (d, v) = (Arc::clone(&draws), Arc::clone(&values));
    hegel::Hegel::new(move |tc: TestCase| {
        d.fetch_add(1, Ordering::SeqCst);
        let z = tc.draw(&generator);
        assert_ne!(z.zone.name, "America/New_York");
        v.fetch_add(1, Ordering::SeqCst);
    })
    .settings(
        hegel::Settings::new()
            .test_cases(100)
            .database(None)
            .suppress_health_check(hegel::HealthCheck::all()),
    )
    .run();
    (draws.load(Ordering::SeqCst), values.load(Ordering::SeqCst))
}

#[test]
fn test_zoned_datetimes_in_a_zone_with_no_value_in_the_bounds_reject_the_test_case() {
    let zone = |name| hegel::generators::temporal::TimeZone::new(name).unwrap();
    let (draws, values) = draws_and_values(in_new_yorks_gap().zones(gs::sampled_from(vec![
        zone("America/New_York"),
        zone("UTC"),
    ])));
    assert!(values > 0 && values < draws, "{values} of {draws}");
}

#[test]
fn test_zoned_datetimes_reject_a_zone_with_no_value_when_the_unknown_zone_has_one() {
    let (draws, values) = draws_and_values(
        in_new_yorks_gap().zones(gs::temporal::zones().names(["America/New_York", UNKNOWN_ZONE])),
    );
    assert!(values > 0 && values < draws, "{values} of {draws}");
}
