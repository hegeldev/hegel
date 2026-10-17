//! Time zone names as a draw (`hegel-c/docs/datetime.md` §6.1): one string
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
use crate::tzdata::{self, Name, Tag, Tags};

/// One allowed name: the name as the caller or the database spells it,
/// the zone it evaluates as (`None` for an opaque name) and its tags.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    codepoints: Vec<u32>,
    pub zone: Option<TimeZone>,
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
/// rebuilds the constraint and reports a configuration with no name in it
/// or, without `opaque`, a name the database lacks.
#[derive(Clone, Debug)]
pub struct TimeZoneSpec {
    names: Option<Vec<String>>,
    links: bool,
    legacy: bool,
    unknown: bool,
    opaque: bool,
    kind: Arc<StringChoice>,
}

impl TimeZoneSpec {
    /// Every bundled name with links on and legacy names off, without
    /// `Etc/Unknown`.
    pub fn new() -> Result<TimeZoneSpec, EngineError> {
        let mut spec = TimeZoneSpec {
            names: None,
            links: true,
            legacy: false,
            unknown: false,
            opaque: false,
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

    /// Restrict the draw to `names`, which the database must know unless
    /// opaque names are accepted; the link and legacy flags do not apply
    /// to an explicit list.
    pub fn set_names(&mut self, names: Vec<String>) -> Result<(), EngineError> {
        self.names = Some(names);
        self.rebuild()
    }

    /// Whether the default set includes link names such as `Asia/Calcutta`.
    pub fn set_links(&mut self, links: bool) -> Result<(), EngineError> {
        self.links = links;
        self.rebuild()
    }

    /// Whether the default set includes legacy names such as `EST5EDT`.
    pub fn set_legacy(&mut self, legacy: bool) -> Result<(), EngineError> {
        self.legacy = legacy;
        self.rebuild()
    }

    /// Whether `Etc/Unknown` is drawn.
    pub fn set_unknown(&mut self, unknown: bool) -> Result<(), EngineError> {
        self.unknown = unknown;
        self.rebuild()
    }

    /// Whether an explicit name the database lacks is accepted as an
    /// opaque zone, which evaluates like `Etc/Unknown`.
    pub fn set_opaque(&mut self, opaque: bool) -> Result<(), EngineError> {
        self.opaque = opaque;
        self.rebuild()
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
        let mut named: Vec<&'static Name> = Vec::new();
        let mut opaque: Vec<Entry> = Vec::new();
        let mut unknown = false;
        match &self.names {
            None => {
                for name in tzdata::names()? {
                    let skip = (name.tags.contains(Tag::Legacy) && !self.legacy)
                        || (name.tags.contains(Tag::Link) && !self.links);
                    if !skip {
                        named.push(name);
                    }
                }
            }
            Some(names) => {
                for name in names {
                    match TimeZone::lookup(name)? {
                        Some(TimeZone::Unknown) => unknown = true,
                        Some(TimeZone::Named(name)) => named.push(name),
                        None if self.opaque => opaque.push(Entry {
                            name: name.clone(),
                            codepoints: name.chars().map(u32::from).collect(),
                            zone: None,
                            tags: Tags::EMPTY,
                        }),
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
        named.sort_by_key(|name| name.rank);
        named.dedup_by(|a, b| a.name == b.name);
        let mut ladder: Vec<Entry> = named.into_iter().map(named_entry).collect();
        opaque.sort_by(|a, b| a.name.cmp(&b.name));
        opaque.dedup_by(|a, b| a.name == b.name);
        ladder.extend(opaque);
        if unknown || self.unknown {
            ladder.push(Entry {
                name: UNKNOWN_NAME.to_string(),
                codepoints: UNKNOWN_NAME.chars().map(u32::from).collect(),
                zone: Some(TimeZone::Unknown),
                tags: Tags::EMPTY.with(Tag::Undefined),
            });
        }
        if ladder.is_empty() {
            return Err(EngineError::InvalidArgument(
                "a time zone draw needs at least one allowed name".to_string(),
            ));
        }
        let max_size = ladder.iter().map(|e| e.codepoints.len()).max().unwrap();
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

fn named_entry(name: &'static tzdata::Name) -> Entry {
    Entry {
        name: name.name.clone(),
        codepoints: name.name.chars().map(u32::from).collect(),
        zone: Some(TimeZone::Named(name)),
        tags: name.tags,
    }
}

/// Draw an allowed name as one string node, answering its entry.
pub(crate) fn draw_timezone<'a>(
    ntc: &mut NativeTestCase,
    spec: &'a TimeZoneSpec,
) -> Result<&'a Entry, EngineError> {
    let constraint = spec.constraint()?;
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
