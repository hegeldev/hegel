//! Time zone names as a draw (`libhegel/docs/datetime.md` §6.1): one string
//! node whose constraint is the set of allowed names, so that a replay
//! checks membership and a shrink walks the rank ladder.
//!
//! A [`TimeZoneSpec`] is what a caller configures: the bundled names with
//! links on and legacy names off, or an explicit list, with or without
//! `Etc/Unknown`, and whether a name the database lacks is accepted as an
//! opaque zone the engine cannot evaluate. Every setter rebuilds the spec's
//! [`TimeZoneConstraint`], which lays the allowed names out on the shrink
//! ladder (bundled names by rank, then opaque names by name, then
//! `Etc/Unknown`) and samples them: half the time uniformly, so a rare link
//! name is reached, and otherwise by feature tag, each tag's names sharing
//! one unit of weight, so a zone with a skipped day is drawn about as often
//! as one with plain daylight saving. The constraint compares and hashes by
//! an identity hashed from the ladder, so two runs configuring the same set
//! agree on the kind.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::hash::{Hash, Hasher};
use rustc_hash::FxHasher;

use super::{Draws, LABEL_TIMEZONE, spanned};
use crate::control::{InternalError, hegel_internal_unwrap};
use crate::native::HashMap;
use crate::native::core::{Constraint, EngineError, NativeTestCase, StringChoice};
use crate::native::intervalsets::IntervalSet;
use crate::native::timezone::{TimeZone, UNKNOWN_NAME};
use crate::native::zone_source::{Description, Lookup, ZoneSource, default_source};
use crate::tzdata::{self, Name, Tag, Tags};

/// What an allowed name evaluates as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    /// A zone of the spec's source, whose data the source supplies when
    /// the name is drawn.
    Zone,
    /// A name the source lacks, which evaluates like `Etc/Unknown`.
    Opaque,
    /// `Etc/Unknown`.
    Unknown,
}

/// One allowed name: the name as the caller or the source spells it,
/// what it evaluates as and its tags.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    codepoints: Vec<u32>,
    pub kind: EntryKind,
    pub tags: Tags,
}

/// The allowed names of a time zone draw as a string constraint; see the
/// module docs.
pub struct TimeZoneConstraint {
    ladder: Vec<Entry>,
    positions: HashMap<Vec<u32>, usize>,
    cumulative: Vec<f64>,
    identity: u64,
}

impl TimeZoneConstraint {
    fn new(ladder: Vec<Entry>) -> TimeZoneConstraint {
        let mut hasher = FxHasher::default();
        for entry in &ladder {
            entry.name.hash(&mut hasher);
        }
        let positions = ladder
            .iter()
            .enumerate()
            .map(|(i, entry)| (entry.codepoints.clone(), i))
            .collect();
        let cumulative = tag_weights(&ladder)
            .into_iter()
            .scan(0.0, |total, weight| {
                *total += weight;
                Some(*total)
            })
            .collect();
        TimeZoneConstraint {
            ladder,
            positions,
            cumulative,
            identity: hasher.finish(),
        }
    }

    /// Whether `text` is an allowed name.
    pub fn accepts(&self, text: &[u32]) -> bool {
        self.positions.contains_key(text)
    }

    /// The lowest-ranked allowed name.
    pub fn simplest(&self) -> &[u32] {
        &self.ladder[0].codepoints
    }

    /// The ladder position of `text`, or `None` when it is not allowed.
    pub fn position(&self, text: &[u32]) -> Option<usize> {
        self.positions.get(text).copied()
    }

    /// The allowed names in ladder order.
    pub fn ladder(&self) -> &[Entry] {
        &self.ladder
    }

    /// The name at ladder position `i`.
    pub fn name_at(&self, i: usize) -> &[u32] {
        &self.ladder[i].codepoints
    }

    pub fn identity(&self) -> u64 {
        self.identity
    }

    /// Draw a ladder position: uniform with probability one half, and
    /// otherwise by feature tag.
    pub(crate) fn sample<D: Draws>(&self, d: &mut D) -> Result<usize, EngineError> {
        let last = self.ladder.len() - 1;
        if d.weighted(0.5, None)? {
            return Ok(d.draw_integer_uniform(0, last as u64)? as usize);
        }
        let total = self.cumulative[last];
        let fraction = d.draw_integer_uniform(0, u64::MAX)? as f64 / 18_446_744_073_709_551_616.0;
        let target = fraction * total;
        Ok(self
            .cumulative
            .partition_point(|&weight| weight <= target)
            .min(last))
    }
}

/// Each name's sampling weight: every tag present in the set is worth one
/// unit shared evenly among the names carrying it, and the names with no
/// tag (opaque ones) share one unit between them.
fn tag_weights(ladder: &[Entry]) -> Vec<f64> {
    let mut counts = [0usize; Tag::ALL.len()];
    let mut untagged = 0usize;
    for entry in ladder {
        if entry.tags.is_empty() {
            untagged += 1;
        }
        for tag in entry.tags.iter() {
            counts[tag as usize] += 1;
        }
    }
    ladder
        .iter()
        .map(|entry| {
            if entry.tags.is_empty() {
                return 1.0 / untagged as f64;
            }
            entry
                .tags
                .iter()
                .map(|tag| 1.0 / counts[tag as usize] as f64)
                .sum()
        })
        .collect()
}

impl PartialEq for TimeZoneConstraint {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl Eq for TimeZoneConstraint {}

impl Hash for TimeZoneConstraint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
    }
}

impl fmt::Debug for TimeZoneConstraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimeZoneConstraint")
            .field("identity", &self.identity)
            .field("names", &self.ladder.len())
            .finish()
    }
}

/// The configuration of a time zone draw; see the module docs. Built with
/// [`TimeZoneSpec::new`] and changed with the setters, each of which
/// rebuilds the constraint and reports, without `opaque`, an explicit name
/// the source lacks, leaving the spec as it was. A spec whose source has
/// no name the link and legacy flags allow is an error only when drawn, so
/// a frontend can attach a source and then set an explicit list.
#[derive(Clone, Debug)]
pub struct TimeZoneSpec {
    names: Option<Vec<String>>,
    links: bool,
    legacy: bool,
    unknown: bool,
    opaque: bool,
    source: Arc<ZoneSource>,
    kind: Arc<StringChoice>,
}

impl TimeZoneSpec {
    /// Every name of the process's default zone source with links on and
    /// legacy names off, without `Etc/Unknown`.
    pub fn new() -> Result<TimeZoneSpec, EngineError> {
        let mut spec = TimeZoneSpec {
            names: None,
            links: true,
            legacy: false,
            unknown: false,
            opaque: false,
            source: default_source(),
            kind: Arc::new(StringChoice {
                intervals: Arc::new(IntervalSet::unicode_scalars()?),
                min_size: 0,
                max_size: 0,
                constraint: None,
            }),
        };
        spec.rebuild()?;
        Ok(spec)
    }

    /// Draw from `source`'s names and evaluate with its data, or with
    /// `None` from the process's default source as it is now.
    pub fn set_source(&mut self, source: Option<Arc<ZoneSource>>) -> Result<(), EngineError> {
        let source = source.unwrap_or_else(default_source);
        self.apply(|spec| spec.source = source)
    }

    /// The zone `entry` evaluates as, its data loaded from the spec's
    /// source when the source has a loader and not yet the data; an
    /// argument error when the source has no data for it.
    pub fn zone_of(&self, entry: &Entry) -> Result<TimeZone, EngineError> {
        match self.lookup(entry)? {
            Lookup::Zone(zone) => Ok(zone),
            Lookup::Unloaded | Lookup::Unknown => Err(EngineError::InvalidArgument(format!(
                "the zone source has no data for the time zone {:?}: it was declared, and the \
                 source has no loader; add its data before drawing it",
                entry.name
            ))),
        }
    }

    /// What the spec's source has for `entry`; see [`Lookup`].
    pub fn lookup(&self, entry: &Entry) -> Result<Lookup, EngineError> {
        match entry.kind {
            EntryKind::Zone => self.source.lookup(&entry.name),
            EntryKind::Opaque | EntryKind::Unknown => Ok(Lookup::Zone(TimeZone::Unknown)),
        }
    }

    /// Restrict the draw to `names`, which the source must know unless
    /// opaque names are accepted; the link and legacy flags do not apply
    /// to an explicit list, and an empty list is an error whether or not
    /// `Etc/Unknown` is drawn.
    pub fn set_names(&mut self, names: Vec<String>) -> Result<(), EngineError> {
        if names.is_empty() {
            return Err(EngineError::InvalidArgument(
                "a time zone draw needs at least one allowed name".to_string(),
            ));
        }
        self.apply(|spec| spec.names = Some(names))
    }

    /// Whether the default set includes link names such as `Asia/Calcutta`.
    pub fn set_links(&mut self, links: bool) -> Result<(), EngineError> {
        self.apply(|spec| spec.links = links)
    }

    /// Whether the default set includes legacy names such as `EST5EDT`.
    pub fn set_legacy(&mut self, legacy: bool) -> Result<(), EngineError> {
        self.apply(|spec| spec.legacy = legacy)
    }

    /// Whether `Etc/Unknown` is drawn.
    pub fn set_unknown(&mut self, unknown: bool) -> Result<(), EngineError> {
        self.apply(|spec| spec.unknown = unknown)
    }

    /// Whether an explicit name the source lacks is accepted as an
    /// opaque zone, which evaluates like `Etc/Unknown`.
    pub fn set_opaque(&mut self, opaque: bool) -> Result<(), EngineError> {
        self.apply(|spec| spec.opaque = opaque)
    }

    /// Make `change` to a copy of the spec and keep the copy once it
    /// rebuilds, so that a rejected change leaves the spec as it was.
    fn apply(&mut self, change: impl FnOnce(&mut TimeZoneSpec)) -> Result<(), EngineError> {
        let mut candidate = self.clone();
        change(&mut candidate);
        candidate.rebuild()?;
        *self = candidate;
        Ok(())
    }

    /// The string choice every name drawn under this spec is recorded as.
    pub fn kind(&self) -> &Arc<StringChoice> {
        &self.kind
    }

    /// The constraint of [`Self::kind`].
    pub fn constraint(&self) -> Result<&Arc<TimeZoneConstraint>, InternalError> {
        Ok(hegel_internal_unwrap!(
            self.kind.constraint.as_ref().and_then(Constraint::timezone),
            "a time zone spec's kind is not under a time zone constraint"
        ))
    }

    fn rebuild(&mut self) -> Result<(), EngineError> {
        let mut catalogued: Vec<(u16, Entry)> = Vec::new();
        let mut uncatalogued: Vec<Entry> = Vec::new();
        let mut opaque: Vec<Entry> = Vec::new();
        let mut unknown = false;
        let mut push = |name: &str, description: Description| {
            let entry = entry(name, EntryKind::Zone, description.tags);
            match description.catalogued {
                Some(catalogued_name) => catalogued.push((catalogued_name.rank, entry)),
                None => uncatalogued.push(entry),
            }
        };
        match &self.names {
            None if self.source.is_bundled() => {
                for name in tzdata::names()? {
                    let skip = (name.tags.contains(Tag::Legacy) && !self.legacy)
                        || (name.tags.contains(Tag::Link) && !self.links);
                    if !skip {
                        push(&name.name, catalogued_description(name));
                    }
                }
            }
            None => {
                for name in self.source.names()? {
                    let description = hegel_internal_unwrap!(
                        self.source.describe(&name)?,
                        "the zone source does not know its own name {name:?}"
                    );
                    let skip = (description.tags.contains(Tag::Legacy) && !self.legacy)
                        || (description.is_link && !self.links);
                    if !skip {
                        push(&name, description);
                    }
                }
            }
            Some(names) => {
                for name in names {
                    if name == UNKNOWN_NAME {
                        unknown = true;
                        continue;
                    }
                    match self.source.describe(name)? {
                        Some(description) => push(name, description),
                        None if self.opaque => {
                            opaque.push(entry(name, EntryKind::Opaque, Tags::EMPTY));
                        }
                        None => {
                            return Err(EngineError::InvalidArgument(format!(
                                "unknown time zone {name:?}; names are matched exactly as the \
                                 IANA database spells them, plus Etc/Unknown"
                            )));
                        }
                    }
                }
            }
        }
        catalogued.sort_by_key(|(rank, _)| *rank);
        catalogued.dedup_by(|a, b| a.1.name == b.1.name);
        let mut ladder: Vec<Entry> = catalogued.into_iter().map(|(_, entry)| entry).collect();
        for mut group in [uncatalogued, opaque] {
            group.sort_by(|a, b| a.name.cmp(&b.name));
            group.dedup_by(|a, b| a.name == b.name);
            ladder.extend(group);
        }
        if unknown || self.unknown {
            ladder.push(entry(
                UNKNOWN_NAME,
                EntryKind::Unknown,
                Tags::EMPTY.with(Tag::Undefined),
            ));
        }
        let max_size = ladder.iter().map(|e| e.codepoints.len()).max().unwrap_or(0);
        self.kind = Arc::new(StringChoice {
            intervals: Arc::clone(&self.kind.intervals),
            min_size: 0,
            max_size,
            constraint: Some(Constraint::TimeZone(Arc::new(TimeZoneConstraint::new(
                ladder,
            )))),
        });
        Ok(())
    }
}

fn entry(name: &str, kind: EntryKind, tags: Tags) -> Entry {
    Entry {
        name: name.to_string(),
        codepoints: name.chars().map(u32::from).collect(),
        kind,
        tags,
    }
}

fn catalogued_description(name: &'static Name) -> Description {
    Description {
        primary: String::new(),
        is_link: name.is_link(),
        tags: name.tags,
        catalogued: Some(name),
    }
}

/// Draw an allowed name as one string node, answering its entry.
pub(crate) fn draw_timezone<'a>(
    ntc: &mut NativeTestCase,
    spec: &'a TimeZoneSpec,
) -> Result<&'a Entry, EngineError> {
    let constraint = spec.constraint()?;
    if constraint.ladder().is_empty() {
        return Err(EngineError::InvalidArgument(
            "a time zone draw has no allowed name: the zone source has none the link and \
             legacy flags allow"
                .to_string(),
        ));
    }
    let text = ntc.draw_string_from(Arc::clone(spec.kind()), |d| {
        let i = constraint.sample(d)?;
        Ok(constraint.name_at(i).to_vec())
    })?;
    let codepoints: Vec<u32> = text.chars().map(u32::from).collect();
    let i = hegel_internal_unwrap!(
        constraint.position(&codepoints),
        "draw_timezone: the recorded name {text:?} is not an allowed one"
    );
    Ok(&constraint.ladder()[i])
}

/// Draw a time zone name allowed by `spec`, wrapped in a span. Shrinks
/// down the rank ladder to the lowest-ranked allowed name, `UTC` when it
/// is allowed.
pub fn generate_timezone(
    ntc: &mut NativeTestCase,
    spec: &TimeZoneSpec,
) -> Result<String, EngineError> {
    spanned(ntc, LABEL_TIMEZONE, |ntc| {
        draw_timezone(ntc, spec).map(|entry| entry.name.clone())
    })
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/draws/timezone_tests.rs"]
mod tests;
