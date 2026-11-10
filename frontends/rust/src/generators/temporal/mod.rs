//! Dates, times, instants, UTC offsets, time zones, durations and calendar
//! spans, as the engine draws them.
//!
//! The generators are [`dates`], [`times`], [`datetimes`], [`instants`],
//! [`offsets`], [`zones`], [`zoned_datetimes`], [`durations`] and
//! [`spans`], plus [`std_durations`] and [`system_times`] over
//! [`std::time::Duration`] and [`std::time::SystemTime`].
//!
//! The values they draw are the plain types of this module, each a public
//! struct or enum of public fields mirroring the engine's value model one
//! to one, with no validation, arithmetic, parsing or conversions: a test
//! that uses a date library wraps the value it draws with
//! [`map`](crate::generators::Generator::map), or draws through the
//! [`extras`](crate::extras) for `jiff`, `chrono` and `time`. The option
//! enums of the builders live here too.
//!
//! Each `Display` implementation prints the text the engine gives the
//! value, the RFC 9557 form a counterexample is printed in, so a failing
//! value reads the same from every language binding:
//! `2024-03-10T01:30:00-05:00[America/New_York]`, with `[gap]` or
//! `[fold=0]`/`[fold=1]` appended to a skipped or repeated wall time
//! (`2024-03-10T02:30:00-04:00[America/New_York][gap]`), `-00:00` for an
//! unknown offset and `Z[Etc/Unknown]` for the unknown zone.
//!
//! Zones are drawn and wall times classified against a [`ZoneSource`].
//! Rust has no standard time zone database, so the default source is the
//! IANA database bundled in the engine, whose release [`tzdb_version`]
//! names. [`set_default_zone_source`] replaces it for the whole process
//! and [`default_zone_source`] returns the current one; a generator reads
//! the default when it is built ([`zones()`], [`zoned_datetimes()`]) and
//! keeps it. [`ZoneGenerator::source`] and
//! [`ZonedDateTimeGenerator::source`] name another source for one
//! generator. The `jiff` and `chrono-tz` integrations draw against their
//! own library's database (`hegel::extras::jiff::zone_source()`,
//! `hegel::extras::chrono::zone_source()`), so the gaps and folds the
//! engine targets are the ones that library computes, and
//! [`ZoneSource::builder`] wraps any other database. The functions
//! [`resolve`], [`offset_at`] and [`info`] query the default source
//! directly, outside any test, and the methods of the same names on
//! [`ZoneSource`] query a given one.
//!
//! ```no_run
//! use hegel::generators as gs;
//! use hegel::generators::temporal::{Date, Weekday};
//!
//! #[hegel::test]
//! fn my_test(tc: hegel::TestCase) {
//!     let min = Date::new(2024, 1, 1);
//!     let d = tc.draw(gs::temporal::dates().min_value(min).weekdays([Weekday::Monday]));
//!     assert!(d >= min);
//! }
//! ```

pub(crate) mod generators;
mod source;
mod system;
mod values;

pub use generators::{
    DateGenerator, DateTimeGenerator, DurationGenerator, InstantGenerator, OffsetGenerator,
    SpanGenerator, TimeGenerator, ZoneGenerator, ZonedDateTimeGenerator, dates, datetimes,
    durations, instants, offsets, spans, times, zoned_datetimes, zones,
};
pub use source::{
    AfterProbe, DstRule, LocalTimeType, RuleDate, RuleDay, ZoneAfter, ZoneData, ZoneLoader,
    ZoneOracle, ZoneRule, ZoneSource, ZoneSourceBuilder, ZoneTransition, default_zone_source,
    set_default_zone_source,
};
pub use system::{StdDurationGenerator, SystemTimeGenerator, std_durations, system_times};
pub use values::*;
