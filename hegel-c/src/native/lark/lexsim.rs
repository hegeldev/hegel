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
//!
//! Both lexers are run over the same texts many times as the generator
//! tries tokens in their contexts, so a [`Scan`] records, along with what
//! a lexer found at the start of a text, how that finding depended on the
//! text's end (the terminals the end left open, and how each would go on
//! with more text), and [`Lexer::extend`] carries it over to the longer
//! texts the same token is part of without matching it again.

use super::ast::{Pattern, PatternKind};
use super::compile::{TermId, TerminalDef};
use crate::native::re::categories::class_ranges;
use crate::native::re::constants::{MAXREPEAT, SRE_FLAG_IGNORECASE};
use crate::native::re::matcher::{MatchError, Matcher, Touch, run_len};
use crate::native::re::parser::{OpCode, ParsedPattern, SetItem, SubPattern};
use crate::native::re::vm::{Program, Resume, has_nested_repeats};
use alloc::sync::Arc;
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
                        SetItem::Category(category) => match class_ranges(*category) {
                            Some(ranges) => self.ranges.extend_from_slice(ranges),
                            None => self.any = true,
                        },
                        SetItem::Negate => self.any = true,
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

/// One terminal's pattern as the simulated lexers match it: the parsed
/// regex, the characters a match can start with (so most terminals are
/// ruled out at a position without running the matcher), for a
/// case-sensitive string terminal the literal itself, compared directly,
/// and the compiled [`Program`] for a pattern that could send the
/// backtracking matcher quadratic, which simulates every path at once.
#[derive(Clone)]
struct TermPattern {
    /// Shared between the clones of the pattern the lexers hold, so the
    /// addresses that identify its repeats (see [`Touch::Tail`]) agree.
    parsed: Arc<ParsedPattern>,
    first: Option<FirstChars>,
    literal: Option<Vec<char>>,
    program: Option<Program>,
}

impl TermPattern {
    fn new(pattern: &Pattern, parsed: ParsedPattern) -> TermPattern {
        let literal = (pattern.kind == PatternKind::Str && pattern.flags.is_empty())
            .then(|| pattern.value.chars().collect());
        TermPattern {
            first: first_chars(&parsed),
            program: (literal.is_none() && has_nested_repeats(&parsed.pattern))
                .then(|| Program::compile(&parsed).ok())
                .flatten(),
            parsed: Arc::new(parsed),
            literal,
        }
    }

    /// The end of the match at `pos`, as CPython's `pattern.match` finds
    /// it.
    fn match_at(&self, text: &[char], pos: usize) -> Result<Option<usize>, MatchError> {
        self.scan(text, pos).0
    }

    /// [`match_at`](Self::match_at), with how the result depended on the
    /// edges of the text.
    fn scan(&self, text: &[char], pos: usize) -> (Result<Option<usize>, MatchError>, Edge) {
        if let Some(first) = &self.first {
            match text.get(pos) {
                Some(&c) if first.contains(c) => {}
                Some(_) => {
                    return (Ok(None), Edge::Fixed);
                }
                None => {
                    return (Ok(None), Edge::Lost);
                }
            }
        }
        if let Some(literal) = &self.literal {
            let rest = &text[pos.min(text.len())..];
            if rest.len() < literal.len() {
                let edge = if literal.starts_with(rest) {
                    Edge::Carry(Carry::Literal)
                } else {
                    Edge::Fixed
                };
                return (Ok(None), edge);
            }
            return (
                Ok(rest.starts_with(literal).then(|| pos + literal.len())),
                Edge::Fixed,
            );
        }
        if let Some(program) = &self.program {
            let (end, touch, resume) = program.scan(text, pos, None);
            let edge = match (touch, resume) {
                (Touch::Never, _) => Edge::Fixed,
                (_, Some(resume)) => Edge::Carry(Carry::Resume(resume)),
                (_, None) => Edge::Lost,
            };
            return (Ok(end), edge);
        }
        match Matcher::new(&self.parsed).match_at_touch(text, pos) {
            Ok((end, Touch::Never)) => (Ok(end), Edge::Fixed),
            Ok((end, Touch::Tail(run_id))) => (Ok(end), Edge::Carry(Carry::Tail(run_id))),
            Ok((end, Touch::End)) => (Ok(end), Edge::Lost),
            Err(e) => (Err(e), Edge::Lost),
        }
    }

    /// How far the unit of the tail run `run_id` (from a [`Touch::Tail`]
    /// this pattern reported) continues from `from`.
    fn run_len(&self, run_id: usize, text: &[char], from: usize) -> Option<usize> {
        run_len(&self.parsed, run_id, text, from)
    }

    fn fullmatch(&self, text: &[char]) -> Result<bool, MatchError> {
        if let Some(literal) = &self.literal {
            return Ok(text == literal.as_slice());
        }
        Matcher::new(&self.parsed).fullmatch(text)
    }
}

struct LexTerminal {
    id: TermId,
    pattern: TermPattern,
    unless: Vec<(TermId, TermPattern)>,
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

/// How a terminal's outcome at the start of a text could change with
/// more text after it, and how to find out.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Carry {
    /// The text is a proper prefix of the terminal's literal: compare
    /// the literal again.
    Literal,
    /// The backtracking matcher's unbounded run `run_id` reached the
    /// end of the text, where the match ended: the match goes on as far
    /// as the run does.
    Tail(usize),
    /// The simulation's threads still waiting for input at the end.
    Resume(Resume),
}

/// How a terminal's outcome depended on the edges of the text.
enum Edge {
    /// Not at all.
    Fixed,
    /// More text after it could change the outcome, as described.
    Carry(Carry),
    /// In a way that cannot be carried over.
    Lost,
}

/// What a lexer found at a position of some text, with how that depended
/// on where the text ended, so that the finding can be carried over to a
/// longer text that begins the same way (see [`Lexer::extend`]).
#[derive(Debug, Clone)]
pub struct Scan {
    pub lexed: Lexed,
    /// The terminals tried whose outcome more text could change, in the
    /// order they were tried, each with how to find out; `lexed` is what
    /// stands when none of them does.
    steps: Vec<(TermId, Carry)>,
    /// Whether the finding can be carried over at all.
    carried: bool,
}

impl Scan {
    fn new(lexed: Lexed) -> Scan {
        Scan {
            lexed,
            steps: Vec::new(),
            carried: !matches!(lexed, Lexed::Unknown),
        }
    }

    fn note(&mut self, id: TermId, edge: Edge) {
        match edge {
            Edge::Fixed => {}
            Edge::Carry(carry) => self.steps.push((id, carry)),
            Edge::Lost => self.carried = false,
        }
    }
}

/// The sorted terminal order of Lark's basic lexer and its keyword
/// re-typing, over the terminals of one grammar.
pub struct Lexer {
    terminals: Vec<LexTerminal>,
    patterns: Vec<Option<TermPattern>>,
    /// Where each terminal (by [`TermId`]) is in `terminals`.
    positions: Vec<Option<usize>>,
}

fn is_subset(flags: &str, of: &str) -> bool {
    flags.chars().all(|f| of.contains(f))
}

impl Lexer {
    /// Build the lexer over `terminals` (indexed by [`TermId`]); terminals
    /// without a pattern take no part.
    pub fn new(terminals: &[TerminalDef]) -> Lexer {
        let mut order: Vec<(TermId, &TerminalDef, TermPattern)> = Vec::new();
        let mut patterns = Vec::with_capacity(terminals.len());
        for (id, term) in terminals.iter().enumerate() {
            let pattern = term
                .parsed
                .as_ref()
                .map(|parsed| TermPattern::new(term.pattern.as_ref().unwrap(), parsed.clone()));
            if let Some(pattern) = &pattern {
                order.push((id, term, pattern.clone()));
            }
            patterns.push(pattern);
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
        let mut unless_lists: Vec<Vec<(TermId, TermPattern)>> = Vec::new();
        for (_, term, tpattern) in &order {
            let mut unless = Vec::new();
            let pattern = term.pattern.as_ref().unwrap();
            if pattern.kind == PatternKind::Re {
                for (sid, sterm, spattern_parsed) in &order {
                    let spattern = sterm.pattern.as_ref().unwrap();
                    if spattern.kind != PatternKind::Str || sterm.priority != term.priority {
                        continue;
                    }
                    let text: Vec<char> = spattern.value.chars().collect();
                    if tpattern.match_at(&text, 0) == Ok(Some(text.len())) {
                        unless.push((*sid, spattern_parsed.clone()));
                        if is_subset(&spattern.flags, &pattern.flags) {
                            embedded.push(*sid);
                        }
                    }
                }
            }
            unless_lists.push(unless);
        }
        let terminals: Vec<LexTerminal> = order
            .into_iter()
            .zip(unless_lists)
            .filter(|((id, _, _), _)| !embedded.contains(id))
            .map(|((id, _, pattern), unless)| LexTerminal {
                id,
                pattern,
                unless,
            })
            .collect();
        let mut positions = alloc::vec![None; patterns.len()];
        for (i, term) in terminals.iter().enumerate() {
            positions[term.id] = Some(i);
        }
        Lexer {
            terminals,
            patterns,
            positions,
        }
    }

    /// Match terminal `id`'s own pattern at `pos`, as the dynamic lexer
    /// does for a terminal the parser expects there; a terminal without
    /// a pattern (one `%declare`d) cannot be checked.
    pub fn match_terminal(&self, id: TermId, text: &[char], pos: usize) -> Lexed {
        self.match_terminal_scan(id, text, pos).lexed
    }

    /// [`match_terminal`](Self::match_terminal), with how the result
    /// depended on the end of the text.
    pub fn match_terminal_scan(&self, id: TermId, text: &[char], pos: usize) -> Scan {
        let Some(pattern) = &self.patterns[id] else {
            return Scan::new(Lexed::Unknown);
        };
        let (result, edge) = pattern.scan(text, pos);
        let mut scan = Scan::new(match result {
            Ok(Some(end)) => Lexed::Token(id, end),
            Ok(None) => Lexed::NoMatch,
            Err(_) => Lexed::Unknown,
        });
        scan.note(id, edge);
        scan
    }

    /// Lex one token of `text` at `pos`, as Lark's basic lexer would.
    pub fn next_token(&self, text: &[char], pos: usize) -> Lexed {
        self.next_token_scan(text, pos).lexed
    }

    /// [`next_token`](Self::next_token), with how the result depended on
    /// the end of the text: a terminal that failed after looking at the
    /// end might match with more text, so the result then stands only for
    /// this text.
    pub fn next_token_scan(&self, text: &[char], pos: usize) -> Scan {
        let mut scan = Scan::new(Lexed::NoMatch);
        if pos >= text.len() {
            scan.carried = false;
            return scan;
        }
        for term in &self.terminals {
            let (result, edge) = term.pattern.scan(text, pos);
            scan.note(term.id, edge);
            match result {
                Ok(Some(end)) => {
                    scan.lexed = Lexed::Token(self.retype(term, &text[pos..end]), end);
                    return scan;
                }
                Ok(None) => {}
                Err(_) => {
                    scan.lexed = Lexed::Unknown;
                    scan.carried = false;
                    return scan;
                }
            }
        }
        scan
    }

    /// The terminal the basic lexer reports a token of `term` with the
    /// text `token` as: a string terminal the text matches completely,
    /// if any, else `term` itself.
    fn retype(&self, term: &LexTerminal, token: &[char]) -> TermId {
        for (sid, spattern) in &term.unless {
            if spattern.fullmatch(token) == Ok(true) {
                return *sid;
            }
        }
        term.id
    }

    /// What `scan`, found at the start of a text of `len` characters,
    /// says about the text that begins the same way at `pos` of `text`:
    /// the first terminal whose outcome the end of the text left open
    /// and that matches on the longer one (re-typed as a keyword, for a
    /// basic-lexer scan, when `retype`), else the same thing shifted to
    /// `pos`; nothing when the finding cannot be carried over.
    pub fn extend(
        &self,
        scan: &Scan,
        text: &[char],
        pos: usize,
        len: usize,
        retype: bool,
    ) -> Option<Lexed> {
        if !scan.carried {
            return None;
        }
        for (id, carry) in &scan.steps {
            let pattern = self.patterns[*id].as_ref()?;
            let end = match carry {
                Carry::Literal => {
                    let literal = pattern.literal.as_ref()?;
                    text[pos..]
                        .starts_with(literal)
                        .then(|| pos + literal.len())
                }
                Carry::Tail(run_id) => Some(pos + len + pattern.run_len(*run_id, text, pos + len)?),
                Carry::Resume(resume) => pattern.program.as_ref()?.resume(resume, text, pos, len).0,
            };
            if let Some(end) = end {
                let id = if retype {
                    self.retype(&self.terminals[self.positions[*id]?], &text[pos..end])
                } else {
                    *id
                };
                return Some(Lexed::Token(id, end));
            }
        }
        Some(match scan.lexed {
            Lexed::Token(id, end) => Lexed::Token(id, pos + end),
            other => other,
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/lexsim_tests.rs"]
mod tests;
