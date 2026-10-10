//! Tests for `src/native/re/constraint.rs`: which strings a regex
//! constraint accepts and what its identity depends on.

use super::*;
use crate::native::re::parser::parse_pattern;
use alloc::vec;

fn constraint(pattern: &str, fullmatch: bool) -> RegexConstraint {
    RegexConstraint::new(pattern, parse_pattern(pattern, 0).unwrap(), fullmatch, None)
}

fn cps(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}

#[test]
fn a_whole_match_constraint_accepts_exactly_the_strings_the_pattern_matches_whole() {
    let c = constraint("[a-z]+@[a-z]+", true);
    assert!(c.accepts(&cps("ab@cd")));
    assert!(!c.accepts(&cps("ab@cd!")));
    assert!(!c.accepts(&cps("!ab@cd")));
    assert!(!c.accepts(&cps("")));
    assert!(!c.accepts(&[0xD800]), "a surrogate is in no pattern");
}

#[test]
fn a_search_constraint_accepts_a_string_the_pattern_matches_anywhere_in() {
    let c = constraint("a.c", false);
    assert!(c.accepts(&cps("abc")));
    assert!(c.accepts(&cps("xxaxcxx")));
    assert!(!c.accepts(&cps("ab")));
    assert!(!c.fullmatch());
}

#[test]
fn the_simplest_string_is_set_once() {
    let c = constraint("a+", true);
    assert_eq!(c.simplest(), None);
    assert_eq!(
        c.simplest_or_init(|| Ok::<_, ()>(cps("a"))).unwrap(),
        cps("a")
    );
    assert_eq!(
        c.simplest_or_init(|| Ok::<_, ()>(cps("aa"))).unwrap(),
        cps("a")
    );
    assert_eq!(c.simplest(), Some(cps("a").as_slice()));
}

#[test]
fn identity_follows_the_pattern_its_flags_the_match_mode_and_the_alphabet() {
    use core::hash::{BuildHasher, Hash};
    fn hash_of(value: &impl Hash) -> u64 {
        rustc_hash::FxBuildHasher.hash_one(value)
    }
    let whole = constraint("a+", true);
    assert_eq!(whole, constraint("a+", true));
    assert_eq!(whole.identity(), constraint("a+", true).identity());
    assert_eq!(hash_of(&whole), hash_of(&constraint("a+", true)));
    assert_ne!(whole, constraint("a+", false));
    assert_ne!(whole.identity(), constraint("a+", false).identity());
    assert_ne!(hash_of(&whole), hash_of(&constraint("a+", false)));
    assert_ne!(whole, constraint("b+", true));
    assert_ne!(
        whole,
        RegexConstraint::new("a+", parse_pattern("(?i)a+", 0).unwrap(), true, None)
    );
    let letters = IntervalSet::new(vec![(u32::from('a'), u32::from('z'))]).unwrap();
    assert_ne!(
        whole,
        RegexConstraint::new("a+", parse_pattern("a+", 0).unwrap(), true, Some(&letters))
    );
    assert!(alloc::format!("{whole:?}").contains("RegexConstraint"));
}
