use jiff::civil::{Date, DateTime, Era, ISOWeekDate, Time, Weekday};
use jiff::tz::{AmbiguousOffset, AmbiguousTimestamp, AmbiguousZoned, Offset, TimeZone};
use jiff::{SignedDuration, Span, Timestamp, Zoned};

use crate::datetime::ZonedDateTime;
use crate::generators::{
    self as gs, BoxedGenerator, DefaultGenerator, Generator, Mapped, ZonedDateTimeGenerator, one_of,
};

use super::{
    DateGenerator, DateTimeGenerator, EraGenerator, ISOWeekDateGenerator, MAX_CIVIL, MIN_CIVIL,
    OffsetGenerator, SignedDurationGenerator, SpanGenerator, TimeGenerator, TimestampGenerator,
    WeekdayGenerator, ZonedGenerator, dates, datetimes, eras, iso_week_dates, offsets,
    signed_durations, spans, time_zones, times, timestamps, weekdays, zoneds,
};

impl DefaultGenerator for Date {
    type Generator = DateGenerator;
    fn default_generator() -> Self::Generator {
        dates()
    }
}

impl DefaultGenerator for Time {
    type Generator = TimeGenerator;
    fn default_generator() -> Self::Generator {
        times()
    }
}

impl DefaultGenerator for DateTime {
    type Generator = DateTimeGenerator;
    fn default_generator() -> Self::Generator {
        datetimes()
    }
}

impl DefaultGenerator for ISOWeekDate {
    type Generator = ISOWeekDateGenerator;
    fn default_generator() -> Self::Generator {
        iso_week_dates()
    }
}

impl DefaultGenerator for Weekday {
    type Generator = WeekdayGenerator;
    fn default_generator() -> Self::Generator {
        weekdays()
    }
}

impl DefaultGenerator for Era {
    type Generator = EraGenerator;
    fn default_generator() -> Self::Generator {
        eras()
    }
}

impl DefaultGenerator for Timestamp {
    type Generator = TimestampGenerator;
    fn default_generator() -> Self::Generator {
        timestamps()
    }
}

impl DefaultGenerator for Span {
    type Generator = SpanGenerator;
    fn default_generator() -> Self::Generator {
        spans()
    }
}

impl DefaultGenerator for SignedDuration {
    type Generator = SignedDurationGenerator;
    fn default_generator() -> Self::Generator {
        signed_durations()
    }
}

impl DefaultGenerator for Offset {
    type Generator = OffsetGenerator;
    fn default_generator() -> Self::Generator {
        offsets()
    }
}

impl DefaultGenerator for TimeZone {
    type Generator = BoxedGenerator<'static, TimeZone>;
    fn default_generator() -> Self::Generator {
        one_of([time_zones().boxed(), offsets().map(TimeZone::fixed).boxed()]).boxed()
    }
}

impl DefaultGenerator for Zoned {
    type Generator = ZonedGenerator;
    fn default_generator() -> Self::Generator {
        zoneds()
    }
}

/// The engine's zoned draw over jiff's zones, with skipped wall times
/// kept and no fold drawn, for the ambiguous types that hold a wall time
/// jiff has not yet resolved.
fn ambiguous_draws() -> ZonedDateTimeGenerator {
    gs::zoned_datetimes()
        .time_zones(time_zones().engine_generator())
        .min_value(super::engine_datetime(MIN_CIVIL))
        .max_value(super::engine_datetime(MAX_CIVIL))
        .imaginary(crate::datetime::Imaginary::Allow)
        .ambiguous(crate::datetime::Ambiguous::Earlier)
}

fn ambiguous_zoned(z: ZonedDateTime) -> AmbiguousZoned {
    TimeZone::get(&z.zone.name)
        .unwrap()
        .to_ambiguous_zoned(super::jiff_datetime(z.civil))
}

fn ambiguous_timestamp(z: ZonedDateTime) -> AmbiguousTimestamp {
    TimeZone::get(&z.zone.name)
        .unwrap()
        .to_ambiguous_timestamp(super::jiff_datetime(z.civil))
}

impl DefaultGenerator for AmbiguousZoned {
    type Generator = Mapped<
        ZonedDateTime,
        AmbiguousZoned,
        fn(ZonedDateTime) -> AmbiguousZoned,
        ZonedDateTimeGenerator,
    >;
    fn default_generator() -> Self::Generator {
        ambiguous_draws().map(ambiguous_zoned)
    }
}

impl DefaultGenerator for AmbiguousTimestamp {
    type Generator = Mapped<
        ZonedDateTime,
        AmbiguousTimestamp,
        fn(ZonedDateTime) -> AmbiguousTimestamp,
        ZonedDateTimeGenerator,
    >;
    fn default_generator() -> Self::Generator {
        ambiguous_draws().map(ambiguous_timestamp)
    }
}

impl DefaultGenerator for AmbiguousOffset {
    type Generator = Mapped<
        ZonedDateTime,
        AmbiguousOffset,
        fn(ZonedDateTime) -> AmbiguousOffset,
        ZonedDateTimeGenerator,
    >;
    fn default_generator() -> Self::Generator {
        ambiguous_draws().map(|z| ambiguous_zoned(z).offset())
    }
}
