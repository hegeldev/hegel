use std::sync::OnceLock;

use chrono::{DateTime, Offset, TimeZone};
use chrono_tz::{OffsetComponents, Tz};

use crate::generators::temporal::{AfterProbe, LocalTimeType, ZoneLoader, ZoneOracle, ZoneSource};

/// The first year probed, before chrono-tz's first transition (1845):
/// each zone's first offset holds before it, as it does in chrono-tz.
const FIRST_YEAR: i32 = 1800;

/// The last year probed: chrono-tz's tables end in 2099, after which it
/// holds the last offset for ever, as the engine then does too.
const LAST_YEAR: i32 = 2100;

fn local_time_type(tz: Tz, second: i64) -> LocalTimeType {
    let offset =
        tz.offset_from_utc_datetime(&DateTime::from_timestamp(second, 0).unwrap().naive_utc());
    LocalTimeType {
        offset: offset.fix().local_minus_utc(),
        is_dst: !offset.dst_offset().is_zero(),
        abbreviation: offset.to_string(),
    }
}

/// Add the zone `name` as chrono-tz has it, probed through its offsets,
/// or nothing when chrono-tz does not know the name.
fn load(source: &ZoneLoader<'_>, name: &str) -> Result<(), crate::generators::temporal::Error> {
    let Ok(tz) = name.parse::<Tz>() else {
        return Ok(());
    };
    source.oracle(
        name,
        ZoneOracle::new(FIRST_YEAR, LAST_YEAR, AfterProbe::Hold, |second| {
            Ok(local_time_type(tz, second))
        }),
    )
}

/// The zone source of chrono-tz's compiled-in IANA database, release
/// `chrono_tz::IANA_TZDB_VERSION`: every name of
/// [`chrono_tz::TZ_VARIANTS`], each zone's data probed from chrono-tz's
/// offsets the first time a draw needs it, so the gaps and folds the
/// engine targets are the ones chrono-tz resolves. After 2099, where
/// chrono-tz's tables end, each zone keeps its last offset, as chrono-tz
/// does. [`zones()`](super::zones) and the
/// `DateTime<chrono_tz::Tz>` draws of [`datetimes()`](super::datetimes)
/// draw against it.
pub fn zone_source() -> ZoneSource {
    static SOURCE: OnceLock<ZoneSource> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            ZoneSource::builder()
                .declare(chrono_tz::TZ_VARIANTS.iter().map(|tz| tz.name()))
                .loader(load)
                .version(chrono_tz::IANA_TZDB_VERSION)
                .build()
                .unwrap()
        })
        .clone()
}
