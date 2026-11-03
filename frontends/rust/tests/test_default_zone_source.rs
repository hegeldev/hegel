mod common;

use common::utils::assert_all_examples;
use hegel::generators as gs;
use hegel::generators::temporal::{
    Date, DateTime, Error, Instant, LocalTimeType, Time, TransitionKind, UtcOffset, ZoneAfter,
    ZoneData, ZoneSource, info, offset_at, resolve, set_default_zone_source,
};

fn datetime(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> DateTime {
    DateTime::new(
        Date::new(year, month, day).unwrap(),
        Time::new(hour, minute, 0, 0).unwrap(),
    )
    .unwrap()
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
    assert_eq!(
        resolve("America/New_York", gap).unwrap().kind,
        TransitionKind::Gap
    );

    let source = ZoneSource::builder()
        .zone("America/New_York", &fixed(-5))
        .declare(["Europe/Paris"])
        .loader(|_, name| Err(Error::from(format!("cannot load {name}"))))
        .build()
        .unwrap();
    set_default_zone_source(&source);

    assert_eq!(
        resolve("America/New_York", gap).unwrap().kind,
        TransitionKind::Unique
    );
    assert_eq!(
        offset_at("America/New_York", Instant::new(1_720_000_000, 0).unwrap())
            .unwrap()
            .offset,
        UtcOffset::Known(-5 * 3600)
    );
    assert!(info("America/New_York").is_some());
    assert_eq!(info("Europe/London"), None);
    assert!(
        resolve("Europe/Paris", gap)
            .unwrap_err()
            .to_string()
            .contains("cannot load Europe/Paris")
    );
    assert_all_examples(gs::temporal::zones().names(["America/New_York"]), |zone| {
        zone.name == "America/New_York"
    });
    assert_all_examples(
        gs::temporal::zoned_datetimes()
            .zones(gs::temporal::zones().names(["America/New_York"]))
            .min_value(datetime(2024, 3, 10, 0, 0))
            .max_value(datetime(2024, 3, 10, 23, 0)),
        |z| z.kind == TransitionKind::Unique,
    );

    let failing = ZoneSource::builder()
        .declare(["Test/Broken"])
        .loader(|_, name| Err(Error::from(format!("cannot load {name}"))))
        .build()
        .unwrap();
    set_default_zone_source(&failing);
    assert_eq!(info("Test/Broken"), None);

    set_default_zone_source(&ZoneSource::bundled());
    assert_eq!(
        resolve("America/New_York", gap).unwrap().kind,
        TransitionKind::Gap
    );
    assert!(info("Europe/London").is_some());
}
