//! Parsing a string against a compiled grammar: an Earley parser with a
//! dynamic lexer, accepting the strings `Lark(grammar, parser="earley")`
//! does with its default `dynamic` lexer, and recovering a derivation
//! tree for the shrinker to work on.
//!
//! The parser is a port of Lark's `xearley.py`: one Earley set per
//! character position, items keyed by production, dot and origin, with
//! Scott's held completions so that nullable rules complete into items
//! predicted after them. There is no tokeniser. At each position every
//! item waiting on a terminal matches that terminal's own regex there
//! (see [`Lexer::match_terminal`]), the advanced item is held back until
//! the column where the match ends, and every `%ignore` terminal is
//! matched there too, carrying the waiting items (and a completed start
//! rule) past the ignored text. A string is accepted when a completed
//! start production spanning all of it is in the last column, including
//! when it ends in ignored text. A terminal without a pattern (one
//! `%declare`d, or supplied by an explicit generator) never matches, as
//! does a terminal whose regex is too involved to run within the
//! matcher's budget; supporting explicit terminals here is a follow-up.
//!
//! The tree recovered is the first derivation found, with the grammar's
//! own structure: a node per rule alternative, per parenthesised group
//! and per repetition, and leaves for the terminals and for the ignored
//! stretches between them. Ambiguity and priorities play no part in
//! acceptance, so neither resolves which derivation is returned.

use super::compile::{Grammar, RuleId, TermId};
use super::lexsim::{Lexed, Lexer};
use super::lower::{Lowered, NtId, Provenance, Symbol, lower};
use crate::native::HashSet;
use alloc::vec::Vec;

/// A derivation of a string. The children of a [`Tree::Rule`] are the
/// items of the alternative's [`Sequence`](super::compile::Sequence) in
/// order, those of a [`Tree::Group`] the items of the chosen alternative,
/// and those of a [`Tree::Repeat`] one tree per repetition, each
/// interleaved with the [`Tree::Ignored`] leaves for ignored text lying
/// between them. The leaves of a tree, in order, cover the parsed text
/// exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tree {
    Rule {
        id: RuleId,
        alt: usize,
        children: Vec<Tree>,
    },
    Group {
        alt: usize,
        children: Vec<Tree>,
    },
    Repeat {
        children: Vec<Tree>,
    },
    /// A terminal's text, as character positions.
    Terminal {
        id: TermId,
        start: usize,
        end: usize,
    },
    /// A stretch of text matched by a `%ignore` terminal.
    Ignored {
        id: TermId,
        start: usize,
        end: usize,
    },
}

impl Tree {
    pub fn children(&self) -> &[Tree] {
        match self {
            Tree::Rule { children, .. }
            | Tree::Group { children, .. }
            | Tree::Repeat { children } => children,
            Tree::Terminal { .. } | Tree::Ignored { .. } => &[],
        }
    }

    /// The [`Tree::Terminal`] and [`Tree::Ignored`] leaves, in text order.
    pub fn leaves(&self) -> Vec<&Tree> {
        let mut leaves = Vec::new();
        let mut stack = alloc::vec![self];
        while let Some(tree) = stack.pop() {
            match tree {
                Tree::Terminal { .. } | Tree::Ignored { .. } => leaves.push(tree),
                _ => stack.extend(tree.children().iter().rev()),
            }
        }
        leaves
    }
}

/// Where a parse failed: the position past which no item could be
/// carried, or the end of the text when it ended before the start rule
/// was complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError {
    pub position: usize,
}

/// A parser for one start rule of a compiled grammar.
pub struct Parser {
    lowered: Lowered,
    start: NtId,
    ignore: Vec<TermId>,
}

#[derive(Clone, Copy)]
enum Back {
    Predicted,
    Token { pred: u32, term: u32 },
    Child { pred: u32, child: u32 },
    Ignored { pred: u32, term: u32 },
}

#[derive(Clone, Copy)]
struct Item {
    prod: u32,
    dot: u32,
    origin: u32,
    col: u32,
    back: Back,
}

#[derive(Default)]
struct Column {
    /// Items that are complete or wait on a nonterminal.
    active: Vec<u32>,
    /// Items waiting on a terminal, with that terminal.
    scan: Vec<(u32, TermId)>,
}

struct Delayed {
    item: u32,
    term: TermId,
    advance: bool,
}

enum Step {
    Leaf(Tree),
    Child(u32),
}

struct Frame {
    item: u32,
    steps: alloc::vec::IntoIter<Step>,
    children: Vec<Tree>,
}

struct Parse<'p> {
    parser: &'p Parser,
    items: Vec<Item>,
    columns: Vec<Column>,
    seen: HashSet<(u32, u32, u32, u32)>,
    /// Items advanced by a match or carried over ignored text, by the
    /// position where the match ends, and how many are still to arrive.
    delayed: Vec<Vec<Delayed>>,
    pending: usize,
    /// The completed empty item of each nonterminal in the column being
    /// worked on, with that column.
    held: Vec<Option<(u32, u32)>>,
    tried: Vec<(TermId, Lexed)>,
}

impl Parser {
    /// A parser for `grammar` from the rule `start`.
    pub fn new(grammar: &Grammar, start: RuleId) -> Parser {
        Parser {
            lowered: lower(grammar),
            start,
            ignore: grammar.ignore.clone(),
        }
    }

    /// Whether `text` is in the grammar, matching terminals with `lexer`
    /// (built from the same grammar's terminals).
    pub fn accepts(&self, lexer: &Lexer, text: &[char]) -> bool {
        self.parse(lexer, text).is_ok()
    }

    /// A derivation of `text`, matching terminals with `lexer` (built
    /// from the same grammar's terminals).
    pub fn parse(&self, lexer: &Lexer, text: &[char]) -> Result<Tree, ParseError> {
        let n = text.len();
        let mut parse = Parse {
            parser: self,
            items: Vec::new(),
            columns: (0..=n).map(|_| Column::default()).collect(),
            seen: HashSet::default(),
            delayed: (0..=n).map(|_| Vec::new()).collect(),
            pending: 0,
            held: alloc::vec![None; self.lowered.by_lhs.len()],
            tried: Vec::new(),
        };
        for &prod in &self.lowered.by_lhs[self.start] {
            parse.add(0, prod as u32, 0, 0, Back::Predicted);
        }
        for col in 0..=n {
            if col > 0 {
                parse.arrive(col);
            }
            parse.predict_and_complete(col);
            if col < n {
                parse.scan(col, lexer, text);
                if parse.pending == 0 {
                    return Err(ParseError { position: col });
                }
            }
        }
        let root = parse.columns[n]
            .active
            .iter()
            .copied()
            .find(|&idx| {
                let item = parse.items[idx as usize];
                let production = &self.lowered.productions[item.prod as usize];
                item.origin == 0
                    && production.lhs == self.start
                    && item.dot as usize == production.rhs.len()
            })
            .ok_or(ParseError { position: n })?;
        Ok(parse.build(root))
    }
}

impl Parse<'_> {
    fn next_symbol(&self, item: &Item) -> Option<Symbol> {
        self.parser.lowered.productions[item.prod as usize]
            .rhs
            .get(item.dot as usize)
            .copied()
    }

    fn add(&mut self, col: usize, prod: u32, dot: u32, origin: u32, back: Back) {
        if !self.seen.insert((col as u32, prod, dot, origin)) {
            return;
        }
        let item = Item {
            prod,
            dot,
            origin,
            col: col as u32,
            back,
        };
        let idx = self.items.len() as u32;
        self.items.push(item);
        match self.next_symbol(&item) {
            Some(Symbol::Terminal(term)) => self.columns[col].scan.push((idx, term)),
            _ => self.columns[col].active.push(idx),
        }
    }

    fn arrive(&mut self, col: usize) {
        let arrivals = core::mem::take(&mut self.delayed[col]);
        self.pending -= arrivals.len();
        for arrival in arrivals {
            let pred = self.items[arrival.item as usize];
            let term = arrival.term as u32;
            if arrival.advance {
                let back = Back::Token {
                    pred: arrival.item,
                    term,
                };
                self.add(col, pred.prod, pred.dot + 1, pred.origin, back);
            } else {
                let back = Back::Ignored {
                    pred: arrival.item,
                    term,
                };
                self.add(col, pred.prod, pred.dot, pred.origin, back);
            }
        }
    }

    fn predict_and_complete(&mut self, col: usize) {
        let parser = self.parser;
        let mut k = 0;
        while k < self.columns[col].active.len() {
            let idx = self.columns[col].active[k];
            k += 1;
            let item = self.items[idx as usize];
            let next = self.next_symbol(&item);
            if let Some(Symbol::Nonterminal(nt)) = next {
                for &prod in &parser.lowered.by_lhs[nt] {
                    self.add(col, prod as u32, 0, col as u32, Back::Predicted);
                }
                if let Some((held_col, child)) = self.held[nt] {
                    if held_col as usize == col {
                        let back = Back::Child { pred: idx, child };
                        self.add(col, item.prod, item.dot + 1, item.origin, back);
                    }
                }
            } else if next.is_none() {
                let lhs = parser.lowered.productions[item.prod as usize].lhs;
                if item.origin as usize == col
                    && self.held[lhs].is_none_or(|(held_col, _)| held_col as usize != col)
                {
                    self.held[lhs] = Some((col as u32, idx));
                }
                let origin = item.origin as usize;
                let originators = self.columns[origin].active.len();
                for j in 0..originators {
                    let pred = self.columns[origin].active[j];
                    let originator = self.items[pred as usize];
                    if self.next_symbol(&originator) == Some(Symbol::Nonterminal(lhs)) {
                        let back = Back::Child { pred, child: idx };
                        self.add(
                            col,
                            originator.prod,
                            originator.dot + 1,
                            originator.origin,
                            back,
                        );
                    }
                }
            }
        }
    }

    fn lex(&mut self, lexer: &Lexer, text: &[char], col: usize, term: TermId) -> Lexed {
        if let Some(&(_, lexed)) = self.tried.iter().find(|(tried, _)| *tried == term) {
            return lexed;
        }
        let lexed = lexer.match_terminal(term, text, col);
        self.tried.push((term, lexed));
        lexed
    }

    /// Where a match of `term` at `col` ends, when it matches some text.
    fn matched(&mut self, lexer: &Lexer, text: &[char], col: usize, term: TermId) -> Option<usize> {
        match self.lex(lexer, text, col, term) {
            Lexed::Token(_, end) if end > col => Some(end),
            _ => None,
        }
    }

    fn delay(&mut self, end: usize, item: u32, term: TermId, advance: bool) {
        self.delayed[end].push(Delayed {
            item,
            term,
            advance,
        });
        self.pending += 1;
    }

    fn scan(&mut self, col: usize, lexer: &Lexer, text: &[char]) {
        let parser = self.parser;
        self.tried.clear();
        let waiting = self.columns[col].scan.len();
        for j in 0..waiting {
            let (idx, term) = self.columns[col].scan[j];
            if let Some(end) = self.matched(lexer, text, col, term) {
                self.delay(end, idx, term, true);
            }
        }
        for &ignore in &parser.ignore {
            let Some(end) = self.matched(lexer, text, col, ignore) else {
                continue;
            };
            for j in 0..waiting {
                let (idx, _) = self.columns[col].scan[j];
                self.delay(end, idx, ignore, false);
            }
            for j in 0..self.columns[col].active.len() {
                let idx = self.columns[col].active[j];
                let item = self.items[idx as usize];
                let production = &parser.lowered.productions[item.prod as usize];
                if production.lhs == parser.start && item.dot as usize == production.rhs.len() {
                    self.delay(end, idx, ignore, false);
                }
            }
        }
    }

    fn provenance(&self, idx: u32) -> Provenance {
        self.parser.lowered.productions[self.items[idx as usize].prod as usize].provenance
    }

    /// The steps of the derivation of a complete item, in order: a leaf
    /// for each terminal and ignored stretch, and the completed item for
    /// each nonterminal. A repetition's chain of one-more productions is
    /// flattened into the steps of the whole repetition.
    fn steps(&self, idx: u32) -> Vec<Step> {
        let mut segments = Vec::new();
        let mut cur = idx;
        loop {
            let mut steps = Vec::new();
            let mut at = cur;
            loop {
                let item = &self.items[at as usize];
                let end = item.col as usize;
                match item.back {
                    Back::Predicted => break,
                    Back::Token { pred, term } => {
                        let start = self.items[pred as usize].col as usize;
                        let id = term as TermId;
                        steps.push(Step::Leaf(Tree::Terminal { id, start, end }));
                        at = pred;
                    }
                    Back::Ignored { pred, term } => {
                        let start = self.items[pred as usize].col as usize;
                        let id = term as TermId;
                        steps.push(Step::Leaf(Tree::Ignored { id, start, end }));
                        at = pred;
                    }
                    Back::Child { pred, child } => {
                        steps.push(Step::Child(child));
                        at = pred;
                    }
                }
            }
            steps.reverse();
            let head = match (self.provenance(cur), steps.first()) {
                (Provenance::RepeatMore, Some(Step::Child(child))) => Some(*child),
                _ => None,
            };
            segments.push(steps);
            match head {
                Some(child) => cur = child,
                None => break,
            }
        }
        let mut out = Vec::new();
        for (k, segment) in segments.into_iter().rev().enumerate() {
            out.extend(segment.into_iter().skip(usize::from(k > 0)));
        }
        out
    }

    fn frame(&self, item: u32) -> Frame {
        Frame {
            item,
            steps: self.steps(item).into_iter(),
            children: Vec::new(),
        }
    }

    fn node(&self, frame: Frame) -> Tree {
        let children = frame.children;
        match self.provenance(frame.item) {
            Provenance::Rule { id, alt } => Tree::Rule { id, alt, children },
            Provenance::Group { alt } => Tree::Group { alt, children },
            Provenance::Repeat | Provenance::RepeatMore => Tree::Repeat { children },
        }
    }

    fn build(&self, root: u32) -> Tree {
        let mut root_frame = self.frame(root);
        let mut stack: Vec<Frame> = Vec::new();
        loop {
            let frame = stack.last_mut().unwrap_or(&mut root_frame);
            match frame.steps.next() {
                Some(Step::Leaf(tree)) => frame.children.push(tree),
                Some(Step::Child(child)) => {
                    let next = self.frame(child);
                    stack.push(next);
                }
                None => match stack.pop() {
                    Some(done) => {
                        let tree = self.node(done);
                        stack
                            .last_mut()
                            .unwrap_or(&mut root_frame)
                            .children
                            .push(tree);
                    }
                    None => return self.node(root_frame),
                },
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/parse_tests.rs"]
mod tests;
