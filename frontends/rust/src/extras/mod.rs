//! Integrations with third-party libraries, each behind a cargo feature of
//! the same name: `chrono` (with `chrono-tz` adding IANA time zones),
//! `jiff`, `rand`, `serde_json` and `time`. The date and time
//! integrations are thin maps over the first-party generators of
//! [`hegel::generators`](crate::generators) and the values of
//! [`hegel::datetime`](crate::datetime), so every library draws from the
//! engine's bundled time zone database with the same targeting and
//! shrinking.

#[cfg(feature = "chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono")))]
pub mod chrono;

#[cfg(feature = "jiff")]
#[cfg_attr(docsrs, doc(cfg(feature = "jiff")))]
pub mod jiff;

#[cfg(feature = "rand")]
#[cfg_attr(docsrs, doc(cfg(feature = "rand")))]
pub mod rand;

#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub mod serde_json;

#[cfg(feature = "time")]
#[cfg_attr(docsrs, doc(cfg(feature = "time")))]
pub mod time;
