//! A simulation of Lark's lexers, used to check that the terminals the
//! generator concatenates would be lexed back into the same tokens: that
//! two adjacent tokens do not merge, that a keyword drawn for an
//! identifier terminal is not re-typed as the keyword, and that an
//! inserted `%ignore` string really is skipped.
//!
//! Two lexers are simulated. The dynamic lexer of Lark's default Earley
//! parser matches each terminal the parser expects, and each `%ignore`
//! terminal, with its own pattern at the current position; what it finds
//! is [`Lexer::match_terminal`], and a string every expected terminal
//! matches at its place is one Lark parses. Lark's `BasicLexer` (and the
//! contextual lexer of its LALR parser) instead sorts the terminals by
//! priority, then longest possible match, then longest pattern source,
//! then name, and takes the first that matches at the current position;
//! string terminals a regex terminal of the same priority matches
//! completely are removed from that order and recognised afterwards
//! instead (`_create_unless`): a token the regex terminal matched is
//! re-typed as the string terminal when the texts agree. That is
//! [`Lexer::next_token`], which the generator uses to prefer strings the
//! stricter lexers read back as intended.

use super::ast::PatternKind;
use super::compile::{TermId, TerminalDef};
use super::{GrammarError, GrammarResult};
use crate::native::re::constants::{MAXREPEAT, SRE_FLAG_IGNORECASE};
use crate::native::re::matcher::Matcher;
use crate::native::re::parser::{OpCode, ParsedPattern, SetItem, SubPattern, parse_pattern};
use alloc::format;
use alloc::vec::Vec;

/// The characters a pattern can start with, as sorted disjoint ranges;
/// `None` when the analysis gave up (so any character might start it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstChars(Vec<(u32, u32)>);

impl FirstChars {
    fn contains(&self, c: char) -> bool {
        let c = c as u32;
        let i = self.0.partition_point(|&(_, hi)| hi < c);
        self.0.get(i).is_some_and(|&(lo, _)| lo <= c)
    }

    fn normalized(mut ranges: Vec<(u32, u32)>) -> Self {
        ranges.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
        for (lo, hi) in ranges {
            match merged.last_mut() {
                Some(last) if lo <= last.1.saturating_add(1) => last.1 = last.1.max(hi),
                _ => merged.push((lo, hi)),
            }
        }
        FirstChars(merged)
    }
}

/// The result of walking a (sub)pattern for its first characters: the
/// ranges collected so far, and whether the walk is still at a point the
/// pattern could skip past without consuming anything.
struct FirstWalk {
    ranges: Vec<(u32, u32)>,
    any: bool,
}

impl FirstWalk {
    fn seq(&mut self, p: &SubPattern) -> bool {
        for op in &p.data {
            if !self.op(op) {
                return false;
            }
        }
        true
    }

    /// Add the first characters of `op`; returns whether `op` can match
    /// the empty string, in which case the walk continues to the next op.
    fn op(&mut self, op: &OpCode) -> bool {
        match op {
            OpCode::Literal(c) => {
                self.ranges.push((*c, *c));
                false
            }
            OpCode::NotLiteral(_) | OpCode::Any | OpCode::GroupRef(_) => {
                self.any = true;
                false
            }
            OpCode::In(items) => {
                for item in items {
                    match item {
                        SetItem::Literal(c) => self.ranges.push((*c, *c)),
                        SetItem::Range(lo, hi) => self.ranges.push((*lo, *hi)),
                        SetItem::Negate | SetItem::Category(_) => self.any = true,
                    }
                }
                false
            }
            OpCode::At(_) | OpCode::Assert { .. } | OpCode::AssertNot { .. } => true,
            OpCode::Failure => false,
            OpCode::Branch(items) => {
                let mut nullable = false;
                for item in items {
                    nullable |= self.seq(item);
                }
                nullable
            }
            OpCode::Subpattern { p, add_flags, .. } => {
                if add_flags & SRE_FLAG_IGNORECASE != 0 {
                    self.any = true;
                }
                self.seq(p)
            }
            OpCode::AtomicGroup(p) => self.seq(p),
            OpCode::GroupRefExists { yes, no, .. } => {
                let mut nullable = self.seq(yes);
                match no {
                    Some(no) => nullable |= self.seq(no),
                    None => nullable = true,
                }
                nullable
            }
            OpCode::MaxRepeat { min, item, .. }
            | OpCode::MinRepeat { min, item, .. }
            | OpCode::PossessiveRepeat { min, item, .. } => {
                let nullable = self.seq(item);
                *min == 0 || nullable
            }
        }
    }
}

/// The characters a match of `parsed` can start with, or `None` when any
/// character might (case-insensitive patterns, categories, references).
pub fn first_chars(parsed: &ParsedPattern) -> Option<FirstChars> {
    if parsed.flags & SRE_FLAG_IGNORECASE != 0 {
        return None;
    }
    let mut walk = FirstWalk {
        ranges: Vec::new(),
        any: false,
    };
    let nullable = walk.seq(&parsed.pattern);
    if walk.any || nullable {
        return None;
    }
    Some(FirstChars::normalized(walk.ranges))
}

struct LexTerminal {
    id: TermId,
    parsed: ParsedPattern,
    first: Option<FirstChars>,
    unless: Vec<(TermId, ParsedPattern)>,
}

/// What the simulated lexer found at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lexed {
    /// A token of the given terminal ending at the given position.
    Token(TermId, usize),
    /// No terminal matches here.
    NoMatch,
    /// The matcher ran out of budget: the text is too involved to check.
    Unknown,
}

/// The sorted terminal order of Lark's basic lexer and its keyword
/// re-typing, over the terminals of one grammar.
pub struct Lexer {
    terminals: Vec<LexTerminal>,
    patterns: Vec<Option<ParsedPattern>>,
}

fn parse_terminal(term: &TerminalDef, regexp: &str) -> GrammarResult<ParsedPattern> {
    parse_pattern(regexp, 0).map_err(|e| {
        GrammarError::new(format!("Cannot compile token {}: {regexp}: {e}", term.name))
    })
}

fn is_subset(flags: &str, of: &str) -> bool {
    flags.chars().all(|f| of.contains(f))
}

impl Lexer {
    /// Build the lexer over `terminals` (indexed by [`TermId`]); terminals
    /// without a pattern take no part.
    pub fn new(terminals: &[TerminalDef]) -> GrammarResult<Lexer> {
        let mut order: Vec<(TermId, &TerminalDef, ParsedPattern)> = Vec::new();
        let mut patterns = Vec::with_capacity(terminals.len());
        for (id, term) in terminals.iter().enumerate() {
            patterns.push(match &term.regexp {
                Some(regexp) => {
                    let parsed = parse_terminal(term, regexp)?;
                    order.push((id, term, parsed.clone()));
                    Some(parsed)
                }
                None => None,
            });
        }
        order.sort_by(|(_, a, _), (_, b, _)| {
            let key = |t: &TerminalDef| {
                let pattern = t.pattern.as_ref().unwrap();
                (
                    -t.priority,
                    -(t.max_width.map_or(i128::from(MAXREPEAT), i128::from)),
                    -(pattern.value.chars().count() as i128),
                )
            };
            key(a).cmp(&key(b)).then_with(|| a.name.cmp(&b.name))
        });
        let mut embedded: Vec<TermId> = Vec::new();
        let mut unless_lists: Vec<Vec<(TermId, ParsedPattern)>> = Vec::new();
        for (_, term, parsed) in &order {
            let mut unless = Vec::new();
            let pattern = term.pattern.as_ref().unwrap();
            if pattern.kind == PatternKind::Re {
                let matcher = Matcher::new(parsed);
                for (sid, sterm, sparsed) in &order {
                    let spattern = sterm.pattern.as_ref().unwrap();
                    if spattern.kind != PatternKind::Str || sterm.priority != term.priority {
                        continue;
                    }
                    let text: Vec<char> = spattern.value.chars().collect();
                    if matcher.match_at(&text, 0) == Ok(Some(text.len())) {
                        unless.push((*sid, sparsed.clone()));
                        if is_subset(&spattern.flags, &pattern.flags) {
                            embedded.push(*sid);
                        }
                    }
                }
            }
            unless_lists.push(unless);
        }
        let terminals = order
            .into_iter()
            .zip(unless_lists)
            .filter(|((id, _, _), _)| !embedded.contains(id))
            .map(|((id, _, parsed), unless)| LexTerminal {
                id,
                first: first_chars(&parsed),
                parsed,
                unless,
            })
            .collect();
        Ok(Lexer {
            terminals,
            patterns,
        })
    }

    /// Match terminal `id`'s own pattern at `pos`, as the dynamic lexer
    /// does for a terminal the parser expects there; a terminal without
    /// a pattern (one `%declare`d) cannot be checked.
    pub fn match_terminal(&self, id: TermId, text: &[char], pos: usize) -> Lexed {
        let Some(parsed) = &self.patterns[id] else {
            return Lexed::Unknown;
        };
        match Matcher::new(parsed).match_at(text, pos) {
            Ok(Some(end)) => Lexed::Token(id, end),
            Ok(None) => Lexed::NoMatch,
            Err(_) => Lexed::Unknown,
        }
    }

    /// Lex one token of `text` at `pos`, as Lark's basic lexer would.
    pub fn next_token(&self, text: &[char], pos: usize) -> Lexed {
        let Some(&c) = text.get(pos) else {
            return Lexed::NoMatch;
        };
        for term in &self.terminals {
            if term.first.as_ref().is_some_and(|first| !first.contains(c)) {
                continue;
            }
            let end = match Matcher::new(&term.parsed).match_at(text, pos) {
                Ok(Some(end)) => end,
                Ok(None) => continue,
                Err(_) => return Lexed::Unknown,
            };
            let token = &text[pos..end];
            for (sid, sparsed) in &term.unless {
                match Matcher::new(sparsed).fullmatch(token) {
                    Ok(true) => return Lexed::Token(*sid, end),
                    Ok(false) => {}
                    Err(_) => return Lexed::Unknown,
                }
            }
            return Lexed::Token(term.id, end);
        }
        Lexed::NoMatch
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/lexsim_tests.rs"]
mod tests;
