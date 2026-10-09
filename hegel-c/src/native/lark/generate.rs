//! Generating strings from an analysed grammar.
//!
//! Every draw first picks a target length: with probability 3/4 a length
//! uniform between the smallest string the start rule can produce and
//! the size cap, otherwise the smallest. The grammar is then walked top
//! down with a character budget, in which every terminal counts for the
//! length of its strings on average ([`StringSpec::typical_width`]), so
//! that the structure a budget buys comes out near the budget. The
//! budget is the target less what the `%ignore` insertions between
//! tokens will add at the string's insertion rate
//! ([`ignore_cost`]). A rule's
//! alternatives are split into those that can grow without bound and
//! those that cannot, a weighted boolean whose probability rises with
//! the budget left over after the cheapest growing alternative chooses
//! between the two classes,
//! and the alternative is drawn uniformly from the class; a sequence
//! splits its spare budget between its flexible items at uniformly drawn
//! cut points, so no position is systematically larger than another; a
//! repetition draws a count that fits its budget and shares the budget
//! equally between the elements, pulling them with the collection
//! protocol so the shrinker can delete them one at a time. Budgets steer
//! the structure only: each terminal is drawn by its own regex generator
//! (redrawn a few times when it alone overruns the size cap), and a
//! string that overruns the cap regardless is discarded and regenerated
//! with half the target, up to [`MAX_ATTEMPTS`] times.
//!
//! Between adjacent tokens the generator inserts a `%ignore` terminal
//! either when the simulated lexer says the two would otherwise lex
//! differently, or at random with a rate drawn once per string. Each
//! terminal string is checked to lex back as itself, and each inserted
//! ignorable to be skipped between its neighbours, with a few retries
//! before the string is accepted as it is. Every token keeps what the
//! lexers found at the start of its text on its own ([`TokenScans`]),
//! with how that depended on the text's edges, and most stretches of
//! tokens are settled from those findings alone ([`GrammarSpec::settled`]):
//! each token reads as itself, nothing about it looks at the text before
//! it, and what the end of its text left open the next token closes
//! without changing the reading. The rest are lexed in full; those checks
//! look at adjacent tokens only, and a backtracking regex can run on much
//! further (an escaped delimiter it prefers to read as such, say, that
//! then lets it reach the next delimiter in the string), so a finished
//! string with such a stretch is lexed once more as a whole and
//! regenerated when it does not read back as its tokens.

use super::analysis::{Alt, Analysis, Node, NodeKind, analyze};
use super::builder::{ImportSource, load_grammar};
use super::compile::{Grammar, RuleId, TermId, compile};
use super::lexsim::{Lexed, Lexer, Scan};
use super::{GrammarError, GrammarResult};
use crate::control::{hegel_internal_assert, hegel_internal_unwrap};
use crate::native::HashMap;
use crate::native::core::{EngineError, ManyState, NativeTestCase, Status};
use crate::native::draws::{
    LABEL_BOOLEAN, LABEL_INTEGER, StringSpec, generate_string_into, many_more, spanned,
};
use crate::native::labels::label_from_name;
use crate::sys::sync::Mutex;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// The default cap on the length of a generated string, in characters.
pub const DEFAULT_MAX_SIZE: u64 = 8192;

/// The default cap on how deeply rules nest.
pub const DEFAULT_MAX_DEPTH: u32 = 200;

/// How many distinct token texts a grammar remembers the lexing of
/// (see [`GrammarSpec::read_token`]) before forgetting them all.
const READINGS_CAPACITY: usize = 1 << 13;

/// The number of strings one draw discards, for being oversize (halving
/// its target each time) or for not lexing back as their tokens, before
/// the test case is rejected.
const MAX_ATTEMPTS: u32 = 9;

/// How often a terminal is redrawn when the basic lexer would not read it
/// back as itself before the draw is accepted anyway.
const LEX_RETRIES: u32 = 3;

/// How many ignorables an insertion tries before giving up (dropping
/// an optional one, or keeping the last when the lexer needs one),
/// how many may follow one another to keep an ignorable the lexer would
/// otherwise run into the next token (a comment, which only a newline
/// ends) apart from it, and how many strings one such chain may draw.
const IGNORE_ATTEMPTS: u32 = 4;
const IGNORE_CHAIN: u32 = 2;
const IGNORE_CHAIN_DRAWS: u32 = 4;
const IGNORE_CHAIN_PATIENCE: u32 = 4;

/// The probability that a draw aims for a large string rather than the
/// smallest one.
const LARGE_TARGET_PROBABILITY: f64 = 0.75;

/// The spare budget, beyond the cheapest growing alternative, at which
/// a rule is as likely to pick a growing alternative as a closing one:
/// the probability of growing is `spare / (spare + GROW_SCALE)`.
const GROW_SCALE: f64 = 1.0;

/// The number of distinct `%ignore` insertion rates a string may draw;
/// the rate is `(k / (IGNORE_RATE_STEPS - 1))^2` for `k` drawn uniformly
/// in `0..IGNORE_RATE_STEPS`, so most strings carry little optional
/// whitespace and a few are saturated with it.
const IGNORE_RATE_STEPS: u64 = 5;

/// Elements of a repetition are pulled with the collection protocol so
/// the shrinker can delete them one at a time; the continue probability
/// is set so a repetition reaches the count it drew with probability
/// about `exp(-1 / COUNT_PERSISTENCE)`, and never stops short more than
/// one time in `MIN_PERSISTENCE`.
const COUNT_PERSISTENCE: f64 = 8.0;
const MIN_PERSISTENCE: f64 = 32.0;

/// The granularity of the log-uniform draw of a repetition's extra
/// element count.
const COUNT_RESOLUTION: u64 = 1 << 20;

const LABEL_GRAMMAR: u64 = label_from_name("hegel.lark");
const LABEL_TERMINAL: u64 = label_from_name("hegel.lark.terminal");
const LABEL_IGNORE: u64 = label_from_name("hegel.lark.ignore");
const LABEL_ELEMENT: u64 = label_from_name("hegel.lark.element");

/// How the strings of one terminal are drawn.
enum TerminalGen {
    /// From the terminal's own pattern, checked against the lexer.
    Pattern(StringSpec),
    /// From a generator supplied for a `%declare`d (or overridden)
    /// terminal, taken as it comes.
    Explicit(Arc<StringSpec>),
}

/// Options for building a [`GrammarSpec`].
pub struct GrammarOptions {
    pub start: String,
    pub explicit: Vec<(String, Arc<StringSpec>)>,
    pub alphabet: Option<Arc<StringSpec>>,
    pub max_size: u64,
    pub max_depth: u32,
}

impl Default for GrammarOptions {
    fn default() -> Self {
        GrammarOptions {
            start: String::from("start"),
            explicit: Vec::new(),
            alphabet: None,
            max_size: DEFAULT_MAX_SIZE,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }
}

/// A grammar ready to generate from: the analysed rules, a generator per
/// terminal, the lexer simulation and the size and depth caps.
pub struct GrammarSpec {
    analysis: Analysis,
    terminals: Vec<TerminalGen>,
    #[cfg(test)]
    terminal_names: Vec<String>,
    ignore: Vec<TermId>,
    ignore_cost: f64,
    lexer: Lexer,
    start: RuleId,
    labels: Vec<u64>,
    max_size: u64,
    max_depth: u32,
    /// What the lexers found in each distinct token text drawn so far,
    /// by terminal, so that a keyword, an operator or a stretch of
    /// whitespace drawn again, in the same string or a later one, is
    /// lexed once; emptied when it holds [`READINGS_CAPACITY`] texts.
    readings: Mutex<ReadingsCache>,
    /// Working space for [`GrammarSpec::scans`] and
    /// [`GrammarSpec::lex_all`], kept from one check to the next so that
    /// the walks allocate nothing.
    scratch: Mutex<ScansScratch>,
    /// The vectors of the roomiest finished [`Generation`], emptied, for
    /// the next draw to work in instead of growing its own from nothing.
    buffers: Mutex<Option<Buffers>>,
}

/// The vectors a [`Generation`] works in, emptied, between one draw and
/// the next.
#[derive(Default)]
struct Buffers {
    text: Vec<char>,
    tokens: Vec<Token>,
    budgets: Vec<u64>,
    candidates: Vec<TermId>,
    chains: HashMap<(TermId, TermId), (u32, u32)>,
}

impl Buffers {
    /// Put these buffers in `slot` when they have room for more text than
    /// the ones there, handing back whichever set is not kept.
    fn offer(self, slot: &mut Option<Buffers>) -> Option<Buffers> {
        match slot {
            Some(held) if held.text.capacity() >= self.text.capacity() => Some(self),
            _ => slot.replace(self),
        }
    }
}

/// The vectors [`GrammarSpec::scans`] works in: where each token lies,
/// which tokens the parser needs, the positions the lexer can reach and
/// the terminals tried at the position being walked; and the tokens
/// [`GrammarSpec::lex_all`] observes, with their extents.
#[derive(Default)]
struct ScansScratch {
    bounds: Vec<(usize, usize)>,
    significant: Vec<usize>,
    reachable: Vec<bool>,
    tried: Vec<(TermId, Lexed)>,
    observed: Vec<(TermId, usize, usize)>,
}

/// What the lexers found in each distinct text drawn for one terminal.
type Readings = HashMap<Vec<char>, Arc<TokenScans>>;

struct ReadingsCache {
    by_terminal: Vec<Readings>,
    len: usize,
}

impl GrammarSpec {
    /// Build a spec from a compiled grammar whose start rules are exactly
    /// the one named in `options`.
    pub fn new(grammar: Grammar, mut options: GrammarOptions) -> GrammarResult<GrammarSpec> {
        let start = grammar.start[0];
        for (name, _) in &options.explicit {
            if grammar.terminal_id(name).is_none() {
                return Err(GrammarError::new(format!(
                    "explicit generator for terminal '{name}', which the grammar does not use"
                )));
            }
        }
        let mut terminals = Vec::with_capacity(grammar.terminals.len());
        let mut term_min = Vec::with_capacity(grammar.terminals.len());
        let mut term_typical = Vec::with_capacity(grammar.terminals.len());
        for term in &grammar.terminals {
            let explicit = options
                .explicit
                .iter()
                .position(|(name, _)| *name == term.name)
                .map(|i| options.explicit.swap_remove(i).1);
            match (explicit, &term.regexp) {
                (Some(spec), _) => {
                    term_min.push(1);
                    term_typical.push(typical_size(&spec, 1));
                    terminals.push(TerminalGen::Explicit(spec));
                }
                (None, Some(regexp)) => {
                    let spec = StringSpec::regex(regexp, true, options.alphabet.as_deref())
                        .map_err(|e| {
                            GrammarError::new(format!(
                                "Cannot generate terminal {} from {regexp}: {e}",
                                term.name
                            ))
                        })?;
                    term_min.push(term.min_width);
                    term_typical.push(typical_size(&spec, term.min_width));
                    terminals.push(TerminalGen::Pattern(spec));
                }
                (None, None) => {
                    return Err(GrammarError::new(format!(
                        "Terminal '{}' is declared but has no definition: supply a generator \
                         for it with explicit",
                        term.name
                    )));
                }
            }
        }
        let ignore_cost = ignore_cost(&grammar.ignore, &term_typical);
        let analysis = analyze(&grammar, term_min, term_typical)?;
        let lexer = Lexer::new(&grammar.terminals);
        let root = &analysis.rules[start];
        if options.max_size < root.min_size {
            return Err(GrammarError::new(format!(
                "max_size is {} but the smallest string '{}' can produce has {} characters",
                options.max_size, root.name, root.min_size
            )));
        }
        if options.max_depth < root.min_depth {
            return Err(GrammarError::new(format!(
                "max_depth is {} but the shallowest string '{}' can produce nests {} rules deep",
                options.max_depth, root.name, root.min_depth
            )));
        }
        let labels = analysis
            .rules
            .iter()
            .map(|rule| label_from_name(&format!("hegel.lark.rule.{}", rule.name)))
            .collect();
        let terminals_len = terminals.len();
        Ok(GrammarSpec {
            analysis,
            terminals,
            #[cfg(test)]
            terminal_names: grammar.terminals.iter().map(|t| t.name.clone()).collect(),
            ignore: grammar.ignore,
            ignore_cost,
            lexer,
            start,
            labels,
            max_size: options.max_size,
            max_depth: options.max_depth,
            readings: Mutex::new(ReadingsCache {
                by_terminal: (0..terminals_len).map(|_| HashMap::default()).collect(),
                len: 0,
            }),
            scratch: Mutex::new(ScansScratch::default()),
            buffers: Mutex::new(None),
        })
    }

    /// The scans and reading of a token of terminal `id` with `text`,
    /// from the ones remembered when the text was drawn before, else
    /// computed (with `own` for the terminal's own scan when the caller
    /// already has it) and remembered.
    fn read_token(&self, id: TermId, text: &[char], own: Option<Scan>) -> Arc<TokenScans> {
        if let Some(found) = self.readings.lock().by_terminal[id].get(text) {
            return Arc::clone(found);
        }
        let scans = Arc::new(self.scan_token(id, text, own));
        let mut readings = self.readings.lock();
        if readings.len >= READINGS_CAPACITY {
            readings.by_terminal.iter_mut().for_each(HashMap::clear);
            readings.len = 0;
        }
        if readings.by_terminal[id]
            .insert(text.to_vec(), Arc::clone(&scans))
            .is_none()
        {
            readings.len += 1;
        }
        scans
    }

    /// The smallest string the grammar can produce, in characters.
    pub fn min_size(&self) -> u64 {
        self.analysis.rules[self.start].min_size
    }

    /// The names of the terminals, indexed by [`TermId`].
    #[cfg(test)]
    pub fn terminal_names(&self) -> &[String] {
        &self.terminal_names
    }

    fn is_ignore(&self, id: TermId) -> bool {
        self.ignore.contains(&id)
    }

    fn same_type(&self, expected: TermId, got: TermId) -> bool {
        expected == got || (self.is_ignore(expected) && self.is_ignore(got))
    }

    fn is_pattern(&self, id: TermId) -> bool {
        matches!(self.terminals[id], TerminalGen::Pattern(_))
    }

    /// Whether the basic lexer reads `token`'s text on its own as one
    /// token: of its own type, or for a token the grammar needs, of
    /// another type (an ignorable read as something the parser would
    /// see is not skipped, see [`GrammarSpec::basic_fit`]).
    fn single(&self, token: &Token) -> bool {
        match token.reading() {
            Reading::Itself => true,
            Reading::Tokens(read) => read.len() == 1 && !self.is_ignore(token.id),
            Reading::Ignorables | Reading::Opaque => false,
        }
    }

    /// The tokens the basic lexer reads `text` as, with their extents,
    /// into `observed`. `known` lists, in order, where the tokens that
    /// make up `text` start, how long each is and what the lexer found at
    /// its start on its own, for the lexer to carry over where it arrives
    /// at such a start (see [`Lexer::extend`]).
    fn lex_all(
        &self,
        text: &[char],
        known: &[(usize, usize, &Scan)],
        observed: &mut Vec<(TermId, usize, usize)>,
    ) -> Result<(), Lexed> {
        observed.clear();
        let mut pos = 0;
        let mut k = 0;
        while pos < text.len() {
            while k < known.len() && known[k].0 < pos {
                k += 1;
            }
            let lexed = match known.get(k) {
                Some(&(start, len, scan)) if start == pos => {
                    self.lexer.extend(scan, text, pos, len, true)
                }
                _ => None,
            }
            .unwrap_or_else(|| self.lexer.next_token(text, pos));
            match lexed {
                Lexed::Token(id, end) => {
                    observed.push((id, pos, end));
                    pos = end;
                }
                other => return Err(other),
            }
        }
        Ok(())
    }

    /// How much of `text`, which terminal `id`'s pattern matches in full,
    /// the pattern takes when matched from the front: less than all of
    /// it after a lazy repeat or an earlier branch of an alternation, in
    /// which case no lexer reads the text back whole but every one reads
    /// back that prefix, itself a string the terminal matches. Also what
    /// the match found, for [`GrammarSpec::scan_token`] when nothing is
    /// cut off.
    fn whole(&self, id: TermId, text: &[char]) -> (usize, Scan) {
        let scan = self.lexer.match_terminal_scan(id, text, 0);
        let end = match scan.lexed {
            Lexed::Token(_, end) => end,
            Lexed::NoMatch | Lexed::Unknown => text.len(),
        };
        (end, scan)
    }

    /// What the lexers find at the start of `text`, the text of a token
    /// of terminal `id`, on its own, and how the basic lexer reads all
    /// of it; `own` is the terminal's own match there when already known.
    fn scan_token(&self, id: TermId, text: &[char], own: Option<Scan>) -> TokenScans {
        let own = own.unwrap_or_else(|| self.lexer.match_terminal_scan(id, text, 0));
        let basic = self.lexer.next_token_scan_knowing(text, 0, id, &own);
        TokenScans {
            basic_follows_own: basic.follows(&own),
            reading: self.reading(id, text, &basic),
            own,
            basic,
        }
    }

    /// How the basic lexer reads the text of terminal `id` on its own,
    /// given what it finds at the start of it.
    fn reading(&self, id: TermId, text: &[char], basic: &Scan) -> Reading {
        if !self.is_pattern(id) {
            return Reading::Opaque;
        }
        let mut scratch = self.scratch.lock();
        let got = &mut scratch.observed;
        if self.lex_all(text, &[(0, text.len(), basic)], got).is_err() {
            return Reading::Opaque;
        }
        if got.len() == 1 && self.same_type(id, got[0].0) {
            Reading::Itself
        } else if self.is_ignore(id) && got.iter().all(|&(g, _, _)| self.is_ignore(g)) {
            Reading::Ignorables
        } else {
            Reading::Tokens(got.iter().map(|&(g, _, _)| g).collect())
        }
    }

    /// Whether the dynamic lexer reads `text` back as `tokens`: it
    /// matches each token's own pattern where the parser expects the
    /// token and the ignorable patterns anywhere between, so the check
    /// walks the positions the lexer can reach. A token's pattern may
    /// start early, inside the ignorables before it, and may run on past
    /// its end, as may an ignorable's: through ignorables and tokens the
    /// grammar can do without (see [`Token::droppable`]), and into the
    /// next token the grammar needs as long as that token's own pattern
    /// still reaches its end from where the run-in stops (a comment that
    /// takes the `\r` of the `\r\n` after it, say); it may never take
    /// all of a needed token. A pattern too involved to match is given
    /// the benefit of the doubt.
    fn scans(&self, text: &[char], tokens: &[Token]) -> bool {
        let mut scratch = self.scratch.lock();
        let ScansScratch {
            bounds,
            significant,
            reachable,
            tried,
            observed: _,
        } = &mut *scratch;
        bounds.clear();
        let mut pos = 0;
        for token in tokens {
            bounds.push((pos, pos + token.len()));
            pos += token.len();
        }
        significant.clear();
        significant.extend((0..tokens.len()).filter(|&i| !self.is_ignore(tokens[i].id)));
        let kept = |k: usize| {
            significant[k..]
                .iter()
                .find(|&&i| !tokens[i].droppable)
                .copied()
        };
        let within = |got: usize, k: usize| kept(k).is_none_or(|i| got < bounds[i].1);
        reachable.clear();
        reachable.resize(text.len() + 1, false);
        reachable[0] = true;
        let mut k = 0;
        let mut t = 0;
        for p in 0..=text.len() {
            if !reachable[p] {
                continue;
            }
            while k < significant.len() && bounds[significant[k]].1 <= p {
                k += 1;
            }
            while t < tokens.len() && bounds[t].0 < p {
                t += 1;
            }
            if p == text.len() {
                return true;
            }
            let starting = (t < tokens.len() && bounds[t].0 == p).then_some(t);
            let carried =
                |scan: &Scan, j: usize| self.lexer.extend(scan, text, p, tokens[j].len(), false);
            tried.clear();
            for (j, &i) in significant.iter().enumerate().skip(k) {
                let id = tokens[i].id;
                let lexed = match tried.iter().find(|(tried, _)| *tried == id) {
                    Some(&(_, lexed)) => lexed,
                    None => {
                        let lexed = starting
                            .filter(|&s| s == i)
                            .and_then(|s| carried(&tokens[s].scans.own, s))
                            .unwrap_or_else(|| self.lexer.match_terminal(id, text, p));
                        tried.push((id, lexed));
                        lexed
                    }
                };
                match lexed {
                    Lexed::Token(_, got) if got >= bounds[i].1 && within(got, j + 1) => {
                        reachable[got] = true;
                    }
                    Lexed::Unknown => reachable[bounds[i].1] = true,
                    _ => {}
                }
                if !tokens[i].droppable {
                    break;
                }
            }
            for &ignore in &self.ignore {
                match self.lexer.match_terminal(ignore, text, p) {
                    Lexed::Token(_, got) if got > p && within(got, k) => reachable[got] = true,
                    Lexed::Unknown => {
                        reachable[kept(k).map_or(text.len(), |i| bounds[i].0.max(p))] = true;
                    }
                    _ => {}
                }
            }
        }
        false
    }

    /// How well the lexers read `text`, the text of `tokens` one after
    /// another, back: [`Fit::Settled`] when what they found at the start
    /// of each token on its own already proves they read it as `tokens`
    /// (see [`GrammarSpec::settled`]), otherwise [`Fit::Wrong`] when the
    /// dynamic lexer does not read it as `tokens` at all (see
    /// [`GrammarSpec::scans`]), [`Fit::Exact`] when the basic lexer reads
    /// it as `tokens` too, and [`Fit::Retyped`] in between.
    fn fit(&self, text: &[char], tokens: &[Token]) -> Fit {
        if self.settled(text, tokens) {
            return Fit::Settled;
        }
        if !self.scans(text, tokens) {
            return Fit::Wrong;
        }
        self.basic_fit(text, tokens).max(Fit::Retyped)
    }

    /// Whether what the lexers found at the start of each token's text on
    /// its own proves that they read `text`, the concatenation of
    /// `tokens`, as exactly those tokens, and would still with any text
    /// after it: every token reads back as one token (of its own type or,
    /// as on its own, another) and its own pattern takes all of it,
    /// nothing a lexer tried on a token after the first looked at the
    /// text before it, and what the end of a token's text left open the
    /// text after it settles without changing what either lexer reads
    /// (see [`Lexer::settle`]).
    fn settled(&self, text: &[char], tokens: &[Token]) -> bool {
        let mut start = 0;
        for (i, token) in tokens.iter().enumerate() {
            let len = token.len();
            let Lexed::Token(read, _) = token.scans.basic.lexed else {
                return false;
            };
            if !self.single(token)
                || !token.whole()
                || (i > 0 && (token.scans.own.anchored || token.scans.basic.anchored))
            {
                return false;
            }
            if i + 1 < tokens.len() {
                let own = self.lexer.settle(&token.scans.own, text, start, len, false);
                if own != Some(Lexed::Token(token.id, start + len)) {
                    return false;
                }
                if !token.scans.basic_follows_own {
                    let basic = self
                        .lexer
                        .settle(&token.scans.basic, text, start, len, true);
                    if basic != Some(Lexed::Token(read, start + len)) {
                        return false;
                    }
                }
            }
            start += len;
        }
        true
    }

    /// How the basic lexer reads the concatenation of `tokens`: every
    /// token that is not ignorable must come out as the token it is, with
    /// the same type and at least the same extent (a newline terminal may
    /// take over the whitespace that begins a comment after it), a token
    /// the lexer reads differently even on its own must come out as that
    /// same reading, and the stretches of ignorable tokens may be read as
    /// any ignorable tokens. The lexer reading a token with another type
    /// but the exact extent is only [`Fit::Retyped`]; anything else that
    /// differs is [`Fit::Wrong`]. A token the lexer cannot read at all
    /// makes the whole check moot.
    fn basic_fit(&self, text: &[char], tokens: &[Token]) -> Fit {
        if tokens.iter().any(|t| *t.reading() == Reading::Opaque) {
            return Fit::Exact;
        }
        let mut known = Vec::with_capacity(tokens.len());
        let mut start = 0;
        for token in tokens {
            known.push((start, token.len(), &token.scans.basic));
            start += token.len();
        }
        let mut scratch = self.scratch.lock();
        let observed = &mut scratch.observed;
        match self.lex_all(text, &known, observed) {
            Ok(()) => {}
            Err(Lexed::NoMatch) => return Fit::Wrong,
            Err(_) => return Fit::Exact,
        }
        let mut regions = Vec::with_capacity(tokens.len());
        let mut pos = 0;
        for token in tokens {
            let end = pos + token.len();
            if !(token.lexable() && self.is_ignore(token.id)) {
                regions.push((pos, end, token));
            }
            pos = end;
        }
        let overlapping = |start: usize, end: usize| {
            regions
                .iter()
                .filter(move |&&(s, e, _)| start < e && s < end)
        };
        let mut fit = Fit::Exact;
        let mut i = 0;
        while i < observed.len() {
            let (got, start, end) = observed[i];
            let mut hits = overlapping(start, end);
            let Some(&(s, e, token)) = hits.next() else {
                if !self.is_ignore(got) {
                    fit = Fit::Retyped;
                }
                i += 1;
                continue;
            };
            if hits.next().is_some() {
                return Fit::Wrong;
            }
            if self.is_ignore(token.id) {
                while i < observed.len() && observed[i].1 < e {
                    if overlapping(observed[i].1, observed[i].2).nth(1).is_some() {
                        return Fit::Wrong;
                    }
                    if !self.is_ignore(observed[i].0) {
                        fit = Fit::Retyped;
                    }
                    i += 1;
                }
                continue;
            }
            let expected: &[TermId] = match token.reading() {
                Reading::Tokens(expected) => expected,
                _ => core::slice::from_ref(&token.id),
            };
            let mut j = i + 1;
            while j < observed.len() && observed[j].1 < e {
                if overlapping(observed[j].1, observed[j].2).nth(1).is_some() {
                    return Fit::Wrong;
                }
                j += 1;
            }
            let group = &observed[i..j];
            let last_end = group[group.len() - 1].2;
            if group.len() != expected.len() {
                return Fit::Wrong;
            }
            if !group
                .iter()
                .zip(expected)
                .all(|(&(g, _, _), &x)| self.same_type(x, g))
            {
                if start == s && last_end == e {
                    fit = Fit::Retyped;
                } else {
                    return Fit::Wrong;
                }
            }
            i = j;
        }
        fit
    }
}

/// The outcome of placing one ignorable (and any chain after it).
enum Placement {
    /// The tokens are in the string and the lexer reads them this well.
    Placed(Fit),
    /// Nothing is left in the string; the ignorable drawn first was of
    /// this terminal, and read back as itself on its own or not, or was
    /// too long for the string.
    Rejected {
        id: TermId,
        lexable: bool,
        oversize: bool,
    },
}

/// How the lexer reads one token's text on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Reading {
    /// As a single token of its own type.
    Itself,
    /// For an ignorable, as several tokens, all of them ignorable.
    Ignorables,
    /// As this sequence of tokens.
    Tokens(Vec<TermId>),
    /// Not at all: the lexer does not know the terminal, cannot read the
    /// text, or ran out of budget.
    Opaque,
}

/// How the lexer reads a stretch of tokens, from worst to best: with
/// tokens merged, split or swallowed, with the right extents but other
/// types, exactly, or exactly and provably so with any text after it
/// (see [`GrammarSpec::settled`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Fit {
    Wrong,
    Retyped,
    Exact,
    Settled,
}

/// Load a Lark grammar and build a [`GrammarSpec`] for it.
pub fn from_lark(
    text: &str,
    source: &dyn ImportSource,
    options: GrammarOptions,
) -> GrammarResult<GrammarSpec> {
    let built = load_grammar(text, None, source)?;
    let grammar = compile(&built, &[options.start.as_str()])?;
    GrammarSpec::new(grammar, options)
}

#[derive(Clone)]
struct Token {
    id: TermId,
    /// Where its text lies in the string being generated
    /// ([`Generation::text`]).
    start: usize,
    end: usize,
    /// Whether the string stays in the grammar without this token: it
    /// is the only token of an element of a repetition that allows none
    /// (`x?`, `[x]`, `x*`), so a lexer that runs it into its neighbour
    /// or skips it leaves a string the parser still accepts.
    droppable: bool,
    scans: Arc<TokenScans>,
}

/// What the lexers found in a token's text on its own, kept with the
/// token (and remembered for the text, see [`GrammarSpec::read_token`])
/// so that the checks on the longer texts the token is part of (its fit
/// with its neighbours, the finished string) need not match it again
/// where the finding carries over (see [`Lexer::extend`]).
struct TokenScans {
    /// The token's own terminal matched at its start.
    own: Scan,
    /// The basic lexer's first token.
    basic: Scan,
    /// How the basic lexer reads the whole text.
    reading: Reading,
    /// Whether `basic` carries over any text exactly as `own` does (see
    /// [`Scan::follows`]): the basic lexer got as far as the token's own
    /// terminal without another leaving anything open, so settling the
    /// token for it (see [`GrammarSpec::settled`]) can only come out as
    /// settling its own match did, re-typed as the basic lexer read the
    /// token on its own.
    basic_follows_own: bool,
}

impl Token {
    fn len(&self) -> usize {
        self.end - self.start
    }

    /// How the basic lexer reads the token's text on its own.
    fn reading(&self) -> &Reading {
        &self.scans.reading
    }

    /// Whether the basic lexer reads the token back as itself on its own
    /// (or, for an ignorable, as nothing but ignorable tokens).
    fn lexable(&self) -> bool {
        matches!(self.reading(), Reading::Itself | Reading::Ignorables)
    }

    /// Whether the token's own pattern takes all of its text.
    fn whole(&self) -> bool {
        self.scans.own.lexed == Lexed::Token(self.id, self.len())
    }
}

enum Step {
    Engine(EngineError),
    TooLong,
    /// The finished string does not lex back as its tokens.
    Misread,
}

impl From<EngineError> for Step {
    fn from(e: EngineError) -> Self {
        Step::Engine(e)
    }
}

struct Generation<'s> {
    spec: &'s GrammarSpec,
    /// The string so far: the text of `tokens`, in order and without
    /// gaps. Terminals are drawn onto its end, and a draw that is not
    /// kept is cut off again.
    text: Vec<char>,
    tokens: Vec<Token>,
    ignore_rate: f64,
    /// Whether some stretch of the tokens was accepted without being
    /// settled (see [`GrammarSpec::settled`]), so that the finished
    /// string has to be lexed as a whole.
    unsettled: bool,
    /// Scratch for [`Generation::sequence`]: the budgets of the items of
    /// every sequence being generated, innermost last.
    budgets: Vec<u64>,
    /// Scratch for [`Generation::insert_ignore`]: the ignorables still
    /// worth drawing.
    candidates: Vec<TermId>,
    /// How often each ignorable drawn to end another's match in a chain
    /// (see [`Generation::ignore_chain`]) has failed and succeeded at
    /// it, by the pair of terminals.
    chains: HashMap<(TermId, TermId), (u32, u32)>,
}

impl<'s> Generation<'s> {
    /// A generation working in the buffers `spec` has to spare.
    fn new(spec: &'s GrammarSpec, ignore_rate: f64) -> Generation<'s> {
        let buffers = spec.buffers.lock().take().unwrap_or_default();
        Generation {
            spec,
            text: buffers.text,
            tokens: buffers.tokens,
            ignore_rate,
            unsettled: false,
            budgets: buffers.budgets,
            candidates: buffers.candidates,
            chains: buffers.chains,
        }
    }

    /// Hand the buffers back to the spec, emptied.
    fn recycle(self) {
        let Generation {
            spec,
            mut text,
            mut tokens,
            mut budgets,
            mut candidates,
            mut chains,
            ..
        } = self;
        text.clear();
        tokens.clear();
        budgets.clear();
        candidates.clear();
        chains.clear();
        let buffers = Buffers {
            text,
            tokens,
            budgets,
            candidates,
            chains,
        };
        let unkept = buffers.offer(&mut spec.buffers.lock());
        drop(unkept);
    }

    /// Forget the string.
    fn reset(&mut self) {
        self.text.clear();
        self.tokens.clear();
        self.unsettled = false;
        self.chains.clear();
    }

    /// The token of terminal `id` whose text is the string from `start`
    /// on, with `own` its own scan when already known.
    fn token(&mut self, id: TermId, start: usize, own: Option<Scan>) -> Token {
        Token {
            id,
            start,
            end: self.text.len(),
            droppable: false,
            scans: self.spec.read_token(id, &self.text[start..], own),
        }
    }

    /// Whether a string `len` characters long is within the size cap.
    fn fits(&self, len: usize) -> bool {
        len as u64 <= self.spec.max_size
    }

    /// The tokens from index `from` on, leaving out the last `pending`
    /// (see [`Generation::maybe_ignore`]), with their text.
    fn window(&self, from: usize, pending: usize) -> (&[char], &[Token]) {
        let tokens = &self.tokens[from..self.tokens.len() - pending];
        let start = tokens.first().map_or(self.text.len(), |t| t.start);
        let end = tokens.last().map_or(start, |t| t.end);
        (&self.text[start..end], tokens)
    }

    /// The token before the last `pending`, if any.
    fn before(&self, pending: usize) -> Option<&Token> {
        self.tokens
            .len()
            .checked_sub(pending + 1)
            .map(|i| &self.tokens[i])
    }
}

/// The `index`th of `candidates`, which there must be.
fn nth_candidate(
    mut candidates: impl Iterator<Item = TermId>,
    index: usize,
) -> Result<TermId, EngineError> {
    Ok(hegel_internal_unwrap!(
        candidates.nth(index),
        "ignore_chain: drew an index past the candidates"
    ))
}

/// Draw an index into `count` candidates, uniformly.
fn draw_index(ntc: &mut NativeTestCase, count: usize) -> Result<usize, Step> {
    let index = spanned(ntc, LABEL_INTEGER, |ntc| {
        ntc.draw_integer_uniform(0, count as u64 - 1)
    })?;
    Ok(index as usize)
}

/// The length of `spec`'s strings on average, as a whole number of
/// characters and never below `min`, the shortest string it can produce.
fn typical_size(spec: &StringSpec, min: u64) -> u64 {
    (spec.typical_width().round() as u64).max(min)
}

/// The characters a `%ignore` insertion adds for each character a
/// token adds, on average, with `term_typical` the typical width of
/// each terminal: a string drawn with insertion rate `r` comes out
/// about `1 + r * ignore_cost` times the budget its tokens were given.
fn ignore_cost(ignore: &[TermId], term_typical: &[u64]) -> f64 {
    let mean = |ids: &mut dyn Iterator<Item = TermId>| {
        let (sum, count) = ids.fold((0u64, 0u64), |(sum, count), id| {
            (sum + term_typical[id], count + 1)
        });
        (count > 0).then(|| sum as f64 / count as f64)
    };
    let Some(inserted) = mean(&mut ignore.iter().copied()) else {
        return 0.0;
    };
    let token = mean(&mut (0..term_typical.len()).filter(|id| !ignore.contains(id)));
    inserted / token.unwrap_or(inserted).max(1.0)
}

/// The budget for the tokens of a string aiming at `target` characters
/// with `%ignore` insertion rate `rate`, never below `min`: the
/// insertions add about `rate * ignore_cost` characters per token one.
fn token_budget(target: u64, min: u64, rate: f64, ignore_cost: f64) -> u64 {
    min.max((target as f64 / (1.0 + rate * ignore_cost)) as u64)
}

/// How many alternatives of a rule [`Generation::choose_alt`] can hold in
/// a bit mask.
const MASKED_ALTS: usize = u64::BITS as usize;

/// The position of the `index`th set bit of `bits`, counting from the
/// lowest, when it has that many.
fn nth_set_bit(mut bits: u64, index: u64) -> Option<u32> {
    for _ in 0..index.min(u64::from(u64::BITS)) {
        bits &= bits.wrapping_sub(1);
    }
    (bits != 0).then_some(bits.trailing_zeros())
}

/// The probability of taking a growing alternative with `spare` budget
/// beyond the rule's minimum.
fn grow_probability(spare: u64) -> f64 {
    let spare = spare as f64;
    spare / (spare + GROW_SCALE)
}

/// Split `total` into `parts` non-negative shares at uniformly drawn cut
/// points.
impl Generation<'_> {
    /// Draw one of `alts` for a rule reached at `depth` with `budget`
    /// characters to spend: between the alternatives that close the
    /// derivation and the ones that can grow without bound (or, when
    /// every alternative can, between the cheapest and the rest, so that
    /// a rule like `sum: product | sum "+" product` does not recurse
    /// regardless of its budget) with a probability that rises with the
    /// budget to spare beyond the cheapest growing alternative, then
    /// uniformly within the class.
    ///
    /// The classes are kept as bit masks over the alternatives, so that
    /// each is sized and indexed without another pass over them; a rule
    /// with more alternatives than a mask holds is drawn from by
    /// [`Generation::choose_alt_scanning`], which answers the same.
    fn choose_alt<'a>(
        &mut self,
        ntc: &mut NativeTestCase,
        alts: &'a [Alt],
        budget: u64,
        depth: u32,
    ) -> Result<&'a Alt, EngineError> {
        if alts.len() > MASKED_ALTS {
            return self.choose_alt_scanning(ntc, alts, budget, depth);
        }
        let max_depth = self.spec.max_depth;
        let mut feasible = 0u64;
        let mut typical = u64::MAX;
        let mut any_closing = false;
        for (i, alt) in alts.iter().enumerate() {
            if alt.min_size <= budget && depth.saturating_add(alt.min_depth) <= max_depth {
                feasible |= 1 << i;
                typical = typical.min(alt.typical_size);
                any_closing |= !alt.growing();
            }
        }
        hegel_internal_assert!(feasible != 0);
        let mut growing = 0u64;
        let mut cheapest_growing = u64::MAX;
        let mut rest = feasible;
        while rest != 0 {
            let i = rest.trailing_zeros();
            rest &= rest - 1;
            let alt = &alts[i as usize];
            let closes = if any_closing {
                !alt.growing()
            } else {
                alt.typical_size == typical
            };
            if !closes {
                growing |= 1 << i;
                cheapest_growing = cheapest_growing.min(alt.typical_size);
            }
        }
        let grow = if growing == 0 {
            false
        } else {
            let p = grow_probability(budget.saturating_sub(cheapest_growing));
            spanned(ntc, LABEL_BOOLEAN, |ntc| ntc.weighted_precise(p, None))?
        };
        let class = if grow { growing } else { feasible & !growing };
        let index = spanned(ntc, LABEL_INTEGER, |ntc| {
            ntc.draw_integer_uniform(0, u64::from(class.count_ones()) - 1)
        })?;
        let chosen = hegel_internal_unwrap!(
            nth_set_bit(class, index),
            "choose_alt: drew an index past the class"
        );
        Ok(&alts[chosen as usize])
    }

    /// [`Generation::choose_alt`] for any number of alternatives, finding
    /// each class by scanning them.
    fn choose_alt_scanning<'a>(
        &mut self,
        ntc: &mut NativeTestCase,
        alts: &'a [Alt],
        budget: u64,
        depth: u32,
    ) -> Result<&'a Alt, EngineError> {
        let feasible = |alt: &Alt| {
            alt.min_size <= budget && depth.saturating_add(alt.min_depth) <= self.spec.max_depth
        };
        let mut count = 0;
        let mut typical = u64::MAX;
        let mut any_closing = false;
        for alt in alts.iter().filter(|alt| feasible(alt)) {
            count += 1;
            typical = typical.min(alt.typical_size);
            any_closing |= !alt.growing();
        }
        hegel_internal_assert!(count > 0);
        let closing = |alt: &Alt| {
            if any_closing {
                !alt.growing()
            } else {
                alt.typical_size == typical
            }
        };
        let in_class = |alt: &Alt, grow: bool| feasible(alt) && closing(alt) != grow;
        let growing = alts.iter().filter(|alt| in_class(alt, true));
        let grow = match growing.clone().map(|alt| alt.typical_size).min() {
            None => false,
            Some(cheapest_growing) => {
                let p = grow_probability(budget.saturating_sub(cheapest_growing));
                spanned(ntc, LABEL_BOOLEAN, |ntc| ntc.weighted_precise(p, None))?
            }
        };
        let size = alts.iter().filter(|alt| in_class(alt, grow)).count();
        let index = spanned(ntc, LABEL_INTEGER, |ntc| {
            ntc.draw_integer_uniform(0, size as u64 - 1)
        })?;
        Ok(hegel_internal_unwrap!(
            alts.iter()
                .filter(|alt| in_class(alt, grow))
                .nth(index as usize),
            "choose_alt: drew an index past the class"
        ))
    }

    fn rule(
        &mut self,
        ntc: &mut NativeTestCase,
        id: RuleId,
        budget: u64,
        depth: u32,
    ) -> Result<(), Step> {
        ntc.start_span(self.spec.labels[id]);
        let result = self.rule_body(ntc, id, budget, depth);
        ntc.stop_span(false);
        result
    }

    fn rule_body(
        &mut self,
        ntc: &mut NativeTestCase,
        id: RuleId,
        budget: u64,
        depth: u32,
    ) -> Result<(), Step> {
        let rule = &self.spec.analysis.rules[id];
        let alt = self.choose_alt(ntc, &rule.alternatives, budget, depth + 1)?;
        self.sequence(ntc, alt, budget, depth + 1)
    }

    fn sequence(
        &mut self,
        ntc: &mut NativeTestCase,
        alt: &Alt,
        budget: u64,
        depth: u32,
    ) -> Result<(), Step> {
        let base = self.budgets.len();
        let result = self.sequence_from(ntc, alt, budget, depth, base);
        self.budgets.truncate(base);
        result
    }

    /// Generate `alt`'s items with budgets held in `budgets` from `base`
    /// on: each item's typical size, plus for the items whose size is
    /// not fixed a share of what is left of `budget`, cut at uniformly
    /// drawn points so that no position is systematically larger than
    /// another; what an item with a maximum size cannot take is shared
    /// equally between the items without one.
    fn sequence_from(
        &mut self,
        ntc: &mut NativeTestCase,
        alt: &Alt,
        budget: u64,
        depth: u32,
        base: usize,
    ) -> Result<(), Step> {
        let flexible = |item: &Node| item.max_size != Some(item.min_size);
        self.budgets
            .extend(alt.items.iter().map(|item| item.typical_size));
        let spare = budget.saturating_sub(alt.typical_size);
        let parts = alt.items.iter().filter(|item| flexible(item)).count();
        if parts > 0 && spare > 0 {
            let cuts = self.budgets.len();
            self.budgets.push(0);
            for _ in 1..parts {
                let cut = spanned(ntc, LABEL_INTEGER, |ntc| ntc.draw_integer_uniform(0, spare))?;
                self.budgets.push(cut);
            }
            self.budgets.push(spare);
            self.budgets[cuts..].sort_unstable();
            let mut leftover = 0u64;
            let mut growing = 0u64;
            let mut cut = cuts;
            for (i, item) in alt.items.iter().enumerate() {
                if !flexible(item) {
                    continue;
                }
                let share = self.budgets[cut + 1] - self.budgets[cut];
                cut += 1;
                match item.max_size {
                    Some(max) => {
                        let capacity = max.saturating_sub(item.typical_size);
                        self.budgets[base + i] += share.min(capacity);
                        leftover += share.saturating_sub(capacity);
                    }
                    None => {
                        self.budgets[base + i] += share;
                        growing += 1;
                    }
                }
            }
            self.budgets.truncate(cuts);
            if let Some(each) = leftover.checked_div(growing) {
                for (i, item) in alt.items.iter().enumerate() {
                    if item.max_size.is_none() {
                        self.budgets[base + i] += each;
                    }
                }
            }
        }
        for (i, item) in alt.items.iter().enumerate() {
            let item_budget = self.budgets[base + i];
            self.item(ntc, item, item_budget, depth)?;
        }
        Ok(())
    }

    fn item(
        &mut self,
        ntc: &mut NativeTestCase,
        node: &Node,
        budget: u64,
        depth: u32,
    ) -> Result<(), Step> {
        match &node.kind {
            NodeKind::Terminal(t) => self.terminal(ntc, *t),
            NodeKind::Rule(r) => self.rule(ntc, *r, budget, depth),
            NodeKind::Group(alts) => {
                let alt = self.choose_alt(ntc, alts, budget, depth)?;
                self.sequence(ntc, alt, budget, depth)
            }
            NodeKind::Repeat { inner, min, max } => {
                self.repeat(ntc, inner, *min, *max, budget, depth)
            }
        }
    }

    fn repeat(
        &mut self,
        ntc: &mut NativeTestCase,
        inner: &Node,
        min: u32,
        max: Option<u32>,
        budget: u64,
        depth: u32,
    ) -> Result<(), Step> {
        let min = u64::from(min);
        let cost = inner.typical_size;
        let spare = budget.saturating_sub(min.saturating_mul(cost));
        let fits_depth = depth.saturating_add(inner.min_depth) <= self.spec.max_depth;
        let feasible_max = if !fits_depth || inner.max_size == Some(0) {
            min
        } else {
            let extra = spare.checked_div(cost).unwrap_or(spare);
            let cap = min.saturating_add(extra);
            max.map_or(cap, |max| cap.min(u64::from(max)))
        };
        let count = if feasible_max > min {
            self.draw_count(ntc, min, feasible_max, spare)?
        } else {
            min
        };
        let each = budget
            .saturating_sub(count.saturating_mul(cost))
            .checked_div(count)
            .map_or(0, |share| cost + share);
        let mut state = ManyState::new(
            usize::try_from(min).unwrap_or(usize::MAX),
            Some(usize::try_from(count).unwrap_or(usize::MAX)),
        );
        state.p_continue =
            1.0 - 1.0 / (COUNT_PERSISTENCE * (count - min) as f64).max(MIN_PERSISTENCE);
        while many_more(ntc, &mut state)? {
            let before = self.tokens.len();
            ntc.start_span(LABEL_ELEMENT);
            let result = self.item(ntc, inner, each, depth);
            ntc.stop_span(false);
            result?;
            if min == 0 {
                self.mark_droppable(before);
            }
        }
        Ok(())
    }

    /// Mark the token an element of an optional repetition produced as
    /// one the grammar can do without, when it produced exactly one.
    fn mark_droppable(&mut self, from: usize) {
        let ignore = &self.spec.ignore;
        let mut significant = self.tokens[from..]
            .iter_mut()
            .filter(|t| !ignore.contains(&t.id));
        if let (Some(token), None) = (significant.next(), significant.next()) {
            token.droppable = true;
        }
    }

    /// Draw how many elements a repetition gets, between `min` and
    /// `feasible_max`: whether to go beyond the minimum at all is decided
    /// like a growing alternative, from the spare budget, and the number
    /// of extra elements is then log-uniform, so a large budget is as
    /// likely to go into a few long elements as into many short ones.
    fn draw_count(
        &mut self,
        ntc: &mut NativeTestCase,
        min: u64,
        feasible_max: u64,
        spare: u64,
    ) -> Result<u64, Step> {
        let p = grow_probability(spare);
        let extend = spanned(ntc, LABEL_BOOLEAN, |ntc| ntc.weighted_precise(p, None))?;
        if !extend {
            return Ok(min);
        }
        let extra_max = feasible_max - min;
        let scale = ((extra_max + 1) as f64).ln();
        let u = spanned(ntc, LABEL_INTEGER, |ntc| {
            ntc.draw_integer_uniform(0, COUNT_RESOLUTION)
        })?;
        let extra = (u as f64 / COUNT_RESOLUTION as f64 * scale).exp() as u64;
        Ok(min + extra.clamp(1, extra_max))
    }

    /// Draw a string for terminal `id`: from its pattern, cut to what the
    /// pattern reads back from the front and redrawn a few times while
    /// the basic lexer would not read it back as that terminal, or from
    /// its explicit generator as it comes.
    fn draw_terminal(&mut self, ntc: &mut NativeTestCase, id: TermId) -> Result<Token, Step> {
        let start = self.text.len();
        match &self.spec.terminals[id] {
            TerminalGen::Explicit(spec) => {
                generate_string_into(ntc, spec, &mut self.text)?;
                Ok(self.token(id, start, None))
            }
            TerminalGen::Pattern(spec) => {
                let mut attempt = 0;
                loop {
                    ntc.start_span(LABEL_TERMINAL);
                    generate_string_into(ntc, spec, &mut self.text)?;
                    attempt += 1;
                    if !self.fits(self.text.len()) {
                        self.text.truncate(start);
                        ntc.stop_span(true);
                        if attempt > LEX_RETRIES {
                            return Err(Step::TooLong);
                        }
                        continue;
                    }
                    let (end, own) = self.spec.whole(id, &self.text[start..]);
                    let own = (start + end == self.text.len()).then_some(own);
                    self.text.truncate(start + end);
                    let token = self.token(id, start, own);
                    let accept = token.lexable() || attempt > LEX_RETRIES;
                    ntc.stop_span(!accept);
                    if accept {
                        return Ok(token);
                    }
                    self.text.truncate(start);
                }
            }
        }
    }

    /// Draw a token of terminal `id` onto the end of the string, with
    /// any ignorables that go before it, and fail when the string has
    /// outgrown the size cap.
    fn terminal(&mut self, ntc: &mut NativeTestCase, id: TermId) -> Result<(), Step> {
        let token = self.draw_terminal(ntc, id)?;
        self.tokens.push(token);
        self.maybe_ignore(ntc, 1)?;
        if !self.fits(self.text.len()) {
            return Err(Step::TooLong);
        }
        Ok(())
    }

    /// Insert ignorable tokens before the last `pending` tokens of the
    /// string (the token just drawn, or none at its end), reporting
    /// whether any were. An ignorable the lexer would run into the token
    /// after it (a comment, say, which only a newline ends) is followed
    /// by another ignorable drawn to keep the two apart, up to
    /// [`IGNORE_CHAIN`] deep; a chain that still changes the token
    /// sequence is redrawn, without the terminal it started from when
    /// that read fine on its own, until the attempts run out. An
    /// ignorable the lexer merely gives another type is kept, since that
    /// is a property of the grammar no redraw will change.
    fn insert_ignore(&mut self, ntc: &mut NativeTestCase, pending: usize) -> Result<bool, Step> {
        let from = (self.tokens.len() - pending).saturating_sub(1);
        self.candidates.clear();
        self.candidates.extend_from_slice(&self.spec.ignore);
        let mut oversize = false;
        for _ in 0..IGNORE_ATTEMPTS {
            ntc.start_span(LABEL_IGNORE);
            let mut draws = IGNORE_CHAIN_DRAWS;
            let index = draw_index(ntc, self.candidates.len())?;
            let id = self.candidates[index];
            let placement = self.place_ignore(ntc, id, from, pending, 0, &mut draws);
            let accept = !matches!(placement, Ok(Placement::Rejected { .. }));
            ntc.stop_span(!accept);
            match placement? {
                Placement::Placed(_) => return Ok(true),
                Placement::Rejected { oversize: true, .. } => oversize = true,
                Placement::Rejected { id, lexable, .. } => {
                    if lexable && self.candidates.len() > 1 {
                        self.candidates.retain(|&candidate| candidate != id);
                    }
                }
            }
        }
        if oversize {
            return Err(Step::TooLong);
        }
        Ok(false)
    }

    /// Draw ignorables after the ones inserted since `from` until the
    /// lexer reads the chain and the pending token back as intended, or
    /// `draws` run out; nothing stays inserted when they do. Only
    /// ignorables that could end the last one's match where it is are
    /// drawn (see [`Lexer::can_stop`]): a comment that runs to the end
    /// of its line is followed by something that can hold a newline,
    /// never by spaces it would swallow. An ignorable that could but, as
    /// drawn, keeps failing to is given up on for the rest of the string
    /// (see [`Generation::hopeful`]), since what the analysis cannot
    /// rule out the draws may: a comment can begin with the newline that
    /// ends the one before it, but rarely does.
    fn ignore_chain(
        &mut self,
        ntc: &mut NativeTestCase,
        from: usize,
        pending: usize,
        depth: u32,
        draws: &mut u32,
    ) -> Result<Fit, Step> {
        while *draws > 0 {
            let fitting = |this: &Self, id: TermId| {
                this.before(pending).is_none_or(|prev| {
                    this.spec.lexer.can_stop(&prev.scans.own, id) && this.hopeful(prev.id, id)
                })
            };
            let candidates = || {
                self.spec
                    .ignore
                    .iter()
                    .copied()
                    .filter(|&id| fitting(self, id))
            };
            let count = candidates().count();
            if count == 0 {
                break;
            }
            *draws -= 1;
            ntc.start_span(LABEL_IGNORE);
            let index = draw_index(ntc, count)?;
            let id = nth_candidate(candidates(), index)?;
            let placement = self.place_ignore(ntc, id, from, pending, depth, draws);
            let accept = !matches!(placement, Ok(Placement::Rejected { .. }));
            ntc.stop_span(!accept);
            if let Placement::Placed(fit) = placement? {
                return Ok(fit);
            }
        }
        Ok(Fit::Wrong)
    }

    /// Whether drawing the ignorable `id` to end a token of `prev` in a
    /// chain is still worth it: it is until it has failed
    /// [`IGNORE_CHAIN_PATIENCE`] times for each time it has succeeded,
    /// and once more.
    fn hopeful(&self, prev: TermId, id: TermId) -> bool {
        self.chains
            .get(&(prev, id))
            .is_none_or(|&(failed, succeeded)| failed < IGNORE_CHAIN_PATIENCE * (succeeded + 1))
    }

    /// Draw one ignorable before the last `pending` tokens and, when the
    /// lexer would run it into the token after it but not into the
    /// tokens before it, a chain of further ignorables after it; the
    /// whole chain from the token at `from` on is what has to lex. The
    /// tokens stay in the string when the result fits. A draw in a chain
    /// (at a `depth` above zero) counts against drawing that ignorable
    /// after that token again until it is placed, and then for it.
    fn place_ignore(
        &mut self,
        ntc: &mut NativeTestCase,
        id: TermId,
        from: usize,
        pending: usize,
        depth: u32,
        draws: &mut u32,
    ) -> Result<Placement, Step> {
        let prev = self.before(pending).map(|prev| prev.id);
        let fit = self.draw_ignore(ntc, id, from, pending)?;
        let pair = prev.filter(|_| depth > 0).map(|prev| (prev, id));
        if let Some(pair) = pair {
            self.chains.entry(pair).or_default().0 += 1;
        }
        let placement = self.place_drawn(ntc, id, fit, from, pending, depth, draws)?;
        if let (Some(pair), Placement::Placed(_)) = (pair, &placement) {
            let (failed, succeeded) = self.chains.entry(pair).or_default();
            *failed -= 1;
            *succeeded += 1;
        }
        Ok(placement)
    }

    /// Keep or remove the ignorable of terminal `id` just inserted
    /// before the last `pending` tokens, which the lexer reads back this
    /// well with its neighbours, or `None` when it was too long for the
    /// string and never inserted.
    fn place_drawn(
        &mut self,
        ntc: &mut NativeTestCase,
        id: TermId,
        fit: Option<Fit>,
        from: usize,
        pending: usize,
        depth: u32,
        draws: &mut u32,
    ) -> Result<Placement, Step> {
        let Some(fit) = fit else {
            return Ok(Placement::Rejected {
                id,
                lexable: false,
                oversize: true,
            });
        };
        if fit > Fit::Wrong {
            self.unsettled |= fit < Fit::Settled;
            return Ok(Placement::Placed(fit));
        }
        if depth < IGNORE_CHAIN && *draws > 0 {
            let (text, tokens) = self.window(from, pending);
            if self.spec.fit(text, tokens) > Fit::Wrong {
                let fit = self.ignore_chain(ntc, from, pending, depth + 1, draws)?;
                if fit > Fit::Wrong {
                    return Ok(Placement::Placed(fit));
                }
            }
        }
        let token = self.remove(pending);
        Ok(Placement::Rejected {
            id: token.id,
            lexable: token.lexable(),
            oversize: false,
        })
    }

    /// Draw an ignorable of terminal `id` into the string before the
    /// last `pending` tokens: how the lexer reads the tokens from `from`
    /// on with it in place, or `None` when it alone would take the
    /// string past the size cap and is left out.
    fn draw_ignore(
        &mut self,
        ntc: &mut NativeTestCase,
        id: TermId,
        from: usize,
        pending: usize,
    ) -> Result<Option<Fit>, Step> {
        let at = self.tokens.len() - pending;
        let start = self
            .tokens
            .get(at)
            .map_or(self.text.len(), |next| next.start);
        let drawn = self.text.len();
        match &self.spec.terminals[id] {
            TerminalGen::Explicit(spec) => generate_string_into(ntc, spec, &mut self.text)?,
            TerminalGen::Pattern(spec) => generate_string_into(ntc, spec, &mut self.text)?,
        }
        let len = self.text.len() - drawn;
        if !self.fits(start + len) {
            self.text.truncate(drawn);
            return Ok(None);
        }
        let scans = self.spec.read_token(id, &self.text[drawn..], None);
        self.text[start..].rotate_left(drawn - start);
        for later in &mut self.tokens[at..] {
            later.start += len;
            later.end += len;
        }
        let token = Token {
            id,
            start,
            end: start + len,
            droppable: false,
            scans,
        };
        self.tokens.insert(at, token);
        let (text, tokens) = self.window(from, 0);
        Ok(Some(self.spec.fit(text, tokens)))
    }

    /// Take the token before the last `pending` out of the string.
    fn remove(&mut self, pending: usize) -> Token {
        let at = self.tokens.len() - pending - 1;
        let token = self.tokens.remove(at);
        self.text.drain(token.start..token.end);
        for later in &mut self.tokens[at..] {
            later.start -= token.len();
            later.end -= token.len();
        }
        token
    }

    /// Decide whether an ignorable goes before the last `pending` tokens
    /// (the token just drawn, or none at the end of the string): always
    /// when a lexer would otherwise read that token and the one before
    /// it differently, otherwise at the string's ignore rate. A
    /// separator the basic lexer needs but none of the ignorables can
    /// provide is left out; one the dynamic lexer needs too makes the
    /// test case invalid, since the string would not be in the grammar.
    /// A grammar without ignorables has nothing to insert, and leaves
    /// what the pair checks cannot settle to the lexing of the whole
    /// string.
    fn maybe_ignore(&mut self, ntc: &mut NativeTestCase, pending: usize) -> Result<(), Step> {
        let at = self.tokens.len() - pending;
        let adjacent = at.checked_sub(1).filter(|_| pending > 0);
        if self.spec.ignore.is_empty() {
            if let Some(from) = adjacent {
                let (text, tokens) = self.window(from, 0);
                self.unsettled |= !self.spec.settled(text, tokens);
            }
            return Ok(());
        }
        let pair = adjacent.map_or(Fit::Settled, |from| {
            let (text, tokens) = self.window(from, 0);
            self.spec.fit(text, tokens)
        });
        let forced = if pair < Fit::Exact { Some(true) } else { None };
        if ntc.weighted(self.ignore_rate, forced)? && self.insert_ignore(ntc, pending)? {
            return Ok(());
        }
        if pair == Fit::Wrong {
            ntc.conclude(Status::Invalid, None);
            return Err(Step::Engine(EngineError::InvalidTestCase));
        }
        self.unsettled |= pair < Fit::Settled;
        Ok(())
    }

    fn run(&mut self, ntc: &mut NativeTestCase, budget: u64) -> Result<(), Step> {
        self.rule(ntc, self.spec.start, budget, 0)?;
        self.maybe_ignore(ntc, 0)?;
        if !self.unsettled || self.spec.scans(&self.text, &self.tokens) {
            Ok(())
        } else {
            Err(Step::Misread)
        }
    }
}

/// Draw a string from `spec`, in a span of its own.
pub fn generate_grammar(
    ntc: &mut NativeTestCase,
    spec: &GrammarSpec,
) -> Result<String, EngineError> {
    spanned(ntc, LABEL_GRAMMAR, |ntc| draw_grammar(ntc, spec))
}

fn draw_grammar(ntc: &mut NativeTestCase, spec: &GrammarSpec) -> Result<String, EngineError> {
    let min = spec.min_size();
    let target = if spec.max_size > min
        && spanned(ntc, LABEL_BOOLEAN, |ntc| {
            ntc.weighted_precise(LARGE_TARGET_PROBABILITY, None)
        })? {
        spanned(ntc, LABEL_INTEGER, |ntc| {
            ntc.draw_integer_uniform(min, spec.max_size)
        })?
    } else {
        min
    };
    let ignore_rate = if spec.ignore.is_empty() {
        0.0
    } else {
        let step = spanned(ntc, LABEL_INTEGER, |ntc| {
            ntc.draw_integer_uniform(0, IGNORE_RATE_STEPS - 1)
        })?;
        let fraction = step as f64 / (IGNORE_RATE_STEPS - 1) as f64;
        fraction * fraction
    };
    let mut generation = Generation::new(spec, ignore_rate);
    let result = generation.attempts(ntc, target, min);
    generation.recycle();
    result
}

impl Generation<'_> {
    /// Draw strings of about `target` characters (never below `min`, the
    /// shortest the grammar allows) until one is kept, halving the target
    /// after each one that is too long.
    fn attempts(
        &mut self,
        ntc: &mut NativeTestCase,
        mut target: u64,
        min: u64,
    ) -> Result<String, EngineError> {
        let mut attempt = 0;
        loop {
            let budget = token_budget(target, min, self.ignore_rate, self.spec.ignore_cost);
            match self.run(ntc, budget) {
                Ok(()) => return Ok(self.text.iter().collect()),
                Err(Step::Engine(e)) => return Err(e),
                Err(step) => {
                    attempt += 1;
                    if attempt >= MAX_ATTEMPTS {
                        ntc.conclude(Status::Invalid, None);
                        return Err(EngineError::InvalidTestCase);
                    }
                    if matches!(step, Step::TooLong) {
                        target = min.max(target / 2);
                    }
                    self.reset();
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/generate_tests.rs"]
mod tests;
