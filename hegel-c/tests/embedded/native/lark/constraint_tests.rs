//! Tests for `src/native/lark/constraint.rs`: acceptance through the
//! parser, the derivation memo and its capacity, and identity across
//! compilations.

use super::*;
use crate::native::lark::builder::{NoImports, load_grammar};
use crate::native::lark::compile::compile;
use core::hash::BuildHasher;

fn compiled(text: &str, start: &str) -> Grammar {
    let built = load_grammar(text, None, &NoImports).unwrap();
    compile(&built, &[start]).unwrap()
}

fn constraint(text: &str, start: &str, max_size: usize, simplest: &str) -> GrammarConstraint {
    let grammar = compiled(text, start);
    let start = grammar.start[0];
    GrammarConstraint::new(grammar, start, max_size, cps(simplest))
}

fn cps(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}

const AB: &str = "start: \"a\" \"b\"*\n";

#[test]
fn accepts_the_strings_the_grammar_derives_and_rejects_the_rest() {
    let c = constraint(AB, "start", 16, "a");
    assert!(c.accepts(&cps("a")));
    assert!(c.accepts(&cps("ab")));
    assert!(c.accepts(&cps("abbb")));
    assert!(!c.accepts(&cps("")));
    assert!(!c.accepts(&cps("b")));
    assert!(!c.accepts(&cps("ba")));
    assert!(!c.accepts(&cps("abc")));
    assert_eq!(c.simplest(), cps("a"));
    assert_eq!(c.max_size(), 16);
    assert_eq!(c.grammar().rules[c.start()].name, "start");
}

#[test]
fn a_codepoint_that_is_no_scalar_value_is_in_no_grammar() {
    let c = constraint("start: /./*\n", "start", 16, "");
    assert!(c.accepts(&cps("xyz")));
    assert!(!c.accepts(&[b'x' as u32, 0xD800]));
    assert_eq!(c.known_len(), 1);
}

#[test]
fn a_string_known_to_derive_is_not_parsed_again() {
    let c = constraint(AB, "start", 16, "a");
    let first = c.tree(&cps("abb")).unwrap();
    let again = c.tree(&cps("abb")).unwrap();
    assert!(Arc::ptr_eq(&first, &again));
    assert_eq!(c.known_len(), 1);
    assert!(c.accepts(&cps("abb")));
    assert_eq!(c.known_len(), 1);
    assert!(!c.accepts(&cps("bb")));
    assert_eq!(c.known_len(), 1);
}

#[test]
fn a_remembered_derivation_is_read_back_and_stands_for_acceptance() {
    let c = constraint(AB, "start", 16, "a");
    let tree = Arc::new(Tree::Repeat {
        children: Vec::new(),
    });
    c.remember(&cps("zzz"), Arc::clone(&tree));
    assert!(Arc::ptr_eq(&c.tree(&cps("zzz")).unwrap(), &tree));
    assert!(c.accepts(&cps("zzz")));
    assert!(!c.accepts(&cps("zz")));
    assert!(!c.accepts(&cps("zzzz")));
}

#[test]
fn the_memo_is_emptied_when_it_reaches_its_capacity() {
    let c = constraint(AB, "start", 16, "a");
    let tree = Arc::new(Tree::Repeat {
        children: Vec::new(),
    });
    for i in 0..KNOWN_CAPACITY as u32 {
        c.remember(&[i], Arc::clone(&tree));
    }
    assert_eq!(c.known_len(), KNOWN_CAPACITY);
    assert!(c.accepts(&[0]));
    c.remember(&[KNOWN_CAPACITY as u32], tree);
    assert_eq!(c.known_len(), 1);
    assert!(!c.accepts(&[0]));
    assert!(c.accepts(&[KNOWN_CAPACITY as u32]));
}

#[test]
fn constraints_compiled_from_the_same_grammar_are_the_same_kind() {
    let a = constraint(AB, "start", 16, "a");
    let b = constraint(AB, "start", 16, "a");
    assert_eq!(a, b);
    assert_eq!(a.identity(), b.identity());
    let hasher = rustc_hash::FxBuildHasher;
    assert_eq!(hasher.hash_one(&a), hasher.hash_one(&b));
}

#[test]
fn the_grammar_the_start_rule_and_the_size_cap_each_tell_constraints_apart() {
    let base = constraint(AB, "start", 16, "a");
    assert_ne!(base, constraint("start: \"a\" \"c\"*\n", "start", 16, "a"));
    assert_ne!(base, constraint(AB, "start", 32, "a"));
    let two = "start: \"a\" other\nother: \"b\"\n";
    let grammar = compiled(two, "start");
    let start = grammar.rule_id("start").unwrap();
    let other = grammar.rule_id("other").unwrap();
    let from_start = GrammarConstraint::new(grammar.clone(), start, 16, cps("ab"));
    let from_other = GrammarConstraint::new(grammar, other, 16, cps("b"));
    assert_ne!(from_start, from_other);
    assert!(from_start.accepts(&cps("ab")));
    assert!(from_other.accepts(&cps("b")));
    assert!(!from_other.accepts(&cps("ab")));
}

#[test]
fn debug_names_the_identity_and_the_start_rule() {
    let c = constraint(AB, "start", 16, "a");
    let shown = format!("{c:?}");
    assert_eq!(
        shown,
        format!(
            "GrammarConstraint {{ identity: {:016x}, start: \"start\" }}",
            c.identity()
        )
    );
}
