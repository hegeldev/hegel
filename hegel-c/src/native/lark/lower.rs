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
//! production adding one more when there is none. The lowering makes no
//! attempt to follow Lark's own EBNF expansion; it only needs to accept
//! the same strings.

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
    /// One more repetition: the repetition's nonterminal followed by its
    /// item.
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
                let lhs = self.fresh();
                let counts = match max {
                    Some(max) => *min..=*max,
                    None => *min..=*min,
                };
                for count in counts {
                    self.productions.push(Production {
                        lhs,
                        rhs: alloc::vec![symbol; count as usize],
                        provenance: Provenance::Repeat,
                    });
                }
                if max.is_none() {
                    self.productions.push(Production {
                        lhs,
                        rhs: alloc::vec![Symbol::Nonterminal(lhs), symbol],
                        provenance: Provenance::RepeatMore,
                    });
                }
                Symbol::Nonterminal(lhs)
            }
        }
    }
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
