//! Tests for `src/native/lark/lower.rs`: that groups and repetitions
//! become nonterminals with the right productions and provenance.

use super::*;
use crate::native::lark::builder::{NoImports, load_grammar};
use crate::native::lark::compile::compile;
use alloc::vec;

fn lowered(text: &str) -> (Grammar, Lowered) {
    let built = load_grammar(text, None, &NoImports).unwrap();
    let grammar = compile(&built, &["start"]).unwrap();
    let lowered = lower(&grammar);
    (grammar, lowered)
}

fn rhs_lens(lowered: &Lowered, lhs: NtId) -> Vec<usize> {
    lowered.by_lhs[lhs]
        .iter()
        .map(|&p| lowered.productions[p].rhs.len())
        .collect()
}

#[test]
fn rules_keep_their_alternatives() {
    let (grammar, lowered) = lowered("start: a \"x\" | b\na: \"a\"\nb: \"b\"\n");
    let start = grammar.rule_id("start").unwrap();
    let a = grammar.rule_id("a").unwrap();
    let b = grammar.rule_id("b").unwrap();
    let x = grammar.terminal_id("X").unwrap();
    assert_eq!(lowered.by_lhs.len(), grammar.rules.len());
    let prods: Vec<&Production> = lowered.by_lhs[start]
        .iter()
        .map(|&p| &lowered.productions[p])
        .collect();
    assert_eq!(
        prods,
        vec![
            &Production {
                lhs: start,
                rhs: vec![Symbol::Nonterminal(a), Symbol::Terminal(x)],
                provenance: Provenance::Rule { id: start, alt: 0 },
            },
            &Production {
                lhs: start,
                rhs: vec![Symbol::Nonterminal(b)],
                provenance: Provenance::Rule { id: start, alt: 1 },
            },
        ]
    );
}

#[test]
fn groups_become_nonterminals_with_one_production_per_alternative() {
    let (grammar, lowered) = lowered("start: (\"a\" | \"b\" \"c\") \"d\"\n");
    let start = grammar.rule_id("start").unwrap();
    let prod = &lowered.productions[lowered.by_lhs[start][0]];
    let Symbol::Nonterminal(group) = prod.rhs[0] else {
        panic!("{prod:?}");
    };
    assert_eq!(group, grammar.rules.len());
    let alts: Vec<(Provenance, usize)> = lowered.by_lhs[group]
        .iter()
        .map(|&p| {
            (
                lowered.productions[p].provenance,
                lowered.productions[p].rhs.len(),
            )
        })
        .collect();
    assert_eq!(
        alts,
        vec![
            (Provenance::Group { alt: 0 }, 1),
            (Provenance::Group { alt: 1 }, 2)
        ]
    );
}

#[test]
fn bounded_repeats_unroll_to_each_count() {
    let (grammar, lowered) = lowered("start: \"a\"~2..4 \"b\"? \"c\"~3\n");
    let start = grammar.rule_id("start").unwrap();
    let prod = &lowered.productions[lowered.by_lhs[start][0]];
    let repeats: Vec<NtId> = prod
        .rhs
        .iter()
        .map(|s| match s {
            Symbol::Nonterminal(nt) => *nt,
            Symbol::Terminal(_) => panic!("{prod:?}"),
        })
        .collect();
    assert_eq!(rhs_lens(&lowered, repeats[0]), vec![2, 3, 4]);
    assert_eq!(rhs_lens(&lowered, repeats[1]), vec![0, 1]);
    assert_eq!(rhs_lens(&lowered, repeats[2]), vec![3]);
    for nt in repeats {
        for &p in &lowered.by_lhs[nt] {
            assert_eq!(lowered.productions[p].provenance, Provenance::Repeat);
        }
    }
}

#[test]
fn unbounded_repeats_are_left_recursive() {
    let (grammar, lowered) = lowered("start: \"a\"* (\"b\" | \"c\")+\n");
    let start = grammar.rule_id("start").unwrap();
    let prod = &lowered.productions[lowered.by_lhs[start][0]];
    let Symbol::Nonterminal(star) = prod.rhs[0] else {
        panic!("{prod:?}");
    };
    let Symbol::Nonterminal(plus) = prod.rhs[1] else {
        panic!("{prod:?}");
    };
    let a = grammar.terminal_id("A").unwrap();
    let star_prods: Vec<&Production> = lowered.by_lhs[star]
        .iter()
        .map(|&p| &lowered.productions[p])
        .collect();
    assert_eq!(
        star_prods,
        vec![
            &Production {
                lhs: star,
                rhs: vec![],
                provenance: Provenance::Repeat,
            },
            &Production {
                lhs: star,
                rhs: vec![Symbol::Nonterminal(star), Symbol::Terminal(a)],
                provenance: Provenance::RepeatMore,
            },
        ]
    );
    let plus_prods: Vec<&Production> = lowered.by_lhs[plus]
        .iter()
        .map(|&p| &lowered.productions[p])
        .collect();
    assert_eq!(plus_prods.len(), 2);
    assert_eq!(plus_prods[0].rhs.len(), 1);
    assert_eq!(plus_prods[0].provenance, Provenance::Repeat);
    let Symbol::Nonterminal(group) = plus_prods[0].rhs[0] else {
        panic!("{plus_prods:?}");
    };
    assert_eq!(
        plus_prods[1].rhs,
        vec![Symbol::Nonterminal(plus), Symbol::Nonterminal(group)]
    );
    assert_eq!(plus_prods[1].provenance, Provenance::RepeatMore);
    assert_eq!(lowered.by_lhs[group].len(), 2);
}
