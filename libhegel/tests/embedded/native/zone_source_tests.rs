//! Tests for `src/native/zone_source.rs`: each input shape reproduces the
//! bundled data, uploads are checked and confined to the civil envelope,
//! oracles are probed and continued, names load lazily, and the
//! catalogue describes what it knows while the classifier describes the
//! rest.

use super::*;
use crate::native::draws::special::{Date, DateTime, Time};
use crate::native::timezone::{transitions, type_at};
use alloc::vec;
use std::sync::atomic::{AtomicUsize, Ordering};

fn real(name: &str) -> &'static [u8] {
    jiff_tzdb::get(name).unwrap().1
}

fn bundled(name: &str) -> &'static Zone {
    tzdata::lookup(name).unwrap().unwrap().zone
}

fn data_of(source: &Arc<ZoneSource>, name: &str) -> &'static Zone {
    source.timezone(name).unwrap().unwrap().data().unwrap()
}

fn zone_type(utoff: i32, is_dst: bool, abbreviation: &str) -> ZoneType {
    ZoneType {
        utoff,
        is_dst,
        abbreviation: abbreviation.to_string(),
    }
}

fn type_of(zone: &Zone, at: i64) -> ZoneType {
    let kind = type_at(zone, at).unwrap();
    zone_type(
        kind.utoff,
        kind.is_dst,
        &zone.abbreviations[kind.abbreviation],
    )
}

fn message(e: EngineError) -> String {
    match e {
        EngineError::InvalidArgument(m) => m,
        other => panic!("{other:?}"),
    }
}

fn year(y: i64) -> i64 {
    year_start(y)
}

fn offsets_agree(a: &Zone, b: &Zone, from: i64, to: i64) -> bool {
    let instants = |z: &Zone| -> Vec<i64> { transitions(z, from, to).map(|t| t.instant).collect() };
    instants(a) == instants(b)
        && (from..to)
            .step_by(7 * DAY as usize + 3_601)
            .all(|t| type_of(a, t) == type_of(b, t))
}

type FooterShape = (ZoneType, Option<(ZoneType, RuleDate, RuleDate)>);

fn footer_of(zone: &Zone) -> Option<FooterShape> {
    let named = |kind: LocalTimeType| {
        zone_type(
            kind.utoff,
            kind.is_dst,
            &zone.abbreviations[kind.abbreviation],
        )
    };
    zone.footer.map(|footer| {
        (
            named(footer.standard),
            footer.dst.map(|dst| (named(dst.kind), dst.start, dst.end)),
        )
    })
}

fn clocks_agree(a: &Zone, b: &Zone) -> bool {
    let wall = |z: &Zone, t: i64| {
        let kind = type_at(z, t).unwrap();
        (kind.utoff, z.abbreviations[kind.abbreviation].clone())
    };
    let explicit = a
        .transitions
        .iter()
        .chain(&b.transitions)
        .map(|t| t.instant);
    let sampled = (year(1800)..year(2200)).step_by(7 * DAY as usize + 3_601);
    explicit
        .chain(sampled)
        .all(|t| wall(a, t) == wall(b, t) && wall(a, t - 1) == wall(b, t - 1))
}

/// jiff-tzdb compiles the 2026e release in the rearguard format, which
/// gives the zones with negative daylight saving time positive daylight
/// saving time instead: their clocks agree with the bundled data, and
/// only their flags differ.
#[test]
fn tzif_data_reproduces_every_bundled_zone() {
    let source = Arc::new(ZoneSource::new());
    let mut differing = Vec::new();
    for name in tzdata::names().unwrap() {
        if name.is_link() {
            continue;
        }
        source.add_tzif(&name.name, real(&name.name)).unwrap();
        let parsed = data_of(&source, &name.name);
        if parsed != name.zone {
            assert!(clocks_agree(parsed, name.zone), "{}", name.name);
            differing.push(name.name.as_str());
        }
    }
    assert_eq!(
        differing,
        [
            "Africa/Casablanca",
            "Africa/El_Aaiun",
            "Africa/Windhoek",
            "Europe/Dublin",
            "Europe/Prague"
        ]
    );
}

#[test]
fn malformed_tzif_data_is_an_argument_error() {
    let source = ZoneSource::new();
    let e = message(source.add_tzif("Test/Zone", b"TZif").unwrap_err());
    assert!(e.contains("zone Test/Zone: malformed TZif data"), "{e}");
}

fn london_rule() -> FooterRule {
    tzif::parse_footer("GMT0BST,M3.5.0/1,M10.5.0").unwrap()
}

#[test]
fn structured_data_with_a_rule_matches_the_bundled_zone() {
    let london = bundled("Europe/London");
    let source = Arc::new(ZoneSource::new());
    let data = ZoneData {
        initial: type_of(london, i64::MIN / 2),
        transitions: london
            .transitions
            .iter()
            .map(|t| (t.instant, type_of(london, t.instant)))
            .collect(),
        after: After::Rule(london_rule()),
    };
    source.add_zone("Europe/London", &data).unwrap();
    assert_eq!(data_of(&source, "Europe/London"), london);
}

#[test]
fn structured_data_is_checked() {
    let source = ZoneSource::new();
    let fixed = |transitions: Vec<(i64, ZoneType)>| ZoneData {
        initial: zone_type(0, false, "A"),
        transitions,
        after: After::Hold,
    };
    let bad = |name: &str, data: ZoneData| message(source.add_zone(name, &data).unwrap_err());
    assert!(
        bad(
            "Test/Zone",
            fixed(vec![(0, zone_type(200_000, false, "B"))])
        )
        .contains("offset")
    );
    let unordered = vec![(5, zone_type(1, false, "B")), (5, zone_type(2, false, "C"))];
    assert!(bad("Test/Zone", fixed(unordered)).contains("strictly ascending"));
    let mut rule = london_rule();
    rule.dst.as_mut().unwrap().1.time = 200 * 3_600;
    let ruled = ZoneData {
        after: After::Rule(rule),
        ..fixed(vec![])
    };
    assert!(bad("Test/Zone", ruled).contains("out of range"));
    for day in [RuleDay::Julian(0), RuleDay::ZeroBased(366)] {
        let mut rule = london_rule();
        rule.dst.as_mut().unwrap().2.day = day;
        let ruled = ZoneData {
            after: After::Rule(rule),
            ..fixed(vec![])
        };
        assert!(bad("Test/Zone", ruled).contains("out of range"));
    }
    for name in ["", UNKNOWN_NAME, "A\0B"] {
        assert!(bad(name, fixed(vec![])).contains("cannot name a zone"));
    }
    let nul = ZoneData {
        initial: zone_type(0, false, "A\0"),
        ..fixed(vec![])
    };
    assert!(bad("Test/Zone", nul).contains("NUL"));
    for last_year in [MIN_YEAR - 1, MAX_YEAR + 1] {
        let infer = ZoneData {
            after: After::Infer { last_year },
            ..fixed(vec![])
        };
        assert!(bad("Test/Zone", infer).contains("last year"));
    }
}

#[test]
fn a_rule_whose_daylight_type_is_its_standard_type_is_the_standard_type_alone() {
    let source = Arc::new(ZoneSource::new());
    let ruled = |dst: ZoneType| {
        let mut rule = london_rule();
        rule.dst.as_mut().unwrap().0 = dst;
        ZoneData {
            initial: zone_type(0, false, "GMT"),
            transitions: vec![],
            after: After::Rule(rule),
        }
    };
    source
        .add_zone("Test/Same", &ruled(zone_type(0, false, "GMT")))
        .unwrap();
    let zone = data_of(&source, "Test/Same");
    let footer = zone.footer.as_ref().unwrap();
    assert!(footer.dst.is_none());
    assert_eq!(transitions(zone, year(1990), year(2100)).count(), 0);
    for at in [year(1990), year(2024) + 200 * DAY, year(2199)] {
        assert_eq!(type_of(zone, at), zone_type(0, false, "GMT"));
    }
    let zone = source.timezone("Test/Same").unwrap().unwrap();
    let july = DateTime {
        date: Date {
            year: 2024,
            month: 7,
            day: 1,
        },
        time: Time {
            hour: 1,
            minute: 30,
            second: 0,
            nanosecond: 0,
        },
    };
    assert_eq!(zone.resolve(&july, false).unwrap().offset, 0);
    for (name, dst) in [
        ("Test/Abbreviation", zone_type(0, false, "IST")),
        ("Test/Flag", zone_type(0, true, "GMT")),
        ("Test/Offset", zone_type(3_600, false, "GMT")),
    ] {
        source.add_zone(name, &ruled(dst.clone())).unwrap();
        let zone = data_of(&source, name);
        assert!(zone.footer.as_ref().unwrap().dst.is_some(), "{name}");
        assert_eq!(type_of(zone, year(2024) + 200 * DAY), dst, "{name}");
    }
}

#[test]
fn a_rule_must_continue_the_last_transition() {
    let source = Arc::new(ZoneSource::new());
    let data = |last: (i64, ZoneType)| ZoneData {
        initial: zone_type(0, false, "LMT"),
        transitions: vec![(year(1990), zone_type(0, false, "GMT")), last],
        after: After::Rule(london_rule()),
    };
    let winter = year(2000) + 10 * DAY;
    let e = message(
        source
            .add_zone(
                "Test/Zone",
                &data((winter, zone_type(-3_600, false, "WAT"))),
            )
            .unwrap_err(),
    );
    assert_eq!(
        e,
        format!(
            "zone Test/Zone: the rule after the last transition does not continue it: the \
             transition at {winter} changes to WAT (UT-3600s), and the rule has GMT (UT+0s) in \
             force then"
        )
    );
    let e = message(
        source
            .add_zone("Test/Zone", &data((winter, zone_type(3_600, true, "BST"))))
            .unwrap_err(),
    );
    assert!(
        e.contains("changes to BST (UT+3600s), and the rule has GMT"),
        "{e}"
    );
    let summer = year(2000) + 200 * DAY;
    source
        .add_zone("Test/Zone", &data((summer, zone_type(3_600, true, "BST"))))
        .unwrap();
    let zone = data_of(&source, "Test/Zone");
    assert_eq!(type_of(zone, summer), zone_type(3_600, true, "BST"));
    assert_eq!(
        type_of(zone, year(2001) + 10 * DAY),
        zone_type(0, false, "GMT")
    );
    let held = ZoneData {
        after: After::Hold,
        ..data((winter, zone_type(-3_600, false, "WAT")))
    };
    source.add_zone("Test/Held", &held).unwrap();
}

#[test]
fn the_bundled_source_cannot_be_changed() {
    let source = ZoneSource::bundled();
    assert!(source.is_bundled());
    let data = ZoneData {
        initial: zone_type(0, false, "A"),
        transitions: vec![],
        after: After::Hold,
    };
    let errors = [
        source.add_zone("Test/Zone", &data).unwrap_err(),
        source.add_tzif("Test/Zone", b"").unwrap_err(),
        source.add_link("Test/Zone", "UTC").unwrap_err(),
        source.declare(["Test/Zone"]).unwrap_err(),
        source.set_loader(None).unwrap_err(),
        source.set_version("x").unwrap_err(),
        source
            .add_oracle(
                "Test/Zone",
                Oracle {
                    offset_at: &mut |_| Ok(zone_type(0, false, "A")),
                    next_transition: None,
                },
                ProbeRange {
                    first_year: 2000,
                    last_year: 2001,
                    infer: false,
                },
            )
            .unwrap_err(),
    ];
    for e in errors {
        assert!(message(e).contains("bundled zone source cannot be changed"));
    }
    assert_eq!(format!("{source:?}"), "ZoneSource(bundled)");
    assert_eq!(source.version(), tzdata::VERSION_CSTR);
}

#[test]
fn data_beyond_the_envelope_is_confined_to_it() {
    let source = Arc::new(ZoneSource::new());
    let data = ZoneData {
        initial: zone_type(0, false, "A"),
        transitions: vec![
            (MIN_INSTANT - 10, zone_type(60, false, "B")),
            (0, zone_type(120, false, "C")),
            (MAX_INSTANT + 10, zone_type(180, false, "D")),
        ],
        after: After::Rule(london_rule()),
    };
    source.add_zone("Test/Edges", &data).unwrap();
    let zone = data_of(&source, "Test/Edges");
    assert_eq!(zone.initial.utoff, 60);
    assert_eq!(zone.transitions.len(), 1);
    assert_eq!(zone.transitions[0].kind.utoff, 120);
    assert!(zone.footer.is_none());
    assert_eq!(zone.abbreviations, vec!["B".to_string(), "C".to_string()]);
}

#[test]
fn identical_data_is_stored_once() {
    let a = Arc::new(ZoneSource::new());
    let b = Arc::new(ZoneSource::new());
    a.add_tzif("Asia/Tokyo", real("Asia/Tokyo")).unwrap();
    b.add_tzif("Asia/Tokyo", real("Asia/Tokyo")).unwrap();
    assert!(core::ptr::eq(
        data_of(&a, "Asia/Tokyo"),
        data_of(&b, "Asia/Tokyo")
    ));
}

fn oracle_range(first_year: i32, last_year: i32, infer: bool) -> ProbeRange {
    ProbeRange {
        first_year,
        last_year,
        infer,
    }
}

fn probe_bundled(name: &str, range: ProbeRange, with_next: bool) -> &'static Zone {
    let zone = bundled(name);
    let source = Arc::new(ZoneSource::new());
    let mut offset_at = |t: i64| Ok(type_of(zone, t));
    let mut next = |t: i64| {
        Ok(transitions(zone, t + 1, MAX_INSTANT + 1)
            .next()
            .map(|t| t.instant))
    };
    let oracle = Oracle {
        offset_at: &mut offset_at,
        next_transition: if with_next { Some(&mut next) } else { None },
    };
    source.add_oracle(name, oracle, range).unwrap();
    data_of(&source, name)
}

#[test]
fn a_probed_oracle_holds_or_infers_beyond_its_range() {
    let new_york = bundled("America/New_York");
    for with_next in [false, true] {
        let held = probe_bundled(
            "America/New_York",
            oracle_range(2000, 2010, false),
            with_next,
        );
        assert!(offsets_agree(held, new_york, year(2000), year(2011)));
        assert!(held.footer.is_none());
        assert_eq!(type_of(held, year(2030) + 180 * DAY).utoff, -18_000);
        assert_eq!(type_of(held, year(1990) + 180 * DAY).utoff, -18_000);
        let inferred = probe_bundled(
            "America/New_York",
            oracle_range(2000, 2010, true),
            with_next,
        );
        assert!(offsets_agree(inferred, new_york, year(2000), year(2100)));
        assert_eq!(footer_of(inferred), footer_of(new_york));
    }
}

#[test]
fn inference_reproduces_the_rules_of_the_bundled_zones() {
    let mut checked = 0;
    for name in tzdata::names().unwrap() {
        let zone = name.zone;
        let settled = zone
            .transitions
            .last()
            .is_none_or(|t| t.instant < year(2029));
        if name.is_link() || !settled {
            continue;
        }
        let probed = probe_bundled(&name.name, oracle_range(1990, 2030, true), true);
        assert_eq!(footer_of(probed), footer_of(zone), "{}", name.name);
        assert!(
            offsets_agree(probed, zone, year(1990), year(2200)),
            "{}",
            name.name
        );
        checked += 1;
    }
    assert!(checked > 300, "{checked}");
}

#[test]
fn a_fixed_offset_continues_without_a_rule() {
    let tokyo = probe_bundled("Asia/Tokyo", oracle_range(2000, 2010, true), false);
    assert_eq!(footer_of(tokyo), footer_of(bundled("Asia/Tokyo")));
}

#[test]
fn irregular_years_are_held() {
    let source = Arc::new(ZoneSource::new());
    let a = zone_type(0, false, "A");
    let b = zone_type(3_600, true, "B");
    let c = zone_type(7_200, true, "C");
    let at = |y: i64, day: i64| year(y) + day * DAY;
    let cases = [
        vec![(at(2020, 90), b.clone()), (at(2020, 300), a.clone())],
        vec![
            (at(2019, 90), b.clone()),
            (at(2019, 300), a.clone()),
            (at(2020, 90), b.clone()),
            (at(2020, 300), a.clone()),
            (at(2020, 310), b.clone()),
        ],
        vec![
            (at(2019, 90), b.clone()),
            (at(2019, 300), a.clone()),
            (at(2020, 90), c.clone()),
            (at(2020, 300), a.clone()),
        ],
        vec![
            (at(2019, 90), b.clone()),
            (at(2019, 300), a.clone()),
            (at(2020, 95), b.clone()),
            (at(2020, 290), a.clone()),
        ],
    ];
    for transitions in cases {
        let data = ZoneData {
            initial: a.clone(),
            transitions,
            after: After::Infer { last_year: 2020 },
        };
        source.add_zone("Test/Irregular", &data).unwrap();
        assert_eq!(data_of(&source, "Test/Irregular").footer, None);
    }
}

#[test]
fn equal_offsets_take_standard_time_from_the_flag_or_the_smaller_offset() {
    let source = Arc::new(ZoneSource::new());
    let at = |y: i64, day: i64| year(y) + day * DAY;
    let alternating = |x: ZoneType, y: ZoneType| {
        (2019..=2020)
            .flat_map(|yr| [(at(yr, 90), y.clone()), (at(yr, 300), x.clone())])
            .collect::<Vec<_>>()
    };
    let cases = [
        (
            zone_type(3_600, false, "A"),
            zone_type(0, false, "B"),
            Some(0),
        ),
        (zone_type(0, true, "A"), zone_type(0, false, "B"), Some(0)),
        (zone_type(0, false, "A"), zone_type(0, false, "B"), None),
    ];
    for (x, y, standard) in cases {
        let data = ZoneData {
            initial: x.clone(),
            transitions: alternating(x, y),
            after: After::Infer { last_year: 2020 },
        };
        source.add_zone("Test/Pair", &data).unwrap();
        let footer = data_of(&source, "Test/Pair").footer;
        assert_eq!(footer.map(|f| f.standard.utoff), standard);
    }
}

#[test]
fn oracle_failures_are_errors() {
    let source = ZoneSource::new();
    let mut fixed = |_: i64| Ok(zone_type(0, false, "A"));
    for range in [
        oracle_range(2001, 2000, false),
        oracle_range(2000, 2000, true),
        oracle_range(MIN_YEAR - 1, 2000, false),
        oracle_range(2000, MAX_YEAR + 1, false),
    ] {
        let oracle = Oracle {
            offset_at: &mut fixed,
            next_transition: None,
        };
        let e = message(source.add_oracle("Test/Zone", oracle, range).unwrap_err());
        assert!(e.contains("probe range"), "{e}");
    }
    let mut failing = |_: i64| Err(EngineError::InvalidArgument("oracle says no".to_string()));
    let oracle = Oracle {
        offset_at: &mut failing,
        next_transition: None,
    };
    let e = source.add_oracle("Test/Zone", oracle, oracle_range(2000, 2001, false));
    assert_eq!(message(e.unwrap_err()), "oracle says no");
    let mut stuck = |t: i64| Ok(Some(t));
    let oracle = Oracle {
        offset_at: &mut fixed,
        next_transition: Some(&mut stuck),
    };
    let e = message(
        source
            .add_oracle("Test/Zone", oracle, oracle_range(2000, 2001, false))
            .unwrap_err(),
    );
    assert!(e.contains("not later"), "{e}");
    let mut flipping = |t: i64| Ok(zone_type(i32::from(t % 2 == 0), false, "A"));
    let mut every_second = |t: i64| Ok(Some(t + 1));
    let oracle = Oracle {
        offset_at: &mut flipping,
        next_transition: Some(&mut every_second),
    };
    let e = message(
        source
            .add_oracle("Test/Zone", oracle, oracle_range(2000, 2001, false))
            .unwrap_err(),
    );
    assert!(e.contains("more than"), "{e}");
    let mut steps = 0usize;
    let mut counted = |t: i64| {
        steps += 1;
        Ok(Some(t + 1))
    };
    let oracle = Oracle {
        offset_at: &mut fixed,
        next_transition: Some(&mut counted),
    };
    let e = message(
        source
            .add_oracle("Test/Zone", oracle, oracle_range(2000, 2001, false))
            .unwrap_err(),
    );
    assert_eq!(
        e,
        format!("zone Test/Zone: probing found more than {MAX_PROBED_TRANSITIONS} transitions")
    );
    assert_eq!(steps, MAX_PROBED_TRANSITIONS + 1);
    let mut changing = |t: i64| Ok(zone_type((t % 1_000) as i32, false, "A"));
    let oracle = Oracle {
        offset_at: &mut changing,
        next_transition: None,
    };
    let e = message(
        source
            .add_oracle("Test/Zone", oracle, oracle_range(2000, 2001, false))
            .unwrap_err(),
    );
    assert_eq!(
        e,
        format!("zone Test/Zone: probing found more than {MAX_PROBED_TRANSITIONS} transitions")
    );
}

#[test]
fn sampling_finds_each_change_to_the_second() {
    let source = Arc::new(ZoneSource::default());
    for change in [year(2000) + 40 * DAY + 12_345, year(2000) + PROBE_STEP] {
        let mut offset_at = |t: i64| Ok(zone_type(if t < change { 0 } else { 60 }, false, "A"));
        let oracle = Oracle {
            offset_at: &mut offset_at,
            next_transition: None,
        };
        source
            .add_oracle("Test/Once", oracle, oracle_range(2000, 2000, false))
            .unwrap();
        let zone = data_of(&source, "Test/Once");
        assert_eq!(zone.transitions.len(), 1);
        assert_eq!(zone.transitions[0].instant, change);
    }
}

#[test]
fn versions_are_recorded_and_interned() {
    let source = ZoneSource::new();
    assert_eq!(source.version(), c"");
    source.set_version("2099z").unwrap();
    assert_eq!(source.version(), c"2099z");
    assert_eq!(
        message(source.set_version("a\0b").unwrap_err()),
        "version \"a\\0b\" contains a NUL byte"
    );
    assert!(core::ptr::eq(
        intern_c_string("version", "2099z").unwrap(),
        source.version()
    ));
}

struct Counting {
    calls: AtomicUsize,
}

fn loading_source(names: &[&str]) -> (Arc<ZoneSource>, Arc<Counting>) {
    let source = Arc::new(ZoneSource::new());
    source.declare(names.iter().copied()).unwrap();
    let counting = Arc::new(Counting {
        calls: AtomicUsize::new(0),
    });
    let counter = Arc::clone(&counting);
    let loader = move |source: &Arc<ZoneSource>, name: &str| {
        counter.calls.fetch_add(1, Ordering::SeqCst);
        match jiff_tzdb::get(name) {
            Some((_, data)) => source.add_tzif(name, data),
            None => Ok(()),
        }
    };
    source.set_loader(Some(Arc::new(loader))).unwrap();
    (source, counting)
}

#[test]
fn a_declared_name_loads_once_when_looked_up() {
    let (source, counting) = loading_source(&["Europe/London", "Asia/Tokyo"]);
    assert_eq!(counting.calls.load(Ordering::SeqCst), 0);
    assert_eq!(data_of(&source, "Europe/London"), bundled("Europe/London"));
    assert_eq!(data_of(&source, "Europe/London"), bundled("Europe/London"));
    assert_eq!(counting.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        source.names().unwrap(),
        vec!["Asia/Tokyo".to_string(), "Europe/London".to_string()]
    );
}

#[test]
fn an_undeclared_name_is_asked_of_the_loader_once() {
    let (source, counting) = loading_source(&[]);
    assert_eq!(data_of(&source, "Asia/Tokyo"), bundled("Asia/Tokyo"));
    assert!(source.timezone("Mars/Base").unwrap().is_none());
    assert!(source.timezone("Mars/Base").unwrap().is_none());
    assert_eq!(counting.calls.load(Ordering::SeqCst), 2);
    assert_eq!(source.names().unwrap(), vec!["Asia/Tokyo".to_string()]);
    source.declare(["Mars/Base"]).unwrap();
    let e = message(source.timezone("Mars/Base").unwrap_err());
    assert!(e.contains("supplied no data"), "{e}");
    assert_eq!(counting.calls.load(Ordering::SeqCst), 3);
}

#[test]
fn loader_failures_are_errors() {
    let source = Arc::new(ZoneSource::new());
    source.declare(["Test/Failing", "Test/Empty"]).unwrap();
    let loader = |_: &Arc<ZoneSource>, name: &str| {
        if name == "Test/Failing" {
            Err(EngineError::InvalidArgument("cannot load".to_string()))
        } else {
            Ok(())
        }
    };
    source.set_loader(Some(Arc::new(loader))).unwrap();
    assert_eq!(
        message(source.lookup("Test/Failing").unwrap_err()),
        "cannot load"
    );
    let e = message(source.lookup("Test/Empty").unwrap_err());
    assert!(e.contains("supplied no data"), "{e}");
}

#[test]
fn a_declared_name_without_a_loader_is_unloaded() {
    let source = Arc::new(ZoneSource::new());
    source.declare(["Europe/London"]).unwrap();
    assert!(matches!(
        source.lookup("Europe/London").unwrap(),
        Lookup::Unloaded
    ));
    let e = message(source.timezone("Europe/London").unwrap_err());
    assert!(e.contains("no data for \"Europe/London\""), "{e}");
    source
        .add_tzif("Europe/London", real("Europe/London"))
        .unwrap();
    source.declare(["Europe/London"]).unwrap();
    assert_eq!(data_of(&source, "Europe/London"), bundled("Europe/London"));
}

#[test]
fn links_are_followed() {
    let source = Arc::new(ZoneSource::new());
    source
        .add_tzif("Asia/Kolkata", real("Asia/Kolkata"))
        .unwrap();
    source.add_link("Asia/Calcutta", "Asia/Kolkata").unwrap();
    source.add_link("Test/Chain", "Asia/Calcutta").unwrap();
    assert_eq!(data_of(&source, "Test/Chain"), bundled("Asia/Kolkata"));
    assert!(message(source.add_link("Test/Self", "Test/Self").unwrap_err()).contains("itself"));
    assert!(message(source.add_link("Test/X", "").unwrap_err()).contains("cannot name"));
    source.add_link("Test/Dangling", "Test/Nowhere").unwrap();
    let e = message(source.lookup("Test/Dangling").unwrap_err());
    assert!(e.contains("does not know"), "{e}");
    source.add_link("Test/A", "Test/B").unwrap();
    source.add_link("Test/B", "Test/A").unwrap();
    let e = message(source.lookup("Test/A").unwrap_err());
    assert!(e.contains("more than 16 links"), "{e}");
    assert!(matches!(
        source.lookup(UNKNOWN_NAME).unwrap(),
        Lookup::Zone(TimeZone::Unknown)
    ));
    assert!(matches!(source.lookup("Nowhere").unwrap(), Lookup::Unknown));
    assert!(format!("{source:?}").contains("names"));
}

fn link_name(depth: usize) -> String {
    format!("Test/Link{depth}")
}

#[test]
fn a_name_may_pass_through_sixteen_links() {
    let source = Arc::new(ZoneSource::new());
    source
        .add_tzif(&link_name(0), real("Asia/Kolkata"))
        .unwrap();
    for depth in 1..=MAX_LINK_DEPTH + 1 {
        source
            .add_link(&link_name(depth), &link_name(depth - 1))
            .unwrap();
    }
    assert_eq!(MAX_LINK_DEPTH, 16);
    let through_links = data_of(&source, &link_name(16));
    assert_eq!(through_links.name, link_name(0));
    assert_eq!(
        through_links.transitions,
        bundled("Asia/Kolkata").transitions
    );
    let e = message(source.lookup(&link_name(17)).unwrap_err());
    assert_eq!(
        e,
        "the zone source's name \"Test/Link17\" passes through more than 16 links"
    );
}

#[test]
fn links_a_loader_supplies_have_the_same_budget() {
    let source = Arc::new(ZoneSource::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let loader = move |source: &Arc<ZoneSource>, name: &str| {
        counter.fetch_add(1, Ordering::SeqCst);
        let depth: usize = name.strip_prefix("Test/Link").unwrap().parse().unwrap();
        if depth == 0 {
            source.add_tzif(name, real("Asia/Kolkata"))
        } else {
            source.add_link(name, &link_name(depth - 1))
        }
    };
    source.set_loader(Some(Arc::new(loader))).unwrap();
    let e = message(source.lookup(&link_name(17)).unwrap_err());
    assert!(e.contains("passes through more than 16 links"), "{e}");
    assert_eq!(calls.load(Ordering::SeqCst), 17);
    assert_eq!(data_of(&source, &link_name(16)).name, link_name(0));
    assert_eq!(calls.load(Ordering::SeqCst), 18);
}

#[test]
fn a_link_to_a_declared_name_loads_its_target() {
    let (source, counting) = loading_source(&["Asia/Kolkata"]);
    source.add_link("Asia/Calcutta", "Asia/Kolkata").unwrap();
    assert_eq!(data_of(&source, "Asia/Calcutta"), bundled("Asia/Kolkata"));
    assert_eq!(counting.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn the_catalogue_describes_the_names_it_has() {
    let source = Arc::new(ZoneSource::new());
    source
        .add_tzif("Asia/Kolkata", real("Europe/London"))
        .unwrap();
    source.add_link("Asia/Calcutta", "Asia/Kolkata").unwrap();
    let catalogue = tzdata::lookup("Asia/Calcutta").unwrap().unwrap();
    let described = source.describe("Asia/Calcutta").unwrap().unwrap();
    assert_eq!(described.tags, catalogue.tags);
    assert_eq!(described.rank().unwrap(), catalogue.rank);
    assert!(described.is_link);
    assert_eq!(described.primary, "Asia/Kolkata");
    assert!(source.describe("Europe/London").unwrap().is_none());
    let bundled_source = ZoneSource::bundled();
    assert_eq!(
        bundled_source
            .describe("Asia/Calcutta")
            .unwrap()
            .unwrap()
            .tags,
        catalogue.tags
    );
    assert!(bundled_source.describe("Mars/Base").unwrap().is_none());
}

#[test]
fn the_classifier_describes_the_names_the_catalogue_lacks() {
    let source = Arc::new(ZoneSource::new());
    source
        .add_tzif("Europe/Atlantis", real("Europe/London"))
        .unwrap();
    source.add_link("Etc/GMT+77", "Europe/Atlantis").unwrap();
    source.declare(["Mars/Base"]).unwrap();
    let atlantis = source.describe("Europe/Atlantis").unwrap().unwrap();
    assert_eq!(atlantis.tags, classify::data_tags(bundled("Europe/London")));
    assert!(!atlantis.is_link);
    assert_eq!(atlantis.catalogued.map(|n| n.rank), None);
    assert_eq!(atlantis.rank().unwrap(), uncatalogued_rank().unwrap());
    assert_eq!(
        usize::from(uncatalogued_rank().unwrap()),
        tzdata::names().unwrap().len()
    );
    let link = source.describe("Etc/GMT+77").unwrap().unwrap();
    assert!(link.is_link);
    assert_eq!(link.primary, "Europe/Atlantis");
    assert_eq!(
        link.tags,
        classify::data_tags(bundled("Europe/London"))
            .with(Tag::Link)
            .with(Tag::InvertedSign)
    );
    let mars = source.describe("Mars/Base").unwrap().unwrap();
    assert!(mars.tags.is_empty());
    assert_eq!(mars.primary, "Mars/Base");
    source.add_link("Mars/Alias", "Mars/Base").unwrap();
    let alias = source.describe("Mars/Alias").unwrap().unwrap();
    assert_eq!(alias.primary, "Mars/Base");
    assert!(alias.is_link);
    assert_eq!(
        ZoneSource::bundled().names().unwrap().len(),
        tzdata::names().unwrap().len()
    );
}
