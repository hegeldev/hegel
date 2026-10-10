use crate::control::{InternalError, hegel_internal_unwrap};
use crate::native::{HashMap, HashSet};
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use hashbrown::hash_map::Entry;

use crate::native::core::state::length_p_continue;
use crate::native::core::{EngineError, ManyState, NativeTestCase, Status};
use crate::native::intervalsets::IntervalSet;
use crate::native::re::constants::{
    AtCode, ChCode, MAXREPEAT, SRE_FLAG_ASCII, SRE_FLAG_DOTALL, SRE_FLAG_IGNORECASE,
    SRE_FLAG_MULTILINE,
};
use crate::native::re::parser::{OpCode, ParsedPattern, SetItem, SubPattern, parse_pattern};
use crate::sys::sync::{Lazy, Mutex};
use crate::unicodedata::{self, Category};

use super::many_more;

fn is_surrogate_cp(cp: u32) -> bool {
    (0xD800..=0xDFFF).contains(&cp)
}

/// Cache key for a `(IN, items)` node's character set: the `SetItem` slice's
/// address and length (stable because the AST is owned by the enclosing
/// [`CompiledRegex`]) plus the active flags (which affect IGNORECASE swaps
/// and ASCII-only filtering).
type InKey = (usize, usize, u32);

/// Cache key for the alphabet-constrained `Any` and `NotLiteral` character
/// sets: the excluded codepoint (`u32::MAX` for `Any`, which has none) plus
/// the flag bits the set depends on.
type CharKey = (u32, u32);

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

/// The BMP minus the codepoint ranges in `excluded`, which are sorted,
/// disjoint, within the BMP, and always include the surrogates.
#[derive(Debug, PartialEq)]
struct Complement {
    excluded: Vec<(u32, u32)>,
    len: u64,
}

impl Complement {
    fn new(mut excluded: Vec<(u32, u32)>) -> Self {
        excluded.push((0xD800, 0xDFFF));
        excluded.sort_unstable();
        let removed: u64 = excluded.iter().map(|(lo, hi)| u64::from(hi - lo) + 1).sum();
        Complement {
            excluded,
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
/// the parsed AST, the optional user alphabet, and cross-draw caches of
/// the alphabet-constrained character sets (category classes like
/// `\w` cost a full alphabet scan to materialise).
#[derive(Debug)]
pub(crate) struct CompiledRegex {
    parsed: ParsedPattern,
    alphabet: Option<IntervalSet>,
    in_cache: Mutex<HashMap<InKey, InSet>>,
    char_cache: Mutex<HashMap<CharKey, Arc<[char]>>>,
}

impl CompiledRegex {
    /// The length of the strings drawn from the pattern on average (see
    /// [`typical_width`]).
    pub(crate) fn typical_width(&self) -> f64 {
        typical_width(&self.parsed.pattern)
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
        Ok(CompiledRegex {
            parsed,
            alphabet,
            in_cache: Mutex::new(HashMap::default()),
            char_cache: Mutex::new(HashMap::default()),
        })
    }
}

/// Draw a string matching `re`, anchored at both ends when `fullmatch`
/// and otherwise padded with draws from the compiled alphabet (or the full
/// codespace when it has none).
///
/// Candidates whose deferred checks fail (a `\b` that the padding broke, a
/// possessive repeat whose committed count the pattern can't actually
/// match, ...) are retried a few times within the same test case — the
/// analogue of Hypothesis's strategy-level `.filter(re.search)` retries —
/// before the whole test case is marked invalid.
/// The characters a drawn string starts out with room for, so that one
/// of ordinary length is built without the buffer growing.
const TYPICAL_CAPACITY: usize = 64;

pub(crate) fn generate_regex(
    ntc: &mut NativeTestCase,
    re: &CompiledRegex,
    fullmatch: bool,
) -> Result<String, EngineError> {
    let mut out = Vec::with_capacity(TYPICAL_CAPACITY);
    generate_regex_into(ntc, re, fullmatch, &mut out)?;
    Ok(out.into_iter().collect())
}

/// Draw a string matching `re` onto the end of `out`, whose earlier text
/// the pattern never sees: an anchor at the start of the pattern is at
/// the start of the drawn string. What an attempt that fails a deferred
/// check or errors added is removed again.
pub(crate) fn generate_regex_into(
    ntc: &mut NativeTestCase,
    re: &CompiledRegex,
    fullmatch: bool,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    const MAX_ATTEMPTS: usize = 5;
    let base = out.len();
    for _ in 0..MAX_ATTEMPTS {
        match generate_regex_attempt(ntc, re, fullmatch, out, base) {
            Ok(true) => return Ok(()),
            Ok(false) => out.truncate(base),
            Err(e) => {
                out.truncate(base);
                return Err(e);
            }
        }
    }
    Err(mark_invalid(ntc))
}

/// One generation attempt, appending to `out` from `base` on. Returns
/// `Ok(false)` when the candidate was generated but failed a deferred
/// check against the final string.
fn generate_regex_attempt(
    ntc: &mut NativeTestCase,
    re: &CompiledRegex,
    fullmatch: bool,
    out: &mut Vec<char>,
    base: usize,
) -> Result<bool, EngineError> {
    let parsed = &re.parsed;
    let alphabet = &re.alphabet;

    let mut state = GenState {
        groups: HashMap::default(),
        flags: parsed.flags,
        fullmatch,
        base,
        pending_anchors: Vec::new(),
        pending_asserts: Vec::new(),
        pending_lookaheads: Vec::new(),
        needs_whole_match: false,
        in_cache: &re.in_cache,
        char_cache: &re.char_cache,
    };

    if parsed.pattern.is_empty() {
        if !fullmatch {
            draw_pad(ntc, alphabet, out)?;
        }
        return Ok(true);
    }

    if !fullmatch {
        draw_prefix(ntc, parsed, alphabet, out, base)?;
    }
    generate_subpattern(ntc, &parsed.pattern, &mut state, alphabet, out)?;
    if !fullmatch {
        draw_suffix(ntc, parsed, alphabet, out)?;
    }

    let needs_final_checks = !state.pending_anchors.is_empty()
        || !state.pending_asserts.is_empty()
        || !state.pending_lookaheads.is_empty()
        || state.needs_whole_match;
    if needs_final_checks {
        let final_chars = &out[base..];
        for anchor in &state.pending_anchors {
            if !at_matches(&anchor.at, final_chars, anchor.char_pos, anchor.flags) {
                return Ok(false);
            }
        }
        for pending in &state.pending_asserts {
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
        for pending in &state.pending_lookaheads {
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
        if state.needs_whole_match && !whole_match(parsed, fullmatch, final_chars, &state.groups) {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Whether the pattern matches `chars` — anywhere for search semantics, or
/// spanning the whole string for `fullmatch`. Used as a post-generation
/// filter (Hypothesis filters every candidate through `re.search`) for the
/// constructs whose generated output is not a match by construction: atomic
/// groups and possessive repeats commit to a repetition count during
/// generation that the pattern's real (non-backtracking) semantics may not
/// admit.
fn whole_match(
    parsed: &ParsedPattern,
    fullmatch: bool,
    chars: &[char],
    groups: &HashMap<u32, Vec<char>>,
) -> bool {
    if fullmatch {
        let mut anchored = parsed.pattern.data.clone();
        anchored.push(OpCode::At(AtCode::EndString));
        match_seq(&anchored, 0, chars, parsed.flags, groups).is_some()
    } else {
        (0..=chars.len()).any(|start| {
            match_seq(&parsed.pattern.data, start, chars, parsed.flags, groups).is_some()
        })
    }
}

/// Mutable state threaded through generation: captured groups (for
/// back-references) and the active regex flags (which change as we descend
/// into `SUBPATTERN` nodes with inline flag modifiers).
struct GenState<'a> {
    groups: HashMap<u32, Vec<char>>,
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
    /// Zero-width anchors (`\b`, `\B`, and `$`/`\Z` in non-final positions)
    /// recorded during generation and checked against the final output
    /// string, since the content that follows them isn't known yet when they
    /// are reached.
    pending_anchors: Vec<PendingAnchor>,
    /// Positive lookaround assertions deferred in fullmatch mode.
    pending_asserts: Vec<PendingAssertNot>,
    /// Negative lookaround assertions recorded during generation. Each entry
    /// captures the assertion body, the active flags, and a snapshot of the
    /// groups at the point of the assertion. We check them against the final
    /// output string in [`generate_regex`], so a candidate that violates one
    /// is redrawn like any other failed deferred check.
    pending_lookaheads: Vec<PendingAssertNot>,
    /// Set when the pattern contains an atomic group or possessive repeat,
    /// whose generated output must be re-validated against the whole pattern.
    needs_whole_match: bool,
    /// The enclosing [`CompiledRegex`]'s cross-draw cache of
    /// alphabet-constrained `IN`-node character sets.
    in_cache: &'a Mutex<HashMap<InKey, InSet>>,
    /// The enclosing [`CompiledRegex`]'s cross-draw cache of
    /// alphabet-constrained `Any` / `NotLiteral` character sets.
    char_cache: &'a Mutex<HashMap<CharKey, Arc<[char]>>>,
}

struct PendingAnchor {
    char_pos: usize,
    at: AtCode,
    flags: u32,
}

#[derive(Clone)]
struct PendingAssertNot {
    char_pos: usize,
    direction: i32,
    pattern: SubPattern,
    flags: u32,
    groups: HashMap<u32, Vec<char>>,
}

/// Draw 0..10 arbitrary characters from `alphabet` (or ASCII 32..126 when
/// no alphabet is given). Used for prefix/suffix padding and for the
/// empty-pattern case.
fn draw_pad(
    ntc: &mut NativeTestCase,
    alphabet: &Option<IntervalSet>,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    let n = ntc.draw_integer_i64(0, 10)?;
    for _ in 0..n {
        let c = draw_any_char(ntc, alphabet)?;
        out.push(c);
    }
    Ok(())
}

/// Return the first non-grouping OpCode in `sp`, descending through SUBPATTERN
/// and ATOMIC_GROUP nodes (which don't consume characters themselves).
///
/// Python's `regex_strategy` doesn't descend like this — it relies on
/// `regex.search` as a post-generation filter. We don't have a Python-compatible
/// regex matcher to filter against, so we peek through non-consuming wrappers
/// instead, which handles the common `(\Afoo\Z)` shape.
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

fn draw_prefix(
    ntc: &mut NativeTestCase,
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
                    draw_pad(ntc, alphabet, out)?;
                    if out.len() > base {
                        out.push('\n');
                    }
                }
                return Ok(());
            }
            _ => {}
        }
    }
    draw_pad(ntc, alphabet, out)
}

fn draw_suffix(
    ntc: &mut NativeTestCase,
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
                    if ntc.weighted(0.5, None)? {
                        out.push('\n');
                        draw_pad(ntc, alphabet, out)?;
                    }
                } else if ntc.weighted(0.5, None)? {
                    out.push('\n');
                }
                return Ok(());
            }
            _ => {}
        }
    }
    draw_pad(ntc, alphabet, out)
}

/// Recursively generate a string from a `SubPattern`, appending to `out`.
fn generate_subpattern(
    ntc: &mut NativeTestCase,
    sp: &SubPattern,
    state: &mut GenState,
    alphabet: &Option<IntervalSet>,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    for op in &sp.data {
        generate_op(ntc, op, state, alphabet, out)?;
    }
    Ok(())
}
fn generate_op(
    ntc: &mut NativeTestCase,
    op: &OpCode,
    state: &mut GenState,
    alphabet: &Option<IntervalSet>,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    match op {
        OpCode::Literal(cp) => {
            let c = codepoint_to_char(*cp)?;
            if state.flags & SRE_FLAG_IGNORECASE != 0 {
                if let Some(sw) = char_swapcase(c) {
                    let which = ntc.draw_integer_i64(0, 1)?;
                    let pick = if which == 0 { c } else { sw };
                    if !alphabet_allows(alphabet, pick) {
                        return Err(mark_invalid(ntc));
                    }
                    out.push(pick);
                    return Ok(());
                }
            }
            if !alphabet_allows(alphabet, c) {
                return Err(mark_invalid(ntc));
            }
            out.push(c);
        }
        OpCode::NotLiteral(cp) => {
            let c = codepoint_to_char(*cp)?;
            if alphabet.is_none() {
                let chars = cached_default_not_literal(c, state.flags);
                emit_from_chars(ntc, &chars, out)?;
            } else {
                let chars = cached_chars(
                    state.char_cache,
                    (*cp, state.flags & SRE_FLAG_IGNORECASE),
                    || {
                        let blacklist = swapcase_blacklist(c, state.flags);
                        gather_chars(alphabet, |c| !blacklist.contains(&c))
                    },
                );
                emit_from_chars(ntc, &chars, out)?;
            }
        }
        OpCode::Any => {
            let allow_newline = state.flags & SRE_FLAG_DOTALL != 0;
            if alphabet.is_none() {
                let n = DEFAULT_ANY_LEN - u64::from(!allow_newline);
                let idx = draw_char_index(ntc, n)?;
                out.push(default_any_char(idx, allow_newline)?);
            } else {
                let chars = cached_chars(
                    state.char_cache,
                    (u32::MAX, state.flags & SRE_FLAG_DOTALL),
                    || gather_chars(alphabet, |c| allow_newline || c != '\n'),
                );
                emit_from_chars(ntc, &chars, out)?;
            }
        }
        OpCode::At(at) => match at {
            AtCode::BeginningString if out.len() > state.base => {
                return Err(mark_invalid(ntc));
            }
            AtCode::BeginningString => {}
            AtCode::Beginning => {
                if state.flags & SRE_FLAG_MULTILINE != 0 {
                    if out.len() > state.base && out.last() != Some(&'\n') {
                        return Err(mark_invalid(ntc));
                    }
                } else if out.len() > state.base {
                    return Err(mark_invalid(ntc));
                }
            }
            AtCode::End | AtCode::EndString | AtCode::Boundary | AtCode::NonBoundary => {
                state.pending_anchors.push(PendingAnchor {
                    char_pos: out.len() - state.base,
                    at: *at,
                    flags: state.flags,
                });
            }
        },
        OpCode::In(items) => {
            let key = (items.as_ptr() as usize, items.len(), state.flags);
            let mut cache = state.in_cache.lock();
            let set = match cache.entry(key) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let computed = if alphabet.is_none() {
                        cached_default_in_set(items, state.flags)?
                    } else {
                        build_in_set(items, state.flags, alphabet)?
                    };
                    entry.insert(computed)
                }
            };
            emit_from_set(ntc, set, out)?;
        }
        OpCode::Branch(items) => {
            let idx = ntc.draw_integer_i64(0, items.len() as i64 - 1)? as usize;
            generate_subpattern(ntc, &items[idx], state, alphabet, out)?;
        }
        OpCode::Subpattern {
            group,
            add_flags,
            del_flags,
            p,
        } => {
            let saved_flags = state.flags;
            state.flags = (state.flags | *add_flags) & !*del_flags;
            let before = out.len();
            generate_subpattern(ntc, p, state, alphabet, out)?;
            state.flags = saved_flags;
            if let Some(gid) = group {
                state.groups.insert(*gid, out[before..].to_vec());
            }
        }
        OpCode::GroupRef(gid) => {
            let Some(val) = state.groups.get(gid) else {
                return Err(mark_invalid(ntc));
            };
            out.extend_from_slice(val);
        }
        OpCode::GroupRefExists {
            cond_group,
            yes,
            no,
        } => {
            if state.groups.contains_key(cond_group) {
                generate_subpattern(ntc, yes, state, alphabet, out)?;
            } else if let Some(no) = no {
                generate_subpattern(ntc, no, state, alphabet, out)?;
            }
        }
        OpCode::Assert { direction, p } => {
            if state.fullmatch {
                state.pending_asserts.push(PendingAssertNot {
                    char_pos: out.len() - state.base,
                    direction: *direction,
                    pattern: p.clone(),
                    flags: state.flags,
                    groups: state.groups.clone(),
                });
            } else {
                generate_subpattern(ntc, p, state, alphabet, out)?;
            }
        }
        OpCode::AssertNot { direction, p } => {
            state.pending_lookaheads.push(PendingAssertNot {
                char_pos: out.len() - state.base,
                direction: *direction,
                pattern: p.clone(),
                flags: state.flags,
                groups: state.groups.clone(),
            });
        }
        OpCode::Failure => {
            return Err(mark_invalid(ntc));
        }
        OpCode::AtomicGroup(p) => {
            state.needs_whole_match = true;
            generate_subpattern(ntc, p, state, alphabet, out)?;
        }
        OpCode::MaxRepeat { min, max, item }
        | OpCode::MinRepeat { min, max, item }
        | OpCode::PossessiveRepeat { min, max, item } => {
            if matches!(op, OpCode::PossessiveRepeat { .. }) {
                state.needs_whole_match = true;
            }
            let min = *min as usize;
            let max = if *max == u32::MAX {
                None
            } else {
                Some(*max as usize)
            };
            let mut ms = ManyState::new(min, max);
            loop {
                if !many_more(ntc, &mut ms)? {
                    break;
                }
                generate_subpattern(ntc, item, state, alphabet, out)?;
            }
        }
    }
    Ok(())
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
    let cache_key = (c as u32, flags & SRE_FLAG_IGNORECASE);
    {
        let guard = CACHE.lock();
        if let Some(cached) = guard.get(&cache_key) {
            return Arc::clone(cached);
        }
    }
    let blacklist = swapcase_blacklist(c, flags);
    let computed: Arc<[char]> = gather_chars(&None, |c| !blacklist.contains(&c)).into();
    CACHE.lock().insert(cache_key, Arc::clone(&computed));
    computed
}

/// Build the set of characters that a `(IN, items)` node can emit, after
/// applying the current flags (IGNORECASE swaps, ASCII restriction) and
/// intersecting with the user-supplied alphabet.
fn build_in_set(
    items: &[SetItem],
    flags: u32,
    alphabet: &Option<IntervalSet>,
) -> Result<InSet, InternalError> {
    let negate = matches!(items.first(), Some(SetItem::Negate));

    let mut positive: Vec<char> = Vec::new();
    let mut categories: Vec<ChCode> = Vec::new();

    for item in items {
        match item {
            SetItem::Negate => {}
            SetItem::Literal(cp) => {
                let c = codepoint_to_char(*cp)?;
                add_with_swapcase(&mut positive, c, flags);
            }
            SetItem::Range(lo, hi) => {
                for cp in *lo..=*hi {
                    if let Some(c) = char::from_u32(cp) {
                        add_with_swapcase(&mut positive, c, flags);
                    }
                }
            }
            SetItem::Category(cat) => {
                categories.push(*cat);
            }
        }
    }

    let ascii_only = flags & SRE_FLAG_ASCII != 0;

    if !negate {
        let mut out: Vec<char> = Vec::new();
        let mut seen: HashSet<char> = HashSet::default();
        for c in positive {
            if ascii_only && (c as u32) >= 128 {
                continue;
            }
            if !alphabet_allows(alphabet, c) {
                continue;
            }
            if seen.insert(c) {
                out.push(c);
            }
        }
        if !categories.is_empty() {
            out.extend(gather_category_class(
                alphabet,
                &categories,
                false,
                &seen,
                ascii_only,
            ));
        }
        Ok(InSet::Listed(out.into()))
    } else {
        let cat_blocks: Vec<ChCode> = categories;
        let mut positive_set: HashSet<char> = HashSet::default();
        for c in positive {
            positive_set.extend(swapcase_blacklist(c, flags));
        }
        let allowed = |c: char| !(ascii_only && (c as u32) >= 128) && !positive_set.contains(&c);
        if cat_blocks.is_empty() && alphabet.is_none() && !ascii_only {
            let excluded = positive_set
                .iter()
                .map(|c| *c as u32)
                .filter(|cp| *cp <= 0xFFFF)
                .map(|cp| (cp, cp))
                .collect();
            Ok(InSet::Complement(Arc::new(Complement::new(excluded))))
        } else if cat_blocks.is_empty() {
            Ok(InSet::Listed(gather_chars(alphabet, allowed).into()))
        } else {
            Ok(InSet::Listed(
                gather_category_class(alphabet, &cat_blocks, true, &positive_set, ascii_only)
                    .into(),
            ))
        }
    }
}
fn add_with_swapcase(v: &mut Vec<char>, c: char, flags: u32) {
    v.push(c);
    if flags & SRE_FLAG_IGNORECASE != 0 {
        if let Some(sw) = char_swapcase(c) {
            v.push(sw);
        }
    }
}

/// Return whether the character `c`, whose General Category is `category`,
/// is in the given CPython character category.
fn in_category(c: char, category: Category, cat: ChCode) -> bool {
    let major = category.as_str().as_bytes()[0];
    match cat {
        ChCode::Digit => category == Category::Nd,
        ChCode::NotDigit => category != Category::Nd,
        ChCode::Space => is_uni_space(c, major),
        ChCode::NotSpace => !is_uni_space(c, major),
        ChCode::Word => is_uni_word(c, major),
        ChCode::NotWord => !is_uni_word(c, major),
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

/// [`in_category`] for a character whose General Category is not yet known.
fn in_category_char(c: char, cat: ChCode) -> bool {
    in_category(c, unicodedata::general_category_char(c), cat)
}

/// Return whether the characters of General Category `category` are in the
/// given CPython character category, going by the General Category alone.
/// Agrees with [`in_category`] for every character from
/// [`CATEGORY_ALONE_FROM`] up: the characters that function singles out
/// (the control and Latin-1 whitespace, `_`) all lie below it.
fn category_in(category: Category, cat: ChCode) -> bool {
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

/// The first codepoint from which [`in_category`] depends on a character's
/// General Category alone, so that [`category_in`] answers for it.
const CATEGORY_ALONE_FROM: u32 = 0x100;

/// Gather, in codepoint order, the characters of `alphabet` (or of the BMP
/// minus the surrogates when no alphabet is given) that are in any of
/// `categories` — or, with `negate`, in none of them — leaving out
/// `excluded` and, with `ascii_only`, everything beyond ASCII. Walks the
/// General Category runs and decides each run at once by [`category_in`]
/// when nothing distinguishes its characters: it starts at or after
/// [`CATEGORY_ALONE_FROM`] and contains no excluded character. The runs
/// below that, and those containing an exclusion, are checked character by
/// character with [`in_category`].
fn gather_category_class(
    alphabet: &Option<IntervalSet>,
    categories: &[ChCode],
    negate: bool,
    excluded: &HashSet<char>,
    ascii_only: bool,
) -> Vec<char> {
    let mut excluded: Vec<u32> = excluded.iter().map(|c| *c as u32).collect();
    excluded.sort_unstable();
    let mut out = Vec::new();
    let mut gather = |lo: u32, hi: u32| {
        let hi = if ascii_only { hi.min(127) } else { hi };
        for (start, end, category) in unicodedata::category_runs_in(lo, hi) {
            let next_excluded = excluded.partition_point(|&cp| cp < start);
            let uniform = start >= CATEGORY_ALONE_FROM
                && excluded.get(next_excluded).is_none_or(|&cp| cp > end);
            if uniform {
                if categories.iter().any(|cat| category_in(category, *cat)) != negate {
                    out.extend((start..=end).filter_map(char::from_u32));
                }
            } else {
                for c in (start..=end).filter_map(char::from_u32) {
                    if excluded.binary_search(&(c as u32)).is_err()
                        && categories.iter().any(|cat| in_category(c, category, *cat)) != negate
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
fn draw_any_char(
    ntc: &mut NativeTestCase,
    alphabet: &Option<IntervalSet>,
) -> Result<char, EngineError> {
    match alphabet {
        None => {
            let cp = ntc.draw_integer_i64(32, 126)?;
            Ok(hegel_internal_unwrap!(
                char::from_u32(cp as u32),
                "draw_any_char: printable-ASCII draw {cp} is not a char"
            ))
        }
        Some(intervals) => {
            let n = intervals.len();
            if n == 0 {
                return Err(mark_invalid(ntc));
            }
            let idx = ntc.draw_integer_i64(0, n as i64 - 1)?;
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
fn emit_from_chars(
    ntc: &mut NativeTestCase,
    chars: &[char],
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    if chars.is_empty() {
        return Err(mark_invalid(ntc));
    }
    let idx = draw_char_index(ntc, chars.len() as u64)?;
    out.push(chars[idx as usize]);
    Ok(())
}

fn emit_from_set(
    ntc: &mut NativeTestCase,
    set: &InSet,
    out: &mut Vec<char>,
) -> Result<(), EngineError> {
    match set {
        InSet::Listed(chars) => emit_from_chars(ntc, chars, out),
        InSet::Complement(complement) => {
            if complement.len == 0 {
                return Err(mark_invalid(ntc));
            }
            let idx = draw_char_index(ntc, complement.len)?;
            out.push(complement.get(idx)?);
            Ok(())
        }
    }
}

/// Draw an index into a character set of `n` (non-zero) characters, biased
/// towards the first 256 when the set is larger than that.
fn draw_char_index(ntc: &mut NativeTestCase, n: u64) -> Result<u64, EngineError> {
    if n > 256 && ntc.weighted(0.8, None)? {
        ntc.draw_integer_uniform(0, 255)
    } else if n > 256 {
        ntc.draw_integer_uniform(256, n - 1)
    } else {
        ntc.draw_integer_uniform(0, n - 1)
    }
}

fn mark_invalid(ntc: &mut NativeTestCase) -> EngineError {
    ntc.conclude(Status::Invalid, None);
    EngineError::InvalidTestCase
}

fn codepoint_to_char(cp: u32) -> Result<char, InternalError> {
    Ok(hegel_internal_unwrap!(
        char::from_u32(cp),
        "invalid codepoint in regex AST: {cp:#x}"
    ))
}

/// Python's `str.swapcase()` on a single char.  Python's definition differs
/// from Rust's only for a handful of title-case codepoints; for the regex
/// strategy the straightforward case-flipping-or-identity behaviour is
/// sufficient, matching what `re.IGNORECASE` does on single-character
/// matches.
///
/// Returns `None` when there is no usable swap: the character is uncased,
/// maps to itself, or its case mapping is more than one codepoint (e.g.
/// `'ß'.to_uppercase()` is `"SS"`, which `re.IGNORECASE` does *not* treat
/// as equal to `'ß'` — Hypothesis guards this with an explicit `re.match`
/// check before offering the swapped form).
fn char_swapcase(c: char) -> Option<char> {
    match swapcase_chars(c).as_slice() {
        [sw] if *sw != c => Some(*sw),
        _ => None,
    }
}

/// All codepoints of `c`'s swapped-case form (possibly several, e.g.
/// `'İ'.to_lowercase()` is `"i\u{307}"`); empty for uncased characters.
fn swapcase_chars(c: char) -> Vec<char> {
    if c.is_lowercase() {
        c.to_uppercase().collect()
    } else if c.is_uppercase() {
        c.to_lowercase().collect()
    } else {
        Vec::new()
    }
}

/// The set of characters `re.IGNORECASE` may treat as equal to `c`: chain
/// `swapcase` to a fixpoint, expanding multi-character results, so e.g.
/// `(?i)[^İ]` excludes {İ, i, U+0307, I}. Port of Hypothesis's fix for
/// issue #2657 ("patterns such as r\"[^\\u0130]+\" where \"i\\u0307\"
/// matches").
fn swapcase_blacklist(c: char, flags: u32) -> Vec<char> {
    let mut blacklist = vec![c];
    if flags & SRE_FLAG_IGNORECASE == 0 {
        return blacklist;
    }
    let mut stack = swapcase_chars(c);
    while let Some(ch) = stack.pop() {
        if !blacklist.contains(&ch) {
            blacklist.push(ch);
            stack.extend(swapcase_chars(ch));
        }
    }
    blacklist
}

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
            if chars_eq(got, want, flags) {
                match_seq(rest, pos + 1, chars, flags, groups)
            } else {
                None
            }
        }
        OpCode::NotLiteral(cp) => {
            let banned = char::from_u32(*cp)?;
            let got = *chars.get(pos)?;
            if chars_eq(got, banned, flags) {
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
            let inner_flags = (flags | *add_flags) & !*del_flags;
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
                if !chars_eq(chars[pos + i], *vc, flags) {
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
fn chars_eq(a: char, b: char, flags: u32) -> bool {
    if a == b {
        return true;
    }
    if flags & SRE_FLAG_IGNORECASE != 0 {
        char_swapcase(a) == Some(b) || char_swapcase(b) == Some(a)
    } else {
        false
    }
}
fn char_matches_set(items: &[SetItem], c: char, flags: u32) -> bool {
    let negate = matches!(items.first(), Some(SetItem::Negate));
    let mut contained = false;
    for item in items {
        match item {
            SetItem::Negate => {}
            SetItem::Literal(cp) => {
                if let Some(lc) = char::from_u32(*cp) {
                    if chars_eq(c, lc, flags) {
                        contained = true;
                    }
                }
            }
            SetItem::Range(lo, hi) => {
                let cp = c as u32;
                if cp >= *lo && cp <= *hi {
                    contained = true;
                } else if flags & SRE_FLAG_IGNORECASE != 0 {
                    if let Some(sw) = char_swapcase(c) {
                        if (sw as u32) >= *lo && (sw as u32) <= *hi {
                            contained = true;
                        }
                    }
                }
            }
            SetItem::Category(cat) => {
                if in_category_char(c, *cat) {
                    contained = true;
                }
            }
        }
    }
    if negate { !contained } else { contained }
}
fn at_matches(at: &AtCode, chars: &[char], pos: usize, flags: u32) -> bool {
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
        AtCode::Boundary => is_word_boundary(chars, pos),
        AtCode::NonBoundary => !is_word_boundary(chars, pos),
    }
}
fn is_word_boundary(chars: &[char], pos: usize) -> bool {
    let before = pos > 0 && in_category_char(chars[pos - 1], ChCode::Word);
    let after = pos < chars.len() && in_category_char(chars[pos], ChCode::Word);
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
