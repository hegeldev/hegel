use std::path::Path;
use std::sync::OnceLock;

use jiff::Timestamp;
use jiff::tz::{TimeZone, TimeZoneDatabase};

use crate::generators::temporal::{
    AfterProbe, Error, LocalTimeType, ZoneLoader, ZoneOracle, ZoneSource,
};

/// The first year jiff's timestamps cover whole.
const FIRST_YEAR: i32 = -9998;

/// The last year probed. Every zone's explicit transitions end by then
/// (Morocco's last, in 2087, are the latest), so the last two years show
/// the rule jiff applies for ever after, which the engine infers.
const LAST_YEAR: i32 = 2100;

fn local_time_type(zone: &TimeZone, second: i64) -> LocalTimeType {
    let info = zone.to_offset_info(Timestamp::from_second(second).unwrap());
    LocalTimeType {
        offset: info.offset().seconds(),
        is_dst: info.dst().is_dst(),
        abbreviation: info.abbreviation().to_string(),
    }
}

fn next_transition(zone: &TimeZone, after: i64) -> Option<i64> {
    zone.following(Timestamp::from_second(after).unwrap())
        .next()
        .map(|transition| transition.timestamp().as_second())
}

/// Add the zone `name` as jiff's database has it, probed through jiff's
/// own lookups, or nothing when jiff does not know the name.
fn load(db: &TimeZoneDatabase, source: &ZoneLoader<'_>, name: &str) -> Result<(), Error> {
    let Ok(zone) = db.get(name) else {
        return Ok(());
    };
    source.oracle(
        name,
        ZoneOracle::new(FIRST_YEAR, LAST_YEAR, AfterProbe::Infer, |second| {
            Ok(local_time_type(&zone, second))
        })
        .next_transition(|after| Ok(next_transition(&zone, after))),
    )
}

/// The release of the IANA database a jiff database reads, from how jiff
/// describes it: the `tzdata.zi` or `+VERSION` file of a zoneinfo
/// directory, or the bundled `jiff-tzdb` release.
fn database_version(description: &str) -> String {
    if description.contains("Bundled(available)") {
        return jiff_tzdb::VERSION.unwrap_or_default().to_string();
    }
    let Some(dir) = description
        .strip_prefix("TimeZoneDatabase(ZoneInfo(")
        .and_then(|rest| rest.strip_suffix("))"))
    else {
        return String::new();
    };
    zoneinfo_version(Path::new(dir))
}

fn zoneinfo_version(dir: &Path) -> String {
    if let Ok(zi) = std::fs::read_to_string(dir.join("tzdata.zi")) {
        if let Some(version) = zi.lines().next().and_then(|l| l.strip_prefix("# version ")) {
            return version.trim().to_string();
        }
    }
    std::fs::read_to_string(dir.join("+VERSION"))
        .map(|version| version.trim().to_string())
        .unwrap_or_default()
}

/// A zone source over a jiff database: its names, each zone's data
/// loaded the first time a draw needs it, probed through jiff's own
/// lookups so the engine classifies wall times exactly as jiff resolves
/// them.
fn source_over(db: &'static TimeZoneDatabase) -> ZoneSource {
    ZoneSource::builder()
        .declare(db.available().map(|name| name.as_str().to_string()))
        .loader(move |source, name| load(db, source, name))
        .version(&database_version(&format!("{db:?}")))
        .build()
        .unwrap()
}

/// The zone source of jiff's time zone database, the one
/// [`jiff::tz::db`] returns and [`TimeZone::get`] reads: the system's
/// zoneinfo directory (`TZDIR`, else `/usr/share/zoneinfo` and the other
/// places jiff searches) or the copy jiff bundles, as jiff's features and
/// the platform decide. Every generator of this module draws against it,
/// so the gaps and folds the engine targets are jiff's own.
///
/// Each zone's data is read the first time a draw needs it, by probing
/// jiff's offsets and transitions; past 2100 the engine extends the
/// rule the last two years show, as jiff does with the zone's TZ string.
/// [`version`](ZoneSource::version) is the database's IANA release when
/// jiff's database records one.
///
/// ```no_run
/// use hegel::extras::jiff as jiff_gs;
/// use hegel::generators as gs;
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let z = tc.draw(gs::temporal::zoned_datetimes()
///         .zones(gs::temporal::zones().source(jiff_gs::zone_source())));
///     assert!(jiff::tz::TimeZone::get(&z.zone.name).is_ok());
/// }
/// ```
pub fn zone_source() -> ZoneSource {
    static SOURCE: OnceLock<ZoneSource> = OnceLock::new();
    SOURCE.get_or_init(|| source_over(jiff::tz::db())).clone()
}

#[cfg(test)]
#[path = "../../../tests/embedded/extras/jiff_source_tests.rs"]
mod tests;
