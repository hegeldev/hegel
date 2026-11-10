//! Tests for `src/native/draws/format.rs`: each value's text is the one
//! `libhegel/docs/datetime.md` §6.5 gives, agrees with jiff's where jiff
//! has the same form, is total over fields outside their documented
//! ranges, and for spans is injective.

use super::*;
use crate::native::draws::scalar::SpanUnit;
use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::vec::Vec;

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8, ns: u32) -> DateTime {
    DateTime {
        date: Date { year, month, day },
        time: Time {
            hour,
            minute,
            second,
            nanosecond: ns,
        },
    }
}

fn known(seconds: i32) -> UtcOffset {
    UtcOffset {
        seconds,
        known: true,
    }
}

fn span_of(units: &[(SpanUnit, i64)]) -> Span {
    let mut s = Span::default();
    for &(unit, value) in units {
        s.units[unit as usize] = value;
    }
    s
}

#[test]
fn dates_use_four_digits_or_a_signed_expanded_year() {
    for (year, month, day, text) in [
        (2024, 3, 10, "2024-03-10"),
        (0, 1, 1, "0000-01-01"),
        (9999, 12, 31, "9999-12-31"),
        (10_000, 1, 1, "+010000-01-01"),
        (-1, 12, 31, "-000001-12-31"),
        (999_999, 12, 31, "+999999-12-31"),
        (-999_999, 1, 1, "-999999-01-01"),
        (-271_821, 4, 20, "-271821-04-20"),
        (2024, 13, 40, "2024-13-40"),
        (i32::MAX, 0, 255, "+2147483647-00-255"),
        (i32::MIN, 1, 1, "-2147483648-01-01"),
    ] {
        assert_eq!(date(&Date { year, month, day }), text);
    }
}

#[test]
fn times_drop_the_trailing_zeros_of_the_fraction() {
    for (hour, minute, second, nanosecond, text) in [
        (2, 30, 0, 0, "02:30:00"),
        (2, 30, 0, 500_000_000, "02:30:00.5"),
        (23, 59, 59, 999_999_999, "23:59:59.999999999"),
        (0, 0, 0, 1, "00:00:00.000000001"),
        (0, 0, 0, 1_000, "00:00:00.000001"),
        (0, 0, 0, 120_000_000, "00:00:00.12"),
        (24, 60, 61, 0, "24:60:61"),
        (255, 255, 255, u32::MAX, "255:255:255.4294967295"),
    ] {
        let t = Time {
            hour,
            minute,
            second,
            nanosecond,
        };
        assert_eq!(time(&t), text);
    }
    assert_eq!(
        datetime(&civil(2024, 3, 10, 2, 30, 0, 250_000_000)),
        "2024-03-10T02:30:00.25"
    );
    assert_eq!(
        datetime(&civil(-44, 3, 15, 12, 0, 0, 0)),
        "-000044-03-15T12:00:00"
    );
}

#[test]
fn timestamps_are_their_utc_datetime() {
    for (second, nanosecond, text) in [
        (0, 0, "1970-01-01T00:00:00Z"),
        (946_684_800, 0, "2000-01-01T00:00:00Z"),
        (-2, 500_000_000, "1969-12-31T23:59:58.5Z"),
        (-1, 999_999_999, "1969-12-31T23:59:59.999999999Z"),
        (-86_400, 0, "1969-12-31T00:00:00Z"),
        (-86_401, 1, "1969-12-30T23:59:59.000000001Z"),
        (253_402_300_800, 0, "+010000-01-01T00:00:00Z"),
        (-62_167_219_201, 0, "-000001-12-31T23:59:59Z"),
        (i64::MAX, 0, "+292277026596-12-04T15:30:07Z"),
        (i64::MIN, 0, "-292277022657-01-27T08:29:52Z"),
    ] {
        assert_eq!(timestamp(&Timestamp { second, nanosecond }), text);
    }
}

#[test]
fn timestamps_agree_with_jiff() {
    let seconds = [
        -377_705_023_201,
        -62_135_596_800,
        -2_208_988_800,
        -86_401,
        -1,
        0,
        1,
        951_782_400,
        1_710_054_000,
        2_147_483_648,
        253_402_207_200,
    ];
    for second in seconds {
        for nanosecond in [0, 1, 500_000_000, 999_999_999, 123_000] {
            let ours = timestamp(&Timestamp { second, nanosecond });
            let theirs = jiff::Timestamp::new(second, nanosecond as i32).unwrap();
            assert_eq!(ours, theirs.to_string(), "{second} {nanosecond}");
            assert_eq!(ours.parse::<jiff::Timestamp>().unwrap(), theirs, "{ours}");
        }
    }
}

#[test]
fn offsets_print_seconds_only_when_they_are_not_zero() {
    for (seconds, text) in [
        (0, "+00:00"),
        (19_800, "+05:30"),
        (-18_000, "-05:00"),
        (-2_670, "-00:44:30"),
        (1, "+00:00:01"),
        (-59, "-00:00:59"),
        (93_599, "+25:59:59"),
        (-93_599, "-25:59:59"),
        (i32::MAX, "+596523:14:07"),
        (i32::MIN, "-596523:14:08"),
    ] {
        assert_eq!(offset(&known(seconds)), text);
    }
    let unknown = UtcOffset {
        seconds: 3_600,
        known: false,
    };
    assert_eq!(offset(&unknown), "-00:00");
}

#[test]
fn durations_are_iso_8601_in_hours_minutes_and_seconds() {
    for (seconds, nanoseconds, text) in [
        (0, 0, "PT0S"),
        (5_405, 500_000_000, "PT1H30M5.5S"),
        (0, -500_000_000, "-PT0.5S"),
        (0, 1, "PT0.000000001S"),
        (86_400, 0, "PT24H"),
        (3_600, 0, "PT1H"),
        (60, 0, "PT1M"),
        (3_601, 0, "PT1H1S"),
        (-90, 0, "-PT1M30S"),
        (-1, -999_999_999, "-PT1.999999999S"),
        (1, -500_000_000, "PT0.5S"),
        (-1, 500_000_000, "-PT0.5S"),
        (60, -1, "PT59.999999999S"),
        (i64::MAX, 999_999_999, "PT2562047788015215H30M7.999999999S"),
        (
            i64::MIN,
            -999_999_999,
            "-PT2562047788015215H30M8.999999999S",
        ),
    ] {
        let d = Duration {
            seconds,
            nanoseconds,
        };
        assert_eq!(duration(&d), text);
    }
}

#[test]
fn durations_agree_with_jiff() {
    for seconds in [
        -90_061_i64,
        -3_600,
        -61,
        -1,
        0,
        1,
        59,
        60,
        3_599,
        86_399,
        90_061,
    ] {
        for nanoseconds in [0_i32, 1, 500_000_000, 999_999_999] {
            let nanoseconds = if seconds < 0 {
                -nanoseconds
            } else {
                nanoseconds
            };
            let ours = duration(&Duration {
                seconds,
                nanoseconds,
            });
            let theirs = jiff::SignedDuration::new(seconds, nanoseconds);
            assert_eq!(
                ours.parse::<jiff::SignedDuration>().unwrap(),
                theirs,
                "{ours}"
            );
        }
    }
}

#[test]
fn spans_list_their_units_with_the_signs_that_tell_them_apart() {
    use SpanUnit::*;
    for (units, text) in [
        (&[][..], "0s"),
        (&[(Years, 1), (Months, 2), (Days, 3)][..], "1y 2mo 3d"),
        (&[(Days, -3), (Hours, -4)][..], "-3d 4h"),
        (&[(Days, -3), (Hours, 4)][..], "-3d +4h"),
        (&[(Years, 1), (Months, -2)][..], "+1y -2mo"),
        (&[(Days, -3)][..], "-3d"),
        (&[(Seconds, 0), (Nanoseconds, 7)][..], "7ns"),
        (
            &[
                (Years, 1),
                (Months, 2),
                (Weeks, 3),
                (Days, 4),
                (Hours, 5),
                (Minutes, 6),
                (Seconds, 7),
                (Milliseconds, 8),
                (Microseconds, 9),
                (Nanoseconds, 10),
            ][..],
            "1y 2mo 3w 4d 5h 6m 7s 8ms 9µs 10ns",
        ),
        (
            &[(Years, i64::MIN), (Nanoseconds, i64::MIN)][..],
            "-9223372036854775808y 9223372036854775808ns",
        ),
        (
            &[(Years, i64::MIN), (Nanoseconds, i64::MAX)][..],
            "-9223372036854775808y +9223372036854775807ns",
        ),
    ] {
        assert_eq!(span(&span_of(units)), text);
    }
}

#[test]
fn span_text_is_injective() {
    let values = [-12, -1, 0, 1, 12];
    let mut seen: BTreeMap<String, Span> = BTreeMap::new();
    let units = [
        SpanUnit::Months,
        SpanUnit::Days,
        SpanUnit::Minutes,
        SpanUnit::Milliseconds,
    ];
    let mut all: Vec<Span> = Vec::new();
    for a in values {
        for b in values {
            for c in values {
                for d in values {
                    all.push(span_of(&[
                        (units[0], a),
                        (units[1], b),
                        (units[2], c),
                        (units[3], d),
                    ]));
                }
            }
        }
    }
    assert_eq!(all.len(), 625);
    for s in all {
        let text = span(&s);
        if let Some(other) = seen.insert(text.clone(), s) {
            panic!("{text} is both {other:?} and {s:?}");
        }
    }
}

#[test]
fn zoned_datetimes_are_rfc_9557_with_the_gap_or_fold_marked() {
    let at = civil(2024, 3, 10, 1, 30, 0, 0);
    for (civil, utc_offset, zone, kind, fold, text) in [
        (
            at,
            known(-18_000),
            "America/New_York",
            Kind::Unique,
            false,
            "2024-03-10T01:30:00-05:00[America/New_York]",
        ),
        (
            civil(2024, 3, 10, 2, 30, 0, 0),
            known(-14_400),
            "America/New_York",
            Kind::Gap,
            false,
            "2024-03-10T02:30:00-04:00[America/New_York][gap]",
        ),
        (
            civil(2024, 11, 3, 1, 30, 0, 0),
            known(-14_400),
            "America/New_York",
            Kind::Fold,
            false,
            "2024-11-03T01:30:00-04:00[America/New_York][fold=0]",
        ),
        (
            civil(2024, 11, 3, 1, 30, 0, 500_000_000),
            known(-18_000),
            "America/New_York",
            Kind::Fold,
            true,
            "2024-11-03T01:30:00.5-05:00[America/New_York][fold=1]",
        ),
        (
            civil(2000, 1, 1, 0, 0, 0, 0),
            UtcOffset {
                seconds: 0,
                known: false,
            },
            UNKNOWN_NAME,
            Kind::Unique,
            false,
            "2000-01-01T00:00:00Z[Etc/Unknown]",
        ),
        (
            civil(2000, 1, 1, 0, 0, 0, 0),
            UtcOffset {
                seconds: 0,
                known: false,
            },
            "Mars/Base",
            Kind::Unique,
            false,
            "2000-01-01T00:00:00-00:00[Mars/Base]",
        ),
        (
            civil(1900, 1, 1, 0, 0, 0, 0),
            known(-2_670),
            "Africa/Monrovia",
            Kind::Unique,
            true,
            "1900-01-01T00:00:00-00:44:30[Africa/Monrovia]",
        ),
        (
            civil(2024, 1, 1, 0, 0, 0, 0),
            known(0),
            "UTC",
            Kind::Unique,
            false,
            "2024-01-01T00:00:00+00:00[UTC]",
        ),
    ] {
        assert_eq!(zoned_datetime(&civil, &utc_offset, zone, kind, fold), text);
    }
}

#[test]
fn unique_zoned_datetimes_parse_as_the_same_instant_in_jiff() {
    for (name, civil, seconds) in [
        ("America/New_York", civil(2024, 3, 10, 1, 30, 0, 0), -18_000),
        (
            "Asia/Kolkata",
            civil(2024, 6, 1, 12, 0, 0, 250_000_000),
            19_800,
        ),
        ("Europe/London", civil(2024, 7, 1, 0, 0, 0, 0), 3_600),
    ] {
        let text = zoned_datetime(&civil, &known(seconds), name, Kind::Unique, false);
        let theirs: jiff::Zoned = text.parse().unwrap();
        assert_eq!(theirs.to_string(), text);
    }
}
