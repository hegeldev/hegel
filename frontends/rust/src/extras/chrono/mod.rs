//! Generators for the [`chrono`] crate.
//!
//! Available behind the `chrono` feature flag. The `chrono-tz` feature adds
//! `zones()`, which draws IANA zones as `chrono_tz::Tz`, and lets
//! [`datetimes()`] draw `DateTime<chrono_tz::Tz>`, both against
//! `zone_source()`, chrono-tz's own database, so the clocks' skipped and
//! repeated wall times the engine targets are the ones chrono-tz has.

mod default;
mod generators;
#[cfg(feature = "chrono-tz")]
mod source;

pub use generators::*;
#[cfg(feature = "chrono-tz")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono-tz")))]
pub use source::zone_source;
