//! Generators for the [`time`] crate.
//!
//! Available behind the `time` feature flag. Every generator here is a
//! thin map over the first-party generators of
//! [`hegel::generators`](crate::generators), so values are drawn by the
//! engine with its targeting and shrinking: [`dates`] over `time::Date`
//! with weekday and month masks, [`times`], [`primitive_datetimes`] and
//! [`durations`] with a [`Precision`](crate::datetime::Precision),
//! [`utc_offsets`] with a [`Granularity`](crate::datetime::Granularity),
//! [`offset_datetimes`] from a wall-clock time in a drawn IANA time zone
//! read with the offset the engine's bundled database says that zone had
//! in force, and [`weekdays`] and
//! [`months`]. The date range follows `time::Date::MIN..=MAX`, so
//! enabling the `time/large-dates` feature widens it to years ±999,999.

mod default;
mod generators;

pub use generators::*;
