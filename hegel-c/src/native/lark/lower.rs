//! Lowering a compiled grammar's EBNF rules to the BNF productions a
//! parser works on, remembering where each production came from so that
//! a derivation can be read back in terms of the grammar's own rules,
//! groups and repetitions.
//!
//! Every [`Sequence`] of a rule becomes one production of that rule.
//! A parenthesised [`Item::Group`] becomes a fresh nonterminal with one
//! production per alternative, and an [`Item::Repeat`] a fresh
//! nonterminal producing exactly the counts it allows: one production of
//! each count up to a bound, or the minimum count plus a left-recursive
//! production adding one more when there is none. Unrolling a bound
//! takes a number of symbols quadratic in it, so a wide range becomes a
//! chain instead: a nonterminal for the minimum count, then one for each
//! further count allowed, producing the one before it with or without
//! one more item; whichever of the two takes fewer symbols is used. The
//! lowering makes no attempt to follow Lark's own EBNF expansion; it only
//! needs to accept the same strings.

use super::compile::{Grammar, Item, RuleId, Sequence, TermId};
use alloc::vec::Vec;

/// A nonterminal of the lowered grammar: the rules of the compiled
/// grammar, by [`RuleId`], followed by the fresh ones for groups and
/// repetitions.
pub type NtId = usize;

pub type ProdId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    Terminal(TermId),
    Nonterminal(NtId),
}

/// What a derivation by a production stands for in the grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Alternative `alt` of the rule `id`.
    Rule { id: RuleId, alt: usize },
    /// Alternative `alt` of a parenthesised group.
    Group { alt: usize },
    /// Some number of repetitions of a repeated item, one per symbol of
    /// the right-hand side.
    Repeat,
    /// The repetitions so far, as a nonterminal standing for fewer of
    /// them, followed by one more item or by nothing.
    RepeatMore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Production {
    pub lhs: NtId,
    pub rhs: Vec<Symbol>,
    pub provenance: Provenance,
}

/// The productions of a lowered grammar, indexed two ways.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lowered {
    pub productions: Vec<Production>,
    /// The productions of each nonterminal, by [`NtId`].
    pub by_lhs: Vec<Vec<ProdId>>,
}

struct Lowering {
    productions: Vec<Production>,
    nonterminals: usize,
}

impl Lowering {
    fn fresh(&mut self) -> NtId {
        self.nonterminals += 1;
        self.nonterminals - 1
    }

    fn sequence(&mut self, seq: &Sequence) -> Vec<Symbol> {
        seq.items.iter().map(|item| self.item(item)).collect()
    }

    fn item(&mut self, item: &Item) -> Symbol {
        match item {
            Item::Terminal(id) => Symbol::Terminal(*id),
            Item::Rule(id) => Symbol::Nonterminal(*id),
            Item::Group(alts) => {
                let lhs = self.fresh();
                for (alt, seq) in alts.iter().enumerate() {
                    let rhs = self.sequence(seq);
                    self.productions.push(Production {
                        lhs,
                        rhs,
                        provenance: Provenance::Group { alt },
                    });
                }
                Symbol::Nonterminal(lhs)
            }
            Item::Repeat { inner, min, max } => {
                let symbol = self.item(inner);
                match *max {
                    None => {
                        let lhs = self.fresh();
                        self.counts(lhs, symbol, *min, *min);
                        self.more(lhs, lhs, Some(symbol));
                        Symbol::Nonterminal(lhs)
                    }
                    Some(max) if unrolled_symbols(*min, max) <= chained_symbols(*min, max) => {
                        let lhs = self.fresh();
                        self.counts(lhs, symbol, *min, max);
                        Symbol::Nonterminal(lhs)
                    }
                    Some(max) => {
                        let mut lhs = self.fresh();
                        self.counts(lhs, symbol, *min, *min);
                        for _ in *min..max {
                            let next = self.fresh();
                            self.more(next, lhs, None);
                            self.more(next, lhs, Some(symbol));
                            lhs = next;
                        }
                        Symbol::Nonterminal(lhs)
                    }
                }
            }
        }
    }

    /// One production of `lhs` for each count from `min` to `max` copies
    /// of `symbol`.
    fn counts(&mut self, lhs: NtId, symbol: Symbol, min: u32, max: u32) {
        for count in min..=max {
            self.productions.push(Production {
                lhs,
                rhs: alloc::vec![symbol; count as usize],
                provenance: Provenance::Repeat,
            });
        }
    }

    /// A production of `lhs` deriving `fewer` repetitions and then
    /// `symbol`, when there is one.
    fn more(&mut self, lhs: NtId, fewer: NtId, symbol: Option<Symbol>) {
        let mut rhs = alloc::vec![Symbol::Nonterminal(fewer)];
        rhs.extend(symbol);
        self.productions.push(Production {
            lhs,
            rhs,
            provenance: Provenance::RepeatMore,
        });
    }
}

/// The right-hand-side symbols of a repetition of `min` to `max` items
/// unrolled to one production per count.
fn unrolled_symbols(min: u32, max: u32) -> u64 {
    (u64::from(min) + u64::from(max)) * (u64::from(max - min) + 1) / 2
}

/// The right-hand-side symbols of a repetition of `min` to `max` items
/// chained one count at a time.
fn chained_symbols(min: u32, max: u32) -> u64 {
    u64::from(min) + 3 * u64::from(max - min)
}

/// Lower `grammar` to BNF.
pub fn lower(grammar: &Grammar) -> Lowered {
    let mut lowering = Lowering {
        productions: Vec::new(),
        nonterminals: grammar.rules.len(),
    };
    for (id, rule) in grammar.rules.iter().enumerate() {
        for (alt, seq) in rule.alternatives.iter().enumerate() {
            let rhs = lowering.sequence(seq);
            lowering.productions.push(Production {
                lhs: id,
                rhs,
                provenance: Provenance::Rule { id, alt },
            });
        }
    }
    let productions = lowering.productions;
    let mut by_lhs = alloc::vec![Vec::new(); lowering.nonterminals];
    for (id, production) in productions.iter().enumerate() {
        by_lhs[production.lhs].push(id);
    }
    Lowered {
        productions,
        by_lhs,
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/lower_tests.rs"]
mod tests;
