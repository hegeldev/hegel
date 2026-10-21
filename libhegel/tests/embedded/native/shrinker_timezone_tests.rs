//! Tests for `src/native/shrinker/timezone.rs`: the ladder pass on strings
//! under a time zone set.

use crate::exchange::drive_no_yield;
use crate::native::core::{ChoiceNode, ChoiceValue, Spans};
use crate::native::draws::timezone::TimeZoneSpec;
use crate::native::shrinker::{ShrinkRun, Shrinker};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

fn cps(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}

fn text_of(node: &ChoiceNode) -> String {
    match node.value() {
        ChoiceValue::String(v) => v.iter().map(|&cp| char::from_u32(cp).unwrap()).collect(),
        _ => unreachable!(),
    }
}

fn spec(names: &[&str]) -> TimeZoneSpec {
    let mut spec = TimeZoneSpec::new().unwrap();
    spec.set_names(names.iter().map(|s| s.to_string()).collect())
        .unwrap();
    spec
}

fn zone_node(spec: &TimeZoneSpec, name: &str) -> ChoiceNode {
    ChoiceNode::string((**spec.kind()).clone(), cps(name), false)
}

fn shrinker(
    nodes: Vec<ChoiceNode>,
    interesting: impl Fn(&[String]) -> bool + Send + 'static,
) -> Shrinker<'static> {
    Shrinker::with_probe(
        Box::new(move |run: ShrinkRun<'_>| match run {
            ShrinkRun::Full(nodes) => {
                let names: Vec<String> = nodes
                    .iter()
                    .map(|n| match n.value() {
                        ChoiceValue::String(v) => {
                            v.iter().map(|&cp| char::from_u32(cp).unwrap()).collect()
                        }
                        _ => String::new(),
                    })
                    .collect();
                (interesting(&names), nodes.to_vec(), Spans::new())
            }
            ShrinkRun::Probe { .. } => (false, Vec::new(), Spans::new()),
        }),
        nodes,
        Spans::new(),
    )
}

const LADDER: [&str; 4] = ["UTC", "America/Matamoros", "Europe/London", "Etc/GMT+1"];

#[test]
fn the_simplest_name_is_tried_first() {
    let spec = spec(&LADDER);
    let mut s = shrinker(vec![zone_node(&spec, "Europe/London")], |_| true);
    drive_no_yield(s.shrink_timezones()).unwrap();
    assert_eq!(text_of(&s.current_nodes[0]), "UTC");
    assert_eq!(s.calls, 1);
}

#[test]
fn the_ladder_is_walked_down_to_the_lowest_name_that_still_fails() {
    let spec = spec(&LADDER);
    let mut s = shrinker(vec![zone_node(&spec, "Etc/GMT+1")], |names| {
        names[0] != "UTC"
    });
    drive_no_yield(s.shrink_timezones()).unwrap();
    assert_eq!(text_of(&s.current_nodes[0]), "America/Matamoros");
}

#[test]
fn a_name_at_the_bottom_of_the_ladder_is_left_alone() {
    let spec = spec(&LADDER);
    let mut s = shrinker(vec![zone_node(&spec, "UTC")], |_| true);
    drive_no_yield(s.shrink_timezones()).unwrap();
    assert_eq!(text_of(&s.current_nodes[0]), "UTC");
    assert_eq!(s.calls, 0);
}

#[test]
fn a_name_nothing_below_accepts_stays() {
    let spec = spec(&LADDER);
    let mut s = shrinker(vec![zone_node(&spec, "Etc/GMT+1")], |names| {
        names[0] == "Etc/GMT+1"
    });
    drive_no_yield(s.shrink_timezones()).unwrap();
    assert_eq!(text_of(&s.current_nodes[0]), "Etc/GMT+1");
    assert_eq!(s.calls, 3);
}

#[test]
fn strings_that_are_not_zone_names_are_skipped() {
    let spec = spec(&LADDER);
    let plain = ChoiceNode::string(
        crate::native::core::StringChoice {
            intervals: spec.kind().intervals.clone(),
            min_size: 0,
            max_size: 8,
            constraint: None,
        },
        cps("x"),
        false,
    );
    let mut s = shrinker(vec![plain, zone_node(&spec, "Etc/GMT+1")], |_| true);
    drive_no_yield(s.shrink_timezones()).unwrap();
    assert_eq!(text_of(&s.current_nodes[0]), "x");
    assert_eq!(text_of(&s.current_nodes[1]), "UTC");
}

#[test]
fn a_node_that_disappears_ends_the_pass() {
    let spec = spec(&LADDER);
    let nodes = vec![
        zone_node(&spec, "Europe/London"),
        zone_node(&spec, "Europe/London"),
    ];
    let mut s = Shrinker::with_probe(
        Box::new(|run: ShrinkRun<'_>| match run {
            ShrinkRun::Full(nodes) => (true, nodes[..1].to_vec(), Spans::new()),
            ShrinkRun::Probe { .. } => (false, Vec::new(), Spans::new()),
        }),
        nodes,
        Spans::new(),
    );
    drive_no_yield(s.shrink_timezones()).unwrap();
    assert_eq!(s.current_nodes.len(), 1);
    assert_eq!(text_of(&s.current_nodes[0]), "UTC");
}
