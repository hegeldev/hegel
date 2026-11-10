//! Tests for `src/tzdata/classify.rs`: the port reproduces the generator
//! script's tags for every bundled name, and its pruning leaves the
//! (already pruned) bundled zones as they are.

use super::*;
use crate::tzdata::{RuleDate, RuleDay, Transition, names};
use std::string::ToString;
use std::vec;

fn union(a: Tags, b: Tags) -> Tags {
    b.iter().fold(a, Tags::with)
}

fn zone_tags(name: &str, zone: &Zone) -> Tags {
    union(data_tags(zone), name_tags(name))
}

fn link_tags(name: &str, target_data_tags: Tags) -> Tags {
    union(target_data_tags.with(Tag::Link), name_tags(name))
}

#[test]
fn the_port_reproduces_every_bundled_tag() {
    for name in names().unwrap() {
        let computed = if name.is_link() {
            link_tags(&name.name, data_tags(name.zone))
        } else {
            zone_tags(&name.name, name.zone)
        };
        assert_eq!(computed, name.tags, "{}", name.name);
    }
}

#[test]
fn pruning_leaves_the_bundled_zones_alone() {
    for name in names().unwrap() {
        let mut zone = name.zone.clone();
        prune_transitions(&mut zone);
        assert_eq!(&zone, name.zone, "{}", name.name);
    }
}

fn local(utoff: i32, is_dst: bool, abbreviation: usize) -> LocalTimeType {
    LocalTimeType {
        utoff,
        is_dst,
        abbreviation,
    }
}

fn london_like(transitions: Vec<Transition>) -> Zone {
    Zone {
        name: "Test/London".to_string(),
        abbreviations: vec!["GMT".to_string(), "BST".to_string()],
        initial: local(0, false, 0),
        transitions,
        footer: Some(Footer {
            standard: local(0, false, 0),
            dst: Some(super::super::DstRule {
                kind: local(3600, true, 1),
                start: RuleDate {
                    day: RuleDay::MonthWeekDay {
                        month: 3,
                        week: 5,
                        weekday: 0,
                    },
                    time: 3600,
                },
                end: RuleDate {
                    day: RuleDay::MonthWeekDay {
                        month: 10,
                        week: 5,
                        weekday: 0,
                    },
                    time: 7200,
                },
            }),
        }),
    }
}

#[test]
fn redundant_records_are_pruned() {
    let repeat = |instant| Transition {
        instant,
        kind: local(0, false, 0),
    };
    let mut zone = london_like(vec![repeat(100), repeat(200)]);
    prune_transitions(&mut zone);
    assert_eq!(zone.transitions, vec![repeat(200)]);
    let dst = Transition {
        instant: 828_234_000,
        kind: local(3600, true, 1),
    };
    let end = 846_378_000;
    let mut fires = london_like(vec![dst, repeat(end), repeat(900_000_000)]);
    prune_transitions(&mut fires);
    assert_eq!(fires.transitions.len(), 3);
    let mut quiet = london_like(vec![dst, repeat(end), repeat(end + 3600)]);
    prune_transitions(&mut quiet);
    assert_eq!(quiet.transitions, vec![dst, repeat(end)]);
    let mut fixed = london_like(vec![repeat(100)]);
    fixed.footer = Some(Footer {
        standard: local(0, false, 0),
        dst: None,
    });
    prune_transitions(&mut fixed);
    assert!(fixed.transitions.is_empty());
    let mut no_footer = london_like(vec![repeat(100)]);
    no_footer.footer = None;
    prune_transitions(&mut no_footer);
    assert!(no_footer.transitions.is_empty());
}

#[test]
fn name_tags_follow_the_spelling() {
    assert!(name_tags("UTC").is_empty());
    assert!(name_tags("Europe/Paris").is_empty());
    assert!(name_tags("EST5EDT").contains(Tag::Legacy));
    assert!(name_tags("Mars/Base").contains(Tag::Legacy));
    assert!(name_tags("Etc/GMT+5").contains(Tag::InvertedSign));
    assert!(name_tags("Etc/GMT-14").contains(Tag::InvertedSign));
    assert!(!name_tags("Etc/GMT+0").contains(Tag::InvertedSign));
    assert!(!name_tags("Etc/GMT+5x").contains(Tag::InvertedSign));
    assert!(!name_tags("Etc/GMT").contains(Tag::InvertedSign));
}

#[test]
fn uncatalogued_names_are_not_legacy_by_their_spelling() {
    assert!(uncatalogued_name_tags("Mars/Base").is_empty());
    assert!(uncatalogued_name_tags("EST5EDT").is_empty());
    assert!(uncatalogued_name_tags("Etc/GMT+77").contains(Tag::InvertedSign));
}

#[test]
fn a_far_future_base_change_is_found() {
    let zone = Zone {
        name: "Test/Far".to_string(),
        abbreviations: vec!["A".to_string(), "B".to_string()],
        initial: local(0, false, 0),
        transitions: vec![Transition {
            instant: 400_000_000_000,
            kind: local(3600, false, 1),
        }],
        footer: None,
    };
    assert!(data_tags(&zone).contains(Tag::BaseChange));
}
