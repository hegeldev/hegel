use time::{
    Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcDateTime, UtcOffset, Weekday,
};

use crate::generators::{DefaultGenerator, Generator, Mapped};

use super::{
    DateGenerator, DurationGenerator, MonthGenerator, OffsetDateTimeGenerator,
    PrimitiveDateTimeGenerator, TimeGenerator, UtcOffsetGenerator, WeekdayGenerator, dates,
    durations, months, offset_datetimes, primitive_datetimes, times, utc_offsets, weekdays,
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

impl DefaultGenerator for PrimitiveDateTime {
    type Generator = PrimitiveDateTimeGenerator;
    fn default_generator() -> Self::Generator {
        primitive_datetimes()
    }
}

impl DefaultGenerator for UtcDateTime {
    type Generator = Mapped<
        PrimitiveDateTime,
        UtcDateTime,
        fn(PrimitiveDateTime) -> UtcDateTime,
        PrimitiveDateTimeGenerator,
    >;
    fn default_generator() -> Self::Generator {
        primitive_datetimes().map(|dt| UtcDateTime::new(dt.date(), dt.time()))
    }
}

impl DefaultGenerator for UtcOffset {
    type Generator = UtcOffsetGenerator;
    fn default_generator() -> Self::Generator {
        utc_offsets()
    }
}

impl DefaultGenerator for OffsetDateTime {
    type Generator = OffsetDateTimeGenerator;
    fn default_generator() -> Self::Generator {
        offset_datetimes()
    }
}

impl DefaultGenerator for Duration {
    type Generator = DurationGenerator;
    fn default_generator() -> Self::Generator {
        durations()
    }
}

impl DefaultGenerator for Weekday {
    type Generator = WeekdayGenerator;
    fn default_generator() -> Self::Generator {
        weekdays()
    }
}

impl DefaultGenerator for Month {
    type Generator = MonthGenerator;
    fn default_generator() -> Self::Generator {
        months()
    }
}
