//! Generators for the [`jiff`] datetime crate.
//!
//! Enabled by the `jiff` feature flag. Each generator here is the
//! engine's draw of the matching [`hegel::generators::temporal`](crate::generators::temporal)
//! value, converted to jiff's type at the boundary: [`dates`], [`times`],
//! [`datetimes`], [`timestamps`], [`offsets`], [`signed_durations`] and
//! [`spans`] with the bounds, precision, granularity and per-unit options
//! of their first-party counterparts; [`zones`], the IANA names of
//! jiff's database, resolved with
//! [`TimeZone::get`](jiff::tz::TimeZone::get); and [`zoneds`], a zone and
//! a wall-clock time in it built with jiff's own resolution rules
//! (`to_ambiguous_zoned` then `earlier` or `later`), skipped wall times
//! avoided by default since a [`Zoned`](jiff::Zoned) cannot hold one.
//! [`iso_week_dates`], [`weekdays`] and [`eras`] are mappings, and every
//! public jiff value type has a [`DefaultGenerator`](crate::generators::DefaultGenerator)
//! impl, so `gs::default::<jiff::Zoned>()` works and the types can be used
//! in a derived generator.
//!
//! The option enums come from [`hegel::generators::temporal`](crate::generators::temporal):
//! [`Precision`](crate::generators::temporal::Precision),
//! [`Granularity`](crate::generators::temporal::Granularity),
//! [`Month`](crate::generators::temporal::Month) and
//! [`Ambiguous`](crate::generators::temporal::Ambiguous).
//!
//! Zones are drawn, and wall times classified, against jiff's own time
//! zone database through [`zone_source`], so the gaps and folds the
//! engine targets are the ones jiff computes, whichever database jiff's
//! features and the system give it.

mod default;
mod generators;
mod source;

pub use generators::*;
pub use source::zone_source;
