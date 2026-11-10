//! Zone sources (`libhegel/docs/zone-sources.md`): where the zone data a
//! time zone or zoned datetime draw classifies, targets and resolves
//! against comes from.
//!
//! A [`ZoneSource`] is either the bundled database or a custom source,
//! which knows a set of names, each holding a zone's data, a link to
//! another name, or nothing yet: a declared name whose data a
//! [`ZoneLoader`] supplies the first time it is needed. Data arrives in
//! the three shapes of §2: TZif bytes ([`ZoneSource::add_tzif`]),
//! structured data ([`ZoneSource::add_zone`]) and an offset oracle the
//! engine probes over a range of years ([`ZoneSource::add_oracle`]),
//! beyond which it holds the last offset or infers a yearly rule from the
//! last two years probed. Whatever the shape, the data is checked (every
//! offset within [`UTOFF_BOUND`], transitions strictly ascending, footer
//! rules valid), confined to the civil envelope (a transition before it
//! folds into the initial type, and one after it ends the zone's history
//! there), pruned of records that change nothing as the bundled table is,
//! and interned: identical zone data is stored once for the life of the
//! process, which is what lets the C ABI hand out its strings as static
//! ones, and bounds the memory to the distinct zones ever loaded.
//!
//! The bundled catalogue still names every zone it knows: a name it has
//! keeps its link and legacy flags, tags and rank whatever source supplies
//! its data, and a name it lacks is a real zone with the rank after every
//! catalogued name and the tags [`classify`] computes from its data.
//!
//! The process-wide default source ([`default_source`]) is the bundled
//! database until [`set_default_source`] replaces it; a time zone spec
//! reads it when it is built and when its source is reset, as the Unicode
//! version is read when a string generator is built.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::ffi::CString;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::CStr;
use core::fmt;
use core::hash::{Hash, Hasher};
use rustc_hash::FxHasher;

use crate::control::{InternalError, hegel_internal_unwrap};
use crate::native::HashMap;
use crate::native::core::EngineError;
use crate::native::draws::special::{
    Date, MAX_YEAR, MIN_YEAR, civil_from_days, days_from_civil, days_in_month,
};
use crate::native::timezone::{
    CStrings, MAX_INSTANT, MIN_INSTANT, TimeZone, UNKNOWN_NAME, abbreviation, footer_type_at,
    rule_instant,
};
use crate::sys::sync::{Lazy, Mutex};
use crate::tzdata::classify;
use crate::tzdata::tzif::{self, FooterRule, ZoneType};
use crate::tzdata::{
    self, DstRule, Footer, LocalTimeType, Name, RuleDate, RuleDay, Tag, Tags, UTOFF_BOUND, Zone,
};

const DAY: i64 = 86_400;
const HOUR: i32 = 3_600;

/// The step an oracle with no next-transition query is sampled at; two
/// changes closer together than this can be missed.
pub const PROBE_STEP: i64 = DAY;

/// The most transitions probing an oracle may find.
pub const MAX_PROBED_TRANSITIONS: usize = 100_000;

/// The most links a name may pass through to reach its data.
const MAX_LINK_DEPTH: usize = 16;

/// The most years back footer inference checks a candidate rule against.
const MAX_OBSERVED_YEARS: i64 = 28;

/// Zone data as the engine evaluates it, interned for the life of the
/// process with what the C ABI and the draws need of it.
#[derive(Debug)]
pub struct SourcedZone {
    pub zone: Zone,
    /// The tags [`classify::data_tags`] gives the data.
    pub data_tags: Tags,
    pub strings: CStrings,
}

/// What a zone has in force after its explicit transitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum After {
    /// The last transition's type, or the initial type, for ever.
    Hold,
    /// This rule, as a TZif footer gives it.
    Rule(FooterRule),
    /// A yearly rule inferred from the transitions of `last_year`, the last
    /// year the transitions cover, and the year before it, preferring the
    /// rule that also fits the most consecutive years before them; the
    /// last type held when they follow no such rule.
    Infer { last_year: i32 },
}

/// Structured zone data (§2, shape 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneData {
    /// The type in force before the first transition.
    pub initial: ZoneType,
    /// Strictly ascending by instant (Unix seconds).
    pub transitions: Vec<(i64, ZoneType)>,
    pub after: After,
}

/// The range an oracle is probed over: whole years `first_year` to
/// `last_year`, the type at the start of `first_year` holding before it
/// and, after `last_year`, the last type held or a rule inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeRange {
    pub first_year: i32,
    pub last_year: i32,
    pub infer: bool,
}

/// An offset oracle (§2, shape 3): the local time type in force at an
/// instant, and optionally the first instant after a given one at which
/// it changes, `None` when it never does again.
pub struct Oracle<'a> {
    pub offset_at: &'a mut dyn FnMut(i64) -> Result<ZoneType, EngineError>,
    pub next_transition: Option<&'a mut dyn FnMut(i64) -> Result<Option<i64>, EngineError>>,
}

/// Supplies data for names a source does not have yet: called with the
/// source and a name the first time a draw or query needs that name's
/// data, it adds the zone (or a link) to the source, or adds nothing when
/// it does not know the name. It may be called from any thread, and more
/// than once for one name when threads race.
pub trait ZoneLoader: Send + Sync {
    fn load(&self, source: &Arc<ZoneSource>, name: &str) -> Result<(), EngineError>;
}

impl<F> ZoneLoader for F
where
    F: Fn(&Arc<ZoneSource>, &str) -> Result<(), EngineError> + Send + Sync,
{
    fn load(&self, source: &Arc<ZoneSource>, name: &str) -> Result<(), EngineError> {
        self(source, name)
    }
}

#[derive(Clone)]
enum Slot {
    Declared,
    Zone(&'static SourcedZone),
    Link(String),
    Missing,
}

struct Custom {
    names: Mutex<BTreeMap<String, Slot>>,
    loader: Mutex<Option<Arc<dyn ZoneLoader>>>,
    version: Mutex<Option<&'static CStr>>,
}

/// A source of zone data; see the module docs.
pub struct ZoneSource {
    custom: Option<Custom>,
}

impl fmt::Debug for ZoneSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.custom {
            None => f.write_str("ZoneSource(bundled)"),
            Some(custom) => f
                .debug_struct("ZoneSource")
                .field("names", &custom.names.lock().len())
                .finish(),
        }
    }
}

/// What a source has for a name.
#[derive(Clone, Copy, Debug)]
pub enum Lookup {
    /// The zone, ready to evaluate.
    Zone(TimeZone),
    /// A name the source knows but has no data for and no loader to ask.
    Unloaded,
    /// A name the source does not know.
    Unknown,
}

/// What a source knows about a name, for the draws' ladder and the info
/// query.
#[derive(Clone, Debug)]
pub struct Description {
    /// The name whose data applies: a link's target, or the name itself.
    pub primary: String,
    pub is_link: bool,
    pub tags: Tags,
    /// The catalogue's entry for the name, which fixes its flags, tags and
    /// rank; `None` for a name the catalogue lacks.
    pub catalogued: Option<&'static Name>,
}

impl Description {
    /// The name's shrink rank: the catalogue's, or [`uncatalogued_rank`].
    pub fn rank(&self) -> Result<u16, InternalError> {
        match self.catalogued {
            Some(name) => Ok(name.rank),
            None => uncatalogued_rank(),
        }
    }
}

/// The rank of every name the catalogue lacks: the one after its last.
pub fn uncatalogued_rank() -> Result<u16, InternalError> {
    Ok(hegel_internal_unwrap!(
        u16::try_from(tzdata::names()?.len()).ok(),
        "the catalogue has more names than ranks"
    ))
}

fn invalid(message: String) -> EngineError {
    EngineError::InvalidArgument(message)
}

fn c_string(what: &str, s: &str) -> Result<CString, EngineError> {
    CString::new(s).map_err(|_| invalid(format!("{what} {s:?} contains a NUL byte")))
}

/// `s` as a NUL-terminated string valid for the life of the process,
/// stored once per distinct string; `what` names it in the error for a
/// string with a NUL byte.
pub fn intern_c_string(what: &str, s: &str) -> Result<&'static CStr, EngineError> {
    static STRINGS: Lazy<Mutex<BTreeMap<String, &'static CStr>>> =
        Lazy::new(|| Mutex::new(BTreeMap::new()));
    let mut strings = STRINGS.lock();
    if let Some(found) = strings.get(s) {
        return Ok(found);
    }
    let leaked: &'static CStr = Box::leak(c_string(what, s)?.into_boxed_c_str());
    strings.insert(s.to_string(), leaked);
    Ok(leaked)
}

fn intern(zone: Zone) -> Result<&'static SourcedZone, EngineError> {
    static INTERNED: Lazy<Mutex<HashMap<u64, Vec<&'static SourcedZone>>>> =
        Lazy::new(|| Mutex::new(HashMap::default()));
    let mut hasher = FxHasher::default();
    zone.hash(&mut hasher);
    let key = hasher.finish();
    let mut table = INTERNED.lock();
    let found = table
        .get(&key)
        .and_then(|bucket| bucket.iter().copied().find(|z| z.zone == zone));
    if let Some(found) = found {
        return Ok(found);
    }
    let strings = CStrings {
        primary_name: c_string("zone name", &zone.name)?,
        abbreviations: zone
            .abbreviations
            .iter()
            .map(|a| c_string("abbreviation", a))
            .collect::<Result<Vec<CString>, EngineError>>()?,
    };
    let data_tags = classify::data_tags(&zone);
    let interned: &'static SourcedZone = Box::leak(Box::new(SourcedZone {
        zone,
        data_tags,
        strings,
    }));
    table.entry(key).or_default().push(interned);
    Ok(interned)
}

fn year_start(year: i64) -> i64 {
    days_from_civil(&Date {
        year: year as i32,
        month: 1,
        day: 1,
    }) * DAY
}

fn utc_year(instant: i64) -> i64 {
    i64::from(civil_from_days(instant.div_euclid(DAY)).year)
}

fn check_rule_date(name: &str, rule: &RuleDate) -> Result<(), EngineError> {
    let valid = match rule.day {
        RuleDay::Julian(n) => (1..=365).contains(&n),
        RuleDay::ZeroBased(n) => n <= 365,
        RuleDay::MonthWeekDay {
            month,
            week,
            weekday,
        } => (1..=12).contains(&month) && (1..=5).contains(&week) && weekday <= 6,
    };
    if !valid || !(-167 * HOUR..=167 * HOUR).contains(&rule.time) {
        return Err(invalid(format!(
            "zone {name}: the rule date {rule:?} is out of range (RFC 9636 §3.3.1)"
        )));
    }
    Ok(())
}

/// The abbreviations `zone` uses, renumbered in the generator script's
/// order, so that no unused one is listed.
fn renumber(zone: &Zone) -> Zone {
    let raw = |kind: LocalTimeType| ZoneType {
        utoff: kind.utoff,
        is_dst: kind.is_dst,
        abbreviation: zone.abbreviations[kind.abbreviation].clone(),
    };
    let transitions: Vec<(i64, ZoneType)> = zone
        .transitions
        .iter()
        .map(|t| (t.instant, raw(t.kind)))
        .collect();
    let footer = zone.footer.map(|f| FooterRule {
        standard: raw(f.standard),
        dst: f.dst.map(|d| (raw(d.kind), d.start, d.end)),
    });
    tzif::build_zone(
        &zone.name,
        &raw(zone.initial),
        &transitions,
        footer.as_ref(),
    )
}

/// Check `zone`, confine it to the civil envelope and prune it; see the
/// module docs. A rule whose daylight-saving type is its standard type
/// changes nothing and is kept as the standard type alone, and a rule
/// that does not have the last explicit transition's type in force at
/// that transition is an argument error: the evaluator answers from the
/// rule from that instant on, so the two would disagree about it.
fn prepare(mut zone: Zone) -> Result<Zone, EngineError> {
    let name = zone.name.clone();
    let mut types: Vec<LocalTimeType> = Vec::from([zone.initial]);
    types.extend(zone.transitions.iter().map(|t| t.kind));
    if let Some(footer) = &mut zone.footer {
        types.push(footer.standard);
        if let Some(dst) = &footer.dst {
            types.push(dst.kind);
            check_rule_date(&name, &dst.start)?;
            check_rule_date(&name, &dst.end)?;
            let (a, b) = (dst.kind, footer.standard);
            let same = a.utoff == b.utoff
                && a.is_dst == b.is_dst
                && zone.abbreviations.get(a.abbreviation) == zone.abbreviations.get(b.abbreviation);
            if same {
                footer.dst = None;
            }
        }
    }
    if let Some(kind) = types
        .iter()
        .find(|t| t.utoff.unsigned_abs() >= UTOFF_BOUND.unsigned_abs())
    {
        return Err(invalid(format!(
            "zone {name}: the offset {} is not within {UTOFF_BOUND} seconds of UT",
            kind.utoff
        )));
    }
    if zone
        .transitions
        .windows(2)
        .any(|pair| pair[0].instant >= pair[1].instant)
    {
        return Err(invalid(format!(
            "zone {name}: transitions are not strictly ascending"
        )));
    }
    let early = zone
        .transitions
        .partition_point(|t| t.instant < MIN_INSTANT);
    if let Some(last_early) = early.checked_sub(1) {
        zone.initial = zone.transitions[last_early].kind;
        zone.transitions.drain(..early);
    }
    let late = zone
        .transitions
        .partition_point(|t| t.instant <= MAX_INSTANT);
    if late < zone.transitions.len() {
        zone.transitions.truncate(late);
        zone.footer = None;
    }
    classify::prune_transitions(&mut zone);
    let zone = renumber(&zone);
    if let (Some(footer), Some(last)) = (&zone.footer, zone.transitions.last()) {
        let ruled = footer_type_at(footer, last.instant)?;
        if ruled != last.kind {
            return Err(invalid(format!(
                "zone {name}: the rule after the last transition does not continue it: the \
                 transition at {} changes to {} (UT{:+}s), and the rule has {} (UT{:+}s) in \
                 force then",
                last.instant,
                abbreviation(&zone, last.kind)?,
                last.kind.utoff,
                abbreviation(&zone, ruled)?,
                ruled.utoff
            )));
        }
    }
    Ok(zone)
}

/// The zone `name` holding `data`, before [`prepare`].
fn zone_of_data(name: &str, data: &ZoneData) -> Result<Zone, EngineError> {
    let rule = match &data.after {
        After::Rule(rule) => Some(rule),
        _ => None,
    };
    let mut zone = tzif::build_zone(name, &data.initial, &data.transitions, rule);
    if let After::Infer { last_year } = data.after {
        if !(MIN_YEAR..=MAX_YEAR).contains(&last_year) {
            return Err(invalid(format!(
                "zone {name}: the last year {last_year} is outside [{MIN_YEAR}, {MAX_YEAR}]"
            )));
        }
        zone.footer = infer_footer(&zone, last_year);
    }
    Ok(zone)
}

/// The types `zone`'s explicit transitions change between.
struct Change {
    instant: i64,
    before: LocalTimeType,
    after: LocalTimeType,
}

/// The candidate rule days naming the date `days` (days since the epoch):
/// the last such weekday of its month, the n-th, then the Julian and
/// zero-based days of the year.
fn rule_days(days: i64) -> Vec<RuleDay> {
    let date = civil_from_days(days);
    let weekday = (days + 4).rem_euclid(7) as u8;
    let month_length = days_in_month(date.year, date.month);
    let mut list = Vec::new();
    if i64::from(date.day) + 7 > month_length {
        list.push(RuleDay::MonthWeekDay {
            month: date.month,
            week: 5,
            weekday,
        });
    }
    let week = (date.day - 1) / 7 + 1;
    if week < 5 {
        list.push(RuleDay::MonthWeekDay {
            month: date.month,
            week,
            weekday,
        });
    }
    let day_of_year = days - year_start(i64::from(date.year)).div_euclid(DAY);
    let leap = days_in_month(date.year, 2) == 29;
    if !(date.month == 2 && date.day == 29) {
        let julian = day_of_year + 1 - i64::from(leap && date.month > 2);
        list.push(RuleDay::Julian(julian as u16));
    }
    list.push(RuleDay::ZeroBased(day_of_year as u16));
    list
}

/// The rule date, read with `utoff`, that falls at the instants in
/// `observed` (one per year, the latest first) for the most consecutive
/// years, and at least for the latest two.
fn matching_rule(observed: &[i64], utoff: i32) -> Option<RuleDate> {
    let later = *observed.first()?;
    let wall = later + i64::from(utoff);
    let mut best: Option<(usize, RuleDate)> = None;
    for shift in [0, -1, 1] {
        let days = wall.div_euclid(DAY) + shift;
        let time = (wall - days * DAY) as i32;
        let year = utc_year(days * DAY);
        for day in rule_days(days) {
            let rule = RuleDate { day, time };
            let score = (0..)
                .zip(observed)
                .take_while(|&(back, &instant)| rule_instant(rule, year - back, utoff) == instant)
                .count();
            if score >= 2 && best.is_none_or(|(top, _)| score > top) {
                best = Some((score, rule));
            }
        }
    }
    best.map(|(_, rule)| rule)
}

/// The footer that continues `zone`'s transitions of `last_year` and the
/// year before it: no rule when neither year changes anything, a yearly
/// daylight-saving rule when both change between the same two types at
/// dates one rule names, and `None`, so that the last type holds, when
/// they follow no such rule.
fn infer_footer(zone: &Zone, last_year: i32) -> Option<Footer> {
    let mut previous = zone.initial;
    let mut changes: Vec<Change> = Vec::new();
    for t in &zone.transitions {
        if t.kind != previous {
            changes.push(Change {
                instant: t.instant,
                before: previous,
                after: t.kind,
            });
        }
        previous = t.kind;
    }
    let year = |y: i64| -> &[Change] {
        let from = changes.partition_point(|c| c.instant < year_start(y));
        let to = changes.partition_point(|c| c.instant < year_start(y + 1));
        &changes[from..to]
    };
    let (earlier, later) = (year(i64::from(last_year) - 1), year(i64::from(last_year)));
    if earlier.is_empty() && later.is_empty() {
        return Some(Footer {
            standard: previous,
            dst: None,
        });
    }
    let [a0, a1] = earlier else { return None };
    let [b0, b1] = later else { return None };
    let (x, y) = (b0.after, b1.after);
    let alternates = |c0: &Change, c1: &Change| {
        c0.before == c1.after
            && c1.before == c0.after
            && [x, y].contains(&c0.after)
            && [x, y].contains(&c1.after)
    };
    if x == y || !alternates(b0, b1) || !alternates(a0, a1) {
        return None;
    }
    let (standard, dst) = if x.is_dst != y.is_dst {
        if x.is_dst { (y, x) } else { (x, y) }
    } else if x.utoff != y.utoff {
        if x.utoff < y.utoff { (x, y) } else { (y, x) }
    } else {
        return None;
    };
    let mut into_dst = Vec::new();
    let mut into_standard = Vec::new();
    for back in 0..MAX_OBSERVED_YEARS {
        let list = year(i64::from(last_year) - back);
        let [c0, c1] = list else { break };
        if !alternates(c0, c1) {
            break;
        }
        for c in list {
            if c.after == dst {
                into_dst.push(c.instant);
            } else {
                into_standard.push(c.instant);
            }
        }
    }
    let start = matching_rule(&into_dst, standard.utoff)?;
    let end = matching_rule(&into_standard, dst.utoff)?;
    Some(Footer {
        standard,
        dst: Some(DstRule {
            kind: dst,
            start,
            end,
        }),
    })
}

/// Probe `oracle` over `range` into structured data; see [`Oracle`].
fn probe(name: &str, oracle: Oracle<'_>, range: ProbeRange) -> Result<ZoneData, EngineError> {
    let ProbeRange {
        first_year,
        last_year,
        infer,
    } = range;
    let years_valid = MIN_YEAR <= first_year
        && first_year <= last_year
        && last_year <= MAX_YEAR
        && (!infer || first_year < last_year);
    if !years_valid {
        return Err(invalid(format!(
            "zone {name}: the probe range {first_year} to {last_year} must be ordered, within \
             [{MIN_YEAR}, {MAX_YEAR}] and, to infer a rule, span two years"
        )));
    }
    let Oracle {
        offset_at,
        next_transition,
    } = oracle;
    let start = year_start(i64::from(first_year));
    let last = year_start(i64::from(last_year) + 1) - 1;
    let initial = offset_at(start)?;
    let mut transitions: Vec<(i64, ZoneType)> = Vec::new();
    let mut current = initial.clone();
    let too_many = || {
        invalid(format!(
            "zone {name}: probing found more than {MAX_PROBED_TRANSITIONS} transitions"
        ))
    };
    let mut push = |instant: i64, kind: ZoneType, current: &mut ZoneType| {
        if transitions.len() == MAX_PROBED_TRANSITIONS {
            return Err(too_many());
        }
        transitions.push((instant, kind.clone()));
        *current = kind;
        Ok(())
    };
    match next_transition {
        Some(next) => {
            let mut at = start;
            let mut reported = 0usize;
            while let Some(instant) = next(at)? {
                if instant <= at {
                    return Err(invalid(format!(
                        "zone {name}: the oracle's next transition after {at} is {instant}, \
                         which is not later"
                    )));
                }
                if instant > last {
                    break;
                }
                reported += 1;
                if reported > MAX_PROBED_TRANSITIONS {
                    return Err(too_many());
                }
                let kind = offset_at(instant)?;
                if kind != current {
                    push(instant, kind, &mut current)?;
                }
                at = instant;
            }
        }
        None => {
            let mut at = start;
            let mut target = (at + PROBE_STEP).min(last);
            while at < last {
                let kind = offset_at(target)?;
                if kind == current {
                    at = target;
                    target = (at + PROBE_STEP).min(last);
                    continue;
                }
                let (mut lo, mut hi, mut hi_kind) = (at, target, kind);
                while hi - lo > 1 {
                    let mid = lo + (hi - lo) / 2;
                    let kind = offset_at(mid)?;
                    if kind == current {
                        lo = mid;
                    } else {
                        (hi, hi_kind) = (mid, kind);
                    }
                }
                push(hi, hi_kind, &mut current)?;
                at = hi;
                if at == target {
                    target = (at + PROBE_STEP).min(last);
                }
            }
        }
    }
    Ok(ZoneData {
        initial,
        transitions,
        after: if infer {
            After::Infer { last_year }
        } else {
            After::Hold
        },
    })
}

fn check_name(name: &str) -> Result<(), EngineError> {
    if name.is_empty() || name == UNKNOWN_NAME || name.contains('\0') {
        return Err(invalid(format!(
            "{name:?} cannot name a zone in a zone source: a name is non-empty, has no NUL \
             byte and is not {UNKNOWN_NAME}"
        )));
    }
    Ok(())
}

impl ZoneSource {
    /// An empty custom source.
    pub fn new() -> ZoneSource {
        ZoneSource {
            custom: Some(Custom {
                names: Mutex::new(BTreeMap::new()),
                loader: Mutex::new(None),
                version: Mutex::new(None),
            }),
        }
    }

    /// The bundled database as a source.
    pub fn bundled() -> Arc<ZoneSource> {
        static BUNDLED: Lazy<Arc<ZoneSource>> = Lazy::new(|| Arc::new(ZoneSource { custom: None }));
        Arc::clone(&BUNDLED)
    }

    pub fn is_bundled(&self) -> bool {
        self.custom.is_none()
    }

    fn custom(&self) -> Result<&Custom, EngineError> {
        self.custom
            .as_ref()
            .ok_or_else(|| invalid("the bundled zone source cannot be changed".to_string()))
    }

    fn insert(&self, name: &str, slot: Slot) -> Result<(), EngineError> {
        check_name(name)?;
        self.custom()?.names.lock().insert(name.to_string(), slot);
        Ok(())
    }

    fn insert_zone(&self, zone: Zone) -> Result<(), EngineError> {
        let custom = self.custom()?;
        let name = zone.name.clone();
        check_name(&name)?;
        let interned = intern(prepare(zone)?)?;
        custom.names.lock().insert(name, Slot::Zone(interned));
        Ok(())
    }

    /// Add (or replace) the zone `name` from structured data.
    pub fn add_zone(&self, name: &str, data: &ZoneData) -> Result<(), EngineError> {
        self.custom()?;
        self.insert_zone(zone_of_data(name, data)?)
    }

    /// Add (or replace) the zone `name` from a TZif file.
    pub fn add_tzif(&self, name: &str, bytes: &[u8]) -> Result<(), EngineError> {
        self.custom()?;
        let zone = tzif::parse_tzif(name, bytes)
            .map_err(|e| invalid(format!("zone {name}: malformed TZif data: {e}")))?;
        self.insert_zone(zone)
    }

    /// Add (or replace) the zone `name` by probing `oracle` over `range`.
    pub fn add_oracle(
        &self,
        name: &str,
        oracle: Oracle<'_>,
        range: ProbeRange,
    ) -> Result<(), EngineError> {
        self.custom()?;
        check_name(name)?;
        let data = probe(name, oracle, range)?;
        self.insert_zone(zone_of_data(name, &data)?)
    }

    /// Make `name` a link to `target`, which need not be known yet.
    pub fn add_link(&self, name: &str, target: &str) -> Result<(), EngineError> {
        check_name(target)?;
        if name == target {
            return Err(invalid(format!("{name:?} cannot be a link to itself")));
        }
        self.insert(name, Slot::Link(target.to_string()))
    }

    /// Declare names whose data the loader supplies later; a name the
    /// source already has is left as it is.
    pub fn declare<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> Result<(), EngineError> {
        let custom = self.custom()?;
        let names: Vec<&str> = names.into_iter().collect();
        names.iter().try_for_each(|name| check_name(name))?;
        let mut table = custom.names.lock();
        for name in names {
            let slot = table.entry(name.to_string()).or_insert(Slot::Declared);
            if matches!(slot, Slot::Missing) {
                *slot = Slot::Declared;
            }
        }
        Ok(())
    }

    /// Set the loader, or with `None` remove it. The loader it replaces
    /// is dropped once no lookup is still calling it, and with no lock of
    /// the source held, since dropping it may run the host's code.
    pub fn set_loader(&self, loader: Option<Arc<dyn ZoneLoader>>) -> Result<(), EngineError> {
        let replaced = core::mem::replace(&mut *self.custom()?.loader.lock(), loader);
        drop(replaced);
        Ok(())
    }

    /// Record which release of its database the source holds. Failure
    /// output does not report it (§6).
    pub fn set_version(&self, version: &str) -> Result<(), EngineError> {
        let custom = self.custom()?;
        *custom.version.lock() = Some(intern_c_string("version", version)?);
        Ok(())
    }

    /// The release of the source's database: the bundled one's, what
    /// [`set_version`](Self::set_version) recorded, or empty.
    pub fn version(&self) -> &'static CStr {
        match &self.custom {
            None => tzdata::VERSION_CSTR,
            Some(custom) => custom.version.lock().unwrap_or(c""),
        }
    }

    /// Every name the source knows, sorted.
    pub fn names(&self) -> Result<Vec<String>, InternalError> {
        match &self.custom {
            None => Ok(tzdata::names()?.iter().map(|n| n.name.clone()).collect()),
            Some(custom) => Ok(custom
                .names
                .lock()
                .iter()
                .filter(|(_, slot)| !matches!(slot, Slot::Missing))
                .map(|(name, _)| name.clone())
                .collect()),
        }
    }

    fn slot(&self, name: &str) -> Option<Slot> {
        self.custom.as_ref()?.names.lock().get(name).cloned()
    }

    /// The zone `name` evaluates as, loading its data when the source has
    /// a loader and not the data; see [`Lookup`]. `Etc/Unknown` is the
    /// unknown zone in every source. A link to a name with no data, or a
    /// name that passes through more than 16 links, however many of them
    /// the loader supplied, is an argument error, as is a
    /// declared name the loader supplies no data for, and a loader's error
    /// is passed on.
    pub fn lookup(self: &Arc<Self>, name: &str) -> Result<Lookup, EngineError> {
        if name == UNKNOWN_NAME {
            return Ok(Lookup::Zone(TimeZone::Unknown));
        }
        let Some(custom) = &self.custom else {
            return Ok(TimeZone::lookup(name)?.map_or(Lookup::Unknown, Lookup::Zone));
        };
        let mut current = name.to_string();
        let mut asked: Vec<String> = Vec::new();
        let mut links = 0;
        loop {
            let slot = self.slot(&current);
            match slot {
                Some(Slot::Zone(zone)) => return Ok(Lookup::Zone(TimeZone::Sourced(zone))),
                Some(Slot::Link(target)) => {
                    links += 1;
                    if links > MAX_LINK_DEPTH {
                        return Err(invalid(format!(
                            "the zone source's name {name:?} passes through more than \
                             {MAX_LINK_DEPTH} links"
                        )));
                    }
                    current = target;
                    continue;
                }
                _ => {}
            }
            let loader = custom.loader.lock().clone();
            let ask = !asked.contains(&current) && !matches!(slot, Some(Slot::Missing));
            if let Some(loader) = loader.filter(|_| ask) {
                loader.load(self, &current)?;
                asked.push(current.clone());
                continue;
            }
            return match slot {
                Some(Slot::Declared) if asked.is_empty() => Ok(Lookup::Unloaded),
                Some(Slot::Declared) => Err(invalid(format!(
                    "the zone source's loader supplied no data for {current:?}"
                ))),
                _ if current == name => {
                    if !asked.is_empty() {
                        custom.names.lock().entry(current).or_insert(Slot::Missing);
                    }
                    Ok(Lookup::Unknown)
                }
                _ => Err(invalid(format!(
                    "the zone source's name {name:?} links to {current:?}, which it does not know"
                ))),
            };
        }
    }

    /// [`lookup`](Self::lookup) as the zone to evaluate: `None` for a name
    /// the source does not know, an argument error for one it has no data
    /// for.
    pub fn timezone(self: &Arc<Self>, name: &str) -> Result<Option<TimeZone>, EngineError> {
        match self.lookup(name)? {
            Lookup::Zone(zone) => Ok(Some(zone)),
            Lookup::Unknown => Ok(None),
            Lookup::Unloaded => Err(unloaded(name)),
        }
    }

    /// What the source knows about `name`, `None` when it does not know
    /// it; `Etc/Unknown` is the caller's to describe. A name the catalogue
    /// lacks has its data loaded, to compute its tags, when the source has
    /// a loader; without one its tags are those of its spelling alone.
    pub fn describe(self: &Arc<Self>, name: &str) -> Result<Option<Description>, EngineError> {
        if self.custom.is_none() {
            return Ok(tzdata::lookup(name)?.map(catalogued));
        }
        let known = matches!(
            self.slot(name),
            Some(Slot::Declared | Slot::Zone(_) | Slot::Link(_))
        );
        if !known && matches!(self.lookup(name)?, Lookup::Unknown) {
            return Ok(None);
        }
        if let Some(entry) = tzdata::lookup(name)? {
            return Ok(Some(catalogued(entry)));
        }
        let is_link = matches!(self.slot(name), Some(Slot::Link(_)));
        let (primary, data_tags) = match self.lookup(name)? {
            Lookup::Zone(TimeZone::Sourced(zone)) => (zone.zone.name.clone(), zone.data_tags),
            _ => (self.link_target(name), Tags::EMPTY),
        };
        let data_tags = if is_link {
            data_tags.with(Tag::Link)
        } else {
            data_tags
        };
        let tags = data_tags
            .iter()
            .fold(classify::uncatalogued_name_tags(name), Tags::with);
        Ok(Some(Description {
            primary,
            is_link,
            tags,
            catalogued: None,
        }))
    }

    /// The name at the end of `name`'s links, as far as the source has
    /// them.
    fn link_target(&self, name: &str) -> String {
        let mut current = name.to_string();
        for _ in 0..MAX_LINK_DEPTH {
            match self.slot(&current) {
                Some(Slot::Link(target)) => current = target,
                _ => break,
            }
        }
        current
    }
}

fn unloaded(name: &str) -> EngineError {
    invalid(format!(
        "the zone source has no data for {name:?}: it was declared, and the source has no loader; \
         add its data before drawing or querying it"
    ))
}

fn catalogued(entry: &'static Name) -> Description {
    Description {
        primary: entry.zone.name.clone(),
        is_link: entry.is_link(),
        tags: entry.tags,
        catalogued: Some(entry),
    }
}

impl Default for ZoneSource {
    fn default() -> Self {
        ZoneSource::new()
    }
}

static DEFAULT_SOURCE: Mutex<Option<Arc<ZoneSource>>> = Mutex::new(None);

/// The process-wide default source: the bundled database until
/// [`set_default_source`] sets another.
pub fn default_source() -> Arc<ZoneSource> {
    DEFAULT_SOURCE
        .lock()
        .clone()
        .unwrap_or_else(ZoneSource::bundled)
}

/// Set the process-wide default source; `None` restores the bundled one.
/// Specs built earlier keep the source they read. The source it replaces
/// is dropped with the lock released, since dropping the last reference
/// to a source drops its loader, which may run the host's code.
pub fn set_default_source(source: Option<Arc<ZoneSource>>) {
    let replaced = core::mem::replace(&mut *DEFAULT_SOURCE.lock(), source);
    drop(replaced);
}

#[cfg(test)]
#[path = "../../tests/embedded/native/zone_source_tests.rs"]
mod tests;
