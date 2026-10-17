//! Generators for the [`chrono`] crate.
//!
//! Available behind the `chrono` feature flag. The `chrono-tz` feature adds
//! `time_zones()`, which draws IANA zones as `chrono_tz::Tz`, and lets
//! [`datetimes()`] draw `DateTime<chrono_tz::Tz>` with the clocks' skipped
//! and repeated wall times resolved the way the engine drew them.

mod default;
mod generators;

pub use generators::*;
