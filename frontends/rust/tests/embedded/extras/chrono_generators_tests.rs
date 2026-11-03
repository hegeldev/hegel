use super::*;
use chrono_tz::Tz;
use hd::{Kind, ZonedDateTime};

fn new_york(
    kind: Kind,
    fold: bool,
    wall: (i32, u32, u32, u32, u32),
    instant: i64,
) -> ZonedDateTime {
    let (year, month, day, hour, minute) = wall;
    let civil = NaiveDate::from_ymd_opt(year, month, day)
        .unwrap()
        .and_hms_opt(hour, minute, 0)
        .unwrap();
    ZonedDateTime {
        zone: hd::TimeZone {
            name: "America/New_York".to_string(),
            primary_name: "America/New_York".to_string(),
        },
        civil: hegel_datetime(civil),
        kind,
        offset_before: -5 * 3600,
        offset_after: -4 * 3600,
        fold,
        offset: hd::UtcOffset::Known(-5 * 3600),
        timestamp: hd::Timestamp {
            second: instant,
            nanosecond: 0,
        },
    }
}

fn value(z: &ZonedDateTime) -> Option<DateTime<Tz>> {
    let instant = DateTime::<Utc>::from_timestamp(z.timestamp.second, 0).unwrap();
    chrono_tz_datetime(z, Tz::America__New_York, instant)
}

#[test]
fn chrono_tz_datetime_keeps_a_unique_wall_time() {
    let z = new_york(Kind::Unique, false, (2024, 1, 1, 12, 0), 1_704_128_400);
    let dt = value(&z).unwrap();
    assert_eq!(dt.naive_local(), naive_datetime(z.civil));
    assert_eq!(dt.timestamp(), 1_704_128_400);
}

#[test]
fn chrono_tz_datetime_picks_the_repeat_the_fold_names() {
    let earlier = new_york(Kind::Fold, false, (2024, 11, 3, 1, 30), 1_730_611_800);
    let later = new_york(Kind::Fold, true, (2024, 11, 3, 1, 30), 1_730_615_400);
    assert_eq!(value(&earlier).unwrap().timestamp(), 1_730_611_800);
    assert_eq!(value(&later).unwrap().timestamp(), 1_730_615_400);
}

#[test]
fn chrono_tz_datetime_shifts_a_gap_both_databases_skip_to_its_instant() {
    let z = new_york(Kind::Gap, false, (2024, 3, 10, 2, 30), 1_710_055_800);
    let dt = value(&z).unwrap();
    assert_eq!(dt.timestamp(), 1_710_055_800);
    assert_eq!(
        dt.naive_local(),
        NaiveDate::from_ymd_opt(2024, 3, 10)
            .unwrap()
            .and_hms_opt(3, 30, 0)
            .unwrap()
    );
}

#[test]
fn chrono_tz_datetime_has_no_value_for_a_wall_time_only_chrono_tz_skips() {
    let z = new_york(Kind::Unique, false, (2024, 3, 10, 2, 30), 1_710_055_800);
    assert_eq!(value(&z), None);
    let z = new_york(Kind::Fold, true, (2024, 3, 10, 2, 30), 1_710_055_800);
    assert_eq!(value(&z), None);
}
