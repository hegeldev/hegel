use super::datetime::{duration_from_c, duration_to_c, precision_to_c};
use super::generators::draw_and_print_value;
use super::{Generator, PrintableGenerator, TestCase, label_from_name};
use crate::datetime::{Precision, SignedDuration, Timestamp, timestamp_from_c, timestamp_to_c};
use crate::pretty::PrettyPrinter;
use crate::test_case::invalid_argument;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DURATION_LABEL: u64 = label_from_name("hegel.durations");
const SYSTEM_TIME_LABEL: u64 = label_from_name("hegel.system_times");

/// The largest duration the engine draws: `i64::MAX` seconds and change.
const MAX_SECONDS: u64 = i64::MAX as u64;

/// Generator for [`Duration`] values. Created by [`durations()`].
///
/// Draws the non-negative half of the engine's duration range, so the
/// maximum generatable duration is `i64::MAX` seconds (about 292 billion
/// years).
pub struct DurationGenerator {
    min_value: Duration,
    max_value: Duration,
    precision: Precision,
}

impl DurationGenerator {
    /// Set the minimum duration (inclusive).
    pub fn min_value(mut self, min: Duration) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum duration (inclusive). Saturates at `i64::MAX`
    /// seconds, the largest generatable duration.
    pub fn max_value(mut self, max: Duration) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only durations that are a whole number of `precision` units.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self
    }
}

fn signed(d: Duration) -> SignedDuration {
    SignedDuration {
        seconds: d.as_secs().min(MAX_SECONDS) as i64,
        nanoseconds: if d.as_secs() > MAX_SECONDS {
            999_999_999
        } else {
            d.subsec_nanos() as i32
        },
    }
}

impl Generator<Duration> for DurationGenerator {
    fn label(&self) -> u64 {
        DURATION_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> Duration {
        if self.min_value.as_secs() > MAX_SECONDS {
            invalid_argument!(
                "min_value exceeds the largest generatable Duration \
                 (i64::MAX seconds, about 292 billion years)"
            );
        }
        if self.min_value > self.max_value {
            invalid_argument!("Cannot have max_value < min_value");
        }
        let d = duration_from_c(tc.generate_duration(
            duration_to_c(signed(self.min_value)),
            duration_to_c(signed(self.max_value)),
            precision_to_c(self.precision),
        ));
        Duration::new(d.seconds as u64, d.nanoseconds as u32)
    }
}

impl PrintableGenerator<Duration> for DurationGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> Duration {
        draw_and_print_value(self, tc, printer)
    }
}

/// Generate [`Duration`] values.
/// By default, generates durations from zero up to `i64::MAX` seconds
/// (approximately 292 billion years) at nanosecond precision; shrinks
/// toward zero. Use builder methods `min_value` and `max_value` to
/// constrain the range. See [`DurationGenerator`] for more details.
///
/// # Example
///
/// ```no_run
/// use std::time::Duration;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let d = tc.draw(hegel::generators::durations()
///         .max_value(Duration::from_secs(60)));
///     assert!(d <= Duration::from_secs(60));
/// }
/// ```
pub fn durations() -> DurationGenerator {
    DurationGenerator {
        min_value: Duration::ZERO,
        max_value: Duration::new(MAX_SECONDS, 999_999_999),
        precision: Precision::Nanoseconds,
    }
}

/// Generator for [`SystemTime`] values. Created by [`system_times()`].
pub struct SystemTimeGenerator {
    min_value: SystemTime,
    max_value: SystemTime,
    precision: Precision,
}

impl SystemTimeGenerator {
    /// Set the earliest time (inclusive).
    pub fn min_value(mut self, min: SystemTime) -> Self {
        self.min_value = min;
        self
    }

    /// Set the latest time (inclusive).
    pub fn max_value(mut self, max: SystemTime) -> Self {
        self.max_value = max;
        self
    }

    /// Draw only times that are a whole number of `precision` units since
    /// the Unix epoch.
    pub fn precision(mut self, precision: Precision) -> Self {
        self.precision = precision;
        self
    }
}

fn timestamp_of(t: SystemTime) -> Timestamp {
    match t.duration_since(UNIX_EPOCH) {
        Ok(since) => Timestamp {
            second: since.as_secs() as i64,
            nanosecond: since.subsec_nanos(),
        },
        Err(before) => {
            let before = before.duration();
            let whole = Timestamp {
                second: -(before.as_secs() as i64),
                nanosecond: 0,
            };
            if before.subsec_nanos() == 0 {
                whole
            } else {
                Timestamp {
                    second: whole.second - 1,
                    nanosecond: 1_000_000_000 - before.subsec_nanos(),
                }
            }
        }
    }
}

fn system_time_of(t: Timestamp) -> SystemTime {
    let nanos = Duration::from_nanos(u64::from(t.nanosecond));
    if t.second >= 0 {
        UNIX_EPOCH + Duration::from_secs(t.second as u64) + nanos
    } else {
        UNIX_EPOCH - Duration::from_secs(t.second.unsigned_abs()) + nanos
    }
}

impl SystemTimeGenerator {
    fn draw_timestamp(&self, tc: &TestCase) -> Timestamp {
        if self.min_value > self.max_value {
            invalid_argument!("Cannot have max_value < min_value");
        }
        timestamp_from_c(tc.generate_timestamp(
            timestamp_to_c(timestamp_of(self.min_value)),
            timestamp_to_c(timestamp_of(self.max_value)),
            precision_to_c(self.precision),
        ))
    }
}

impl Generator<SystemTime> for SystemTimeGenerator {
    fn label(&self) -> u64 {
        SYSTEM_TIME_LABEL
    }

    fn do_draw(&self, tc: &TestCase) -> SystemTime {
        system_time_of(self.draw_timestamp(tc))
    }
}

impl PrintableGenerator<SystemTime> for SystemTimeGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> SystemTime {
        let t = self.draw_timestamp(tc);
        printer.text(&t.to_string());
        system_time_of(t)
    }
}

/// Generate [`SystemTime`] values.
///
/// By default any time from the Unix epoch to the end of year 9999 at
/// nanosecond precision, a range every platform's `SystemTime` holds;
/// widen it with `min_value` and `max_value` where the platform allows,
/// up to the engine's years ±999,999. Shrinks toward 2000-01-01T00:00:00Z.
/// Built on [`timestamps()`](super::timestamps), whose targeting of interesting instants it
/// shares and whose RFC 9557 form (`2000-01-01T00:00:00Z`) a drawn value
/// prints in. On Windows a `SystemTime` counts 100-nanosecond ticks, so
/// there the value is the drawn instant truncated to that unit while the
/// printed form keeps the nanoseconds; draw at
/// [`Precision::Microseconds`] or coarser for the two to agree. See
/// [`SystemTimeGenerator`] for builder methods.
///
/// # Example
///
/// ```no_run
/// use std::time::{Duration, UNIX_EPOCH};
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let max = UNIX_EPOCH + Duration::from_secs(1 << 31);
///     let t = tc.draw(hegel::generators::system_times().max_value(max));
///     assert!(t <= max);
/// }
/// ```
pub fn system_times() -> SystemTimeGenerator {
    SystemTimeGenerator {
        min_value: UNIX_EPOCH,
        max_value: UNIX_EPOCH + Duration::new(253_402_300_799, 999_999_999),
        precision: Precision::Nanoseconds,
    }
}
