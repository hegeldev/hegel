//! Tests for `src/native/draws/timezone.rs`: which names a spec allows,
//! how the draw samples and records them, and how a recorded name replays.

use super::*;
use crate::native::core::{ChoiceValue, NativeTestCase};
use crate::native::draws::source::RngDraws;
use crate::native::rng::EngineRng;
use alloc::collections::BTreeSet;
use alloc::vec;

fn fresh_ntc(seed: u64) -> NativeTestCase {
    NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap()
}

fn cps(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}

fn names_of(spec: &TimeZoneSpec) -> Vec<String> {
    spec.constraint()
        .unwrap()
        .ladder()
        .iter()
        .map(|e| e.name.clone())
        .collect()
}

fn spec_of(names: &[&str]) -> TimeZoneSpec {
    let mut spec = TimeZoneSpec::new().unwrap();
    spec.set_names(names.iter().map(|s| s.to_string()).collect())
        .unwrap();
    spec
}

fn tags_of(name: &str) -> Tags {
    tzdata::lookup(name).unwrap().unwrap().tags
}

#[test]
fn the_default_set_is_every_bundled_name_with_links_and_without_legacy_names() {
    let spec = TimeZoneSpec::new().unwrap();
    let allowed: BTreeSet<String> = names_of(&spec).into_iter().collect();
    for name in tzdata::names().unwrap() {
        let expected = !name.tags.contains(Tag::Legacy);
        assert_eq!(allowed.contains(&name.name), expected, "{}", name.name);
    }
    assert!(allowed.contains("Asia/Calcutta"));
    assert!(!allowed.contains("EST5EDT"));
    assert!(!allowed.contains(UNKNOWN_NAME));
    assert_eq!(names_of(&spec)[0], "UTC");
    assert_eq!(spec.kind().min_size, 0);
    assert_eq!(
        spec.kind().max_size,
        allowed.iter().map(|n| n.chars().count()).max().unwrap()
    );
}

#[test]
fn the_ladder_is_the_rank_order() {
    let spec = TimeZoneSpec::new().unwrap();
    let ranks: Vec<u16> = spec
        .constraint()
        .unwrap()
        .ladder()
        .iter()
        .map(|e| match e.zone {
            Some(TimeZone::Named(name)) => name.rank,
            _ => unreachable!(),
        })
        .collect();
    assert!(ranks.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn the_flags_switch_links_legacy_names_and_the_unknown_zone() {
    let mut spec = TimeZoneSpec::new().unwrap();
    spec.set_links(false).unwrap();
    let allowed: BTreeSet<String> = names_of(&spec).into_iter().collect();
    assert!(!allowed.contains("Asia/Calcutta"));
    assert!(allowed.contains("Asia/Kolkata"));
    assert!(allowed.iter().all(|n| !tags_of(n).contains(Tag::Link)));
    spec.set_legacy(true).unwrap();
    let allowed: BTreeSet<String> = names_of(&spec).into_iter().collect();
    assert!(allowed.contains("EST5EDT"));
    assert!(!allowed.contains("US/Eastern"));
    spec.set_links(true).unwrap();
    assert!(names_of(&spec).contains(&"US/Eastern".to_string()));
    spec.set_unknown(true).unwrap();
    let names = names_of(&spec);
    assert_eq!(names.last().unwrap(), UNKNOWN_NAME);
    let unknown = spec.constraint().unwrap().ladder().last().unwrap();
    assert!(matches!(unknown.zone, Some(TimeZone::Unknown)));
    assert_eq!(unknown.tags, Tags::EMPTY.with(Tag::Undefined));
    spec.set_unknown(false).unwrap();
    assert_ne!(names_of(&spec).last().unwrap(), UNKNOWN_NAME);
}

#[test]
fn an_explicit_list_is_ranked_deduplicated_and_may_name_the_unknown_zone() {
    let spec = spec_of(&[
        "Europe/London",
        "UTC",
        "Etc/Unknown",
        "Europe/London",
        "Asia/Kolkata",
        "EST5EDT",
    ]);
    let names = names_of(&spec);
    assert_eq!(names[0], "UTC");
    assert_eq!(names.last().unwrap(), UNKNOWN_NAME);
    assert_eq!(names.len(), 5);
    let ranks: Vec<u16> = names[..4]
        .iter()
        .map(|n| tzdata::lookup(n).unwrap().unwrap().rank)
        .collect();
    assert!(ranks.windows(2).all(|w| w[0] < w[1]));
    let mut without_utc = spec_of(&["Europe/London", "America/Santiago"]);
    assert_eq!(names_of(&without_utc)[0], "Europe/London");
    assert_eq!(
        without_utc.constraint().unwrap().simplest(),
        cps("Europe/London").as_slice()
    );
    without_utc.set_unknown(true).unwrap();
    assert_eq!(names_of(&without_utc).last().unwrap(), UNKNOWN_NAME);
}

#[test]
fn an_unknown_name_is_an_error_unless_opaque_names_are_accepted() {
    let mut spec = TimeZoneSpec::new().unwrap();
    let err = spec
        .set_names(vec!["Mars/Olympus_Mons".to_string()])
        .unwrap_err();
    assert!(
        matches!(&err, EngineError::InvalidArgument(msg) if msg.contains("Mars/Olympus_Mons")),
        "{err:?}"
    );
    spec.set_opaque(true).unwrap();
    spec.set_names(vec![
        "Mars/Olympus_Mons".to_string(),
        "Europe/Paris".to_string(),
        "Mars/Olympus_Mons".to_string(),
        "Europe/Unknown_Place".to_string(),
        "UTC".to_string(),
    ])
    .unwrap();
    assert_eq!(
        names_of(&spec),
        vec![
            "UTC",
            "Europe/Paris",
            "Europe/Unknown_Place",
            "Mars/Olympus_Mons"
        ]
    );
    let opaque = &spec.constraint().unwrap().ladder()[3];
    assert!(opaque.zone.is_none());
    assert!(opaque.tags.is_empty());
    let err = spec.set_opaque(false).unwrap_err();
    assert!(matches!(err, EngineError::InvalidArgument(_)));
    let err = spec.set_names(Vec::new()).unwrap_err();
    assert!(
        matches!(&err, EngineError::InvalidArgument(msg) if msg.contains("at least one")),
        "{err:?}"
    );
    let mut spec = TimeZoneSpec::new().unwrap();
    spec.set_unknown(true).unwrap();
    let err = spec.set_names(Vec::new()).unwrap_err();
    assert!(
        matches!(&err, EngineError::InvalidArgument(msg) if msg.contains("at least one")),
        "{err:?}"
    );
    assert_ne!(names_of(&spec), vec![UNKNOWN_NAME]);
}

#[test]
fn a_rejected_change_leaves_the_spec_as_it_was() {
    let mut spec = spec_of(&["UTC", "Europe/London"]);
    let before = names_of(&spec);
    let kind = Arc::clone(spec.kind());
    let err = spec
        .set_names(vec!["Mars/Olympus_Mons".to_string()])
        .unwrap_err();
    assert!(matches!(err, EngineError::InvalidArgument(_)));
    assert_eq!(names_of(&spec), before);
    assert_eq!(spec.kind(), &kind);
    spec.set_links(false).unwrap();
    assert_eq!(names_of(&spec), before);
    let mut ntc = fresh_ntc(0);
    let name = generate_timezone(&mut ntc, &spec).unwrap();
    assert!(before.contains(&name), "{name}");
    spec.set_opaque(true).unwrap();
    spec.set_names(vec!["Mars/Olympus_Mons".to_string()])
        .unwrap();
    spec.set_opaque(false).unwrap_err();
    assert_eq!(names_of(&spec), vec!["Mars/Olympus_Mons"]);
    spec.set_names(vec!["UTC".to_string(), "Mars/Olympus_Mons".to_string()])
        .unwrap();
    assert_eq!(names_of(&spec), vec!["UTC", "Mars/Olympus_Mons"]);
    spec.set_unknown(true).unwrap();
    assert_eq!(
        names_of(&spec),
        vec!["UTC", "Mars/Olympus_Mons", UNKNOWN_NAME]
    );
}

#[test]
fn constraints_compare_by_their_ladder() {
    let a = TimeZoneSpec::new().unwrap();
    let b = TimeZoneSpec::new().unwrap();
    assert_eq!(a.constraint().unwrap(), b.constraint().unwrap());
    assert_eq!(a.kind(), b.kind());
    let mut c = TimeZoneSpec::new().unwrap();
    c.set_links(false).unwrap();
    assert_ne!(a.constraint().unwrap(), c.constraint().unwrap());
    assert_ne!(
        a.constraint().unwrap().identity(),
        c.constraint().unwrap().identity()
    );
    let debug = format!("{:?}", a.constraint().unwrap());
    assert!(debug.contains("TimeZoneConstraint"), "{debug}");
    assert!(debug.contains("names"), "{debug}");
    let mut set = crate::native::HashSet::default();
    assert!(set.insert(Arc::clone(a.constraint().unwrap())));
    assert!(!set.insert(Arc::clone(b.constraint().unwrap())));
    assert!(set.insert(Arc::clone(c.constraint().unwrap())));
}

#[test]
fn every_drawn_name_is_allowed_and_every_allowed_name_is_drawn() {
    let spec = spec_of(&["UTC", "Europe/London", "Asia/Calcutta", "Pacific/Apia"]);
    let mut seen = BTreeSet::new();
    for seed in 0..200 {
        let mut ntc = fresh_ntc(seed);
        let name = generate_timezone(&mut ntc, &spec).unwrap();
        assert!(spec.constraint().unwrap().accepts(&cps(&name)), "{name}");
        seen.insert(name);
        assert_eq!(ntc.nodes.len(), 1);
        let (sc, v) = ntc.nodes[0].data.as_string().unwrap();
        assert_eq!(sc, spec.kind().as_ref());
        assert!(sc.validate(v));
    }
    assert_eq!(seen.len(), 4);
}

#[test]
fn the_default_set_reaches_rare_features_and_rare_names() {
    let spec = TimeZoneSpec::new().unwrap();
    let mut whole_day = 0;
    let mut dst_other = 0;
    let mut seen = BTreeSet::new();
    let draws = 3000;
    for seed in 0..draws {
        let mut ntc = fresh_ntc(seed);
        let name = generate_timezone(&mut ntc, &spec).unwrap();
        let tags = tags_of(&name);
        whole_day += usize::from(tags.contains(Tag::WholeDay));
        dst_other += usize::from(tags.contains(Tag::DstOther));
        seen.insert(name);
    }
    let allowed = names_of(&spec).len();
    assert!(seen.len() > allowed / 2, "{} of {allowed}", seen.len());
    assert!((30..=400).contains(&whole_day), "{whole_day}");
    assert!((20..=300).contains(&dst_other), "{dst_other}");
}

#[test]
fn the_tag_weighted_sampler_shares_one_unit_per_tag() {
    let spec = spec_of(&["UTC", "Europe/London", "Pacific/Apia"]);
    let constraint = spec.constraint().unwrap();
    let weights = tag_weights(constraint.ladder());
    let mut by_tag = [0.0f64; Tag::ALL.len()];
    for (entry, weight) in constraint.ladder().iter().zip(&weights) {
        for tag in entry.tags.iter() {
            by_tag[tag as usize] += weight / entry.tags.len() as f64;
        }
    }
    let total: f64 = weights.iter().sum();
    let tags_present = by_tag.iter().filter(|w| **w > 0.0).count();
    assert!((total - tags_present as f64).abs() < 1e-9, "{total}");
    let mut opaque = TimeZoneSpec::new().unwrap();
    opaque.set_opaque(true).unwrap();
    opaque
        .set_names(vec!["A/B".to_string(), "C/D".to_string()])
        .unwrap();
    assert_eq!(
        tag_weights(opaque.constraint().unwrap().ladder()),
        vec![0.5, 0.5]
    );
}

#[test]
fn both_samplers_stay_within_the_ladder() {
    let spec = spec_of(&["UTC", "Europe/London", "Pacific/Apia"]);
    let constraint = spec.constraint().unwrap();
    let mut counts = [0usize; 3];
    for seed in 0..600 {
        let mut rng = EngineRng::seeded(seed);
        let mut draws = RngDraws::new(&mut rng, Default::default());
        let i = constraint.sample(&mut draws).unwrap();
        counts[i] += 1;
    }
    assert!(counts.iter().all(|&c| c > 50), "{counts:?}");
}

#[test]
fn a_stored_name_the_spec_no_longer_allows_is_punned_to_the_simplest() {
    let wide = spec_of(&["UTC", "Europe/London", "Pacific/Apia"]);
    let stored = ChoiceValue::String(cps("Pacific/Apia"));
    let mut ntc = NativeTestCase::for_choices(core::slice::from_ref(&stored), None, None);
    assert_eq!(generate_timezone(&mut ntc, &wide).unwrap(), "Pacific/Apia");
    let narrow = spec_of(&["Europe/London", "America/Santiago"]);
    assert!(!narrow.kind().validate(&cps("Pacific/Apia")));
    let mut ntc = NativeTestCase::for_choices(&[stored], None, None);
    assert_eq!(
        generate_timezone(&mut ntc, &narrow).unwrap(),
        "Europe/London"
    );
    assert_eq!(narrow.kind().unit().unwrap(), cps("Europe/London"));
}

#[test]
fn the_zone_node_sorts_by_ladder_position() {
    let spec = spec_of(&["UTC", "Pacific/Apia", "Europe/London"]);
    let node = |name: &str| {
        crate::native::core::ChoiceNode::string(spec.kind().as_ref().clone(), cps(name), false)
    };
    let (utc, london, apia) = (node("UTC"), node("Europe/London"), node("Pacific/Apia"));
    assert!(utc.sort_key_ref() < london.sort_key_ref());
    assert!(london.sort_key_ref() < apia.sort_key_ref());
    let stranger = node("Mars/Olympus_Mons");
    assert!(apia.sort_key_ref() < stranger.sort_key_ref());
}

#[test]
fn a_drawn_name_maps_back_to_its_entry() {
    let mut spec = TimeZoneSpec::new().unwrap();
    spec.set_opaque(true).unwrap();
    spec.set_names(vec!["UTC".to_string(), "Mars/Olympus_Mons".to_string()])
        .unwrap();
    spec.set_unknown(true).unwrap();
    let mut seen = BTreeSet::new();
    for seed in 0..100 {
        let mut ntc = fresh_ntc(seed);
        let entry = draw_timezone(&mut ntc, &spec).unwrap();
        match entry.name.as_str() {
            "UTC" => assert!(matches!(entry.zone, Some(TimeZone::Named(n)) if n.name == "UTC")),
            "Mars/Olympus_Mons" => assert!(entry.zone.is_none()),
            UNKNOWN_NAME => assert!(matches!(entry.zone, Some(TimeZone::Unknown))),
            other => panic!("{other}"),
        }
        seen.insert(entry.name.clone());
    }
    assert_eq!(seen.len(), 3);
}
