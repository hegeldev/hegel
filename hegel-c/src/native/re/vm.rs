//! A simulation of a parsed pattern on all of its paths at once, in the
//! manner of Pike's VM: the pattern is compiled to a small program whose
//! alternatives and repeats are splits, and the simulation advances a
//! list of threads through the text one character at a time, keeping
//! them in the order CPython's backtracking would try them. The first
//! thread to reach the end of the program in that order gives the same
//! match end as [`Matcher`](super::matcher::Matcher) does, in time
//! proportional to the text rather than to the paths through it.
//!
//! The program can express patterns without backreferences, conditional
//! groups, atomic groups or possessive repeats, whose repeated items
//! cannot match empty (where CPython has special rules for stopping),
//! and whose unrolled size stays within [`PROGRAM_LIMIT`]; anything else
//! is [`Unsupported`] and stays with the backtracking matcher. The
//! simulation costs more per character than backtracking does on the
//! common run of a pattern, so it earns its keep on the patterns where
//! backtracking can go quadratic: a repeat nested in a repeat (see
//! [`has_nested_repeats`]).
//!
//! Because the threads alive at any position are the whole state of a
//! run, a run that reaches the end of its text with threads still
//! waiting for input can hand them back ([`Resume`]) and be carried on
//! later over a longer text that begins the same way, instead of
//! starting over.

use super::constants::{AtCode, MAXREPEAT, SRE_FLAG_DOTALL};
use super::matcher::{Touch, at_matches, combine_flags, literal_eq, set_matches};
use super::parser::{OpCode, ParsedPattern, SetItem, SubPattern};
use super::width::get_width;
use crate::sys::sync::Mutex;
use alloc::vec::Vec;

/// The most instructions a program may have, counting the repeats it
/// unrolls.
const PROGRAM_LIMIT: usize = 4096;

/// Marks a stack entry in [`Program::follow`] that only adds its
/// instruction to the thread list, so a lazy loop's own thread can come
/// after everything reachable from its exit.
const EMIT: usize = 1 << (usize::BITS - 1);

/// The pattern uses a construct the simulation cannot express.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsupported;

/// A single-character test.
#[derive(Clone)]
enum Unit {
    Literal(u32),
    NotLiteral(u32),
    Any,
    /// A set, with which ASCII characters it holds (under the flags of
    /// the instruction it belongs to) as a bitmap, filled in by
    /// [`Compiler::emit`].
    In(Vec<SetItem>, [u64; 2]),
}

impl Unit {
    fn matches(&self, c: char, flags: u32) -> bool {
        match self {
            Unit::Literal(cp) => literal_eq(*cp, c, flags),
            Unit::NotLiteral(cp) => !literal_eq(*cp, c, flags),
            Unit::Any => c != '\n' || flags & SRE_FLAG_DOTALL != 0,
            Unit::In(items, ascii) => match u32::from(c) {
                cp if cp < 128 => ascii[(cp >> 6) as usize] >> (cp & 63) & 1 != 0,
                _ => set_matches(items, c, flags),
            },
        }
    }
}

fn ascii_table(items: &[SetItem], flags: u32) -> [u64; 2] {
    let mut table = [0; 2];
    for cp in 0..128u32 {
        if set_matches(items, char::from_u32(cp).unwrap_or('\0'), flags) {
            table[(cp >> 6) as usize] |= 1 << (cp & 63);
        }
    }
    table
}

#[derive(Clone)]
enum Inst {
    Unit(Unit, u32),
    /// An unbounded repeat of one unit: consume a character and stay,
    /// or continue after; `greedy` says which comes first.
    Loop(Unit, u32, bool),
    At(AtCode, u32),
    /// A lookaround: whether the program at `entry` matches at this
    /// position (ahead), or ends exactly here from one of the widths
    /// `behind` back.
    Look {
        negate: bool,
        behind: Option<(usize, usize)>,
        entry: usize,
    },
    /// Continue at both targets, the first in preference.
    Split(usize, usize),
    Jmp(usize),
    Fail,
    Match,
}

/// A compiled pattern.
pub struct Program {
    insts: Vec<Inst>,
    /// Thread lists from earlier runs, kept to save allocating them again.
    scratch: Mutex<Vec<Scratch>>,
}

impl Clone for Program {
    fn clone(&self) -> Program {
        Program {
            insts: self.insts.clone(),
            scratch: Mutex::new(Vec::new()),
        }
    }
}

/// Whether `p` repeats something that itself contains a repeat, the
/// shape on which backtracking can take time quadratic in the text (each
/// way of splitting a run between the inner and outer repeats is tried
/// in turn when what follows fails).
pub fn has_nested_repeats(p: &SubPattern) -> bool {
    p.data.iter().any(|op| match op {
        OpCode::MaxRepeat { item, .. }
        | OpCode::MinRepeat { item, .. }
        | OpCode::PossessiveRepeat { item, .. } => has_repeat(item),
        OpCode::Branch(alternatives) => alternatives.iter().any(has_nested_repeats),
        OpCode::Subpattern { p, .. } | OpCode::AtomicGroup(p) => has_nested_repeats(p),
        _ => false,
    })
}

fn has_repeat(p: &SubPattern) -> bool {
    p.data.iter().any(|op| match op {
        OpCode::MaxRepeat { .. } | OpCode::MinRepeat { .. } | OpCode::PossessiveRepeat { .. } => {
            true
        }
        OpCode::Branch(alternatives) => alternatives.iter().any(has_repeat),
        OpCode::Subpattern { p, .. } | OpCode::AtomicGroup(p) => has_repeat(p),
        _ => false,
    })
}

struct Compiler<'p> {
    insts: Vec<Inst>,
    /// Lookaround bodies still to compile, with the instruction that
    /// refers to each.
    pending: Vec<(usize, &'p SubPattern, u32)>,
}

impl<'p> Compiler<'p> {
    fn emit(&mut self, mut inst: Inst) -> Result<usize, Unsupported> {
        if self.insts.len() >= PROGRAM_LIMIT {
            return Err(Unsupported);
        }
        if let Inst::Unit(Unit::In(items, ascii), flags)
        | Inst::Loop(Unit::In(items, ascii), flags, _) = &mut inst
        {
            *ascii = ascii_table(items, *flags);
        }
        self.insts.push(inst);
        Ok(self.insts.len() - 1)
    }

    fn seq(&mut self, p: &'p SubPattern, flags: u32) -> Result<(), Unsupported> {
        for op in &p.data {
            if let Some(unit) = unit_of(op) {
                self.emit(Inst::Unit(unit, flags))?;
                continue;
            }
            match op {
                OpCode::At(at) => self.emit(Inst::At(*at, flags))?,
                OpCode::Failure => self.emit(Inst::Fail)?,
                OpCode::Branch(alternatives) => {
                    self.branch(alternatives, flags)?;
                    continue;
                }
                OpCode::Subpattern {
                    add_flags,
                    del_flags,
                    p,
                    ..
                } => {
                    self.seq(p, combine_flags(flags, *add_flags, *del_flags))?;
                    continue;
                }
                OpCode::Assert { direction, p } => self.look(false, *direction, p, flags)?,
                OpCode::AssertNot { direction, p } => self.look(true, *direction, p, flags)?,
                OpCode::MaxRepeat { min, max, item } => {
                    self.repeat(item, *min, *max, true, flags)?;
                    continue;
                }
                OpCode::MinRepeat { min, max, item } => {
                    self.repeat(item, *min, *max, false, flags)?;
                    continue;
                }
                OpCode::GroupRef(_)
                | OpCode::GroupRefExists { .. }
                | OpCode::AtomicGroup(_)
                | OpCode::PossessiveRepeat { .. } => return Err(Unsupported),
                OpCode::Literal(_) | OpCode::NotLiteral(_) | OpCode::Any | OpCode::In(_) => {
                    unreachable!("units are compiled above")
                }
            };
        }
        Ok(())
    }

    fn branch(&mut self, alternatives: &'p [SubPattern], flags: u32) -> Result<(), Unsupported> {
        let mut jumps = Vec::new();
        for (i, alternative) in alternatives.iter().enumerate() {
            if i + 1 == alternatives.len() {
                self.seq(alternative, flags)?;
                break;
            }
            let split = self.emit(Inst::Split(0, 0))?;
            self.seq(alternative, flags)?;
            jumps.push(self.emit(Inst::Jmp(0))?);
            self.insts[split] = Inst::Split(split + 1, self.insts.len());
        }
        let end = self.insts.len();
        for jump in jumps {
            self.insts[jump] = Inst::Jmp(end);
        }
        Ok(())
    }

    fn look(
        &mut self,
        negate: bool,
        direction: i32,
        p: &'p SubPattern,
        flags: u32,
    ) -> Result<usize, Unsupported> {
        let behind = (direction < 0).then(|| {
            let (lo, hi) = get_width(p);
            let lo = usize::try_from(lo).unwrap_or(usize::MAX);
            let hi = hi.map_or(usize::MAX, |hi| usize::try_from(hi).unwrap_or(usize::MAX));
            (lo, hi)
        });
        let index = self.emit(Inst::Look {
            negate,
            behind,
            entry: 0,
        })?;
        self.pending.push((index, p, flags));
        Ok(index)
    }

    fn repeat(
        &mut self,
        item: &'p SubPattern,
        min: u32,
        max: u32,
        greedy: bool,
        flags: u32,
    ) -> Result<(), Unsupported> {
        if get_width(item).0 == 0 {
            return Err(Unsupported);
        }
        for _ in 0..min {
            self.seq(item, flags)?;
        }
        if max == MAXREPEAT {
            if let Some((unit, unit_flags)) = single_unit(item, flags) {
                self.emit(Inst::Loop(unit, unit_flags, greedy))?;
                return Ok(());
            }
            let split = self.emit(Inst::Split(0, 0))?;
            self.seq(item, flags)?;
            self.emit(Inst::Jmp(split))?;
            self.insts[split] = split_inst(split + 1, self.insts.len(), greedy);
            return Ok(());
        }
        let mut splits = Vec::new();
        for _ in min..max {
            splits.push(self.emit(Inst::Split(0, 0))?);
            self.seq(item, flags)?;
        }
        let exit = self.insts.len();
        for split in splits {
            self.insts[split] = split_inst(split + 1, exit, greedy);
        }
        Ok(())
    }
}

fn unit_of(op: &OpCode) -> Option<Unit> {
    match op {
        OpCode::Literal(cp) => Some(Unit::Literal(*cp)),
        OpCode::NotLiteral(cp) => Some(Unit::NotLiteral(*cp)),
        OpCode::Any => Some(Unit::Any),
        OpCode::In(items) => Some(Unit::In(items.clone(), [0; 2])),
        _ => None,
    }
}

/// CPython's `_simple`: a one-op pattern that is a unit, or a
/// non-capturing group around one, with the flags in force inside it.
fn single_unit(p: &SubPattern, flags: u32) -> Option<(Unit, u32)> {
    match p.data.as_slice() {
        [
            OpCode::Subpattern {
                group: None,
                add_flags,
                del_flags,
                p,
            },
        ] => single_unit(p, combine_flags(flags, *add_flags, *del_flags)),
        [op] => unit_of(op).map(|unit| (unit, flags)),
        _ => None,
    }
}

fn split_inst(body: usize, exit: usize, greedy: bool) -> Inst {
    if greedy {
        Inst::Split(body, exit)
    } else {
        Inst::Split(exit, body)
    }
}

/// The threads still waiting for input at the end of a text, in order of
/// preference, with the match found by then (as an offset from where the
/// run started): enough to carry on when more text follows, see
/// [`Program::resume`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resume {
    pcs: Vec<usize>,
    found: Option<usize>,
}

/// The threads alive at one position, in order of preference, with a
/// generation stamp per instruction recording which list last took it.
struct Threads {
    pcs: Vec<usize>,
    marks: Vec<usize>,
}

struct Scratch {
    current: Threads,
    next: Threads,
    stack: Vec<usize>,
    generation: usize,
}

impl Scratch {
    fn new(size: usize) -> Scratch {
        let threads = || Threads {
            pcs: Vec::with_capacity(size),
            marks: alloc::vec![0; size],
        };
        Scratch {
            current: threads(),
            next: threads(),
            stack: Vec::with_capacity(size),
            generation: 0,
        }
    }
}

impl Program {
    /// Compile `parsed`, or report that it uses something the program
    /// cannot express.
    pub fn compile(parsed: &ParsedPattern) -> Result<Program, Unsupported> {
        let mut compiler = Compiler {
            insts: Vec::new(),
            pending: Vec::new(),
        };
        compiler.seq(&parsed.pattern, parsed.flags)?;
        compiler.emit(Inst::Match)?;
        while let Some((index, p, flags)) = compiler.pending.pop() {
            let entry = compiler.insts.len();
            compiler.seq(p, flags)?;
            compiler.emit(Inst::Match)?;
            if let Inst::Look { entry: target, .. } = &mut compiler.insts[index] {
                *target = entry;
            }
        }
        Ok(Program {
            insts: compiler.insts,
            scratch: Mutex::new(Vec::new()),
        })
    }

    fn take_scratch(&self) -> Scratch {
        self.scratch
            .lock()
            .pop()
            .unwrap_or_else(|| Scratch::new(self.insts.len()))
    }

    fn return_scratch(&self, scratch: Scratch) {
        self.scratch.lock().push(scratch);
    }

    /// The end of the match at `start` that CPython's
    /// `pattern.match(text, start)` finds, or with `exact_end` the one
    /// among the matches ending there that CPython's backtracking finds
    /// first (`pattern.fullmatch(text)` for the text's end), with how the
    /// match depended on the edges of the text (never [`Touch::Tail`]:
    /// the threads alive at the end are carried whole instead) and, when
    /// threads were still waiting for input there and nothing else
    /// looked at an edge, those threads, for [`resume`](Self::resume). A
    /// `start` past the end of the text is clamped to it, as CPython
    /// does.
    pub fn scan(
        &self,
        text: &[char],
        start: usize,
        exact_end: Option<usize>,
    ) -> (Option<usize>, Touch, Option<Resume>) {
        let mut scratch = self.take_scratch();
        let found = self.run_from(0, text, start.min(text.len()), exact_end, &mut scratch);
        self.return_scratch(scratch);
        found
    }

    /// Carry on a run that [`scan`](Self::scan) started at `start` of a
    /// text of `consumed` characters, on `text`, which continues that
    /// text after `start + consumed`: the same result as a run over all
    /// of `text` from `start` would give.
    pub fn resume(
        &self,
        resume: &Resume,
        text: &[char],
        start: usize,
        consumed: usize,
    ) -> (Option<usize>, Touch, Option<Resume>) {
        let mut scratch = self.take_scratch();
        scratch.current.pcs.clear();
        scratch.current.pcs.extend_from_slice(&resume.pcs);
        let found = resume.found.map(|found| start + found);
        let out = self.steps(
            text,
            start,
            start + consumed,
            None,
            found,
            Touch::Never,
            &mut scratch,
        );
        self.return_scratch(scratch);
        out
    }

    fn run_from(
        &self,
        entry: usize,
        text: &[char],
        start: usize,
        exact_end: Option<usize>,
        scratch: &mut Scratch,
    ) -> (Option<usize>, Touch, Option<Resume>) {
        let mut touch = Touch::Never;
        scratch.generation += 1;
        scratch.current.pcs.clear();
        let generation = scratch.generation;
        self.follow(
            &mut scratch.current,
            &mut scratch.stack,
            generation,
            entry,
            text,
            start,
            &mut touch,
        );
        self.steps(text, start, start, exact_end, None, touch, scratch)
    }

    /// Advance the threads in `scratch.current` from `pos` to the end of
    /// the text or until none is left.
    #[allow(clippy::too_many_arguments)]
    fn steps(
        &self,
        text: &[char],
        start: usize,
        mut pos: usize,
        exact_end: Option<usize>,
        mut found: Option<usize>,
        mut touch: Touch,
        scratch: &mut Scratch,
    ) -> (Option<usize>, Touch, Option<Resume>) {
        let Scratch {
            current,
            next,
            stack,
            generation,
        } = scratch;
        while !current.pcs.is_empty() {
            *generation += 1;
            next.pcs.clear();
            let c = text.get(pos).copied();
            let mut waiting = 0;
            for i in 0..current.pcs.len() {
                let pc = current.pcs[i];
                let target = match &self.insts[pc] {
                    Inst::Match => {
                        if exact_end.is_none_or(|end| end == pos) {
                            found = Some(pos);
                            break;
                        }
                        continue;
                    }
                    Inst::Unit(unit, flags) => {
                        c.is_some_and(|c| unit.matches(c, *flags)).then_some(pc + 1)
                    }
                    Inst::Loop(unit, flags, _) => {
                        c.is_some_and(|c| unit.matches(c, *flags)).then_some(pc)
                    }
                    Inst::At(..)
                    | Inst::Look { .. }
                    | Inst::Split(..)
                    | Inst::Jmp(_)
                    | Inst::Fail => {
                        unreachable!("only characters and matches wait for input")
                    }
                };
                waiting = i + 1;
                if let Some(target) = target {
                    self.follow(next, stack, *generation, target, text, pos + 1, &mut touch);
                }
            }
            if c.is_none() {
                if waiting == 0 {
                    return (found, touch, None);
                }
                let resume = (touch == Touch::Never && exact_end.is_none()).then(|| Resume {
                    pcs: current.pcs[..waiting].to_vec(),
                    found: found.map(|found| found - start),
                });
                return (found, Touch::End, resume);
            }
            core::mem::swap(current, next);
            pos += 1;
        }
        (found, touch, None)
    }

    /// Add to `list` the threads reachable from `pc` at `pos` without
    /// consuming input, in order of preference.
    fn follow(
        &self,
        list: &mut Threads,
        stack: &mut Vec<usize>,
        generation: usize,
        pc: usize,
        text: &[char],
        pos: usize,
        touch: &mut Touch,
    ) {
        stack.clear();
        stack.push(pc);
        while let Some(pc) = stack.pop() {
            if pc & EMIT != 0 {
                list.pcs.push(pc & !EMIT);
                continue;
            }
            if list.marks[pc] == generation {
                continue;
            }
            list.marks[pc] = generation;
            match &self.insts[pc] {
                Inst::Jmp(target) => stack.push(*target),
                Inst::Split(first, second) => {
                    stack.push(*second);
                    stack.push(*first);
                }
                Inst::At(at, flags) => {
                    if pos == 0
                        || (!matches!(at, AtCode::Beginning | AtCode::BeginningString)
                            && pos + 1 >= text.len())
                    {
                        *touch = Touch::End;
                    }
                    if at_matches(*at, text, pos, *flags) {
                        stack.push(pc + 1);
                    }
                }
                Inst::Look {
                    negate,
                    behind,
                    entry,
                } => {
                    let (matched, inner) = self.look(*behind, *entry, text, pos);
                    if behind.is_some_and(|(_, hi)| hi > pos) || inner != Touch::Never {
                        *touch = Touch::End;
                    }
                    if matched != *negate {
                        stack.push(pc + 1);
                    }
                }
                Inst::Fail => {}
                Inst::Loop(_, _, greedy) => {
                    if *greedy {
                        list.pcs.push(pc);
                        stack.push(pc + 1);
                    } else {
                        stack.push(pc | EMIT);
                        stack.push(pc + 1);
                    }
                }
                Inst::Unit(..) | Inst::Match => list.pcs.push(pc),
            }
        }
    }

    /// Whether the lookaround body at `entry` matches at `pos`: ahead of
    /// it, or ending exactly at it from one of its widths `behind` back
    /// (CPython only accepts fixed-width lookbehinds, which the matcher
    /// extends to any of a variable width's widths).
    fn look(
        &self,
        behind: Option<(usize, usize)>,
        entry: usize,
        text: &[char],
        pos: usize,
    ) -> (bool, Touch) {
        let mut scratch = self.take_scratch();
        let mut touch = Touch::Never;
        let mut run = |start: usize, exact_end: Option<usize>| {
            let (found, inner, _) = self.run_from(entry, text, start, exact_end, &mut scratch);
            if inner != Touch::Never {
                touch = Touch::End;
            }
            found.is_some()
        };
        let found = match behind {
            None => run(pos, None),
            Some((lo, hi)) => (lo..=hi.min(pos)).any(|w| run(pos - w, Some(pos))),
        };
        self.return_scratch(scratch);
        (found, touch)
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/re_vm_tests.rs"]
mod tests;
