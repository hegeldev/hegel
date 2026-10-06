//! Backtracking matcher for the pattern AST produced by [`super::parser`],
//! replicating CPython's `_sre` engine closely enough that a match ends
//! where CPython's `pattern.match(text, pos)` would end: alternatives are
//! tried left to right and the first that lets the rest of the pattern
//! succeed wins, greedy repeats give back one item at a time, lazy repeats
//! take one more item at a time, and possessive repeats and atomic groups
//! never give anything back.
//!
//! The grammar generator uses it to check that a generated terminal lexes
//! back to itself (Hypothesis's `lexes_entirely` filter) and to simulate a
//! lexer at the boundary between two generated tokens, which needs
//! CPython's leftmost-first semantics rather than leftmost-longest.
//!
//! Text is indexed by codepoint, so callers pass `&[char]`. Matching is a
//! recursive descent with continuation passing; a step budget bounds the
//! work on patterns with catastrophic backtracking and a depth budget
//! bounds the Rust stack a repeat of a compound item can consume.

use alloc::vec::Vec;

use super::constants::{
    AtCode, ChCode, MAXREPEAT, SRE_FLAG_DOTALL, SRE_FLAG_IGNORECASE, SRE_FLAG_MULTILINE,
    SRE_FLAG_UNICODE, TYPE_FLAGS,
};
use super::parser::{OpCode, ParsedPattern, SetItem, SubPattern};
use crate::native::HashMap;
use crate::sys::sync::Lazy;
use crate::unicodedata;

/// The default number of matching steps one match may take before it is
/// abandoned with [`MatchError::StepLimit`].
pub const DEFAULT_STEP_LIMIT: u64 = 1_000_000;

/// The default nesting depth of pattern sequences one match may reach
/// before it is abandoned with [`MatchError::DepthLimit`]. Only repeats of
/// items with capture groups, assertions or nested repeats recurse per
/// iteration (two levels each); repeats of single characters and of
/// alternations of plain character sequences run on an explicit stack.
/// Unoptimised builds use about 1 KiB of stack per level, so this keeps a
/// match within half of a 2 MiB thread stack.
pub const DEFAULT_DEPTH_LIMIT: usize = 1_000;

/// Why a match was abandoned without an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchError {
    /// The step budget ran out (catastrophic backtracking).
    StepLimit,
    /// The recursion budget ran out (a long repeat of a compound item).
    DepthLimit,
}

impl core::fmt::Display for MatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MatchError::StepLimit => write!(f, "regex match exceeded its step limit"),
            MatchError::DepthLimit => write!(f, "regex match exceeded its nesting depth limit"),
        }
    }
}

type MatchResult = Result<bool, MatchError>;

/// A matcher for one parsed pattern.
pub struct Matcher<'p> {
    pattern: &'p SubPattern,
    flags: u32,
    step_limit: u64,
    depth_limit: usize,
}

/// Per-match mutable state: the text, the capture groups set so far, and
/// the remaining budgets.
struct State<'t> {
    text: &'t [char],
    marks: Vec<Option<(usize, usize)>>,
    steps: u64,
    step_limit: u64,
    depth: usize,
    depth_limit: usize,
}

impl State<'_> {
    fn step(&mut self) -> Result<(), MatchError> {
        self.steps += 1;
        if self.steps > self.step_limit {
            return Err(MatchError::StepLimit);
        }
        Ok(())
    }

    fn enter(&mut self) -> Result<(), MatchError> {
        self.depth += 1;
        if self.depth > self.depth_limit {
            return Err(MatchError::DepthLimit);
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn group(&self, g: u32) -> Option<(usize, usize)> {
        self.marks.get(g as usize).copied().flatten()
    }

    fn set_group(&mut self, g: u32, value: Option<(usize, usize)>) {
        let g = g as usize;
        if self.marks.len() <= g {
            self.marks.resize(g + 1, None);
        }
        self.marks[g] = value;
    }
}

/// A continuation: what must match after the current construct, starting
/// at the given position.
type Cont<'a, 't> = &'a mut dyn FnMut(&mut State<'t>, usize) -> MatchResult;

/// A single-character matching op, the items CPython compiles to
/// `REPEAT_ONE` / `MIN_REPEAT_ONE` so a repeat of them runs as a counted
/// loop rather than one recursion per iteration.
#[derive(Clone, Copy)]
enum UnitOp<'a> {
    Literal(u32),
    NotLiteral(u32),
    Any,
    In(&'a [SetItem]),
}

fn as_unit(op: &OpCode) -> Option<UnitOp<'_>> {
    match op {
        OpCode::Literal(cp) => Some(UnitOp::Literal(*cp)),
        OpCode::NotLiteral(cp) => Some(UnitOp::NotLiteral(*cp)),
        OpCode::Any => Some(UnitOp::Any),
        OpCode::In(items) => Some(UnitOp::In(items)),
        _ => None,
    }
}

/// CPython's `_simple`: a one-op pattern that is a unit, or a
/// non-capturing group around one, with the flags in force inside it.
fn single_unit(p: &SubPattern, flags: u32) -> Option<(UnitOp<'_>, u32)> {
    match p.data.as_slice() {
        [
            OpCode::Subpattern {
                group: None,
                add_flags,
                del_flags,
                p,
            },
        ] => single_unit(p, combine_flags(flags, *add_flags, *del_flags)),
        [op] => as_unit(op).map(|unit| (unit, flags)),
        _ => None,
    }
}

/// One alternative of a repeat item handled on the explicit stack: a
/// sequence of units, each with the flags in force for it.
type UnitSequence<'a> = Vec<(UnitOp<'a>, u32)>;

/// Collect the alternatives of `p` into `out` when `p` is a sequence of
/// units or an alternation (possibly nested in non-capturing groups) of
/// such sequences, the shape of most grammar terminals' repeats. Returns
/// false, leaving `out` unspecified, when `p` contains anything else.
fn unit_alternatives<'a>(p: &'a SubPattern, flags: u32, out: &mut Vec<UnitSequence<'a>>) -> bool {
    match p.data.as_slice() {
        [
            OpCode::Subpattern {
                group: None,
                add_flags,
                del_flags,
                p,
            },
        ] => unit_alternatives(p, combine_flags(flags, *add_flags, *del_flags), out),
        [OpCode::Branch(alternatives)] => alternatives
            .iter()
            .all(|alternative| unit_alternatives(alternative, flags, out)),
        ops => {
            let mut sequence = Vec::with_capacity(ops.len());
            for op in ops {
                match as_unit(op) {
                    Some(unit) => sequence.push((unit, flags)),
                    None => return false,
                }
            }
            out.push(sequence);
            true
        }
    }
}

/// The end of `sequence` matched at `pos`, if it matches there.
fn sequence_end(text: &[char], pos: usize, sequence: &UnitSequence<'_>) -> Option<usize> {
    for (offset, &(unit, flags)) in sequence.iter().enumerate() {
        if !text
            .get(pos + offset)
            .is_some_and(|&c| unit_matches(unit, c, flags))
        {
            return None;
        }
    }
    Some(pos + sequence.len())
}

/// The first alternative at index `from` or later that matches at `pos`,
/// with its end.
fn first_alternative(
    text: &[char],
    alternatives: &[UnitSequence<'_>],
    pos: usize,
    from: usize,
) -> Option<(usize, usize)> {
    alternatives
        .iter()
        .enumerate()
        .skip(from)
        .find_map(|(index, alternative)| {
            sequence_end(text, pos, alternative).map(|end| (index, end))
        })
}

/// CPython's `_combine_flags`: a scoped type flag (`a`/`u`/`L`) replaces
/// the inherited one rather than adding to it.
fn combine_flags(mut flags: u32, add_flags: u32, del_flags: u32) -> u32 {
    if add_flags & TYPE_FLAGS != 0 {
        flags &= !TYPE_FLAGS;
    }
    (flags | add_flags) & !del_flags
}

impl<'p> Matcher<'p> {
    /// A matcher for `parsed` with the default budgets.
    pub fn new(parsed: &'p ParsedPattern) -> Self {
        Matcher {
            pattern: &parsed.pattern,
            flags: parsed.flags,
            step_limit: DEFAULT_STEP_LIMIT,
            depth_limit: DEFAULT_DEPTH_LIMIT,
        }
    }

    /// Replace the step and depth budgets.
    pub fn with_limits(mut self, step_limit: u64, depth_limit: usize) -> Self {
        self.step_limit = step_limit;
        self.depth_limit = depth_limit;
        self
    }

    fn state<'t>(&self, text: &'t [char]) -> State<'t> {
        State {
            text,
            marks: Vec::new(),
            steps: 0,
            step_limit: self.step_limit,
            depth: 0,
            depth_limit: self.depth_limit,
        }
    }

    /// The end of the match CPython's `pattern.match(text, start)` finds,
    /// or `None` when the pattern does not match at `start`. A `start`
    /// past the end of the text is clamped to it, as CPython does.
    pub fn match_at(&self, text: &[char], start: usize) -> Result<Option<usize>, MatchError> {
        let start = start.min(text.len());
        let mut state = self.state(text);
        let mut end = None;
        let matched = self.seq(
            &mut state,
            &self.pattern.data,
            start,
            self.flags,
            &mut |_, pos| {
                end = Some(pos);
                Ok(true)
            },
        )?;
        Ok(if matched { end } else { None })
    }

    /// Whether the pattern matches the whole of `text`, like CPython's
    /// `pattern.fullmatch(text)`: unlike [`match_at`](Self::match_at)
    /// reaching the end, this backtracks into the pattern looking for a
    /// match that ends exactly at the end.
    pub fn fullmatch(&self, text: &[char]) -> Result<bool, MatchError> {
        let mut state = self.state(text);
        self.seq(
            &mut state,
            &self.pattern.data,
            0,
            self.flags,
            &mut |_, pos| Ok(pos == text.len()),
        )
    }

    /// Match the sequence `ops` at `pos`, then the continuation `k`.
    /// Single-character ops and position assertions advance in a loop;
    /// everything else hands the rest of the sequence to
    /// [`compound`](Self::compound). A call that returns `Ok(false)` leaves
    /// the capture groups as it found them.
    fn seq<'t>(
        &self,
        st: &mut State<'t>,
        ops: &[OpCode],
        pos: usize,
        flags: u32,
        k: Cont<'_, 't>,
    ) -> MatchResult {
        st.enter()?;
        let result = self.seq_body(st, ops, pos, flags, k);
        st.leave();
        result
    }

    fn seq_body<'t>(
        &self,
        st: &mut State<'t>,
        ops: &[OpCode],
        mut pos: usize,
        flags: u32,
        k: Cont<'_, 't>,
    ) -> MatchResult {
        let mut index = 0;
        loop {
            st.step()?;
            let Some(op) = ops.get(index) else {
                return k(st, pos);
            };
            if let Some(unit) = as_unit(op) {
                match st.text.get(pos) {
                    Some(&c) if unit_matches(unit, c, flags) => {
                        pos += 1;
                        index += 1;
                    }
                    _ => return Ok(false),
                }
                continue;
            }
            match op {
                OpCode::At(at) => {
                    if !at_matches(*at, st.text, pos, flags) {
                        return Ok(false);
                    }
                    index += 1;
                }
                OpCode::Failure => return Ok(false),
                _ => return self.compound(st, op, &ops[index + 1..], pos, flags, k),
            }
        }
    }

    /// Match one compound op at `pos`, then `rest`, then `k`.
    fn compound<'t>(
        &self,
        st: &mut State<'t>,
        op: &OpCode,
        rest: &[OpCode],
        pos: usize,
        flags: u32,
        k: Cont<'_, 't>,
    ) -> MatchResult {
        match op {
            OpCode::Branch(alternatives) => {
                for alternative in alternatives {
                    if self.seq(st, &alternative.data, pos, flags, &mut |st, end| {
                        self.seq(st, rest, end, flags, k)
                    })? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            OpCode::Subpattern {
                group,
                add_flags,
                del_flags,
                p,
            } => {
                let inner = combine_flags(flags, *add_flags, *del_flags);
                match *group {
                    None => self.seq(st, &p.data, pos, inner, &mut |st, end| {
                        self.seq(st, rest, end, flags, k)
                    }),
                    Some(g) => self.seq(st, &p.data, pos, inner, &mut |st, end| {
                        let saved = st.group(g);
                        st.set_group(g, Some((pos, end)));
                        let result = self.seq(st, rest, end, flags, k);
                        if !matches!(result, Ok(true)) {
                            st.set_group(g, saved);
                        }
                        result
                    }),
                }
            }
            OpCode::GroupRef(g) => {
                let Some((start, end)) = st.group(*g) else {
                    return Ok(false);
                };
                let len = end - start;
                if pos + len > st.text.len() {
                    return Ok(false);
                }
                for offset in 0..len {
                    if !backref_eq(st.text[start + offset], st.text[pos + offset], flags) {
                        return Ok(false);
                    }
                }
                self.seq(st, rest, pos + len, flags, k)
            }
            OpCode::GroupRefExists {
                cond_group,
                yes,
                no,
            } => {
                if st.group(*cond_group).is_some() {
                    self.seq(st, &yes.data, pos, flags, &mut |st, end| {
                        self.seq(st, rest, end, flags, k)
                    })
                } else if let Some(no) = no {
                    self.seq(st, &no.data, pos, flags, &mut |st, end| {
                        self.seq(st, rest, end, flags, k)
                    })
                } else {
                    self.seq(st, rest, pos, flags, k)
                }
            }
            OpCode::Assert { direction, p } => {
                let saved = st.marks.clone();
                if !self.lookaround(st, *direction, p, pos, flags)? {
                    return Ok(false);
                }
                if self.seq(st, rest, pos, flags, k)? {
                    return Ok(true);
                }
                st.marks = saved;
                Ok(false)
            }
            OpCode::AssertNot { direction, p } => {
                let saved = st.marks.clone();
                if self.lookaround(st, *direction, p, pos, flags)? {
                    st.marks = saved;
                    return Ok(false);
                }
                self.seq(st, rest, pos, flags, k)
            }
            OpCode::AtomicGroup(p) => {
                let saved = st.marks.clone();
                let Some(end) = self.first_end(st, p, pos, flags)? else {
                    return Ok(false);
                };
                if self.seq(st, rest, end, flags, k)? {
                    return Ok(true);
                }
                st.marks = saved;
                Ok(false)
            }
            OpCode::MaxRepeat { min, max, item } => {
                if let Some((unit, unit_flags)) = single_unit(item, flags) {
                    return self.greedy_unit(st, unit, unit_flags, *min, *max, rest, pos, flags, k);
                }
                let mut alternatives = Vec::new();
                if unit_alternatives(item, flags, &mut alternatives) {
                    return self.greedy_sequences(
                        st,
                        &alternatives,
                        *min,
                        *max,
                        rest,
                        pos,
                        flags,
                        k,
                    );
                }
                self.greedy(st, item, *min, *max, 0, None, rest, pos, flags, k)
            }
            OpCode::MinRepeat { min, max, item } => {
                if let Some((unit, unit_flags)) = single_unit(item, flags) {
                    return self.lazy_unit(st, unit, unit_flags, *min, *max, rest, pos, flags, k);
                }
                let mut alternatives = Vec::new();
                if unit_alternatives(item, flags, &mut alternatives) {
                    return self.lazy_sequences(st, &alternatives, *min, *max, rest, pos, flags, k);
                }
                self.lazy(st, item, *min, *max, 0, None, rest, pos, flags, k)
            }
            OpCode::PossessiveRepeat { min, max, item } => {
                self.possessive(st, item, *min, *max, rest, pos, flags, k)
            }
            OpCode::Literal(_)
            | OpCode::NotLiteral(_)
            | OpCode::Any
            | OpCode::In(_)
            | OpCode::At(_)
            | OpCode::Failure => unreachable!("compound: {:?} is handled by seq_body", op),
        }
    }

    /// The end of the first match of `p` at `pos` in CPython's order, with
    /// no backtracking into it afterwards: the body of an atomic group, a
    /// possessive iteration, or a lookahead.
    fn first_end<'t>(
        &self,
        st: &mut State<'t>,
        p: &SubPattern,
        pos: usize,
        flags: u32,
    ) -> Result<Option<usize>, MatchError> {
        let mut end = None;
        let matched = self.seq(st, &p.data, pos, flags, &mut |_, e| {
            end = Some(e);
            Ok(true)
        })?;
        Ok(if matched { end } else { None })
    }

    /// Whether the lookaround body `p` matches at `pos`: ahead of it for a
    /// positive `direction`, ending exactly at it for a negative one.
    /// CPython only accepts fixed-width lookbehinds; a variable-width one
    /// (which the parser does not reject) is accepted at any of its widths.
    fn lookaround<'t>(
        &self,
        st: &mut State<'t>,
        direction: i32,
        p: &SubPattern,
        pos: usize,
        flags: u32,
    ) -> MatchResult {
        if direction > 0 {
            return Ok(self.first_end(st, p, pos, flags)?.is_some());
        }
        let (lo, hi) = self.width(p);
        let mut w = lo;
        while w <= hi && w <= pos {
            if self.seq(st, &p.data, pos - w, flags, &mut |_, end| Ok(end == pos))? {
                return Ok(true);
            }
            w += 1;
        }
        Ok(false)
    }

    /// CPython's `MAX_UNTIL`: `count` iterations of `item` are done; take
    /// another if allowed and the previous one consumed something, else
    /// fall through to the rest.
    fn greedy<'t>(
        &self,
        st: &mut State<'t>,
        item: &SubPattern,
        min: u32,
        max: u32,
        count: u32,
        last_pos: Option<usize>,
        rest: &[OpCode],
        pos: usize,
        flags: u32,
        k: Cont<'_, 't>,
    ) -> MatchResult {
        st.step()?;
        if count < min {
            return self.seq(st, &item.data, pos, flags, &mut |st, end| {
                self.greedy(st, item, min, max, count + 1, last_pos, rest, end, flags, k)
            });
        }
        if (max == MAXREPEAT || count < max)
            && last_pos != Some(pos)
            && self.seq(st, &item.data, pos, flags, &mut |st, end| {
                self.greedy(
                    st,
                    item,
                    min,
                    max,
                    count + 1,
                    Some(pos),
                    rest,
                    end,
                    flags,
                    k,
                )
            })?
        {
            return Ok(true);
        }
        self.seq(st, rest, pos, flags, k)
    }

    /// CPython's `MIN_UNTIL`: once `min` iterations are done, try the rest
    /// first and take another iteration only if that fails.
    fn lazy<'t>(
        &self,
        st: &mut State<'t>,
        item: &SubPattern,
        min: u32,
        max: u32,
        count: u32,
        last_pos: Option<usize>,
        rest: &[OpCode],
        pos: usize,
        flags: u32,
        k: Cont<'_, 't>,
    ) -> MatchResult {
        st.step()?;
        if count < min {
            return self.seq(st, &item.data, pos, flags, &mut |st, end| {
                self.lazy(st, item, min, max, count + 1, last_pos, rest, end, flags, k)
            });
        }
        if self.seq(st, rest, pos, flags, k)? {
            return Ok(true);
        }
        if (max != MAXREPEAT && count >= max) || last_pos == Some(pos) {
            return Ok(false);
        }
        self.seq(st, &item.data, pos, flags, &mut |st, end| {
            self.lazy(
                st,
                item,
                min,
                max,
                count + 1,
                Some(pos),
                rest,
                end,
                flags,
                k,
            )
        })
    }

    /// CPython's `REPEAT_ONE`: count the most units that match, then give
    /// them back one at a time.
    fn greedy_unit<'t>(
        &self,
        st: &mut State<'t>,
        unit: UnitOp<'_>,
        unit_flags: u32,
        min: u32,
        max: u32,
        rest: &[OpCode],
        pos: usize,
        flags: u32,
        k: Cont<'_, 't>,
    ) -> MatchResult {
        let limit = repeat_limit(max);
        let mut count = 0;
        while count < limit
            && st
                .text
                .get(pos + count)
                .is_some_and(|&c| unit_matches(unit, c, unit_flags))
        {
            count += 1;
        }
        if count < min as usize {
            return Ok(false);
        }
        loop {
            st.step()?;
            if self.seq(st, rest, pos + count, flags, k)? {
                return Ok(true);
            }
            if count == min as usize {
                return Ok(false);
            }
            count -= 1;
        }
    }

    /// CPython's `MIN_REPEAT_ONE`: take the minimum, then one more unit
    /// each time the rest fails.
    fn lazy_unit<'t>(
        &self,
        st: &mut State<'t>,
        unit: UnitOp<'_>,
        unit_flags: u32,
        min: u32,
        max: u32,
        rest: &[OpCode],
        pos: usize,
        flags: u32,
        k: Cont<'_, 't>,
    ) -> MatchResult {
        let limit = repeat_limit(max);
        let next_matches = |st: &State<'t>, count: usize| {
            st.text
                .get(pos + count)
                .is_some_and(|&c| unit_matches(unit, c, unit_flags))
        };
        let mut count = 0;
        while count < min as usize {
            if !next_matches(st, count) {
                return Ok(false);
            }
            count += 1;
        }
        loop {
            st.step()?;
            if self.seq(st, rest, pos + count, flags, k)? {
                return Ok(true);
            }
            if count >= limit || !next_matches(st, count) {
                return Ok(false);
            }
            count += 1;
        }
    }

    /// [`greedy`](Self::greedy) for an item that is an alternation of unit
    /// sequences, on an explicit stack of `(iteration start, alternative)`
    /// frames so that long repeats cost heap rather than Rust stack. The
    /// order is CPython's: at each iteration try the alternatives in
    /// order, each followed by further iterations, and only when all fail
    /// try the rest of the pattern at the iteration's start.
    fn greedy_sequences<'t>(
        &self,
        st: &mut State<'t>,
        alternatives: &[UnitSequence<'_>],
        min: u32,
        max: u32,
        rest: &[OpCode],
        pos: usize,
        flags: u32,
        k: Cont<'_, 't>,
    ) -> MatchResult {
        let min = min as usize;
        let mut frames: Vec<(usize, usize)> = Vec::new();
        let mut cur = pos;
        loop {
            while repeat_allowed(&frames, min, max, cur) {
                st.step()?;
                match first_alternative(st.text, alternatives, cur, 0) {
                    Some((index, end)) => {
                        frames.push((cur, index));
                        cur = end;
                    }
                    None => break,
                }
            }
            if frames.len() >= min && self.seq(st, rest, cur, flags, k)? {
                return Ok(true);
            }
            loop {
                let Some((start, index)) = frames.pop() else {
                    return Ok(false);
                };
                st.step()?;
                if let Some((next, end)) =
                    first_alternative(st.text, alternatives, start, index + 1)
                {
                    frames.push((start, next));
                    cur = end;
                    break;
                }
                if frames.len() >= min && self.seq(st, rest, start, flags, k)? {
                    return Ok(true);
                }
            }
        }
    }

    /// [`lazy`](Self::lazy) for an item that is an alternation of unit
    /// sequences, on an explicit stack like
    /// [`greedy_sequences`](Self::greedy_sequences): at each iteration past
    /// the minimum try the rest of the pattern first, then the alternatives
    /// in order.
    fn lazy_sequences<'t>(
        &self,
        st: &mut State<'t>,
        alternatives: &[UnitSequence<'_>],
        min: u32,
        max: u32,
        rest: &[OpCode],
        pos: usize,
        flags: u32,
        k: Cont<'_, 't>,
    ) -> MatchResult {
        let min = min as usize;
        let mut frames: Vec<(usize, usize)> = Vec::new();
        let mut cur = pos;
        loop {
            st.step()?;
            if frames.len() >= min && self.seq(st, rest, cur, flags, k)? {
                return Ok(true);
            }
            if repeat_allowed(&frames, min, max, cur) {
                if let Some((index, end)) = first_alternative(st.text, alternatives, cur, 0) {
                    frames.push((cur, index));
                    cur = end;
                    continue;
                }
            }
            loop {
                let Some((start, index)) = frames.pop() else {
                    return Ok(false);
                };
                st.step()?;
                if let Some((next, end)) =
                    first_alternative(st.text, alternatives, start, index + 1)
                {
                    frames.push((start, next));
                    cur = end;
                    break;
                }
            }
        }
    }

    /// CPython's `POSSESSIVE_REPEAT`: each iteration takes the first match
    /// of `item`, iteration stops at the first failure or empty match, and
    /// nothing is given back when the rest fails.
    fn possessive<'t>(
        &self,
        st: &mut State<'t>,
        item: &SubPattern,
        min: u32,
        max: u32,
        rest: &[OpCode],
        pos: usize,
        flags: u32,
        k: Cont<'_, 't>,
    ) -> MatchResult {
        let saved = st.marks.clone();
        let mut count = 0;
        let mut cur = pos;
        while count < min {
            let Some(end) = self.first_end(st, item, cur, flags)? else {
                st.marks = saved;
                return Ok(false);
            };
            cur = end;
            count += 1;
        }
        while max == MAXREPEAT || count < max {
            let Some(end) = self.first_end(st, item, cur, flags)? else {
                break;
            };
            count += 1;
            if end == cur {
                break;
            }
            cur = end;
        }
        if self.seq(st, rest, cur, flags, k)? {
            return Ok(true);
        }
        st.marks = saved;
        Ok(false)
    }

    /// CPython's `getwidth`: the least and most characters `p` can match,
    /// saturating. A backreference has the width of its group.
    fn width(&self, p: &SubPattern) -> (usize, usize) {
        let mut lo = 0usize;
        let mut hi = 0usize;
        for op in &p.data {
            let (l, h) = match op {
                OpCode::Branch(alternatives) => {
                    let mut l = usize::MAX;
                    let mut h = 0;
                    for alternative in alternatives {
                        let (al, ah) = self.width(alternative);
                        l = l.min(al);
                        h = h.max(ah);
                    }
                    (l, h)
                }
                OpCode::Subpattern { p, .. } | OpCode::AtomicGroup(p) => self.width(p),
                OpCode::MaxRepeat { min, max, item }
                | OpCode::MinRepeat { min, max, item }
                | OpCode::PossessiveRepeat { min, max, item } => {
                    let (il, ih) = self.width(item);
                    let h = if *max == MAXREPEAT {
                        usize::MAX
                    } else {
                        ih.saturating_mul(*max as usize)
                    };
                    (il.saturating_mul(*min as usize), h)
                }
                OpCode::GroupRef(g) => match find_group(self.pattern, *g) {
                    Some(group) => self.width(group),
                    None => (0, 0),
                },
                OpCode::GroupRefExists { yes, no, .. } => {
                    let (yl, yh) = self.width(yes);
                    let (nl, nh) = match no {
                        Some(no) => self.width(no),
                        None => (0, 0),
                    };
                    (yl.min(nl), yh.max(nh))
                }
                _ => {
                    if as_unit(op).is_some() {
                        (1, 1)
                    } else {
                        (0, 0)
                    }
                }
            };
            lo = lo.saturating_add(l);
            hi = hi.saturating_add(h);
        }
        (lo, hi)
    }
}

/// The subpattern of capture group `g`, searched for through the whole
/// pattern.
fn find_group(p: &SubPattern, g: u32) -> Option<&SubPattern> {
    for op in &p.data {
        let found = match op {
            OpCode::Subpattern {
                group: Some(id), p, ..
            } if *id == g => Some(p),
            OpCode::Subpattern { p, .. } | OpCode::AtomicGroup(p) => find_group(p, g),
            OpCode::Branch(alternatives) => alternatives.iter().find_map(|a| find_group(a, g)),
            OpCode::MaxRepeat { item, .. }
            | OpCode::MinRepeat { item, .. }
            | OpCode::PossessiveRepeat { item, .. } => find_group(item, g),
            OpCode::GroupRefExists { yes, no, .. } => {
                find_group(yes, g).or_else(|| no.as_ref().and_then(|no| find_group(no, g)))
            }
            OpCode::Assert { p, .. } | OpCode::AssertNot { p, .. } => find_group(p, g),
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

fn repeat_limit(max: u32) -> usize {
    if max == MAXREPEAT {
        usize::MAX
    } else {
        max as usize
    }
}

/// Whether another iteration may start at `cur` given the iterations in
/// `frames`: always below the minimum, and otherwise below the maximum
/// and not at the position the previous post-minimum iteration started
/// from (CPython's `last_ptr` guard against looping on empty matches).
fn repeat_allowed(frames: &[(usize, usize)], min: usize, max: u32, cur: usize) -> bool {
    let count = frames.len();
    if count < min {
        return true;
    }
    if count >= repeat_limit(max) {
        return false;
    }
    let last_start = if count > min {
        frames.last().map(|&(start, _)| start)
    } else {
        None
    };
    last_start != Some(cur)
}

fn unit_matches(op: UnitOp<'_>, c: char, flags: u32) -> bool {
    match op {
        UnitOp::Literal(cp) => literal_eq(cp, c, flags),
        UnitOp::NotLiteral(cp) => !literal_eq(cp, c, flags),
        UnitOp::Any => c != '\n' || flags & SRE_FLAG_DOTALL != 0,
        UnitOp::In(items) => set_matches(items, c, flags),
    }
}

/// CPython's `LITERAL` / `LITERAL_IGNORE` / `LITERAL_UNI_IGNORE` test:
/// an uncased pattern character matches exactly; a cased one matches any
/// text character with the same lowercase form, or (Unicode mode) one of
/// its `_casefix` equivalents. The parser only emits valid codepoints.
fn literal_eq(cp: u32, c: char, flags: u32) -> bool {
    char::from_u32(cp).is_some_and(|l| char_eq(l, c, flags))
}

fn char_eq(l: char, c: char, flags: u32) -> bool {
    if flags & SRE_FLAG_IGNORECASE == 0 {
        return c == l;
    }
    let unicode = flags & SRE_FLAG_UNICODE != 0;
    if !is_cased(l, unicode) {
        return c == l;
    }
    let lo = fold(l, unicode);
    let probe = fold(c, unicode);
    probe == lo || (unicode && extra_cases(lo).contains(&probe))
}

/// CPython's `GROUPREF_IGNORE` / `GROUPREF_UNI_IGNORE` test: lowercase
/// forms must agree, with no `_casefix` equivalents.
fn backref_eq(a: char, b: char, flags: u32) -> bool {
    if flags & SRE_FLAG_IGNORECASE == 0 {
        return a == b;
    }
    let unicode = flags & SRE_FLAG_UNICODE != 0;
    fold(a, unicode) == fold(b, unicode)
}

/// CPython's `IN` / `IN_IGNORE` / `IN_UNI_IGNORE` test. Under IGNORECASE
/// the set is tested against the lowercase form of the character, and
/// contains the lowercase forms of its members plus their `_casefix`
/// equivalents.
fn set_matches(items: &[SetItem], c: char, flags: u32) -> bool {
    let negate = matches!(items.first(), Some(SetItem::Negate));
    let ignore_case = flags & SRE_FLAG_IGNORECASE != 0;
    let unicode = flags & SRE_FLAG_UNICODE != 0;
    let probe = if ignore_case { fold(c, unicode) } else { c };
    let mut equivalents: Vec<char> = alloc::vec![probe];
    if ignore_case && unicode {
        equivalents.extend_from_slice(extra_cases(probe));
    }
    let contained = items.iter().any(|item| match item {
        SetItem::Negate => false,
        SetItem::Literal(cp) => char::from_u32(*cp).is_some_and(|member| {
            let member = if ignore_case {
                fold(member, unicode)
            } else {
                member
            };
            equivalents.contains(&member)
        }),
        SetItem::Range(lo, hi) => equivalents
            .iter()
            .any(|&e| range_contains_folded(*lo, *hi, e, ignore_case, unicode)),
        SetItem::Category(category) => in_category(probe, *category, unicode),
    });
    contained != negate
}

/// Whether some member of the range `[lo, hi]` has lowercase form `e`
/// (or is `e` itself, which is all that counts without IGNORECASE).
fn range_contains_folded(lo: u32, hi: u32, e: char, ignore_case: bool, unicode: bool) -> bool {
    let in_range = |x: char| (lo..=hi).contains(&(x as u32));
    if in_range(e) {
        return true;
    }
    if !ignore_case {
        return false;
    }
    if unicode {
        return unfold(e).iter().any(|&x| in_range(x));
    }
    e.is_ascii_lowercase() && in_range(e.to_ascii_uppercase())
}

/// CPython's `sre_lower_unicode` / `sre_lower_ascii`: the simple
/// lowercase mapping. Rust's full mapping agrees with the simple one
/// everywhere except `İ`, whose full mapping is `i` plus a combining dot
/// and whose simple mapping is `i`: taking the first character covers it.
fn fold(c: char, unicode: bool) -> char {
    if unicode {
        c.to_lowercase().next().unwrap_or(c)
    } else {
        c.to_ascii_lowercase()
    }
}

/// CPython's `unicode_iscased` / `ascii_iscased`.
fn is_cased(c: char, unicode: bool) -> bool {
    if !unicode {
        return c.is_ascii_alphabetic();
    }
    if fold(c, true) != c {
        return true;
    }
    let mut upper = c.to_uppercase();
    !(upper.next() == Some(c) && upper.next().is_none())
}

/// Every character other than `l` whose simple lowercase form is `l`,
/// from a table built on first use by folding the whole codespace.
fn unfold(l: char) -> &'static [char] {
    static UNFOLD: Lazy<HashMap<char, Vec<char>>> = Lazy::new(|| {
        let mut table: HashMap<char, Vec<char>> = HashMap::default();
        for c in (0u32..=0x10FFFF).filter_map(char::from_u32) {
            let folded = fold(c, true);
            if folded != c {
                table.entry(folded).or_default().push(c);
            }
        }
        table
    });
    UNFOLD.get(&l).map_or(&[], Vec::as_slice)
}

/// CPython's `re._casefix._EXTRA_CASES`: pairs of lowercase characters
/// that `re.IGNORECASE` treats as equal although their lowercase forms
/// differ, because they share an uppercase form (`s` and `ſ`, `k` and
/// the Kelvin sign's lowercase `k`, the Greek symbol variants, ...).
fn extra_cases(l: char) -> &'static [char] {
    match l {
        '\u{69}' => &['\u{131}'],
        '\u{73}' => &['\u{17f}'],
        '\u{b5}' => &['\u{3bc}'],
        '\u{131}' => &['\u{69}'],
        '\u{17f}' => &['\u{73}'],
        '\u{345}' => &['\u{3b9}', '\u{1fbe}'],
        '\u{390}' => &['\u{1fd3}'],
        '\u{3b0}' => &['\u{1fe3}'],
        '\u{3b2}' => &['\u{3d0}'],
        '\u{3b5}' => &['\u{3f5}'],
        '\u{3b8}' => &['\u{3d1}'],
        '\u{3b9}' => &['\u{345}', '\u{1fbe}'],
        '\u{3ba}' => &['\u{3f0}'],
        '\u{3bc}' => &['\u{b5}'],
        '\u{3c0}' => &['\u{3d6}'],
        '\u{3c1}' => &['\u{3f1}'],
        '\u{3c2}' => &['\u{3c3}'],
        '\u{3c3}' => &['\u{3c2}'],
        '\u{3c6}' => &['\u{3d5}'],
        '\u{3d0}' => &['\u{3b2}'],
        '\u{3d1}' => &['\u{3b8}'],
        '\u{3d5}' => &['\u{3c6}'],
        '\u{3d6}' => &['\u{3c0}'],
        '\u{3f0}' => &['\u{3ba}'],
        '\u{3f1}' => &['\u{3c1}'],
        '\u{3f5}' => &['\u{3b5}'],
        '\u{432}' => &['\u{1c80}'],
        '\u{434}' => &['\u{1c81}'],
        '\u{43e}' => &['\u{1c82}'],
        '\u{441}' => &['\u{1c83}'],
        '\u{442}' => &['\u{1c84}', '\u{1c85}'],
        '\u{44a}' => &['\u{1c86}'],
        '\u{463}' => &['\u{1c87}'],
        '\u{1c80}' => &['\u{432}'],
        '\u{1c81}' => &['\u{434}'],
        '\u{1c82}' => &['\u{43e}'],
        '\u{1c83}' => &['\u{441}'],
        '\u{1c84}' => &['\u{442}', '\u{1c85}'],
        '\u{1c85}' => &['\u{442}', '\u{1c84}'],
        '\u{1c86}' => &['\u{44a}'],
        '\u{1c87}' => &['\u{463}'],
        '\u{1c88}' => &['\u{a64b}'],
        '\u{1e61}' => &['\u{1e9b}'],
        '\u{1e9b}' => &['\u{1e61}'],
        '\u{1fbe}' => &['\u{345}', '\u{3b9}'],
        '\u{1fd3}' => &['\u{390}'],
        '\u{1fe3}' => &['\u{3b0}'],
        '\u{a64b}' => &['\u{1c88}'],
        '\u{fb05}' => &['\u{fb06}'],
        '\u{fb06}' => &['\u{fb05}'],
        _ => &[],
    }
}

/// CPython's category tests: the `UNI_` variants in Unicode mode, the
/// ASCII ones otherwise.
fn in_category(c: char, category: ChCode, unicode: bool) -> bool {
    match category {
        ChCode::Digit => is_digit(c, unicode),
        ChCode::NotDigit => !is_digit(c, unicode),
        ChCode::Space => is_space(c, unicode),
        ChCode::NotSpace => !is_space(c, unicode),
        ChCode::Word => is_word(c, unicode),
        ChCode::NotWord => !is_word(c, unicode),
    }
}

fn is_digit(c: char, unicode: bool) -> bool {
    if unicode {
        unicodedata::is_in_group_char(c, "Nd")
    } else {
        c.is_ascii_digit()
    }
}

fn is_space(c: char, unicode: bool) -> bool {
    if unicode {
        matches!(
            c,
            ' ' | '\t'
                | '\n'
                | '\r'
                | '\x0b'
                | '\x0c'
                | '\x1c'
                | '\x1d'
                | '\x1e'
                | '\x1f'
                | '\u{85}'
        ) || unicodedata::is_in_group_char(c, "Z")
    } else {
        matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c')
    }
}

fn is_word(c: char, unicode: bool) -> bool {
    if unicode {
        c == '_' || unicodedata::is_in_group_char(c, "L") || unicodedata::is_in_group_char(c, "N")
    } else {
        c == '_' || c.is_ascii_alphanumeric()
    }
}

fn at_matches(at: AtCode, text: &[char], pos: usize, flags: u32) -> bool {
    let multiline = flags & SRE_FLAG_MULTILINE != 0;
    match at {
        AtCode::BeginningString => pos == 0,
        AtCode::Beginning => pos == 0 || (multiline && text[pos - 1] == '\n'),
        AtCode::EndString => pos == text.len(),
        AtCode::End => {
            pos == text.len() || (text[pos] == '\n' && (multiline || pos + 1 == text.len()))
        }
        AtCode::Boundary => is_word_boundary(text, pos, flags),
        AtCode::NonBoundary => !is_word_boundary(text, pos, flags),
    }
}

fn is_word_boundary(text: &[char], pos: usize, flags: u32) -> bool {
    let unicode = flags & SRE_FLAG_UNICODE != 0;
    let before = pos > 0 && is_word(text[pos - 1], unicode);
    let after = pos < text.len() && is_word(text[pos], unicode);
    before != after
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/re_matcher_tests.rs"]
mod tests;
