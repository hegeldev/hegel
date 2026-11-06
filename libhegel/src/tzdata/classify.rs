//! The feature tags of a zone, and the pruning of records that change
//! nothing: a port of `compute_zone_tags`, `name_tags` and
//! `prune_transitions` in `scripts/generate_tzdata.py`, whose docstring
//! defines each tag. Zone sources use it to tag a name the bundled
//! catalogue lacks (`libhegel/docs/zone-sources.md` §1), and the bundled
//! table's tags are what it computes for every bundled zone.
//!
//! The port follows the script step by step, with two changes that give
//! the same tags for any zone and bound the work for one a source made
//! up: the footer rule's transitions are listed from 1960 at the earliest
//! rather than from the year of a last explicit transition before that,
//! since the rule repeats every year and only its state from 1969 on is
//! looked at; and the per-year offsets of `base_change` are computed in
//! one pass rather than one scan per year.

use alloc::vec::Vec;

use super::{Footer, LocalTimeType, Tag, Tags, Zone};
use crate::native::draws::special::civil_from_days;
use crate::native::timezone::rule_instant;

const DAY: i64 = 86_400;
const HOUR: i32 = 3_600;

/// The areas of current `Area/Location` names; a name in any other area,
/// or with none, is legacy (except `UTC`).
const CURRENT_AREAS: [&str; 11] = [
    "Africa",
    "America",
    "Antarctica",
    "Arctic",
    "Asia",
    "Atlantic",
    "Australia",
    "Europe",
    "Indian",
    "Pacific",
    "Etc",
];

/// The first year whose footer-rule transitions are listed; see the module
/// docs.
const FIRST_RULE_YEAR: i64 = 1960;

/// A transition: from `instant` on, `after` applies in place of `before`.
#[derive(Clone, Copy, Debug)]
struct Change {
    instant: i64,
    before: LocalTimeType,
    after: LocalTimeType,
}

fn abbreviation(zone: &Zone, kind: LocalTimeType) -> Option<&str> {
    zone.abbreviations
        .get(kind.abbreviation)
        .map(|a| a.as_str())
}

/// Whether `a` and `b` are the same local time type: offset, flag and
/// abbreviation.
fn same_type(zone: &Zone, a: LocalTimeType, b: LocalTimeType) -> bool {
    a.utoff == b.utoff && a.is_dst == b.is_dst && abbreviation(zone, a) == abbreviation(zone, b)
}

/// Whether going from `before` to `after` is a transition: a change of
/// offset or abbreviation.
fn is_transition(zone: &Zone, before: LocalTimeType, after: LocalTimeType) -> bool {
    before.utoff != after.utoff || abbreviation(zone, before) != abbreviation(zone, after)
}

fn utc_year(instant: i64) -> i64 {
    i64::from(civil_from_days(instant.div_euclid(DAY)).year)
}

fn year_start(year: i64) -> i64 {
    let date = crate::native::draws::special::Date {
        year: year as i32,
        month: 1,
        day: 1,
    };
    crate::native::draws::special::days_from_civil(&date) * DAY
}

fn last_explicit_year(zone: &Zone) -> i64 {
    zone.transitions
        .last()
        .map_or(1969, |t| utc_year(t.instant))
}

/// The footer rule's two transitions in `year`, start then end.
fn footer_transitions(footer: &Footer, year: i64) -> impl Iterator<Item = Change> {
    let standard = footer.standard;
    footer
        .dst
        .map(|dst| {
            [
                Change {
                    instant: rule_instant(dst.start, year, standard.utoff),
                    before: standard,
                    after: dst.kind,
                },
                Change {
                    instant: rule_instant(dst.end, year, dst.kind.utoff),
                    before: dst.kind,
                    after: standard,
                },
            ]
        })
        .into_iter()
        .flatten()
}

/// The zone's transitions in instant order: the explicit ones that change
/// the offset or abbreviation, then the footer rule's from the year of the
/// last explicit transition through the second year after it (or through
/// 1972 when the explicit ones end before 1970).
fn zone_changes(zone: &Zone) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut previous = zone.initial;
    for t in &zone.transitions {
        if is_transition(zone, previous, t.kind) {
            changes.push(Change {
                instant: t.instant,
                before: previous,
                after: t.kind,
            });
        }
        previous = t.kind;
    }
    if let Some(footer) = &zone.footer {
        let last = zone.transitions.last().map(|t| t.instant);
        let last_year = last_explicit_year(zone);
        for year in last_year.max(FIRST_RULE_YEAR)..last_year.max(1970) + 3 {
            for change in footer_transitions(footer, year) {
                if last.is_none_or(|last| change.instant > last) {
                    changes.push(change);
                }
            }
        }
    }
    changes.sort_by_key(|c| c.instant);
    changes
}

/// The local time types in force at some instant in `start <= t < end`.
fn types_between(zone: &Zone, changes: &[Change], start: i64, end: i64) -> Vec<LocalTimeType> {
    let at = changes.partition_point(|c| c.instant <= start);
    let current = at.checked_sub(1).map_or(zone.initial, |i| changes[i].after);
    let mut types = Vec::from([current]);
    types.extend(
        changes[at..]
            .iter()
            .take_while(|c| c.instant < end)
            .map(|c| c.after),
    );
    types
}

fn lowest_offset_in_year(zone: &Zone, changes: &[Change], year: i64) -> i32 {
    types_between(zone, changes, year_start(year), year_start(year + 1))
        .iter()
        .map(|t| t.utoff)
        .min()
        .unwrap_or(zone.initial.utoff)
}

fn all_offsets(zone: &Zone) -> Vec<i32> {
    let mut offsets = Vec::from([zone.initial.utoff]);
    offsets.extend(zone.transitions.iter().map(|t| t.kind.utoff));
    if let Some(footer) = &zone.footer {
        offsets.push(footer.standard.utoff);
        offsets.extend(footer.dst.map(|dst| dst.kind.utoff));
    }
    offsets
}

/// The tags `zone`'s data earn, leaving out those of its name.
pub fn data_tags(zone: &Zone) -> Tags {
    let mut tags = Tags::EMPTY;
    let changes = zone_changes(zone);
    let recent_from = changes.partition_point(|c| c.instant < 0);
    let recent = &changes[recent_from..];
    let recent_types = types_between(zone, &changes, 0, i64::MAX);

    if recent.is_empty() {
        tags = tags.with(Tag::Fixed);
    }
    if recent_types.iter().all(|t| t.utoff % HOUR == 0) {
        tags = tags.with(Tag::WholeHour);
    }
    if recent_types
        .iter()
        .any(|t| t.utoff % HOUR != 0 && t.utoff % 60 == 0)
    {
        tags = tags.with(Tag::Fractional);
    }
    if all_offsets(zone).iter().any(|o| o % 60 != 0) {
        tags = tags.with(Tag::Seconds);
    }
    if let Some(dst) = zone.footer.and_then(|f| f.dst.map(|d| (f.standard, d))) {
        let delta = (dst.1.kind.utoff - dst.0.utoff).unsigned_abs();
        tags = tags.with(if delta == HOUR.unsigned_abs() {
            Tag::Dst
        } else {
            Tag::DstOther
        });
    }

    if changes.iter().any(|c| {
        (i64::from(c.after.utoff) - i64::from(c.before.utoff)).unsigned_abs() >= DAY.unsigned_abs()
    }) {
        tags = tags.with(Tag::WholeDay);
    }
    let mut per_year: Vec<i64> = Vec::new();
    for change in recent {
        let wall_before = change.instant + i64::from(change.before.utoff);
        let wall_after = change.instant + i64::from(change.after.utoff);
        if change.before.utoff != change.after.utoff {
            per_year.push(utc_year(wall_before));
        } else {
            tags = tags.with(Tag::SameOffset);
        }
        if wall_before.rem_euclid(DAY) == 0 || wall_after.rem_euclid(DAY) == 0 {
            tags = tags.with(Tag::Midnight);
        }
        if wall_before.div_euclid(DAY) != wall_after.div_euclid(DAY) {
            tags = tags.with(Tag::SpansMidnight);
        }
    }
    per_year.sort_unstable();
    if per_year.windows(3).any(|w| w[0] == w[2]) {
        tags = tags.with(Tag::Multi);
    }

    let mut previous = lowest_offset_in_year(zone, &changes, 1969);
    let mut year = 1970;
    let end = last_explicit_year(zone) + 2;
    while year < end {
        let current = lowest_offset_in_year(zone, &changes, year);
        if current != previous {
            tags = tags.with(Tag::BaseChange);
            break;
        }
        previous = current;
        let next = changes.get(changes.partition_point(|c| c.instant <= year_start(year)));
        year = match next {
            Some(c) if c.instant < year_start(year + 1) => year + 1,
            Some(c) => (year + 1).max(utc_year(c.instant)),
            None => end,
        };
    }

    if recent_types
        .iter()
        .any(|&t| abbreviation(zone, t) == Some("-00"))
    {
        tags = tags.with(Tag::Undefined);
    }
    tags
}

/// The tags a name earns by its spelling: `legacy` outside the current
/// areas (except `UTC`) and `inverted_sign` for the `Etc/GMT+5` family.
pub fn name_tags(name: &str) -> Tags {
    let mut tags = Tags::EMPTY;
    let area = name.split_once('/').map(|(area, _)| area);
    if name != "UTC" && !area.is_some_and(|area| CURRENT_AREAS.contains(&area)) {
        tags = tags.with(Tag::Legacy);
    }
    let inverted = name
        .strip_prefix("Etc/GMT")
        .and_then(|rest| rest.strip_prefix(['+', '-']))
        .is_some_and(|digits| {
            digits.starts_with(|c: char| ('1'..='9').contains(&c))
                && digits.bytes().all(|b| b.is_ascii_digit())
        });
    if inverted {
        tags = tags.with(Tag::InvertedSign);
    }
    tags
}

/// The tags a name the bundled catalogue lacks earns by its spelling:
/// those of [`name_tags`] without `legacy`, since the area of a name IANA
/// does not have says nothing about whether it is current.
pub fn uncatalogued_name_tags(name: &str) -> Tags {
    name_tags(name)
        .iter()
        .filter(|&tag| tag != Tag::Legacy)
        .fold(Tags::EMPTY, Tags::with)
}

fn union(a: Tags, b: Tags) -> Tags {
    b.iter().fold(a, Tags::with)
}

/// The tags of the zone `name` whose data is `zone`.
pub fn zone_tags(name: &str, zone: &Zone) -> Tags {
    union(data_tags(zone), name_tags(name))
}

/// The tags of the link `name` given the data tags of its target.
pub fn link_tags(name: &str, target_data_tags: Tags) -> Tags {
    union(target_data_tags.with(Tag::Link), name_tags(name))
}

/// The footer rule's type at `instant`, or `None` when no rule transition
/// precedes it within the two years before.
fn rule_type_at(footer: &Footer, instant: i64) -> Option<LocalTimeType> {
    let year = utc_year(instant);
    let mut changes: Vec<Change> = (year - 2..=year)
        .flat_map(|y| footer_transitions(footer, y))
        .collect();
    changes.sort_by_key(|c| c.instant);
    changes
        .iter()
        .take_while(|c| c.instant <= instant)
        .last()
        .map(|c| c.after)
}

/// Whether `record`, the zone's last record when `last`, repeats the type
/// in force before it, the kept record `preceding` or the initial type,
/// and changes no answer the evaluator gives.
fn is_redundant(
    zone: &Zone,
    record: &super::Transition,
    preceding: Option<&super::Transition>,
    last: bool,
) -> bool {
    let previous = preceding.map_or(zone.initial, |p| p.kind);
    if !same_type(zone, record.kind, previous) {
        return false;
    }
    if !last {
        return true;
    }
    let Some(footer) = &zone.footer else {
        return true;
    };
    if footer.dst.is_none() {
        return same_type(zone, footer.standard, record.kind);
    }
    let Some(preceding) = preceding else {
        return false;
    };
    let start = preceding.instant;
    let fires = (utc_year(start)..=utc_year(record.instant))
        .flat_map(|y| footer_transitions(footer, y))
        .any(|c| start < c.instant && c.instant <= record.instant);
    if fires {
        return false;
    }
    rule_type_at(footer, start).is_some_and(|t| same_type(zone, t, record.kind))
}

/// Drop the records that change nothing, as the generator script does
/// before writing the bundled table.
pub fn prune_transitions(zone: &mut Zone) {
    let count = zone.transitions.len();
    let mut kept: Vec<super::Transition> = Vec::with_capacity(count);
    for (index, record) in zone.transitions.iter().enumerate() {
        if !is_redundant(zone, record, kept.last(), index + 1 == count) {
            kept.push(*record);
        }
    }
    zone.transitions = kept;
}

#[cfg(test)]
#[path = "../../tests/embedded/tzdata/classify_tests.rs"]
mod tests;
