//! Tests for `src/native/shrinker/grammar.rs`: the grammar-directed pass
//! on strings under a grammar.

use crate::exchange::drive_no_yield;
use crate::native::core::{ChoiceNode, ChoiceValue, Spans, StringChoice};
use crate::native::draws::{StringSpec, TextAlphabet};
use crate::native::lark::builder::{NoImports, load_grammar};
use crate::native::lark::compile::compile;
use crate::native::lark::constraint::GrammarConstraint;
use crate::native::shrinker::{ShrinkRun, Shrinker};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

fn cps(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}

fn text_of(value: &ChoiceValue) -> String {
    match value {
        ChoiceValue::String(v) => v.iter().map(|&cp| char::from_u32(cp).unwrap()).collect(),
        _ => unreachable!(),
    }
}

fn first_text(nodes: &[ChoiceNode]) -> Option<String> {
    match nodes.first()?.value() {
        ChoiceValue::String(v) => Some(v.iter().map(|&cp| char::from_u32(cp).unwrap()).collect()),
        _ => None,
    }
}

fn grammar_node(grammar_text: &str, simplest: &str, value: &str, forced: bool) -> ChoiceNode {
    grammar_node_with(grammar_text, &[], simplest, value, forced)
}

/// A string node under `grammar_text` whose `%declare`d terminals in
/// `explicit` match what the given generators draw.
fn grammar_node_with(
    grammar_text: &str,
    explicit: &[(&str, Arc<StringSpec>)],
    simplest: &str,
    value: &str,
    forced: bool,
) -> ChoiceNode {
    let built = load_grammar(grammar_text, None, &NoImports).unwrap();
    let grammar = compile(&built, &["start"]).unwrap();
    let start = grammar.start[0];
    let explicit: Vec<_> = explicit
        .iter()
        .map(|(name, spec)| (grammar.terminal_id(name).unwrap(), Arc::clone(spec)))
        .collect();
    let constraint = GrammarConstraint::new(grammar, &explicit, start, 64, u32::MAX).unwrap();
    constraint
        .simplest_or_init(|| Ok::<_, crate::control::InternalError>(cps(simplest)))
        .unwrap();
    ChoiceNode::string(
        StringChoice {
            intervals: Arc::clone(constraint.intervals()),
            min_size: 0,
            max_size: 64,
            grammar: Some(Arc::new(constraint)),
        },
        cps(value),
        forced,
    )
}

fn shrinker_where(
    nodes: Vec<ChoiceNode>,
    interesting: impl Fn(&str) -> bool + Send + 'static,
) -> Shrinker<'static> {
    Shrinker::with_probe(
        Box::new(move |run: ShrinkRun<'_>| match run {
            ShrinkRun::Full(nodes) => (
                first_text(nodes).is_some_and(|s| interesting(&s)),
                nodes.to_vec(),
                Spans::new(),
            ),
            ShrinkRun::Probe { .. } => (false, Vec::new(), Spans::new()),
        }),
        nodes,
        Spans::new(),
    )
}

const INT_LIST: &str = "start: INT (\",\" INT)*\nINT: /[0-9]+/\n";
const NESTED: &str = "start: \"[\" (start (\",\" start)*)? \"]\" | \"x\"\n";

#[test]
fn an_int_list_shrinks_to_two_zeros() {
    let node = grammar_node(INT_LIST, "0", "12,3,45", false);
    let mut shrinker = shrinker_where(vec![node], |s| s.contains(','));
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "0,0");
}

#[test]
fn the_whole_shrink_reaches_the_same_answer() {
    let node = grammar_node(INT_LIST, "0", "12,3,45", false);
    let mut shrinker = shrinker_where(vec![node], |s| s.contains(','));
    drive_no_yield(shrinker.shrink()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "0,0");
}

#[test]
fn every_repetition_is_deleted_at_once_when_the_result_is_still_interesting() {
    let node = grammar_node(NESTED, "x", "[x,x,x]", false);
    let mut shrinker = shrinker_where(vec![node], |s| s.starts_with('['));
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "[]");
}

/// A string of up to three of `c`, or none at all.
fn up_to_three(c: char) -> Arc<StringSpec> {
    let alphabet = TextAlphabet {
        categories: Some(vec![]),
        include_characters: Some(String::from(c)),
        ..TextAlphabet::default()
    };
    Arc::new(StringSpec::text(&alphabet, 0, 3).unwrap())
}

/// X's text is deleted outright, Y's cannot be, and the empty X left
/// behind is passed over on the next round.
#[test]
fn a_leaf_is_deleted_when_the_grammar_still_accepts_the_text() {
    let node = grammar_node_with(
        "start: \"a\" X Y \"c\"\n%declare X Y\n",
        &[("X", up_to_three('x')), ("Y", up_to_three('y'))],
        "axc",
        "axyc",
        false,
    );
    let mut shrinker = shrinker_where(vec![node], |s| s.contains('y'));
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "ayc");
}

#[test]
fn a_rule_is_replaced_by_a_smaller_node_of_the_same_rule_inside_it() {
    let node = grammar_node(NESTED, "x", "[x,[x,[x,x]]]", false);
    let mut shrinker = shrinker_where(vec![node], |s| s.contains("[x,x]"));
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "[x,x]");
}

#[test]
fn repetitions_are_deleted_in_windows() {
    let node = grammar_node("start: \"a\"*\n", "", &"a".repeat(23), false);
    let mut shrinker = shrinker_where(vec![node], |s| s.len() >= 10);
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "a".repeat(10));
}

#[test]
fn ignored_text_is_cut_and_what_must_stay_is_shortened() {
    let grammar = "start: \"a\" \"b\" \"c\"\n%ignore WS\nWS: / +/\n";
    let node = grammar_node(grammar, "abc", "a   b  c", false);
    let mut shrinker = shrinker_where(vec![node], |s| s.len() >= 5);
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "a  bc");
    let node = grammar_node(grammar, "abc", "a   b  c", false);
    let mut shrinker = shrinker_where(vec![node], |s| s.contains("a "));
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "a bc");
}

#[test]
fn a_leaf_is_lowered_character_by_character() {
    let node = grammar_node("start: /[a-z]+/\n", "a", "zqz", false);
    let mut shrinker = shrinker_where(vec![node], |s| s.len() == 3);
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "aaa");
}

#[test]
fn a_rule_is_replaced_by_the_smallest_text_seen_for_it() {
    let node = grammar_node(NESTED, "x", "[[x,x],[x,x]]", false);
    let mut shrinker = shrinker_where(vec![node], |s| s.starts_with("[[x,x],"));
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "[[x,x],x]");
}

#[test]
fn a_string_the_grammar_does_not_derive_is_left_alone() {
    let node = grammar_node("start: \"a\"\n", "a", "zzz", false);
    let mut shrinker = shrinker_where(vec![node], |_| false);
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "zzz");
}

#[test]
fn a_forced_string_and_a_string_without_a_grammar_are_left_alone() {
    let forced = grammar_node(INT_LIST, "0", "12,3", true);
    let plain = ChoiceNode::string(
        StringChoice {
            intervals: Arc::new(
                crate::native::intervalsets::IntervalSet::new(vec![(b'a' as u32, b'z' as u32)])
                    .unwrap(),
            ),
            min_size: 0,
            max_size: 8,
            grammar: None,
        },
        cps("zz"),
        false,
    );
    let mut shrinker = shrinker_where(vec![forced, plain], |_| true);
    drive_no_yield(shrinker.shrink_grammar_strings()).unwrap();
    assert_eq!(text_of(&shrinker.current_nodes[0].value()), "12,3");
    assert_eq!(text_of(&shrinker.current_nodes[1].value()), "zz");
}
