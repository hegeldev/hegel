//! The canonical text of the temporal values (`libhegel/docs/datetime.md`
//! §6.5): what `hegel_format_*` returns, and so what every frontend's own
//! temporal value types print as.
//!
//! Every function is total: a field outside the range its type documents
//! is printed as it stands rather than refused, since the text is for a
//! failure report, where a wrong value must still be shown.
//!
//! - A date is `YYYY-MM-DD`, the year four digits from 0 to 9999 and
//!   otherwise a sign and at least six digits (ISO 8601's expanded year):
//!   `2024-03-10`, `+010000-01-01`, `-000001-12-31`.
//! - A time is `HH:MM:SS`, followed when the nanosecond is not zero by `.`
//!   and its nine digits without trailing zeros: `02:30:00`, `02:30:00.5`.
//! - A datetime is the date, `T` and the time.
//! - A timestamp is its UTC datetime and `Z`: `1969-12-31T23:59:58.5Z`
//!   for `{-2, 500000000}`.
//! - An offset is `+HH:MM` or `-HH:MM`, followed by `:SS` when the seconds
//!   are not zero, and `-00:00` when unknown: `+05:30`, `-00:44:30`.
//! - A duration is ISO 8601's `PT` form in hours, minutes and seconds,
//!   each present when not zero, the seconds carrying the fraction, with a
//!   leading `-` when negative and `PT0S` for zero: `PT1H30M5.5S`,
//!   `-PT0.5S`.
//! - A span is its non-zero units, largest first, each the count and the
//!   unit's suffix (`y mo w d h m s ms µs ns`), separated by spaces, and
//!   `0s` for zero. Signs make the text injective: with no negative unit
//!   there is none (`1y 2mo`); with no positive unit there is one leading
//!   `-` (`-3d 4h` is -3 days and -4 hours); with both, every unit carries
//!   its own `+` or `-` (`-3d +4h`).
//! - A zoned datetime is RFC 9557's form: the datetime, the offset (`Z`
//!   in the zone `Etc/Unknown`), the zone name in brackets and, for a
//!   skipped wall time, `[gap]`, or for a repeated one `[fold=0]` (the
//!   earlier instant) or `[fold=1]`:
//!   `2024-11-03T01:30:00-05:00[America/New_York][fold=1]`.

use alloc::format;
use alloc::string::String;

use super::scalar::{Duration, Span, UtcOffset};
use super::special::{Date, DateTime, Time, civil_of_days};
use crate::native::timezone::{Kind, Timestamp, UNKNOWN_NAME};

const NS: i128 = 1_000_000_000;
const DAY: i64 = 86_400;

/// The suffix of each span unit, in `SpanUnit` order.
const SPAN_SUFFIXES: [&str; 10] = ["y", "mo", "w", "d", "h", "m", "s", "ms", "µs", "ns"];

fn civil_date(year: i64, month: u8, day: u8) -> String {
    if (0..=9999).contains(&year) {
        format!("{year:04}-{month:02}-{day:02}")
    } else {
        format!("{year:+07}-{month:02}-{day:02}")
    }
}

/// `.` and the nine digits of `nanosecond` without trailing zeros, or
/// nothing when it is zero.
fn fraction(nanosecond: u32) -> String {
    if nanosecond == 0 {
        return String::new();
    }
    let digits = format!("{nanosecond:09}");
    format!(".{}", digits.trim_end_matches('0'))
}

pub fn date(d: &Date) -> String {
    civil_date(i64::from(d.year), d.month, d.day)
}

pub fn time(t: &Time) -> String {
    let (hour, minute, second) = (t.hour, t.minute, t.second);
    format!(
        "{hour:02}:{minute:02}:{second:02}{}",
        fraction(t.nanosecond)
    )
}

pub fn datetime(dt: &DateTime) -> String {
    format!("{}T{}", date(&dt.date), time(&dt.time))
}

pub fn timestamp(ts: &Timestamp) -> String {
    let (year, month, day) = civil_of_days(ts.second.div_euclid(DAY));
    let second_of_day = ts.second.rem_euclid(DAY);
    let time = time(&Time {
        hour: (second_of_day / 3_600) as u8,
        minute: (second_of_day / 60 % 60) as u8,
        second: (second_of_day % 60) as u8,
        nanosecond: ts.nanosecond,
    });
    format!("{}T{time}Z", civil_date(year, month, day))
}

pub fn offset(o: &UtcOffset) -> String {
    if !o.known {
        return String::from("-00:00");
    }
    let sign = if o.seconds < 0 { '-' } else { '+' };
    let magnitude = o.seconds.unsigned_abs();
    let (hours, minutes, seconds) = (magnitude / 3_600, magnitude / 60 % 60, magnitude % 60);
    if seconds == 0 {
        format!("{sign}{hours:02}:{minutes:02}")
    } else {
        format!("{sign}{hours:02}:{minutes:02}:{seconds:02}")
    }
}

pub fn duration(d: &Duration) -> String {
    let total = i128::from(d.seconds) * NS + i128::from(d.nanoseconds);
    let magnitude = total.unsigned_abs();
    let whole = magnitude / NS.unsigned_abs();
    let (hours, minutes, seconds) = (whole / 3_600, whole / 60 % 60, whole % 60);
    let nanosecond = (magnitude % NS.unsigned_abs()) as u32;
    let mut text = String::from(if total < 0 { "-PT" } else { "PT" });
    if hours != 0 {
        text.push_str(&format!("{hours}H"));
    }
    if minutes != 0 {
        text.push_str(&format!("{minutes}M"));
    }
    if seconds != 0 || nanosecond != 0 || whole == 0 {
        text.push_str(&format!("{seconds}{}S", fraction(nanosecond)));
    }
    text
}

pub fn span(s: &Span) -> String {
    let any_negative = s.units.iter().any(|&v| v < 0);
    let any_positive = s.units.iter().any(|&v| v > 0);
    let mut text = String::new();
    if any_negative && !any_positive {
        text.push('-');
    }
    let mut written = false;
    for (&value, suffix) in s.units.iter().zip(SPAN_SUFFIXES) {
        if value == 0 {
            continue;
        }
        if written {
            text.push(' ');
        }
        written = true;
        if any_negative && any_positive {
            text.push_str(&format!("{value:+}{suffix}"));
        } else {
            text.push_str(&format!("{}{suffix}", value.unsigned_abs()));
        }
    }
    if !written {
        text.push_str("0s");
    }
    text
}

/// The zoned datetime at the wall time `civil` in the zone `zone`, with
/// the offset in force, what the wall time is there and, for a repeated
/// one, which instant it means.
pub fn zoned_datetime(
    civil: &DateTime,
    utc_offset: &UtcOffset,
    zone: &str,
    kind: Kind,
    fold: bool,
) -> String {
    let mut text = datetime(civil);
    if zone == UNKNOWN_NAME {
        text.push('Z');
    } else {
        text.push_str(&offset(utc_offset));
    }
    text.push_str(&format!("[{zone}]"));
    match kind {
        Kind::Unique => {}
        Kind::Gap => text.push_str("[gap]"),
        Kind::Fold => text.push_str(if fold { "[fold=1]" } else { "[fold=0]" }),
    }
    text
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/draws/format_tests.rs"]
mod tests;
