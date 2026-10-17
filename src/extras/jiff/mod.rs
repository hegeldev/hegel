//! Generators for the [`jiff`] datetime crate.
//!
//! Enabled by the `jiff` feature flag. Each generator here is the
//! engine's draw of the matching [`hegel::datetime`](crate::datetime)
//! value, converted to jiff's type at the boundary: [`dates`], [`times`],
//! [`datetimes`], [`timestamps`], [`offsets`], [`signed_durations`] and
//! [`spans`] with the bounds, precision, granularity and per-unit options
//! of their first-party counterparts; [`time_zones`], the IANA names both
//! jiff's database and the engine's know, resolved with
//! [`TimeZone::get`](jiff::tz::TimeZone::get); and [`zoneds`], a zone and
//! a wall-clock time in it built with jiff's own resolution rules
//! (`to_ambiguous_zoned` then `earlier` or `later`), skipped wall times
//! avoided by default since a [`Zoned`](jiff::Zoned) cannot hold one.
//! [`iso_week_dates`], [`weekdays`] and [`eras`] are mappings, and every
//! public jiff value type has a [`DefaultGenerator`](crate::generators::DefaultGenerator)
//! impl, so `gs::default::<jiff::Zoned>()` works and the types can be used
//! in a derived generator.
//!
//! The option enums come from [`hegel::datetime`](crate::datetime):
//! [`Precision`](crate::datetime::Precision),
//! [`Granularity`](crate::datetime::Granularity),
//! [`Month`](crate::datetime::Month),
//! [`Imaginary`](crate::datetime::Imaginary) and
//! [`Ambiguous`](crate::datetime::Ambiguous).
//!
//! The engine's bundled time zone database decides which zone and wall
//! time are drawn; jiff's database decides what the value means. The two
//! may differ by a release, which
//! [`tzdb_version`](crate::datetime::tzdb_version) reports.

mod default;
mod generators;

pub use generators::*;
