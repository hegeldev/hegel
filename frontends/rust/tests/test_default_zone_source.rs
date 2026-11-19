mod common;

use common::utils::{assert_all_examples, find_any};
use hegel::generators as gs;
use hegel::generators::temporal::{
    Date, DateTime, Error, Instant, LocalTimeType, Time, TimeZone, TransitionKind, UtcOffset,
    ZoneAfter, ZoneData, ZoneSource, default_zone_source, info, offset_at, resolve,
    set_default_zone_source,
};

fn datetime(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> DateTime {
    DateTime::new(Date::new(year, month, day), Time::new(hour, minute, 0, 0))
}

fn fixed(hours: i32) -> ZoneData {
    ZoneData {
        initial: LocalTimeType {
            offset: hours * 3600,
            is_dst: false,
            abbreviation: "FIX".to_string(),
        },
        transitions: vec![],
        after: ZoneAfter::Hold,
    }
}

/// The default source is process-wide, so everything that changes it
/// runs in this one test, alone in its binary.
#[test]
fn test_the_default_zone_source_is_what_queries_and_zones_use() {
    let gap = datetime(2024, 3, 10, 2, 30);
    let new_york_day = || {
        gs::temporal::zoned_datetimes()
            .zones(gs::temporal::zones().names(["America/New_York"]))
            .min_value(datetime(2024, 3, 10, 0, 0))
            .max_value(datetime(2024, 3, 10, 23, 0))
    };
    assert_eq!(default_zone_source(), ZoneSource::bundled());
    assert_eq!(
        resolve("America/New_York", gap).unwrap().kind,
        TransitionKind::Gap
    );
    let built_on_bundled = new_york_day();
    let any_zone_on_bundled = gs::temporal::zoned_datetimes();
    let named_on_bundled =
        gs::temporal::zoned_datetimes().zones(gs::just(TimeZone::new("Europe/London").unwrap()));

    let source = ZoneSource::builder()
        .zone("America/New_York", &fixed(-5))
        .declare(["Europe/Paris"])
        .loader(|_, name| Err(Error::from(format!("cannot load {name}"))))
        .build()
        .unwrap();
    let previous = set_default_zone_source(source.clone());
    assert_eq!(previous, ZoneSource::bundled());
    assert_eq!(default_zone_source(), source);
    assert_ne!(default_zone_source(), previous);

    assert_eq!(
        resolve("America/New_York", gap).unwrap().kind,
        TransitionKind::Unique
    );
    assert_eq!(
        offset_at("America/New_York", Instant::new(1_720_000_000, 0))
            .unwrap()
            .offset,
        UtcOffset::Known(-5 * 3600)
    );
    assert!(info("America/New_York").unwrap().is_some());
    assert!(TimeZone::new("Europe/London").is_err());
    assert!(
        resolve("Europe/Paris", gap)
            .unwrap_err()
            .to_string()
            .contains("cannot load Europe/Paris")
    );
    assert!(
        info("Europe/London")
            .unwrap_err()
            .to_string()
            .contains("cannot load Europe/London")
    );
    assert_all_examples(gs::temporal::zones().names(["America/New_York"]), |zone| {
        zone.name == "America/New_York"
    });
    assert_all_examples(new_york_day(), |z| z.kind == TransitionKind::Unique);

    find_any(built_on_bundled, |z| z.kind == TransitionKind::Gap);
    find_any(any_zone_on_bundled, |z| z.zone.name == "Asia/Calcutta");
    assert_all_examples(named_on_bundled, |z| z.zone.name == "Europe/London");

    let built_on_source = new_york_day();
    let restored = set_default_zone_source(previous);
    assert_eq!(restored, source);
    assert_eq!(default_zone_source(), ZoneSource::bundled());
    assert_eq!(
        resolve("America/New_York", gap).unwrap().kind,
        TransitionKind::Gap
    );
    assert!(info("Europe/London").unwrap().is_some());
    assert_all_examples(built_on_source, |z| z.kind == TransitionKind::Unique);
}
