use super::*;

fn drawn(
    zone: &str,
    second: i64,
    civil: NaiveDateTime,
    kind: hd::TransitionKind,
) -> hd::ZonedDateTime {
    hd::ZonedDateTime {
        zone: hd::TimeZone {
            name: zone.to_string(),
            primary_name: zone.to_string(),
        },
        civil: hegel_datetime(civil),
        kind,
        offset_before: hd::UtcOffset::Known(0),
        offset_after: hd::UtcOffset::Known(0),
        fold: false,
        offset: hd::UtcOffset::Known(0),
        instant: hd::Instant::new(second, 0),
    }
}

fn london_afternoon() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2024, 6, 1)
        .unwrap()
        .and_hms_opt(13, 0, 0)
        .unwrap()
}

#[test]
fn a_draw_chrono_tz_reads_back_as_drawn_is_kept() {
    let civil = london_afternoon();
    let second = civil.and_utc().timestamp() - 3600;
    let dt = chrono_tz_datetime(&drawn(
        "Europe/London",
        second,
        civil,
        hd::TransitionKind::Unique,
    ))
    .unwrap();
    assert_eq!(dt.timezone(), chrono_tz::Tz::Europe__London);
    assert_eq!(dt.naive_local(), civil);
}

#[test]
fn a_draw_chrono_tz_reads_back_as_another_wall_time_is_dropped() {
    let civil = london_afternoon();
    let second = civil.and_utc().timestamp();
    for kind in [hd::TransitionKind::Unique, hd::TransitionKind::Fold] {
        assert!(chrono_tz_datetime(&drawn("Europe/London", second, civil, kind)).is_none());
    }
}

#[test]
fn a_skipped_wall_time_is_kept_as_its_instant() {
    let civil = london_afternoon();
    let second = civil.and_utc().timestamp();
    let dt = chrono_tz_datetime(&drawn(
        "Europe/London",
        second,
        civil,
        hd::TransitionKind::Gap,
    ))
    .unwrap();
    assert_eq!(dt.timestamp(), second);
}

#[test]
fn an_instant_chrono_cannot_hold_is_dropped() {
    let civil = london_afternoon();
    let unique = hd::TransitionKind::Unique;
    assert!(chrono_tz_datetime(&drawn("UTC", i64::MAX, civil, unique)).is_none());
    let last = DateTime::<Utc>::MAX_UTC.timestamp();
    assert!(chrono_tz_datetime(&drawn("Asia/Tokyo", last, civil, unique)).is_none());
}
