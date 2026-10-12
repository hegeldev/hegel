use crate::control::{InternalError, hegel_internal_error, hegel_internal_unwrap};
use crate::native::{HashMap, HashSet};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use hashbrown::hash_map::Entry;

use crate::native::core::state::{boolean_threshold, length_p_continue};
use crate::native::core::{Constraint, EngineError, ManyState, Status, StringChoice};
use crate::native::draws::source::{RngDraws, SimplestDraws};
use crate::native::intervalsets::IntervalSet;
use crate::native::re::casefold;
use crate::native::re::constants::{
    AtCode, ChCode, MAXREPEAT, SRE_FLAG_ASCII, SRE_FLAG_DOTALL, SRE_FLAG_IGNORECASE,
    SRE_FLAG_MULTILINE,
};
use crate::native::re::constraint::{RegexConstraint, matches as pattern_matches};
use crate::native::re::matcher::combine_flags;
use crate::native::re::parser::{OpCode, ParsedPattern, SetItem, SubPattern, parse_pattern};
use crate::native::rng::EngineRng;
use crate::sys::sync::{Lazy, Mutex};
use crate::unicodedata::{self, Category};
use alloc::string::ToString;

use super::{Draws, many_more};

fn is_surrogate_cp(cp: u32) -> bool {
    (0xD800..=0xDFFF).contains(&cp)
}

/// The flag bits that change which characters a literal or class matches.
const CASE_FLAGS: u32 = SRE_FLAG_IGNORECASE | SRE_FLAG_ASCII;

/// Cache key for a `(IN, items)` node's character set: the `SetItem` slice's
/// address and length (stable because the AST is owned by the enclosing
/// [`CompiledRegex`]) plus the [`CASE_FLAGS`] in force (which affect
/// IGNORECASE folding and the ASCII-only categories).
type InKey = (usize, usize, u32);

/// Cache key for the alphabet-constrained character sets of the nodes that
/// emit one character without a class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CharKey {
    /// A literal's candidates under the [`CASE_FLAGS`] in force.
    Literal(char, u32),
    /// A negated literal's characters under the [`CASE_FLAGS`] in force.
    NotLiteral(char, u32),
    /// The wildcard's characters, with the newline or without.
    Any(bool),
}

/// Cache key for a subpattern's producibility: the address of its op slice
/// (stable, see [`InKey`]) plus the active flags.
type ProducibleKey = (usize, u32);

/// A [`CompiledRegex`]'s cross-draw caches of the alphabet-constrained
/// character sets (category classes like `\w` cost a full alphabet scan to
/// materialise) and of which subpatterns the alphabet can produce at all.
#[derive(Debug)]
pub(crate) struct Caches {
    in_cache: Mutex<HashMap<InKey, InSet>>,
    char_cache: Mutex<HashMap<CharKey, Arc<[char]>>>,
    producible: Mutex<HashMap<ProducibleKey, bool>>,
}

impl Default for Caches {
    fn default() -> Self {
        Caches {
            in_cache: Mutex::new(HashMap::default()),
            char_cache: Mutex::new(HashMap::default()),
            producible: Mutex::new(HashMap::default()),
        }
    }
}

/// The characters a `(IN, items)` node can emit, in codepoint order.
#[derive(Debug, Clone, PartialEq)]
enum InSet {
    /// Every character the node can emit, listed.
    Listed(Arc<[char]>),
    /// The BMP minus a few excluded codepoints: what a negated class
    /// without categories rules out under the default alphabet, indexed in
    /// closed form like [`default_any_char`] so the 64K-codepoint scan that
    /// would list the set never runs.
    Complement(Arc<Complement>),
}

impl InSet {
    /// Whether the node can emit no character at all.
    fn is_empty(&self) -> bool {
        match self {
            InSet::Listed(chars) => chars.is_empty(),
            InSet::Complement(complement) => complement.len == 0,
        }
    }
}

/// The BMP minus the codepoint ranges in `excluded`, which are sorted,
/// within the BMP, separated by at least one codepoint, and always include
/// the surrogates.
#[derive(Debug, PartialEq)]
struct Complement {
    excluded: Vec<(u32, u32)>,
    len: u64,
}

impl Complement {
    /// The complement of `excluded`, whose ranges may touch or overlap:
    /// they are merged, so that indexing walks one range per gap.
    fn new(mut excluded: Vec<(u32, u32)>) -> Self {
        excluded.push((0xD800, 0xDFFF));
        excluded.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(excluded.len());
        for (lo, hi) in excluded {
            match merged.last_mut() {
                Some(last) if lo <= last.1 + 1 => last.1 = last.1.max(hi),
                _ => merged.push((lo, hi)),
            }
        }
        let removed: u64 = merged.iter().map(|(lo, hi)| u64::from(hi - lo) + 1).sum();
        Complement {
            excluded: merged,
            len: 0x10000 - removed,
        }
    }

    /// The `idx`th remaining character in codepoint order.
    fn get(&self, idx: u64) -> Result<char, InternalError> {
        let mut cp = hegel_internal_unwrap!(
            u32::try_from(idx).ok(),
            "Complement::get: index {idx} is not a codepoint"
        );
        for &(lo, hi) in &self.excluded {
            if cp < lo {
                break;
            }
            cp += hi - lo + 1;
        }
        codepoint_to_char(cp)
    }
}

/// A regex pattern compiled once at string-generator construction time:
/// the parsed AST, the optional user alphabet, and the [`Caches`] shared
/// by every draw from it.
#[derive(Debug)]
pub struct CompiledRegex {
    pattern: String,
    parsed: ParsedPattern,
    alphabet: Option<IntervalSet>,
    /// The one string the pattern matches whole, when it is a literal
    /// (see [`literal_of`]).
    literal: Option<Box<[char]>>,
    /// Whether a drawn string is checked against the pattern with the
    /// matcher before it is accepted (see [`approximated`]).
    verified: bool,
    /// Whether the alphabet cannot supply some subpattern (see
    /// [`producible_sub`]), so that a draw chooses among an alternation's
    /// producible branches and skips an optional repeat of an
    /// unproducible body; when it can supply every one, those lookups
    /// are skipped.
    unproducible: bool,
    caches: Caches,
}

impl CompiledRegex {
    /// The length of the strings drawn from the pattern on average (see
    /// [`typical_width`]).
    pub(crate) fn typical_width(&self) -> f64 {
        typical_width(&self.parsed.pattern)
    }

    /// The parsed pattern.
    pub(crate) fn parsed(&self) -> &ParsedPattern {
        &self.parsed
    }

    /// Parse `pattern`, reporting an invalid-argument diagnostic so a bad
    /// pattern surfaces at construction time rather than mid-draw.
    pub(crate) fn compile(
        pattern: &str,
        alphabet: Option<IntervalSet>,
    ) -> Result<Self, EngineError> {
        let parsed = parse_pattern(pattern, 0).map_err(|e| {
            EngineError::InvalidArgument(format!("invalid regex pattern {pattern:?}: {e}"))
        })?;
        let literal = literal_of(&parsed, &alphabet);
        let verified = approximated(&parsed.pattern);
        let caches = Caches::default();
        producible_sub(&parsed.pattern, parsed.flags, &alphabet, &caches)?;
        let unproducible = caches
            .producible
            .lock()
            .values()
            .any(|&producible| !producible);
        Ok(CompiledRegex {
            pattern: pattern.to_string(),
            parsed,
            alphabet,
            literal,
            verified,
            unproducible,
            caches,
        })
    }
}

impl CompiledRegex {
    /// The string choice a string drawn from the pattern is recorded as:
    /// over the alphabet (every scalar value when there is none), up to
    /// [`REGEX_MAX_SIZE`] long, and under the pattern as its constraint.
    pub(crate) fn kind(&self, fullmatch: bool) -> Result<StringChoice, EngineError> {
        let intervals = match &self.alphabet {
            Some(alphabet) => alphabet.clone(),
            None => IntervalSet::unicode_scalars()?,
        };
        let constraint = RegexConstraint::new(
            &self.pattern,
            self.parsed.clone(),
            fullmatch,
            self.alphabet.as_ref(),
        );
        Ok(StringChoice {
            intervals: Arc::new(intervals),
            min_size: 0,
            max_size: REGEX_MAX_SIZE,
            constraint: Some(Constraint::Regex(Arc::new(constraint))),
        })
    }
}

/// The most characters a string drawn from a pattern may have, so that
/// the choice recording it has a bound; a draw past it is invalid.
pub(crate) const REGEX_MAX_SIZE: usize = 1 << 16;

/// Seeds tried for the simplest string when the simplest walk fails a
/// deferred check, before giving the pattern up.
const SIMPLEST_FALLBACK_SEEDS: u64 = 8;

/// Set `kind`'s constraint's simplest string if it is not yet: the
/// string drawn when every decision goes the simplest way, or, when that
/// fails a deferred check (padding a `\b` needs, say), the first of a
/// few seeded draws that does not. A pattern with no string either way
/// concludes the test case invalid, as a draw that finds no string does.
pub(crate) fn ensure_simplest<D: Draws>(
    d: &mut D,
    re: &CompiledRegex,
    fullmatch: bool,
    kind: &StringChoice,
) -> Result<(), EngineError> {
    let Some(Constraint::Regex(constraint)) = &kind.constraint else {
        hegel_internal_error!("ensure_simplest: the kind is not under a regex");
    };
    let found = constraint.simplest_or_init(|| {
        if let Ok(simplest) = generate_regex(&mut SimplestDraws::default(), re, fullmatch) {
            return Ok(codepoints(&simplest));
        }
        for seed in 0..SIMPLEST_FALLBACK_SEEDS {
            let mut rng = EngineRng::seeded(seed);
            let mut draws = RngDraws::new(&mut rng, Default::default());
            if let Ok(simplest) = generate_regex(&mut draws, re, fullmatch) {
                return Ok(codepoints(&simplest));
            }
        }
        Err(())
    });
    if found.is_err() {
        return Err(mark_invalid(d));
    }
    Ok(())
}

fn codepoints(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}

/// Draw a string matching `re` from `d` as the codepoints a string choice
/// records, concluding the test case invalid when it is over `max_size`.
pub(crate) fn draw_regex<D: Draws>(
    d: &mut D,
    re: &CompiledRegex,
    fullmatch: bool,
    max_size: usize,
) -> Result<Vec<u32>, EngineError> {
    let mut out = Vec::with_capacity(TYPICAL_CAPACITY);
    generate_regex_into(d, re, fullmatch, &mut out)?;
    if out.len() > max_size {
        return Err(mark_invalid(d));
    }
    Ok(out.into_iter().map(u32::from).collect())
}

/// The characters a drawn string starts out with room for, so that one
/// of ordinary length is built without the buffer growing.
const TYPICAL_CAPACITY: usize = 64;

/// Draw a string matching `re`, anchored at both ends when `fullmatch`
/// and otherwise padded with draws from the compiled alphabet (or the full
/// codespace when it has none).
///
/// Candidates whose deferred checks fail (a `\b` that the padding broke, a
/// possessive repeat whose committed count the pattern can't actually
/// match, ...) are retried a few times within the same test case — the
/// analogue of Hypothesis's strategy-level `.filter(re.search)` retries —
/// before the whole test case is marked invalid.
pub(crate) fn generate_regex<D: Draws>(
    d: &mut D,
    re: &CompiledRegex,
    fullmatch: bool,
) -> Result<String, EngineError> {
    let mut out = Vec::with_capacity(TYPICAL_CAPACITY);
    generate_regex_into(d, re, fullmatch, &mut out)?;
    Ok(out.into_iter().collect())
}

/// Draw a string matching `re` onto the end of `out`, whose earlier text
/// the pattern never sees: an anchor at the start of the pattern is at
/// the start of the drawn string. What an attempt that fails a deferred
/// check or errors added is removed again.
pub(crate) fn generate_regex_into<D: Draws>(
    d: &mut D,
    re: &CompiledRegex,
    fullmatch: bool,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    const MAX_ATTEMPTS: usize = 5;
    if fullmatch {
        if let Some(literal) = &re.literal {
            out.extend_from_slice(literal);
            return Ok(());
        }
    }
    let base = out.len();
    for _ in 0..MAX_ATTEMPTS {
        match generate_regex_attempt(d, re, fullmatch, out, base) {
            Ok(true) => return Ok(()),
            Ok(false) => out.truncate(base),
            Err(e) => {
                out.truncate(base);
                return Err(e);
            }
        }
    }
    Err(mark_invalid(d))
}

/// The text a pattern of literal characters alone, under no case
/// folding, matches whole, when every character is in the alphabet: what
/// a whole-match draw from it emits without walking it. `None` for any
/// other pattern, which is walked.
fn literal_of(parsed: &ParsedPattern, alphabet: &Option<IntervalSet>) -> Option<Box<[char]>> {
    if parsed.flags & SRE_FLAG_IGNORECASE != 0 {
        return None;
    }
    parsed
        .pattern
        .data
        .iter()
        .map(|op| match op {
            OpCode::Literal(cp) => char::from_u32(*cp).filter(|&c| alphabet_allows(alphabet, c)),
            _ => None,
        })
        .collect()
}

/// One generation attempt, appending to `out` from `base` on. Returns
/// `Ok(false)` when the candidate was generated but failed a deferred
/// check against the final string.
fn generate_regex_attempt<D: Draws>(
    d: &mut D,
    re: &CompiledRegex,
    fullmatch: bool,
    out: &mut Vec<char>,
    base: usize,
) -> Result<bool, EngineError> {
    let parsed = &re.parsed;
    let alphabet = &re.alphabet;

    let mut state = GenState {
        groups: HashMap::default(),
        capture: parsed.group_refs,
        flags: parsed.flags,
        fullmatch,
        base,
        deferred: None,
        unproducible: re.unproducible,
        caches: &re.caches,
    };

    if parsed.pattern.is_empty() {
        if !fullmatch {
            draw_pad(d, alphabet, out)?;
        }
        return Ok(true);
    }

    if !fullmatch {
        draw_prefix(d, parsed, alphabet, out, base)?;
    }
    generate_subpattern(d, &parsed.pattern, &mut state, alphabet, out)?;
    if !fullmatch {
        draw_suffix(d, parsed, alphabet, out)?;
    }

    if let Some(deferred) = &state.deferred {
        let final_chars = &out[base..];
        for anchor in &deferred.pending_anchors {
            if !at_matches(&anchor.at, final_chars, anchor.char_pos, anchor.flags) {
                return Ok(false);
            }
        }
        for pending in &deferred.pending_asserts {
            let holds = if pending.direction >= 0 {
                match_seq(
                    &pending.pattern.data,
                    pending.char_pos,
                    final_chars,
                    pending.flags,
                    &pending.groups,
                )
                .is_some()
            } else {
                (0..=pending.char_pos).any(|start| {
                    match_seq(
                        &pending.pattern.data,
                        start,
                        final_chars,
                        pending.flags,
                        &pending.groups,
                    ) == Some(pending.char_pos)
                })
            };
            if !holds {
                return Ok(false);
            }
        }
        for pending in &deferred.pending_lookaheads {
            let matches = if pending.direction >= 0 {
                match_seq(
                    &pending.pattern.data,
                    pending.char_pos,
                    final_chars,
                    pending.flags,
                    &pending.groups,
                )
                .is_some()
            } else {
                (0..=pending.char_pos).any(|start| {
                    match_seq(
                        &pending.pattern.data,
                        start,
                        final_chars,
                        pending.flags,
                        &pending.groups,
                    ) == Some(pending.char_pos)
                })
            };
            if matches {
                return Ok(false);
            }
        }
    }
    if re.verified && !pattern_matches(parsed, fullmatch, &out[base..]) {
        return Ok(false);
    }

    Ok(true)
}

/// Whether `ops` has a construct the generator only approximates, so that
/// what it draws has to be checked with the matcher: position assertions
/// on word boundaries and lookarounds, whose deferred checks are an
/// approximation of the matcher's; back references and conditionals,
/// whose text is copied from a group the pattern need not match the same
/// way; and atomic groups and possessive repeats, which commit to a count
/// the pattern need not admit. Characters, classes and literals are drawn
/// from exactly the characters the matcher accepts (see [`casefold`]),
/// so a pattern of those alone is not checked.
fn approximated(ops: &SubPattern) -> bool {
    ops.data.iter().any(|op| match op {
        OpCode::At(AtCode::Boundary | AtCode::NonBoundary)
        | OpCode::GroupRef(_)
        | OpCode::GroupRefExists { .. }
        | OpCode::Assert { .. }
        | OpCode::AssertNot { .. }
        | OpCode::AtomicGroup(_)
        | OpCode::PossessiveRepeat { .. } => true,
        OpCode::Subpattern { p, .. } => approximated(p),
        OpCode::Branch(items) => items.iter().any(approximated),
        OpCode::MaxRepeat { item, .. } | OpCode::MinRepeat { item, .. } => approximated(item),
        OpCode::Literal(_)
        | OpCode::NotLiteral(_)
        | OpCode::In(_)
        | OpCode::Any
        | OpCode::At(_)
        | OpCode::Failure => false,
    })
}

/// Mutable state threaded through generation: captured groups (for
/// back-references) and the active regex flags (which change as we descend
/// into `SUBPATTERN` nodes with inline flag modifiers).
struct GenState<'a> {
    groups: HashMap<u32, Vec<char>>,
    /// Whether a back-reference or conditional group somewhere in the
    /// pattern reads `groups` back (see [`ParsedPattern::group_refs`]);
    /// when nothing does, a capturing group's text is not kept.
    capture: bool,
    flags: u32,
    /// Whether the draw is anchored at both ends. Affects how lookaround
    /// assertions are generated: in fullmatch mode their bodies must not be
    /// emitted (the pattern has to consume the entire output), so they become
    /// deferred checks instead.
    fullmatch: bool,
    /// Where in the output buffer the drawn string starts: the text
    /// before it belongs to the caller, and the anchors and deferred
    /// checks never look at it.
    base: usize,
    /// The checks against the final string the draw has recorded so far,
    /// once it has recorded one; most patterns record none, so the draw
    /// neither builds nor drops them.
    deferred: Option<Box<Deferred<'a>>>,
    /// Whether some subpattern is unproducible (see
    /// [`CompiledRegex::unproducible`]).
    unproducible: bool,
    /// The enclosing [`CompiledRegex`]'s cross-draw caches.
    caches: &'a Caches,
}

impl<'a> GenState<'a> {
    /// The draw's deferred checks, to record one.
    fn deferred(&mut self) -> &mut Deferred<'a> {
        self.deferred.get_or_insert_with(Default::default)
    }
}

/// What a draw checks against its final string once that is known, since
/// the content after an anchor or a lookaround is not known when they are
/// reached; a candidate that fails one is redrawn.
#[derive(Default)]
struct Deferred<'a> {
    /// Zero-width anchors (`\b`, `\B`, and `$`/`\Z` in non-final positions)
    /// recorded during generation.
    pending_anchors: Vec<PendingAnchor>,
    /// Positive lookaround assertions deferred in fullmatch mode.
    pending_asserts: Vec<PendingAssertNot<'a>>,
    /// Negative lookaround assertions recorded during generation. Each entry
    /// captures the assertion body, the active flags, and a snapshot of the
    /// groups at the point of the assertion.
    pending_lookaheads: Vec<PendingAssertNot<'a>>,
}

struct PendingAnchor {
    char_pos: usize,
    at: AtCode,
    flags: u32,
}

struct PendingAssertNot<'a> {
    char_pos: usize,
    direction: i32,
    /// The assertion's body, in the pattern being drawn from.
    pattern: &'a SubPattern,
    flags: u32,
    groups: HashMap<u32, Vec<char>>,
}

/// Draw 0..10 arbitrary characters from `alphabet` (or ASCII 32..126 when
/// no alphabet is given). Used for prefix/suffix padding and for the
/// empty-pattern case.
fn draw_pad<D: Draws>(
    d: &mut D,
    alphabet: &Option<IntervalSet>,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    let n = d.draw_integer_i64(0, 10)?;
    for _ in 0..n {
        let c = draw_any_char(d, alphabet)?;
        out.push(c);
    }
    Ok(())
}

/// Return the first non-grouping OpCode in `sp`, descending through SUBPATTERN
/// and ATOMIC_GROUP nodes (which don't consume characters themselves).
///
/// Python's `regex_strategy` doesn't descend like this — it relies on
/// `regex.search` as a post-generation filter. Peeking through the
/// non-consuming wrappers instead handles the common `(\Afoo\Z)` shape
/// without a candidate having to be matched and redrawn.
fn effective_first(sp: &SubPattern) -> Option<&OpCode> {
    let first = sp.data.first()?;
    match first {
        OpCode::Subpattern { p, .. } | OpCode::AtomicGroup(p) => effective_first(p),
        _ => Some(first),
    }
}

fn effective_last(sp: &SubPattern) -> Option<&OpCode> {
    let last = sp.data.last()?;
    match last {
        OpCode::Subpattern { p, .. } | OpCode::AtomicGroup(p) => effective_last(p),
        _ => Some(last),
    }
}

fn draw_prefix<D: Draws>(
    d: &mut D,
    parsed: &ParsedPattern,
    alphabet: &Option<IntervalSet>,
    out: &mut Vec<char>,
    base: usize,
) -> Result<(), EngineError> {
    if let Some(OpCode::At(at)) = effective_first(&parsed.pattern) {
        match at {
            AtCode::BeginningString => return Ok(()),
            AtCode::Beginning => {
                if parsed.flags & SRE_FLAG_MULTILINE != 0 && alphabet_allows(alphabet, '\n') {
                    draw_pad(d, alphabet, out)?;
                    if out.len() > base {
                        out.push('\n');
                    }
                }
                return Ok(());
            }
            _ => {}
        }
    }
    draw_pad(d, alphabet, out)
}

fn draw_suffix<D: Draws>(
    d: &mut D,
    parsed: &ParsedPattern,
    alphabet: &Option<IntervalSet>,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    if let Some(OpCode::At(at)) = effective_last(&parsed.pattern) {
        match at {
            AtCode::EndString => return Ok(()),
            AtCode::End => {
                if !alphabet_allows(alphabet, '\n') {
                    return Ok(());
                }
                if parsed.flags & SRE_FLAG_MULTILINE != 0 {
                    if d.weighted(0.5, None)? {
                        out.push('\n');
                        draw_pad(d, alphabet, out)?;
                    }
                } else if d.weighted(0.5, None)? {
                    out.push('\n');
                }
                return Ok(());
            }
            _ => {}
        }
    }
    draw_pad(d, alphabet, out)
}

/// Recursively generate a string from a `SubPattern`, appending to `out`.
fn generate_subpattern<'a, D: Draws>(
    d: &mut D,
    sp: &'a SubPattern,
    state: &mut GenState<'a>,
    alphabet: &Option<IntervalSet>,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    for op in &sp.data {
        generate_op(d, op, state, alphabet, out)?;
    }
    Ok(())
}
fn generate_op<'a, D: Draws>(
    d: &mut D,
    op: &'a OpCode,
    state: &mut GenState<'a>,
    alphabet: &Option<IntervalSet>,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    match op {
        OpCode::Literal(cp) => {
            let c = codepoint_to_char(*cp)?;
            if state.flags & SRE_FLAG_IGNORECASE == 0 || !casefold::is_cased(c, state.flags) {
                if !alphabet_allows(alphabet, c) {
                    return Err(mark_invalid(d));
                }
                out.push(c);
                return Ok(());
            }
            let candidates = cached_chars(
                &state.caches.char_cache,
                CharKey::Literal(c, state.flags & CASE_FLAGS),
                || literal_candidates(c, state.flags, alphabet),
            );
            if candidates.is_empty() {
                return Err(mark_invalid(d));
            }
            let idx = pick_index(d, candidates.len())?;
            out.push(candidates[idx]);
        }
        OpCode::NotLiteral(cp) => {
            let c = codepoint_to_char(*cp)?;
            let chars = not_literal_chars(c, state.flags, alphabet, state.caches);
            emit_from_chars(d, &chars, out)?;
        }
        OpCode::Any => {
            if alphabet.is_none() {
                let allow_newline = state.flags & SRE_FLAG_DOTALL != 0;
                let n = DEFAULT_ANY_LEN - u64::from(!allow_newline);
                let idx = draw_char_index(d, n)?;
                out.push(default_any_char(idx, allow_newline)?);
            } else {
                let chars = any_chars(state.flags, alphabet, state.caches);
                emit_from_chars(d, &chars, out)?;
            }
        }
        OpCode::At(at) => match at {
            AtCode::BeginningString if out.len() > state.base => {
                return Err(mark_invalid(d));
            }
            AtCode::BeginningString => {}
            AtCode::Beginning => {
                if state.flags & SRE_FLAG_MULTILINE != 0 {
                    if out.len() > state.base && out.last() != Some(&'\n') {
                        return Err(mark_invalid(d));
                    }
                } else if out.len() > state.base {
                    return Err(mark_invalid(d));
                }
            }
            AtCode::End | AtCode::EndString | AtCode::Boundary | AtCode::NonBoundary => {
                let anchor = PendingAnchor {
                    char_pos: out.len() - state.base,
                    at: *at,
                    flags: state.flags,
                };
                state.deferred().pending_anchors.push(anchor);
            }
        },
        OpCode::In(items) => {
            let mut cache = state.caches.in_cache.lock();
            let set = in_set(&mut cache, items, state.flags, alphabet)?;
            emit_from_set(d, set, out)?;
        }
        OpCode::Branch(items) => {
            let item = if state.unproducible {
                let mut live: Vec<&SubPattern> = Vec::with_capacity(items.len());
                for item in items {
                    if producible_sub(item, state.flags, alphabet, state.caches)? {
                        live.push(item);
                    }
                }
                if live.is_empty() {
                    return Err(mark_invalid(d));
                }
                live[pick_index(d, live.len())?]
            } else {
                &items[pick_index(d, items.len())?]
            };
            generate_subpattern(d, item, state, alphabet, out)?;
        }
        OpCode::Subpattern {
            group,
            add_flags,
            del_flags,
            p,
        } => {
            let saved_flags = state.flags;
            state.flags = combine_flags(state.flags, *add_flags, *del_flags);
            let before = out.len();
            generate_subpattern(d, p, state, alphabet, out)?;
            state.flags = saved_flags;
            if state.capture {
                if let Some(gid) = group {
                    state.groups.insert(*gid, out[before..].to_vec());
                }
            }
        }
        OpCode::GroupRef(gid) => {
            let Some(val) = state.groups.get(gid) else {
                return Err(mark_invalid(d));
            };
            out.extend_from_slice(val);
        }
        OpCode::GroupRefExists {
            cond_group,
            yes,
            no,
        } => {
            if state.groups.contains_key(cond_group) {
                generate_subpattern(d, yes, state, alphabet, out)?;
            } else if let Some(no) = no {
                generate_subpattern(d, no, state, alphabet, out)?;
            }
        }
        OpCode::Assert { direction, p } => {
            if state.fullmatch {
                let pending = PendingAssertNot {
                    char_pos: out.len() - state.base,
                    direction: *direction,
                    pattern: p,
                    flags: state.flags,
                    groups: state.groups.clone(),
                };
                state.deferred().pending_asserts.push(pending);
            } else {
                generate_subpattern(d, p, state, alphabet, out)?;
            }
        }
        OpCode::AssertNot { direction, p } => {
            let pending = PendingAssertNot {
                char_pos: out.len() - state.base,
                direction: *direction,
                pattern: p,
                flags: state.flags,
                groups: state.groups.clone(),
            };
            state.deferred().pending_lookaheads.push(pending);
        }
        OpCode::Failure => {
            return Err(mark_invalid(d));
        }
        OpCode::AtomicGroup(p) => {
            generate_subpattern(d, p, state, alphabet, out)?;
        }
        OpCode::MaxRepeat { min, max, item }
        | OpCode::MinRepeat { min, max, item }
        | OpCode::PossessiveRepeat { min, max, item } => {
            if *min == 0
                && state.unproducible
                && !producible_sub(item, state.flags, alphabet, state.caches)?
            {
                return Ok(());
            }
            let min = *min as usize;
            let max = if *max == u32::MAX {
                None
            } else {
                Some(*max as usize)
            };
            let mut ms = ManyState::new(min, max);
            if let [OpCode::In(items)] = item.data.as_slice() {
                return repeat_in(d, &mut ms, items, state, alphabet, out);
            }
            loop {
                if !many_more(d, &mut ms)? {
                    break;
                }
                generate_subpattern(d, item, state, alphabet, out)?;
            }
        }
    }
    Ok(())
}

/// Draw a repeat whose item is one character class: the class's set is
/// looked up once and every repetition emits from it, drawing exactly
/// what [`generate_op`] on the item would draw at each repetition without
/// its lookup. The first repetition is decided before the set is looked
/// up, as the generic loop would, so a repeat of nothing never builds it,
/// and an empty set marks the test case invalid where its first emission
/// would have. The set's form is settled once too, and the loop over the
/// repetitions is kept in a frame of its own, so that each repetition
/// costs its draws and little else.
#[inline(never)]
fn repeat_in<D: Draws>(
    d: &mut D,
    ms: &mut ManyState,
    items: &[SetItem],
    state: &GenState,
    alphabet: &Option<IntervalSet>,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    if !many_more(d, ms)? {
        return Ok(());
    }
    let mut cache = state.caches.in_cache.lock();
    match in_set(&mut cache, items, state.flags, alphabet)? {
        InSet::Listed(chars) => {
            if chars.is_empty() {
                return Err(mark_invalid(d));
            }
            let n = chars.len() as u64;
            loop {
                let idx = draw_char_index(d, n)?;
                out.push(chars[idx as usize]);
                if !many_more(d, ms)? {
                    return Ok(());
                }
            }
        }
        InSet::Complement(complement) => {
            if complement.len == 0 {
                return Err(mark_invalid(d));
            }
            loop {
                let idx = draw_char_index(d, complement.len)?;
                out.push(complement.get(idx)?);
                if !many_more(d, ms)? {
                    return Ok(());
                }
            }
        }
    }
}

/// Draw an index into `n` (non-zero) candidates, without drawing at all
/// when there is only one.
fn pick_index<D: Draws>(d: &mut D, n: usize) -> Result<usize, EngineError> {
    if n == 1 {
        return Ok(0);
    }
    Ok(d.draw_integer_i64(0, n as i64 - 1)? as usize)
}

/// Whether the alphabet can supply every character `sp` needs, under
/// `flags`: false when some literal, class or wildcard in it has no
/// candidates, when every branch of an alternation is unproducible, or when
/// a required repetition's body is. Anchors, assertions and back-references
/// are taken as producible: they fail (if at all) on the final string, not
/// for want of characters. Every subpattern's answer is remembered in the
/// caches, and every one of `sp`'s is worked out even once the answer is
/// settled, so that one ask at the root answers for the whole pattern.
///
/// A repetition whose body is unproducible is generated zero times rather
/// than rejected, and an alternation picks only among its producible
/// branches, so `(?-i:Ā)*k` over an ASCII alphabet produces `k` instead of
/// tripping the filter-too-much health check.
fn producible_sub(
    sp: &SubPattern,
    flags: u32,
    alphabet: &Option<IntervalSet>,
    caches: &Caches,
) -> Result<bool, InternalError> {
    if sp.data.is_empty() {
        return Ok(true);
    }
    let key = (sp.data.as_ptr() as usize, flags);
    if let Some(&cached) = caches.producible.lock().get(&key) {
        return Ok(cached);
    }
    let mut result = true;
    for op in &sp.data {
        result &= producible_op(op, flags, alphabet, caches)?;
    }
    caches.producible.lock().insert(key, result);
    Ok(result)
}

fn producible_op(
    op: &OpCode,
    flags: u32,
    alphabet: &Option<IntervalSet>,
    caches: &Caches,
) -> Result<bool, InternalError> {
    Ok(match op {
        OpCode::Literal(cp) => {
            !literal_candidates(codepoint_to_char(*cp)?, flags, alphabet).is_empty()
        }
        OpCode::NotLiteral(cp) => {
            !not_literal_chars(codepoint_to_char(*cp)?, flags, alphabet, caches).is_empty()
        }
        OpCode::Any => alphabet.is_none() || !any_chars(flags, alphabet, caches).is_empty(),
        OpCode::In(items) => {
            let mut cache = caches.in_cache.lock();
            !in_set(&mut cache, items, flags, alphabet)?.is_empty()
        }
        OpCode::At(_) | OpCode::GroupRef(_) | OpCode::AssertNot { .. } => true,
        OpCode::Failure => false,
        OpCode::Branch(items) => {
            let mut any = false;
            for item in items {
                any |= producible_sub(item, flags, alphabet, caches)?;
            }
            any
        }
        OpCode::Subpattern {
            add_flags,
            del_flags,
            p,
            ..
        } => producible_sub(
            p,
            combine_flags(flags, *add_flags, *del_flags),
            alphabet,
            caches,
        )?,
        OpCode::GroupRefExists { yes, no, .. } => {
            let yes = producible_sub(yes, flags, alphabet, caches)?;
            let no = match no {
                Some(no) => producible_sub(no, flags, alphabet, caches)?,
                None => true,
            };
            yes || no
        }
        OpCode::Assert { p, .. } | OpCode::AtomicGroup(p) => {
            producible_sub(p, flags, alphabet, caches)?
        }
        OpCode::MaxRepeat { min, item, .. }
        | OpCode::MinRepeat { min, item, .. }
        | OpCode::PossessiveRepeat { min, item, .. } => {
            producible_sub(item, flags, alphabet, caches)? || *min == 0
        }
    })
}

/// The characters a cased literal `c` may be emitted as under IGNORECASE,
/// `c` first: every character Python treats as equal to it (see
/// [`casefold::push_equivalents`]), restricted to the alphabet.
fn literal_candidates(c: char, flags: u32, alphabet: &Option<IntervalSet>) -> Vec<char> {
    let mut candidates = Vec::new();
    casefold::push_equivalents(&mut candidates, c, flags);
    let mut seen: HashSet<char> = HashSet::default();
    candidates.retain(|&x| alphabet_allows(alphabet, x) && seen.insert(x));
    candidates
}

/// The characters a `(NOT_LITERAL, c)` node may emit under `flags`: every
/// alphabet character that does not match the literal `c`.
fn not_literal_chars(
    c: char,
    flags: u32,
    alphabet: &Option<IntervalSet>,
    caches: &Caches,
) -> Arc<[char]> {
    if alphabet.is_none() {
        return cached_default_not_literal(c, flags);
    }
    cached_chars(
        &caches.char_cache,
        CharKey::NotLiteral(c, flags & CASE_FLAGS),
        || gather_chars(alphabet, |x| !casefold::literal_matches(c, x, flags)),
    )
}

/// The characters an `ANY` node may emit under `flags` from an alphabet:
/// the alphabet minus the newline unless DOTALL. Under the default
/// alphabet the node indexes its characters in closed form instead (see
/// [`default_any_char`]).
fn any_chars(flags: u32, alphabet: &Option<IntervalSet>, caches: &Caches) -> Arc<[char]> {
    let allow_newline = flags & SRE_FLAG_DOTALL != 0;
    cached_chars(&caches.char_cache, CharKey::Any(allow_newline), || {
        gather_chars(alphabet, |c| allow_newline || c != '\n')
    })
}

/// The set an `In` node with `items` emits from under `flags`, from the
/// regex's own cache, built and remembered there on the first ask.
fn in_set<'c>(
    cache: &'c mut HashMap<InKey, InSet>,
    items: &[SetItem],
    flags: u32,
    alphabet: &Option<IntervalSet>,
) -> Result<&'c InSet, InternalError> {
    let key = (items.as_ptr() as usize, items.len(), flags & CASE_FLAGS);
    match cache.entry(key) {
        Entry::Occupied(entry) => Ok(entry.into_mut()),
        Entry::Vacant(entry) => {
            let computed = if alphabet.is_none() {
                cached_default_in_set(items, flags)?
            } else {
                build_in_set(items, flags, alphabet)?
            };
            Ok(entry.insert(computed))
        }
    }
}

/// Cached version of [`build_in_set`] for the default (no-alphabet) case.
///
/// Category-driven classes like `\w`, `\s`, `[^\d]` require a full
/// 65 536-codepoint BMP scan to compute, and the parser yields distinct
/// `SetItem` slices for distinct regex patterns — so the state-level
/// pointer cache only helps within one draw. Patterns like `\w` alone
/// cost ~35ms per draw in debug; 10 draws trips the 1s TooSlow health
/// check on slower CI runners. Since the default alphabet is fixed, we
/// can memoise across draws (and across patterns).
fn cached_default_in_set(items: &[SetItem], flags: u32) -> Result<InSet, InternalError> {
    type Cache = Mutex<HashMap<(Vec<SetItem>, u32), InSet>>;
    static CACHE: Lazy<Cache> = Lazy::new(|| Mutex::new(HashMap::default()));
    let cache_key = (
        items.to_vec(),
        flags & (SRE_FLAG_IGNORECASE | SRE_FLAG_ASCII),
    );
    {
        let guard = CACHE.lock();
        if let Some(cached) = guard.get(&cache_key) {
            return Ok(cached.clone());
        }
    }
    let computed = build_in_set(items, flags, &None)?;
    CACHE.lock().insert(cache_key, computed.clone());
    Ok(computed)
}

/// How many characters an `Any` node can emit under the default alphabet
/// with DOTALL: the whole BMP minus the surrogates. Without DOTALL the
/// newline is excluded too, so the count is one less.
const DEFAULT_ANY_LEN: u64 = 0x10000 - 0x800;

/// The `idx`th character (in codepoint order) that an `Any` node can emit
/// under the default alphabet: the BMP minus surrogates, minus `'\n'`
/// without DOTALL. Equivalent to indexing the [`gather_chars`] scan of that
/// set, computed in closed form so the 64K-codepoint scan never runs.
fn default_any_char(idx: u64, allow_newline: bool) -> Result<char, InternalError> {
    let mut cp = hegel_internal_unwrap!(
        u32::try_from(idx).ok(),
        "default_any_char: index {idx} is not a codepoint"
    );
    if !allow_newline && cp >= '\n' as u32 {
        cp += 1;
    }
    if cp >= 0xD800 {
        cp += 0x800;
    }
    codepoint_to_char(cp)
}

/// Look up `key` in a per-[`CompiledRegex`] character-set cache, computing
/// and inserting it on a miss.
fn cached_chars<F: FnOnce() -> Vec<char>>(
    cache: &Mutex<HashMap<CharKey, Arc<[char]>>>,
    key: CharKey,
    compute: F,
) -> Arc<[char]> {
    {
        let guard = cache.lock();
        if let Some(cached) = guard.get(&key) {
            return Arc::clone(cached);
        }
    }
    let computed: Arc<[char]> = compute().into();
    cache.lock().insert(key, Arc::clone(&computed));
    computed
}

/// Cached character set for `NotLiteral` nodes with the default alphabet.
/// Same rationale as `cached_default_in_set`: `gather_chars` scans the
/// entire BMP (~64K codepoints) and is too expensive to repeat per draw.
fn cached_default_not_literal(c: char, flags: u32) -> Arc<[char]> {
    type Cache = Mutex<HashMap<(u32, u32), Arc<[char]>>>;
    static CACHE: Lazy<Cache> = Lazy::new(|| Mutex::new(HashMap::default()));
    let cache_key = (c as u32, flags & CASE_FLAGS);
    {
        let guard = CACHE.lock();
        if let Some(cached) = guard.get(&cache_key) {
            return Arc::clone(cached);
        }
    }
    let computed: Arc<[char]> =
        gather_chars(&None, |x| !casefold::literal_matches(c, x, flags)).into();
    CACHE.lock().insert(cache_key, Arc::clone(&computed));
    computed
}

/// Build the set of characters that a `(IN, items)` node can emit under
/// `flags`, intersected with the user-supplied alphabet.
///
/// A positive class lists its literals and ranges in pattern order (each
/// followed by its case-equivalents under IGNORECASE, see
/// [`casefold::push_equivalents`]) and then the characters of its
/// categories; a negated class is everything else. The ASCII flag narrows
/// only the categories and the case folding, never the explicit
/// characters — `(?a)Ā` still matches `Ā` in Python.
fn build_in_set(
    items: &[SetItem],
    flags: u32,
    alphabet: &Option<IntervalSet>,
) -> Result<InSet, InternalError> {
    let negate = matches!(items.first(), Some(SetItem::Negate));
    let ascii = flags & SRE_FLAG_ASCII != 0;

    let mut members: Vec<char> = Vec::new();
    let mut categories: Vec<ChCode> = Vec::new();

    for item in items {
        match item {
            SetItem::Negate => {}
            SetItem::Literal(cp) => {
                casefold::push_equivalents(&mut members, codepoint_to_char(*cp)?, flags);
            }
            SetItem::Range(lo, hi) => {
                for c in (*lo..=*hi).filter_map(char::from_u32) {
                    casefold::push_equivalents(&mut members, c, flags);
                }
            }
            SetItem::Category(cat) => {
                categories.push(*cat);
            }
        }
    }

    if !negate {
        let mut out: Vec<char> = Vec::new();
        let mut seen: HashSet<char> = HashSet::default();
        for c in members {
            if alphabet_allows(alphabet, c) && seen.insert(c) {
                out.push(c);
            }
        }
        if !categories.is_empty() {
            out.extend(gather_category_class(
                alphabet,
                &categories,
                false,
                &seen,
                ascii,
            ));
        }
        Ok(InSet::Listed(out.into()))
    } else {
        let excluded: HashSet<char> = members.into_iter().collect();
        if categories.is_empty() && alphabet.is_none() {
            let excluded = excluded
                .iter()
                .map(|c| *c as u32)
                .filter(|cp| *cp <= 0xFFFF)
                .map(|cp| (cp, cp))
                .collect();
            Ok(InSet::Complement(Arc::new(Complement::new(excluded))))
        } else if categories.is_empty() {
            Ok(InSet::Listed(
                gather_chars(alphabet, |c| !excluded.contains(&c)).into(),
            ))
        } else {
            Ok(InSet::Listed(
                gather_category_class(alphabet, &categories, true, &excluded, ascii).into(),
            ))
        }
    }
}

/// Return whether `c` is in the given CPython character category, with the
/// ASCII flag's narrower definitions (`\d` is `[0-9]`, `\s` is
/// `[ \t\n\r\f\v]`, `\w` is `[a-zA-Z0-9_]`) when `ascii`.
fn in_category(c: char, cat: ChCode, ascii: bool) -> bool {
    in_category_of(c, unicodedata::general_category_char(c), cat, ascii)
}

/// [`in_category`] for a character whose General Category is already
/// known.
fn in_category_of(c: char, category: Category, cat: ChCode, ascii: bool) -> bool {
    let major = category.as_str().as_bytes()[0];
    match cat {
        ChCode::Digit if ascii => c.is_ascii_digit(),
        ChCode::Digit => category == Category::Nd,
        ChCode::NotDigit => !in_category_of(c, category, ChCode::Digit, ascii),
        ChCode::Space if ascii => matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c'),
        ChCode::Space => is_uni_space(c, major),
        ChCode::NotSpace => !in_category_of(c, category, ChCode::Space, ascii),
        ChCode::Word if ascii => c == '_' || c.is_ascii_alphanumeric(),
        ChCode::Word => is_uni_word(c, major),
        ChCode::NotWord => !in_category_of(c, category, ChCode::Word, ascii),
    }
}

fn is_uni_space(c: char, major: u8) -> bool {
    matches!(
        c,
        ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\x1f' | '\u{85}'
    ) || major == b'Z'
}

fn is_uni_word(c: char, major: u8) -> bool {
    c == '_' || major == b'L' || major == b'N'
}

/// Whether `c` is a word character for `\b`, under the ASCII flag's
/// definition when `ascii`.
fn is_word(c: char, ascii: bool) -> bool {
    in_category(c, ChCode::Word, ascii)
}

/// Return whether the characters of General Category `category` are in the
/// given CPython character category, going by the General Category alone.
/// Agrees with [`in_category_of`] for every character from
/// [`CATEGORY_ALONE_FROM`] up: the characters that function singles out
/// (the control and Latin-1 whitespace, `_`) all lie below it, and under
/// the ASCII flag no character from there up is a digit, a space or a
/// word character.
fn category_in(category: Category, cat: ChCode, ascii: bool) -> bool {
    if ascii {
        return matches!(cat, ChCode::NotDigit | ChCode::NotSpace | ChCode::NotWord);
    }
    let major = category.as_str().as_bytes()[0];
    let letter_or_number = major == b'L' || major == b'N';
    match cat {
        ChCode::Digit => category == Category::Nd,
        ChCode::NotDigit => category != Category::Nd,
        ChCode::Space => major == b'Z',
        ChCode::NotSpace => major != b'Z',
        ChCode::Word => letter_or_number,
        ChCode::NotWord => !letter_or_number,
    }
}

/// The first codepoint from which [`in_category_of`] depends on a
/// character's General Category alone, so that [`category_in`] answers
/// for it.
const CATEGORY_ALONE_FROM: u32 = 0x100;

/// Gather, in codepoint order, the characters of `alphabet` (or of the BMP
/// minus the surrogates when no alphabet is given) that are in any of
/// `categories` — or, with `negate`, in none of them — leaving out
/// `excluded`, with the categories' ASCII definitions when `ascii`. Walks
/// the General Category runs and decides each run at once by
/// [`category_in`] when nothing distinguishes its characters: it starts at
/// or after [`CATEGORY_ALONE_FROM`] and contains no excluded character.
/// The runs below that, and those containing an exclusion, are checked
/// character by character with [`in_category_of`].
fn gather_category_class(
    alphabet: &Option<IntervalSet>,
    categories: &[ChCode],
    negate: bool,
    excluded: &HashSet<char>,
    ascii: bool,
) -> Vec<char> {
    let mut excluded: Vec<u32> = excluded.iter().map(|c| *c as u32).collect();
    excluded.sort_unstable();
    let mut out = Vec::new();
    let mut gather = |lo: u32, hi: u32| {
        for (start, end, category) in unicodedata::category_runs_in(lo, hi) {
            let next_excluded = excluded.partition_point(|&cp| cp < start);
            let uniform = start >= CATEGORY_ALONE_FROM
                && excluded.get(next_excluded).is_none_or(|&cp| cp > end);
            if uniform {
                if categories
                    .iter()
                    .any(|cat| category_in(category, *cat, ascii))
                    != negate
                {
                    out.extend((start..=end).filter_map(char::from_u32));
                }
            } else {
                for c in (start..=end).filter_map(char::from_u32) {
                    if excluded.binary_search(&(c as u32)).is_err()
                        && categories
                            .iter()
                            .any(|cat| in_category_of(c, category, *cat, ascii))
                            != negate
                    {
                        out.push(c);
                    }
                }
            }
        }
    };
    match alphabet {
        None => gather(0, 0xFFFF),
        Some(intervals) => {
            for &(start, end) in &intervals.intervals {
                gather(start, end);
            }
        }
    }
    out
}

/// Gather all characters in `alphabet` (or BMP-minus-surrogates when no
/// alphabet is given) that satisfy `predicate`. The default scan is bounded
/// to the BMP to keep `.`, `\w`, `[^a]`, etc. tractable on the unconstrained
/// path.
fn gather_chars<F: Fn(char) -> bool>(alphabet: &Option<IntervalSet>, predicate: F) -> Vec<char> {
    let mut out = Vec::new();
    match alphabet {
        None => {
            out.reserve(0x10000 - 0x800);
            for cp in (0u32..0xD800).chain(0xE000..0x10000) {
                if let Some(c) = char::from_u32(cp) {
                    if predicate(c) {
                        out.push(c);
                    }
                }
            }
        }
        Some(intervals) => {
            for &(start, end) in &intervals.intervals {
                for cp in start..=end {
                    if let Some(c) = char::from_u32(cp) {
                        if predicate(c) {
                            out.push(c);
                        }
                    }
                }
            }
        }
    }
    out
}

fn alphabet_allows(alphabet: &Option<IntervalSet>, c: char) -> bool {
    match alphabet {
        None => !is_surrogate_cp(c as u32),
        Some(intervals) => intervals.contains(c as u32),
    }
}

/// Pick an arbitrary character from the alphabet. Used for prefix/suffix
/// padding — must never fail if the alphabet is non-empty.
fn draw_any_char<D: Draws>(d: &mut D, alphabet: &Option<IntervalSet>) -> Result<char, EngineError> {
    match alphabet {
        None => {
            let cp = d.draw_integer_i64(32, 126)?;
            Ok(hegel_internal_unwrap!(
                char::from_u32(cp as u32),
                "draw_any_char: printable-ASCII draw {cp} is not a char"
            ))
        }
        Some(intervals) => {
            let n = intervals.len();
            if n == 0 {
                return Err(mark_invalid(d));
            }
            let idx = d.draw_integer_i64(0, n as i64 - 1)?;
            let cp = hegel_internal_unwrap!(
                intervals.get(idx as isize),
                "draw_any_char: alphabet index {idx} out of bounds"
            );
            Ok(hegel_internal_unwrap!(
                char::from_u32(cp),
                "draw_any_char: alphabet codepoint {cp} is not a char"
            ))
        }
    }
}
fn emit_from_chars<D: Draws>(
    d: &mut D,
    chars: &[char],
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    if chars.is_empty() {
        return Err(mark_invalid(d));
    }
    let idx = draw_char_index(d, chars.len() as u64)?;
    out.push(chars[idx as usize]);
    Ok(())
}

fn emit_from_set<D: Draws>(d: &mut D, set: &InSet, out: &mut Vec<char>) -> Result<(), EngineError> {
    match set {
        InSet::Listed(chars) => emit_from_chars(d, chars, out),
        InSet::Complement(complement) => {
            if complement.len == 0 {
                return Err(mark_invalid(d));
            }
            let idx = draw_char_index(d, complement.len)?;
            out.push(complement.get(idx)?);
            Ok(())
        }
    }
}

/// How likely a character drawn from a set of more than 256 is beyond
/// its first 256, with the byte the engine compares against for it (see
/// [`boolean_threshold`]). The coin is for going beyond, so that its
/// simplest answer, no, keeps the simplest character the set's first.
const BEYOND_256_PROBABILITY: f64 = 0.2;
const BEYOND_256_THRESHOLD: u8 = boolean_threshold(BEYOND_256_PROBABILITY);

/// Draw an index into a character set of `n` (non-zero) characters, biased
/// towards the first 256 when the set is larger than that.
#[inline]
fn draw_char_index<D: Draws>(d: &mut D, n: u64) -> Result<u64, EngineError> {
    if n > 256 && d.weighted_against(BEYOND_256_PROBABILITY, BEYOND_256_THRESHOLD, None)? {
        d.draw_integer_uniform(256, n - 1)
    } else if n > 256 {
        d.draw_integer_uniform(0, 255)
    } else {
        d.draw_integer_uniform(0, n - 1)
    }
}

fn mark_invalid<D: Draws>(d: &mut D) -> EngineError {
    d.conclude(Status::Invalid);
    EngineError::InvalidTestCase
}

fn codepoint_to_char(cp: u32) -> Result<char, InternalError> {
    Ok(hegel_internal_unwrap!(
        char::from_u32(cp),
        "invalid codepoint in regex AST: {cp:#x}"
    ))
}

/// Where the ops `ops` match in `chars` from `pos`, with CPython's
/// semantics for the characters they consume (see [`casefold`]) and the
/// positions they assert; the end of the match, or `None`. The generator's
/// own check of a deferred lookaround or anchor against the string it
/// drew, not a full matcher: the patterns it approximates are verified by
/// the engine's (see [`approximated`]).
fn match_seq(
    ops: &[OpCode],
    pos: usize,
    chars: &[char],
    flags: u32,
    groups: &HashMap<u32, Vec<char>>,
) -> Option<usize> {
    let Some((first, rest)) = ops.split_first() else {
        return Some(pos);
    };
    match first {
        OpCode::Literal(cp) => {
            let want = char::from_u32(*cp)?;
            let got = *chars.get(pos)?;
            if casefold::literal_matches(want, got, flags) {
                match_seq(rest, pos + 1, chars, flags, groups)
            } else {
                None
            }
        }
        OpCode::NotLiteral(cp) => {
            let banned = char::from_u32(*cp)?;
            let got = *chars.get(pos)?;
            if casefold::literal_matches(banned, got, flags) {
                None
            } else {
                match_seq(rest, pos + 1, chars, flags, groups)
            }
        }
        OpCode::Any => {
            let got = *chars.get(pos)?;
            if got == '\n' && flags & SRE_FLAG_DOTALL == 0 {
                None
            } else {
                match_seq(rest, pos + 1, chars, flags, groups)
            }
        }
        OpCode::In(items) => {
            let got = *chars.get(pos)?;
            if char_matches_set(items, got, flags) {
                match_seq(rest, pos + 1, chars, flags, groups)
            } else {
                None
            }
        }
        OpCode::At(at) => {
            if at_matches(at, chars, pos, flags) {
                match_seq(rest, pos, chars, flags, groups)
            } else {
                None
            }
        }
        OpCode::Branch(branches) => {
            for br in branches {
                let mut combined = br.data.clone();
                combined.extend_from_slice(rest);
                if let Some(end) = match_seq(&combined, pos, chars, flags, groups) {
                    return Some(end);
                }
            }
            None
        }
        OpCode::Subpattern {
            add_flags,
            del_flags,
            p,
            ..
        } => {
            let inner_flags = combine_flags(flags, *add_flags, *del_flags);
            let end = match_seq(&p.data, pos, chars, inner_flags, groups)?;
            match_seq(rest, end, chars, flags, groups)
        }
        OpCode::AtomicGroup(p) => {
            let end = match_seq(&p.data, pos, chars, flags, groups)?;
            match_seq(rest, end, chars, flags, groups)
        }
        OpCode::GroupRef(gid) => {
            let val = groups.get(gid)?;
            if pos + val.len() > chars.len() {
                return None;
            }
            for (i, vc) in val.iter().enumerate() {
                if !casefold::groupref_eq(chars[pos + i], *vc, flags) {
                    return None;
                }
            }
            match_seq(rest, pos + val.len(), chars, flags, groups)
        }
        OpCode::GroupRefExists {
            cond_group,
            yes,
            no,
        } => {
            let mut combined = if groups.contains_key(cond_group) {
                yes.data.clone()
            } else if let Some(n) = no {
                n.data.clone()
            } else {
                Vec::new()
            };
            combined.extend_from_slice(rest);
            match_seq(&combined, pos, chars, flags, groups)
        }
        OpCode::Assert { p, .. } => {
            if match_seq(&p.data, pos, chars, flags, groups).is_some() {
                match_seq(rest, pos, chars, flags, groups)
            } else {
                None
            }
        }
        OpCode::AssertNot { p, .. } => {
            if match_seq(&p.data, pos, chars, flags, groups).is_none() {
                match_seq(rest, pos, chars, flags, groups)
            } else {
                None
            }
        }
        OpCode::Failure => None,
        OpCode::MaxRepeat { min, max, item } => {
            let mn = *min as usize;
            let mx = if *max == u32::MAX {
                None
            } else {
                Some(*max as usize)
            };
            let mut positions = vec![pos];
            let mut cur = pos;
            let mut zero_width = false;
            loop {
                if let Some(m) = mx {
                    if positions.len() > m {
                        break;
                    }
                }
                match match_seq(&item.data, cur, chars, flags, groups) {
                    Some(next) if next > cur => {
                        cur = next;
                        positions.push(cur);
                    }
                    Some(_) => {
                        zero_width = true;
                        break;
                    }
                    None => break,
                }
            }
            let mn_eff = if zero_width { 0 } else { mn };
            if positions.len() - 1 < mn_eff {
                return None;
            }
            for i in (mn_eff..positions.len()).rev() {
                if let Some(end) = match_seq(rest, positions[i], chars, flags, groups) {
                    return Some(end);
                }
            }
            None
        }
        OpCode::MinRepeat { min, max, item } => {
            let mn = *min as usize;
            let mx = if *max == u32::MAX {
                None
            } else {
                Some(*max as usize)
            };
            let mut cur = pos;
            let mut count = 0usize;
            while count < mn {
                let next = match_seq(&item.data, cur, chars, flags, groups)?;
                if next <= cur {
                    count = mn;
                    break;
                }
                cur = next;
                count += 1;
            }
            loop {
                if let Some(end) = match_seq(rest, cur, chars, flags, groups) {
                    return Some(end);
                }
                if let Some(m) = mx {
                    if count >= m {
                        return None;
                    }
                }
                let next = match_seq(&item.data, cur, chars, flags, groups)?;
                if next <= cur {
                    return None;
                }
                cur = next;
                count += 1;
            }
        }
        OpCode::PossessiveRepeat { min, max, item } => {
            let mn = *min as usize;
            let mx = if *max == u32::MAX {
                None
            } else {
                Some(*max as usize)
            };
            let mut cur = pos;
            let mut count = 0usize;
            let mut zero_width = false;
            loop {
                if let Some(m) = mx {
                    if count >= m {
                        break;
                    }
                }
                match match_seq(&item.data, cur, chars, flags, groups) {
                    Some(next) if next > cur => {
                        cur = next;
                        count += 1;
                    }
                    Some(_) => {
                        zero_width = true;
                        break;
                    }
                    None => break,
                }
            }
            if count < mn && !zero_width {
                return None;
            }
            match_seq(rest, cur, chars, flags, groups)
        }
    }
}
/// Whether `c` matches the character class `items` under `flags`, with
/// Python's semantics: under IGNORECASE a literal or range matches every
/// character case-equal to one of its members (see [`casefold`]), and under
/// the ASCII flag the categories narrow to their ASCII definitions while
/// explicit characters and ranges keep matching outside ASCII.
fn char_matches_set(items: &[SetItem], c: char, flags: u32) -> bool {
    let negate = matches!(items.first(), Some(SetItem::Negate));
    let ascii = flags & SRE_FLAG_ASCII != 0;
    let contained = items.iter().any(|item| match item {
        SetItem::Negate => false,
        SetItem::Literal(cp) => {
            char::from_u32(*cp).is_some_and(|lc| casefold::literal_matches(lc, c, flags))
        }
        SetItem::Range(lo, hi) => casefold::range_matches(*lo, *hi, c, flags),
        SetItem::Category(cat) => in_category(c, *cat, ascii),
    });
    negate != contained
}

fn at_matches(at: &AtCode, chars: &[char], pos: usize, flags: u32) -> bool {
    let ascii = flags & SRE_FLAG_ASCII != 0;
    match at {
        AtCode::BeginningString => pos == 0,
        AtCode::Beginning => {
            if flags & SRE_FLAG_MULTILINE != 0 {
                pos == 0 || chars[pos - 1] == '\n'
            } else {
                pos == 0
            }
        }
        AtCode::End => {
            if flags & SRE_FLAG_MULTILINE != 0 {
                pos == chars.len() || chars[pos] == '\n'
            } else {
                pos == chars.len() || (pos + 1 == chars.len() && chars[pos] == '\n')
            }
        }
        AtCode::EndString => pos == chars.len(),
        AtCode::Boundary => is_word_boundary(chars, pos, ascii),
        AtCode::NonBoundary => !is_word_boundary(chars, pos, ascii),
    }
}

fn is_word_boundary(chars: &[char], pos: usize, ascii: bool) -> bool {
    let before = pos > 0 && is_word(chars[pos - 1], ascii);
    let after = pos < chars.len() && is_word(chars[pos], ascii);
    before != after
}

/// The length of the strings the generator draws from `p` on average, in
/// codepoints: one for each op that emits a character, the mean over the
/// branches of an alternation (and over the two arms of a conditional),
/// the length of the group a backreference repeats, and for a repeat the
/// number of elements the collection protocol pulls on average (see
/// [`typical_count`]) times the length of one.
pub(crate) fn typical_width(p: &SubPattern) -> f64 {
    let mut groups = Vec::new();
    typical_of(p, &mut groups)
}

fn typical_of(p: &SubPattern, groups: &mut Vec<Option<f64>>) -> f64 {
    let mut total = 0.0;
    for op in &p.data {
        total += match op {
            OpCode::Literal(_) | OpCode::NotLiteral(_) | OpCode::Any | OpCode::In(_) => 1.0,
            OpCode::At(_) | OpCode::Assert { .. } | OpCode::AssertNot { .. } | OpCode::Failure => {
                0.0
            }
            OpCode::Branch(items) => {
                let sum: f64 = items.iter().map(|item| typical_of(item, groups)).sum();
                sum / items.len().max(1) as f64
            }
            OpCode::AtomicGroup(inner) => typical_of(inner, groups),
            OpCode::Subpattern { group, p, .. } => {
                let width = typical_of(p, groups);
                if let Some(g) = group {
                    let index = *g as usize;
                    if groups.len() <= index {
                        groups.resize(index + 1, None);
                    }
                    groups[index] = Some(width);
                }
                width
            }
            OpCode::GroupRef(g) => groups.get(*g as usize).copied().flatten().unwrap_or(0.0),
            OpCode::GroupRefExists { yes, no, .. } => {
                let no = no.as_ref().map_or(0.0, |no| typical_of(no, groups));
                (typical_of(yes, groups) + no) / 2.0
            }
            OpCode::MaxRepeat { min, max, item }
            | OpCode::MinRepeat { min, max, item }
            | OpCode::PossessiveRepeat { min, max, item } => {
                let max = (*max != MAXREPEAT).then_some(*max as usize);
                typical_count(*min as usize, max) * typical_of(item, groups)
            }
        };
    }
    total
}

/// How many elements a collection of `min_size` to `max_size` elements
/// has on average: the collection protocol goes on past the minimum with
/// the probability [`length_p_continue`] sets each time, so the count
/// beyond it is geometric, cut off at the maximum.
pub(crate) fn typical_count(min_size: usize, max_size: Option<usize>) -> f64 {
    let p = length_p_continue(min_size, max_size);
    let extra = match max_size {
        None => p / (1.0 - p),
        Some(max_size) => p * (1.0 - p.powf((max_size - min_size) as f64)) / (1.0 - p),
    };
    min_size as f64 + extra
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/draws/regex_tests.rs"]
mod tests;
