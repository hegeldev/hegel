//! Tests for `src/native/lark/lower.rs`: that groups and repetitions
//! become nonterminals with the right productions and provenance.

use super::*;
use crate::native::lark::builder::{NoImports, load_grammar};
use crate::native::lark::compile::compile;
use crate::native::lark::lexsim::Lexer;
use crate::native::lark::parse::{Parser, Tree};
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

fn repeat_nonterminals(grammar: &Grammar, lowered: &Lowered) -> Vec<NtId> {
    let start = grammar.rule_id("start").unwrap();
    let prod = &lowered.productions[lowered.by_lhs[start][0]];
    prod.rhs
        .iter()
        .map(|s| match s {
            Symbol::Nonterminal(nt) => *nt,
            Symbol::Terminal(_) => panic!("{prod:?}"),
        })
        .collect()
}

#[test]
fn narrow_bounded_repeats_unroll_to_each_count() {
    let (grammar, lowered) = lowered("start: \"a\"~1..3 \"b\"? \"c\"~3 \"d\"~2..3\n");
    let repeats = repeat_nonterminals(&grammar, &lowered);
    assert_eq!(rhs_lens(&lowered, repeats[0]), vec![1, 2, 3]);
    assert_eq!(rhs_lens(&lowered, repeats[1]), vec![0, 1]);
    assert_eq!(rhs_lens(&lowered, repeats[2]), vec![3]);
    assert_eq!(rhs_lens(&lowered, repeats[3]), vec![2, 3]);
    for nt in repeats {
        for &p in &lowered.by_lhs[nt] {
            assert_eq!(lowered.productions[p].provenance, Provenance::Repeat);
        }
    }
}

#[test]
fn wide_bounded_repeats_chain_one_count_at_a_time() {
    let (grammar, lowered) = lowered("start: \"a\"~2..5\n");
    let a = grammar.terminal_id("A").unwrap();
    let top = repeat_nonterminals(&grammar, &lowered)[0];
    let base = top - 3;
    assert_eq!(
        lowered.by_lhs[base]
            .iter()
            .map(|&p| &lowered.productions[p])
            .collect::<Vec<_>>(),
        vec![&Production {
            lhs: base,
            rhs: vec![Symbol::Terminal(a); 2],
            provenance: Provenance::Repeat,
        }]
    );
    for lhs in base + 1..=top {
        assert_eq!(
            lowered.by_lhs[lhs]
                .iter()
                .map(|&p| &lowered.productions[p])
                .collect::<Vec<_>>(),
            vec![
                &Production {
                    lhs,
                    rhs: vec![Symbol::Nonterminal(lhs - 1)],
                    provenance: Provenance::RepeatMore,
                },
                &Production {
                    lhs,
                    rhs: vec![Symbol::Nonterminal(lhs - 1), Symbol::Terminal(a)],
                    provenance: Provenance::RepeatMore,
                },
            ]
        );
    }
}

#[test]
fn the_smaller_of_unrolling_and_chaining_is_chosen() {
    assert_eq!(unrolled_symbols(0, 1), 1);
    assert_eq!(chained_symbols(0, 1), 3);
    assert_eq!(unrolled_symbols(2, 3), 5);
    assert_eq!(chained_symbols(2, 3), 5);
    assert_eq!(unrolled_symbols(2, 4), 9);
    assert_eq!(chained_symbols(2, 4), 8);
    assert_eq!(unrolled_symbols(1 << 20, 1 << 20), 1 << 20);
    assert_eq!(
        unrolled_symbols(0, 1 << 20),
        (1u64 << 20) * ((1 << 20) + 1) / 2
    );
}

#[test]
fn huge_bounded_repeats_lower_linearly() {
    let (_, lowered) = lowered("start: \"a\"~0..20000 \"b\"~10000..20000\n");
    assert_eq!(
        lowered.productions.len(),
        1 + (1 + 2 * 20000) + (1 + 2 * 10000)
    );
    let symbols: usize = lowered.productions.iter().map(|p| p.rhs.len()).sum();
    assert_eq!(symbols, 2 + 3 * 20000 + 10000 + 3 * 10000);
}

#[test]
fn chained_repeats_parse_like_unrolled_ones() {
    let (grammar, _) = lowered("start: \"a\"~2..5 \"b\"~0..7\n");
    let lexer = Lexer::new(&grammar.terminals);
    let parser = Parser::new(&grammar, grammar.start[0]);
    let accepts = |text: &str| parser.accepts(&lexer, &text.chars().collect::<Vec<char>>());
    assert!(!accepts("a"));
    for n in 2..=5 {
        assert!(accepts(&"a".repeat(n)), "{n}");
    }
    assert!(!accepts("aaaaaa"));
    assert!(accepts("aabbbbbbb"));
    assert!(!accepts("aabbbbbbbb"));
    let tree = parser
        .parse(&lexer, &"aaaabb".chars().collect::<Vec<char>>())
        .unwrap();
    let Tree::Rule { children, .. } = &tree else {
        panic!("{tree:?}");
    };
    assert!(matches!(&children[0], Tree::Repeat { children } if children.len() == 4));
    assert!(matches!(&children[1], Tree::Repeat { children } if children.len() == 2));
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
