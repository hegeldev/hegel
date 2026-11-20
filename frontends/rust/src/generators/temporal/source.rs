use std::marker::PhantomData;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use super::values::{
    DateTime, Error, Instant, Month, TimeZone, Weekday, ZoneInfo, ZoneOffset, ZonedDateTime,
    info_in, offset_at_in, resolve_in, zone_in,
};
use crate::ffi;
use crate::ffi::sys as hegel_c;

/// A local time type of a zone: an offset, its daylight-saving flag and
/// its abbreviation, as a TZif file (RFC 9636) records one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalTimeType {
    /// Seconds east of UTC, less than two days in magnitude.
    pub offset: i32,
    /// Whether the type is daylight saving time. Informational: reported
    /// by [`ZoneSource::offset_at`], never used to classify a wall time.
    pub is_dst: bool,
    /// The abbreviation, such as `EST`; no NUL bytes.
    pub abbreviation: String,
}

impl LocalTimeType {
    fn to_c(&self) -> hegel_c::hegel_zone_type_t {
        hegel_c::hegel_zone_type_t {
            utoff: self.offset,
            is_dst: self.is_dst,
            abbreviation: view(&self.abbreviation),
        }
    }
}

fn view(s: &str) -> hegel_c::hegel_string_t {
    hegel_c::hegel_string_t {
        data: s.as_ptr().cast(),
        len: s.len(),
    }
}

/// An explicit transition of a zone: from `instant` (Unix seconds) on,
/// `local_type` is in force.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ZoneTransition {
    pub instant: i64,
    pub local_type: LocalTimeType,
}

/// The day a yearly rule's transition happens on, as a POSIX TZ string
/// names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleDay {
    /// `Jn`: the day of the year in `[1, 365]`, February 29 never counted.
    Julian(u16),
    /// `n`: the zero-based day of the year in `[0, 365]`, February 29
    /// counted.
    ZeroBased(u16),
    /// `Mm.w.d`: the `week`th `weekday` of `month`, `week` in `[1, 5]`
    /// with 5 meaning the last.
    MonthWeekday {
        month: Month,
        week: u8,
        weekday: Weekday,
    },
}

/// When a yearly rule's transition happens: `time` seconds (within ±167
/// hours) after local midnight of `day`, in the offset in force before the
/// transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleDate {
    pub day: RuleDay,
    pub time: i32,
}

impl RuleDate {
    fn to_c(self) -> hegel_c::hegel_zone_rule_date_t {
        use hegel_c::hegel_zone_rule_day_t as c;
        let (kind, day, month, week, weekday) = match self.day {
            RuleDay::Julian(day) => (c::HEGEL_ZONE_RULE_DAY_JULIAN, day, 0, 0, 0),
            RuleDay::ZeroBased(day) => (c::HEGEL_ZONE_RULE_DAY_ZERO_BASED, day, 0, 0, 0),
            RuleDay::MonthWeekday {
                month,
                week,
                weekday,
            } => (
                c::HEGEL_ZONE_RULE_DAY_MONTH_WEEK_DAY,
                0,
                month as u8 + 1,
                week,
                (weekday as u8 + 1) % 7,
            ),
        };
        hegel_c::hegel_zone_rule_date_t {
            kind: kind as u32,
            day,
            month,
            week,
            weekday,
            time: self.time,
        }
    }
}

/// The daylight-saving half of a [`ZoneRule`]: `local_type` is in force
/// from `start` to `end` each year.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DstRule {
    pub local_type: LocalTimeType,
    pub start: RuleDate,
    pub end: RuleDate,
}

/// A zone's rule for every year after its last explicit transition, the
/// footer of a TZif file: a standard type and, when the zone observes
/// daylight saving time, a [`DstRule`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ZoneRule {
    pub standard: LocalTimeType,
    pub dst: Option<DstRule>,
}

/// What a zone does after its last explicit transition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ZoneAfter {
    /// The last type stays in force.
    Hold,
    /// The rule applies.
    Rule(ZoneRule),
    /// A yearly rule inferred from the transitions of the last two years
    /// up to `last_year`, the last year the transitions cover, or the last
    /// type held when they show none.
    Infer { last_year: i32 },
}

/// A zone's data as fields, the TZif model of RFC 9636.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ZoneData {
    /// The type in force before the first transition.
    pub initial: LocalTimeType,
    /// The explicit transitions, strictly ascending by instant.
    pub transitions: Vec<ZoneTransition>,
    pub after: ZoneAfter,
}

/// What an oracle's zone does after the last year probed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AfterProbe {
    /// The last type stays in force, as chrono-tz does after its table
    /// ends.
    Hold,
    /// A yearly rule inferred from the last two years probed, closer for a
    /// library that applies the IANA rule for ever. The probe range must
    /// span two years at least.
    Infer,
}

type OffsetFn<'a> = Box<dyn FnMut(i64) -> Result<LocalTimeType, Error> + 'a>;
type NextTransitionFn<'a> = Box<dyn FnMut(i64) -> Result<Option<i64>, Error> + 'a>;

/// An offset oracle: a library that answers lookups but does not expose
/// its zone data, probed into explicit transitions over the years
/// `first_year` to `last_year` when it is added to a source.
///
/// With [`next_transition`](Self::next_transition) the probe asks for
/// each transition in turn; without it, it samples the offset once a day
/// and bisects each change to the second, which misses a pair of changes
/// less than a day apart. Before `first_year` the type at its start holds;
/// after `last_year`, [`AfterProbe`] decides. The callbacks run on the
/// thread adding the oracle, during that call only.
pub struct ZoneOracle<'a> {
    offset_at: OffsetFn<'a>,
    next_transition: Option<NextTransitionFn<'a>>,
    first_year: i32,
    last_year: i32,
    after: AfterProbe,
}

impl<'a> ZoneOracle<'a> {
    /// An oracle probed over `first_year..=last_year` (within ±999,999)
    /// through `offset_at`, the local time type in force at an instant in
    /// Unix seconds.
    pub fn new(
        first_year: i32,
        last_year: i32,
        after: AfterProbe,
        offset_at: impl FnMut(i64) -> Result<LocalTimeType, Error> + 'a,
    ) -> Self {
        ZoneOracle {
            offset_at: Box::new(offset_at),
            next_transition: None,
            first_year,
            last_year,
            after,
        }
    }

    /// Probe through `next_transition`, the first instant after the one
    /// given at which the type in force changes, or `None` when it never
    /// changes again.
    pub fn next_transition(
        mut self,
        next_transition: impl FnMut(i64) -> Result<Option<i64>, Error> + 'a,
    ) -> Self {
        self.next_transition = Some(Box::new(next_transition));
        self
    }
}

/// A zone source being filled: the builder's target and a loader's
/// argument. Each method adds to the source, replacing what it had for
/// the name, or reports the engine's reason for rejecting the data.
pub struct ZoneLoader<'a> {
    target: ffi::ZoneTarget,
    _source: PhantomData<&'a ()>,
}

impl ZoneLoader<'_> {
    /// Add the zone `name` from a TZif file (RFC 9636, version 2 or
    /// later), such as one under `/usr/share/zoneinfo`.
    pub fn tzif(&self, name: &str, bytes: &[u8]) -> Result<(), Error> {
        Ok(self.target.add_tzif(name, bytes)?)
    }

    /// Add the zone `name` from its data as fields.
    pub fn zone(&self, name: &str, data: &ZoneData) -> Result<(), Error> {
        let transitions: Vec<hegel_c::hegel_zone_transition_t> = data
            .transitions
            .iter()
            .map(|t| hegel_c::hegel_zone_transition_t {
                instant: t.instant,
                local_type: t.local_type.to_c(),
            })
            .collect();
        let unused = LocalTimeType {
            offset: 0,
            is_dst: false,
            abbreviation: String::new(),
        };
        let unused_date = RuleDate {
            day: RuleDay::Julian(1),
            time: 0,
        };
        let (after, standard, dst, last_year) = match &data.after {
            ZoneAfter::Hold => (
                hegel_c::hegel_zone_after_t::HEGEL_ZONE_AFTER_HOLD,
                &unused,
                None,
                0,
            ),
            ZoneAfter::Rule(rule) => (
                hegel_c::hegel_zone_after_t::HEGEL_ZONE_AFTER_RULE,
                &rule.standard,
                rule.dst.as_ref(),
                0,
            ),
            ZoneAfter::Infer { last_year } => (
                hegel_c::hegel_zone_after_t::HEGEL_ZONE_AFTER_INFER,
                &unused,
                None,
                *last_year,
            ),
        };
        let c_data = hegel_c::hegel_zone_data_t {
            initial: data.initial.to_c(),
            transitions: transitions.as_ptr(),
            transition_count: transitions.len(),
            after: after as u32,
            standard: standard.to_c(),
            has_dst: dst.is_some(),
            dst: dst.map_or(&unused, |dst| &dst.local_type).to_c(),
            dst_start: dst.map_or(unused_date, |dst| dst.start).to_c(),
            dst_end: dst.map_or(unused_date, |dst| dst.end).to_c(),
            last_year,
        };
        Ok(self.target.add_zone(name, &c_data)?)
    }

    /// Add the zone `name` by probing `oracle`, which is called on this
    /// thread before this returns.
    pub fn oracle(&self, name: &str, oracle: ZoneOracle<'_>) -> Result<(), Error> {
        let ZoneOracle {
            mut offset_at,
            next_transition,
            first_year,
            last_year,
            after,
        } = oracle;
        let mut offset = |instant| {
            offset_at(instant)
                .map(|t| ffi::ZoneTypeRaw {
                    utoff: t.offset,
                    is_dst: t.is_dst,
                    abbreviation: t.abbreviation,
                })
                .map_err(|e| e.to_string())
        };
        let mut next = next_transition
            .map(|mut next| move |after| next(after).map_err(|e: Error| e.to_string()));
        let after = match after {
            AfterProbe::Hold => hegel_c::hegel_zone_after_t::HEGEL_ZONE_AFTER_HOLD,
            AfterProbe::Infer => hegel_c::hegel_zone_after_t::HEGEL_ZONE_AFTER_INFER,
        };
        Ok(self.target.add_oracle(
            name,
            ffi::OracleCallbacks {
                offset_at: &mut offset,
                next_transition: next.as_mut().map(|f| f as &mut ffi::NextTransitionFn<'_>),
                first_year,
                last_year,
                after: after as u32,
            },
        )?)
    }

    /// Make `name` a link to `target`, whose data then applies to both.
    /// The target need not have data, or be known, yet.
    pub fn link(&self, name: &str, target: &str) -> Result<(), Error> {
        Ok(self.target.add_link(name, target)?)
    }

    /// Declare names the source knows whose data its loader supplies the
    /// first time a draw or query needs it. A name the source already has
    /// keeps what it has.
    pub fn declare<S: Into<String>>(
        &self,
        names: impl IntoIterator<Item = S>,
    ) -> Result<(), Error> {
        let names: Vec<String> = names.into_iter().map(Into::into).collect();
        Ok(self.target.declare(&names)?)
    }
}

/// A source of time zone data: the database a zone draw and the queries
/// classify, target and resolve wall times against.
///
/// The engine's bundled IANA database, [`ZoneSource::bundled`], is the
/// default source of the Rust frontend until [`set_default_zone_source`]
/// replaces it for the whole process; [`default_zone_source`] is the
/// current one. A generator reads the default when it is built, and
/// draws against another source when given one with
/// [`ZoneGenerator::source`](super::ZoneGenerator::source) or
/// [`ZonedDateTimeGenerator::source`](super::ZonedDateTimeGenerator::source).
/// The `jiff` and `chrono-tz` integrations' generators draw against their
/// own library's database (`hegel::extras::jiff::zone_source()` and
/// `hegel::extras::chrono::zone_source()`), so a drawn wall time's gap or
/// fold is the one that library computes.
///
/// [`ZoneSource::builder`] fills a source from any other database, in
/// whatever shape it exposes: TZif files, zone data as fields, or an
/// offset oracle probed into transitions, each added up front or by a
/// loader the first time a name is needed. Zone names, link and legacy
/// flags, feature tags and the shrink order still come from the bundled
/// catalogue, so a name it has keeps them whatever its data, and a name it
/// lacks is drawn last, tagged from its data.
///
/// A source is cheap to clone, and may be shared between threads; two
/// sources are equal when they are clones of one another. Build a source
/// once, in a `static`, rather than in a test body, which runs once per
/// test case.
///
/// ```no_run
/// use std::sync::LazyLock;
///
/// use hegel::generators as gs;
/// use hegel::generators::temporal::ZoneSource;
///
/// // Two zones of a zoneinfo directory, each read the first time a draw
/// // needs it.
/// static ZONEINFO: LazyLock<ZoneSource> = LazyLock::new(|| {
///     ZoneSource::builder()
///         .declare(["America/New_York", "Europe/London"])
///         .loader(|source, name| match std::fs::read(format!("/usr/share/zoneinfo/{name}")) {
///             Ok(bytes) => source.tzif(name, &bytes),
///             Err(_) => Ok(()),
///         })
///         .build()
///         .unwrap()
/// });
///
/// #[hegel::test]
/// fn my_test(tc: hegel::TestCase) {
///     let z = tc.draw(gs::temporal::zoned_datetimes().source(ZONEINFO.clone()));
///     assert!(z.zone.name == "America/New_York" || z.zone.name == "Europe/London");
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ZoneSource {
    handle: Arc<ffi::ZoneSourceHandle>,
}

impl PartialEq for ZoneSource {
    fn eq(&self, other: &ZoneSource) -> bool {
        Arc::ptr_eq(&self.handle, &other.handle)
    }
}

impl Eq for ZoneSource {}

impl ZoneSource {
    /// The engine's bundled IANA database, whose release is
    /// [`tzdb_version`](super::tzdb_version).
    pub fn bundled() -> ZoneSource {
        static BUNDLED: OnceLock<ZoneSource> = OnceLock::new();
        BUNDLED
            .get_or_init(|| ZoneSource {
                handle: Arc::new(ffi::ZoneSourceHandle::bundled()),
            })
            .clone()
    }

    /// A builder for a source over another database, starting empty.
    pub fn builder() -> ZoneSourceBuilder {
        ZoneSourceBuilder {
            handle: ffi::ZoneSourceHandle::new(),
            error: None,
        }
    }

    /// The release of the source's database: the bundled release for
    /// [`ZoneSource::bundled`], otherwise what
    /// [`ZoneSourceBuilder::version`] recorded, or an empty string.
    pub fn version(&self) -> String {
        self.handle.version()
    }

    /// What the source knows about `name`, as [`info`](super::info)
    /// reports it for the default source: `None` for a name the source
    /// lacks, and an error when loading the name's data fails.
    pub fn info(&self, name: &str) -> Result<Option<ZoneInfo>, Error> {
        info_in(&self.handle, name)
    }

    /// The zone `name` names in this source, with the primary name the
    /// source resolves it to, or an error when the source does not know
    /// the name or fails to load it. Names are matched exactly as the
    /// source spells them.
    pub fn zone(&self, name: &str) -> Result<TimeZone, Error> {
        zone_in(&self.handle, name)
    }

    /// [`resolve`](super::resolve) against this source.
    pub fn resolve(&self, name: &str, civil: DateTime) -> Result<ZonedDateTime, Error> {
        resolve_in(&self.handle, name, civil)
    }

    /// [`offset_at`](super::offset_at) against this source.
    pub fn offset_at(&self, name: &str, instant: Instant) -> Result<ZoneOffset, Error> {
        offset_at_in(&self.handle, name, instant)
    }

    pub(crate) fn handle(&self) -> &ffi::ZoneSourceHandle {
        &self.handle
    }
}

/// The default zone source this frontend last set, `None` standing for
/// the bundled database libhegel starts with. The lock is held across the
/// engine call that changes the default, so the two never disagree.
static DEFAULT_ZONE_SOURCE: Mutex<Option<ZoneSource>> = Mutex::new(None);

/// The process-wide default zone source: the bundled database
/// ([`ZoneSource::bundled`]) until [`set_default_zone_source`] replaces
/// it.
///
/// A generator reads the default when it is built, by
/// [`zones()`](super::zones), [`zoned_datetimes()`](super::zoned_datetimes)
/// or a generator made from them, and keeps the source it read: changing
/// the default afterwards does not change what that generator draws
/// against. The queries [`info`](super::info), [`resolve`](super::resolve)
/// and [`offset_at`](super::offset_at) and [`TimeZone::new`] read it at
/// each call.
pub fn default_zone_source() -> ZoneSource {
    let default = DEFAULT_ZONE_SOURCE
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    default.clone().unwrap_or_else(ZoneSource::bundled)
}

/// Make `source` the process-wide default zone source
/// ([`default_zone_source`]) and return the one it replaces, so a caller
/// can put that back afterwards. Generators built from now on, and the
/// queries without a source, use `source`; a generator already built
/// keeps the source it read.
///
/// The default is shared by every test of the process, and tests run on
/// several threads, so set it once before any test runs (or in a test
/// binary of its own) rather than around one test.
pub fn set_default_zone_source(source: ZoneSource) -> ZoneSource {
    let mut default = DEFAULT_ZONE_SOURCE
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    source.handle.set_default();
    default.replace(source).unwrap_or_else(ZoneSource::bundled)
}

/// Builds a [`ZoneSource`]. Created by [`ZoneSource::builder`].
///
/// Each step is applied as it is called; the first one the engine rejects
/// is the error [`build`](Self::build) returns, and the steps after it are
/// skipped.
pub struct ZoneSourceBuilder {
    handle: ffi::ZoneSourceHandle,
    error: Option<Error>,
}

impl ZoneSourceBuilder {
    fn step(mut self, f: impl FnOnce(&ZoneLoader<'_>) -> Result<(), Error>) -> Self {
        if self.error.is_none() {
            let loader = ZoneLoader {
                target: self.handle.target(),
                _source: PhantomData,
            };
            self.error = f(&loader).err();
        }
        self
    }

    /// Add the zone `name` from a TZif file; see [`ZoneLoader::tzif`].
    pub fn tzif(self, name: &str, bytes: &[u8]) -> Self {
        self.step(|source| source.tzif(name, bytes))
    }

    /// Add the zone `name` from its data as fields; see
    /// [`ZoneLoader::zone`].
    pub fn zone(self, name: &str, data: &ZoneData) -> Self {
        self.step(|source| source.zone(name, data))
    }

    /// Add the zone `name` by probing `oracle` now; see
    /// [`ZoneLoader::oracle`].
    pub fn oracle(self, name: &str, oracle: ZoneOracle<'_>) -> Self {
        self.step(|source| source.oracle(name, oracle))
    }

    /// Make `name` a link to `target`; see [`ZoneLoader::link`].
    pub fn link(self, name: &str, target: &str) -> Self {
        self.step(|source| source.link(name, target))
    }

    /// Declare names whose data the [`loader`](Self::loader) supplies the
    /// first time a draw or query needs it, so a zone generator over the
    /// source can draw them before any is loaded.
    pub fn declare<S: Into<String>>(self, names: impl IntoIterator<Item = S>) -> Self {
        self.step(|source| source.declare(names))
    }

    /// Supply the data of a name the source lacks the first time a draw or
    /// query needs it: `loader` adds that name's zone or a link through the
    /// [`ZoneLoader`] it is given, or adds nothing when it does not know the
    /// name, which the source then remembers as unknown. It is called from
    /// whichever thread draws or queries, possibly concurrently and more
    /// than once for a name when threads race to load it, and must not
    /// draw. An `Err`, or a panic, fails the draw or query that needed the
    /// name as a usage error carrying its message. A second call replaces
    /// the loader. The loader, and what it captures, is dropped when it is
    /// replaced or when the source's last user is gone: every clone of the
    /// source, every generator drawing against it and the process default
    /// if it was made that.
    pub fn loader<F>(self, loader: F) -> Self
    where
        F: Fn(&ZoneLoader<'_>, &str) -> Result<(), Error> + Send + Sync + 'static,
    {
        self.handle
            .set_loader(Box::new(move |target: ffi::ZoneTarget, name: &str| {
                let source = ZoneLoader {
                    target,
                    _source: PhantomData,
                };
                loader(&source, name).map_err(|e| e.to_string())
            }));
        self
    }

    /// Record which release of its database the source holds, such as
    /// `"2025b"`. Failure output does not report it.
    pub fn version(self, version: &str) -> Self {
        self.handle.set_version(version);
        self
    }

    /// The source, or the error of the first step the engine rejected.
    pub fn build(self) -> Result<ZoneSource, Error> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(ZoneSource {
                handle: Arc::new(self.handle),
            }),
        }
    }
}
