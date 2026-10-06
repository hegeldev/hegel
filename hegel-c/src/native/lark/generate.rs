//! Generating strings from an analysed grammar.
//!
//! Every draw first picks a target length: with probability 3/4 a length
//! uniform between the smallest string the start rule can produce and
//! the size cap, otherwise the smallest. The grammar is then walked top
//! down with that target as a character budget. A rule's alternatives
//! are split into those that can grow without bound and those that
//! cannot, a weighted boolean whose probability rises with the budget
//! left over after the rule's minimum chooses between the two classes,
//! and the alternative is drawn uniformly from the class; a sequence
//! splits its spare budget between its flexible items at uniformly drawn
//! cut points, so no position is systematically larger than another; a
//! repetition draws a count that fits its budget and shares the budget
//! equally between the elements, pulling them with the collection
//! protocol so the shrinker can delete them one at a time. Budgets steer
//! the structure only: each terminal is drawn by its own regex generator,
//! and a string that overruns the size cap regardless is discarded and
//! regenerated with half the target, up to [`MAX_ATTEMPTS`] times.
//!
//! Between adjacent tokens the generator inserts a `%ignore` terminal
//! either when the simulated lexer says the two would otherwise lex
//! differently, or at random with a rate drawn once per string. Each
//! terminal string is checked to lex back as itself, and each inserted
//! ignorable to be skipped between its neighbours, with a few retries
//! before the string is accepted as it is.

use super::analysis::{Alt, Analysis, Node, NodeKind, analyze};
use super::builder::{ImportSource, load_grammar};
use super::compile::{Grammar, RuleId, TermId, compile};
use super::lexsim::{Lexed, Lexer};
use super::{GrammarError, GrammarResult};
use crate::control::hegel_internal_assert;
use crate::native::core::{EngineError, ManyState, NativeTestCase, Status};
use crate::native::draws::{
    LABEL_BOOLEAN, LABEL_INTEGER, StringSpec, generate_string, many_more, spanned,
};
use crate::native::labels::label_from_name;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The default cap on the length of a generated string, in characters.
pub const DEFAULT_MAX_SIZE: u64 = 8192;

/// The default cap on how deeply rules nest.
pub const DEFAULT_MAX_DEPTH: u32 = 200;

/// The number of oversize strings one draw discards (halving its target
/// each time) before the test case is rejected.
const MAX_ATTEMPTS: u32 = 9;

/// How often a terminal is redrawn when the lexer would not read it back
/// as itself, and an ignorable when the lexer would not skip it between
/// its neighbours, before the draw is accepted anyway.
const LEX_RETRIES: u32 = 3;

/// How many strings a terminal may draw that its own pattern does not
/// read back whole before the test case is rejected.
const TERMINAL_ATTEMPTS: u32 = 20;

/// How many ignorables an insertion tries before giving up (dropping
/// an optional one, or keeping the last when the lexer needs one),
/// how many may follow one another to keep an ignorable the lexer would
/// otherwise run into the next token (a comment, which only a newline
/// ends) apart from it, and how many strings one such chain may draw.
const IGNORE_ATTEMPTS: u32 = 4;
const IGNORE_CHAIN: u32 = 2;
const IGNORE_CHAIN_DRAWS: u32 = 4;

/// The probability that a draw aims for a large string rather than the
/// smallest one.
const LARGE_TARGET_PROBABILITY: f64 = 0.75;

/// The spare budget at which a rule is as likely to pick a growing
/// alternative as a closing one: the probability of growing is
/// `spare / (spare + GROW_SCALE)`.
const GROW_SCALE: f64 = 8.0;

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

const LABEL_TERMINAL: u64 = label_from_name("hegel.lark.terminal");
const LABEL_IGNORE: u64 = label_from_name("hegel.lark.ignore");
const LABEL_ELEMENT: u64 = label_from_name("hegel.lark.element");

/// How the strings of one terminal are drawn.
enum TerminalGen {
    /// From the terminal's own pattern, checked against the lexer.
    Pattern(StringSpec),
    /// From a generator supplied for a `%declare`d (or overridden)
    /// terminal, taken as it comes.
    Explicit(StringSpec),
}

/// Options for building a [`GrammarSpec`].
pub struct GrammarOptions {
    pub start: String,
    pub explicit: Vec<(String, StringSpec)>,
    pub alphabet: Option<StringSpec>,
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
    terminal_names: Vec<String>,
    ignore: Vec<TermId>,
    lexer: Lexer,
    start: RuleId,
    labels: Vec<u64>,
    max_size: u64,
    max_depth: u32,
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
        for term in &grammar.terminals {
            let explicit = options
                .explicit
                .iter()
                .position(|(name, _)| *name == term.name)
                .map(|i| options.explicit.swap_remove(i).1);
            match (explicit, &term.regexp) {
                (Some(spec), _) => {
                    terminals.push(TerminalGen::Explicit(spec));
                    term_min.push(1);
                }
                (None, Some(regexp)) => {
                    let spec = StringSpec::regex(regexp, true, options.alphabet.as_ref()).map_err(
                        |e| {
                            GrammarError::new(format!(
                                "Cannot generate terminal {} from {regexp}: {e}",
                                term.name
                            ))
                        },
                    )?;
                    terminals.push(TerminalGen::Pattern(spec));
                    term_min.push(term.min_width);
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
        let analysis = analyze(&grammar, term_min)?;
        let lexer = Lexer::new(&grammar.terminals)?;
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
        Ok(GrammarSpec {
            analysis,
            terminals,
            terminal_names: grammar.terminals.iter().map(|t| t.name.clone()).collect(),
            ignore: grammar.ignore,
            lexer,
            start,
            labels,
            max_size: options.max_size,
            max_depth: options.max_depth,
        })
    }

    /// The smallest string the grammar can produce, in characters.
    pub fn min_size(&self) -> u64 {
        self.analysis.rules[self.start].min_size
    }

    /// The names of the terminals, indexed by [`TermId`].
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

    /// The tokens the lexer reads `text` as, with their extents.
    fn lex_all(&self, text: &[char]) -> Result<Vec<(TermId, usize, usize)>, Lexed> {
        let mut observed = Vec::new();
        let mut pos = 0;
        while pos < text.len() {
            match self.lexer.next_token(text, pos) {
                Lexed::Token(id, end) => {
                    observed.push((id, pos, end));
                    pos = end;
                }
                other => return Err(other),
            }
        }
        Ok(observed)
    }

    /// Whether terminal `id`'s pattern, matched at the start of `text`,
    /// takes all of it: a text it matches in full but stops short of
    /// when read from the front (a lazy repeat, or an earlier branch of
    /// an alternation) is one no lexer reads back whole.
    fn complete(&self, id: TermId, text: &[char]) -> bool {
        match self.lexer.match_terminal(id, text, 0) {
            Lexed::Token(_, end) => end == text.len(),
            Lexed::NoMatch => false,
            Lexed::Unknown => true,
        }
    }

    /// How the basic lexer reads the text of terminal `id` on its own.
    fn reading(&self, id: TermId, text: &[char]) -> Reading {
        if !self.is_pattern(id) {
            return Reading::Opaque;
        }
        match self.lex_all(text) {
            Err(_) => Reading::Opaque,
            Ok(got) if got.len() == 1 && self.same_type(id, got[0].0) => Reading::Itself,
            Ok(got) if self.is_ignore(id) && got.iter().all(|&(g, _, _)| self.is_ignore(g)) => {
                Reading::Itself
            }
            Ok(got) => Reading::Tokens(got.into_iter().map(|(g, _, _)| g).collect()),
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
    fn scans(&self, text: &[char], tokens: &[&Token]) -> bool {
        let mut bounds = Vec::with_capacity(tokens.len());
        let mut pos = 0;
        for token in tokens {
            bounds.push((pos, pos + token.text.len()));
            pos += token.text.len();
        }
        let significant: Vec<usize> = (0..tokens.len())
            .filter(|&i| !self.is_ignore(tokens[i].id))
            .collect();
        let kept = |k: usize| {
            significant[k..]
                .iter()
                .find(|&&i| !tokens[i].droppable)
                .copied()
        };
        let within = |got: usize, k: usize| kept(k).is_none_or(|i| got < bounds[i].1);
        let mut reachable = alloc::vec![false; text.len() + 1];
        reachable[0] = true;
        let mut k = 0;
        for p in 0..=text.len() {
            if !reachable[p] {
                continue;
            }
            while k < significant.len() && bounds[significant[k]].1 <= p {
                k += 1;
            }
            if p == text.len() {
                return true;
            }
            for (j, &i) in significant.iter().enumerate().skip(k) {
                match self.lexer.match_terminal(tokens[i].id, text, p) {
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

    /// How well the lexers read the concatenation of `tokens` back:
    /// [`Fit::Wrong`] when the dynamic lexer does not read it as `tokens`
    /// at all (see [`GrammarSpec::scans`]), [`Fit::Exact`] when the basic
    /// lexer reads it as `tokens` too, and [`Fit::Retyped`] in between.
    fn fit(&self, tokens: &[&Token]) -> Fit {
        let text: Vec<char> = tokens.iter().flat_map(|t| t.text.iter().copied()).collect();
        if !self.scans(&text, tokens) {
            return Fit::Wrong;
        }
        self.basic_fit(&text, tokens).max(Fit::Retyped)
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
    fn basic_fit(&self, text: &[char], tokens: &[&Token]) -> Fit {
        if tokens.iter().any(|t| t.reading == Reading::Opaque) {
            return Fit::Exact;
        }
        let observed = match self.lex_all(text) {
            Ok(observed) => observed,
            Err(Lexed::NoMatch) => return Fit::Wrong,
            Err(_) => return Fit::Exact,
        };
        let mut regions = Vec::with_capacity(tokens.len());
        let mut pos = 0;
        for token in tokens {
            let end = pos + token.text.len();
            if !(token.reading == Reading::Itself && self.is_ignore(token.id)) {
                regions.push((pos, end, *token));
            }
            pos = end;
        }
        let overlapping = |start: usize, end: usize| {
            regions
                .iter()
                .filter(move |&&(s, e, _)| start < e && s < end)
                .collect::<Vec<_>>()
        };
        let mut fit = Fit::Exact;
        let mut i = 0;
        while i < observed.len() {
            let (got, start, end) = observed[i];
            let hits = overlapping(start, end);
            let Some(&&(s, e, token)) = hits.first() else {
                if !self.is_ignore(got) {
                    fit = Fit::Retyped;
                }
                i += 1;
                continue;
            };
            if hits.len() > 1 {
                return Fit::Wrong;
            }
            if self.is_ignore(token.id) {
                while i < observed.len() && observed[i].1 < e {
                    if overlapping(observed[i].1, observed[i].2).len() > 1 {
                        return Fit::Wrong;
                    }
                    if !self.is_ignore(observed[i].0) {
                        fit = Fit::Retyped;
                    }
                    i += 1;
                }
                continue;
            }
            let expected: &[TermId] = match &token.reading {
                Reading::Itself => core::slice::from_ref(&token.id),
                Reading::Tokens(expected) => expected,
                Reading::Opaque => return Fit::Exact,
            };
            let mut j = i + 1;
            while j < observed.len() && observed[j].1 < e {
                if overlapping(observed[j].1, observed[j].2).len() > 1 {
                    return Fit::Wrong;
                }
                j += 1;
            }
            let group = &observed[i..j];
            let last_end = group[group.len() - 1].2;
            if group.len() != expected.len() || start > s || last_end < e {
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
    /// The tokens were pushed and the lexer reads them this well.
    Placed(Fit),
    /// Nothing was pushed; the ignorable drawn first is handed back.
    Rejected(Token),
}

/// How the lexer reads one token's text on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Reading {
    /// As a single token of its own type (or, for an ignorable, as
    /// nothing but ignorable tokens).
    Itself,
    /// As this sequence of tokens.
    Tokens(Vec<TermId>),
    /// Not at all: the lexer does not know the terminal, cannot read the
    /// text, or ran out of budget.
    Opaque,
}

/// How the lexer reads a stretch of tokens, from worst to best: with
/// tokens merged, split or swallowed, with the right extents but other
/// types, or exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Fit {
    Wrong,
    Retyped,
    Exact,
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

struct Token {
    id: TermId,
    text: Vec<char>,
    reading: Reading,
    /// Whether the string stays in the grammar without this token: it
    /// is the only token of an element of a repetition that allows none
    /// (`x?`, `[x]`, `x*`), so a lexer that runs it into its neighbour
    /// or skips it leaves a string the parser still accepts.
    droppable: bool,
}

impl Token {
    /// Whether the basic lexer reads the token back as itself on its own.
    fn lexable(&self) -> bool {
        self.reading == Reading::Itself
    }
}

enum Step {
    Engine(EngineError),
    TooLong,
}

impl From<EngineError> for Step {
    fn from(e: EngineError) -> Self {
        Step::Engine(e)
    }
}

struct Generation<'s> {
    spec: &'s GrammarSpec,
    tokens: Vec<Token>,
    consumed: u64,
    ignore_rate: f64,
}

/// The probability of taking a growing alternative with `spare` budget
/// beyond the rule's minimum.
fn grow_probability(spare: u64) -> f64 {
    let spare = spare as f64;
    spare / (spare + GROW_SCALE)
}

/// Split `total` into `parts` non-negative shares at uniformly drawn cut
/// points.
fn split_budget(
    ntc: &mut NativeTestCase,
    total: u64,
    parts: usize,
) -> Result<Vec<u64>, EngineError> {
    let mut cuts = Vec::with_capacity(parts + 1);
    cuts.push(0);
    for _ in 1..parts {
        cuts.push(spanned(ntc, LABEL_INTEGER, |ntc| {
            ntc.draw_integer_uniform(0, total)
        })?);
    }
    cuts.push(total);
    cuts.sort_unstable();
    Ok(cuts.windows(2).map(|w| w[1] - w[0]).collect())
}

impl Generation<'_> {
    fn choose_alt<'a>(
        &mut self,
        ntc: &mut NativeTestCase,
        alts: &'a [Alt],
        budget: u64,
        depth: u32,
    ) -> Result<&'a Alt, EngineError> {
        let feasible = |alt: &&Alt| {
            alt.min_size <= budget && depth.saturating_add(alt.min_depth) <= self.spec.max_depth
        };
        let closing: Vec<&Alt> = alts
            .iter()
            .filter(feasible)
            .filter(|a| !a.growing())
            .collect();
        let growing: Vec<&Alt> = alts
            .iter()
            .filter(feasible)
            .filter(|a| a.growing())
            .collect();
        hegel_internal_assert!(
            !closing.is_empty() || !growing.is_empty(),
            "no alternative fits a budget of {budget} at depth {depth}"
        );
        let class = if closing.is_empty() {
            growing
        } else if growing.is_empty() {
            closing
        } else {
            let min_size = alts.iter().map(|a| a.min_size).min().unwrap_or(0);
            let p = grow_probability(budget - min_size);
            let grow = spanned(ntc, LABEL_BOOLEAN, |ntc| ntc.weighted_precise(p, None))?;
            if grow { growing } else { closing }
        };
        let index = spanned(ntc, LABEL_INTEGER, |ntc| {
            ntc.draw_integer_uniform(0, class.len() as u64 - 1)
        })?;
        Ok(class[index as usize])
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
        let spare = budget.saturating_sub(alt.min_size);
        let flexible: Vec<usize> = alt
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.max_size != Some(item.min_size))
            .map(|(i, _)| i)
            .collect();
        let mut budgets: Vec<u64> = alt.items.iter().map(|item| item.min_size).collect();
        if !flexible.is_empty() && spare > 0 {
            let shares = split_budget(ntc, spare, flexible.len())?;
            let mut leftover = 0u64;
            let mut growing = 0u64;
            for (&i, share) in flexible.iter().zip(shares) {
                let item = &alt.items[i];
                match item.max_size {
                    Some(max) => {
                        let capacity = max - item.min_size;
                        budgets[i] += share.min(capacity);
                        leftover += share.saturating_sub(capacity);
                    }
                    None => {
                        budgets[i] += share;
                        growing += 1;
                    }
                }
            }
            if let Some(each) = leftover.checked_div(growing) {
                for &i in &flexible {
                    if alt.items[i].max_size.is_none() {
                        budgets[i] += each;
                    }
                }
            }
        }
        for (item, item_budget) in alt.items.iter().zip(budgets) {
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
        let cost = inner.min_size;
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

    /// Draw a string for terminal `id`: from its pattern, redrawn a few
    /// times while the lexer would not read it back as that terminal, or
    /// from its explicit generator as it comes.
    fn draw_terminal(&mut self, ntc: &mut NativeTestCase, id: TermId) -> Result<Token, Step> {
        match &self.spec.terminals[id] {
            TerminalGen::Explicit(spec) => Ok(Token {
                id,
                text: generate_string(ntc, spec)?.chars().collect(),
                reading: Reading::Opaque,
                droppable: false,
            }),
            TerminalGen::Pattern(spec) => {
                let mut attempt = 0;
                loop {
                    ntc.start_span(LABEL_TERMINAL);
                    let text: Vec<char> = generate_string(ntc, spec)?.chars().collect();
                    let complete = self.spec.complete(id, &text);
                    let reading = self.spec.reading(id, &text);
                    let token = Token {
                        id,
                        text,
                        reading,
                        droppable: false,
                    };
                    attempt += 1;
                    let accept = complete && (token.lexable() || attempt > LEX_RETRIES);
                    ntc.stop_span(!accept);
                    if accept {
                        return Ok(token);
                    }
                    if attempt >= TERMINAL_ATTEMPTS {
                        ntc.conclude(Status::Invalid, None);
                        return Err(Step::Engine(EngineError::InvalidTestCase));
                    }
                }
            }
        }
    }

    fn terminal(&mut self, ntc: &mut NativeTestCase, id: TermId) -> Result<(), Step> {
        let token = self.draw_terminal(ntc, id)?;
        self.emit(ntc, token)
    }

    fn consume(&mut self, len: usize) -> Result<(), Step> {
        self.consumed = self.consumed.saturating_add(len as u64);
        if self.consumed > self.spec.max_size {
            return Err(Step::TooLong);
        }
        Ok(())
    }

    /// How the lexer reads the ignorable `ign` after the tokens emitted
    /// from index `from` on and before `next`.
    fn ignore_fit(&self, from: usize, ign: &Token, next: Option<&Token>) -> Fit {
        let tokens: Vec<&Token> = self.tokens[from..]
            .iter()
            .chain([ign])
            .chain(next)
            .collect();
        self.spec.fit(&tokens)
    }

    /// Insert ignorable tokens between the last token emitted and `next`,
    /// reporting whether any were. An ignorable the lexer would run into
    /// the token after it (a comment, say, which only a newline ends) is
    /// followed by another ignorable drawn to keep the two apart, up to
    /// [`IGNORE_CHAIN`] deep; a chain that still changes the token
    /// sequence is redrawn, without the terminal it started from when
    /// that read fine on its own, until the attempts run out. An
    /// ignorable the lexer merely gives another type is kept, since that
    /// is a property of the grammar no redraw will change.
    fn insert_ignore(
        &mut self,
        ntc: &mut NativeTestCase,
        next: Option<&Token>,
    ) -> Result<bool, Step> {
        let from = self.tokens.len().saturating_sub(1);
        let mut candidates = self.spec.ignore.clone();
        for _ in 0..IGNORE_ATTEMPTS {
            ntc.start_span(LABEL_IGNORE);
            let mut draws = IGNORE_CHAIN_DRAWS;
            let placement = self.place_ignore(ntc, &candidates, from, next, 0, &mut draws);
            let accept = !matches!(placement, Ok(Placement::Rejected(_)));
            ntc.stop_span(!accept);
            match placement? {
                Placement::Placed(_) => return Ok(true),
                Placement::Rejected(token) => {
                    if token.reading == Reading::Itself && candidates.len() > 1 {
                        candidates.retain(|&id| id != token.id);
                    }
                }
            }
        }
        Ok(false)
    }

    /// Draw ignorables after the ones pushed since `from` until the
    /// lexer reads the chain and `next` back as intended, or `draws`
    /// run out; nothing stays pushed when they do.
    fn ignore_chain(
        &mut self,
        ntc: &mut NativeTestCase,
        from: usize,
        next: Option<&Token>,
        depth: u32,
        draws: &mut u32,
    ) -> Result<Fit, Step> {
        let candidates = self.spec.ignore.clone();
        while *draws > 0 {
            *draws -= 1;
            ntc.start_span(LABEL_IGNORE);
            let placement = self.place_ignore(ntc, &candidates, from, next, depth, draws);
            let accept = !matches!(placement, Ok(Placement::Rejected(_)));
            ntc.stop_span(!accept);
            if let Placement::Placed(fit) = placement? {
                return Ok(fit);
            }
        }
        Ok(Fit::Wrong)
    }

    /// Draw one ignorable before `next` and, when the lexer would run it
    /// into `next` but not into the tokens before it, a chain of further
    /// ignorables after it; the whole chain from the token at `from` on is
    /// what has to lex. The tokens are pushed when the result fits.
    fn place_ignore(
        &mut self,
        ntc: &mut NativeTestCase,
        candidates: &[TermId],
        from: usize,
        next: Option<&Token>,
        depth: u32,
        draws: &mut u32,
    ) -> Result<Placement, Step> {
        let (token, fit) = self.draw_ignore(ntc, candidates, from, next)?;
        if fit > Fit::Wrong {
            self.push(token)?;
            return Ok(Placement::Placed(fit));
        }
        if depth < IGNORE_CHAIN && *draws > 0 && self.ignore_fit(from, &token, None) > Fit::Wrong {
            let (count, consumed) = (self.tokens.len(), self.consumed);
            self.push(token)?;
            let fit = self.ignore_chain(ntc, from, next, depth + 1, draws)?;
            if fit > Fit::Wrong {
                return Ok(Placement::Placed(fit));
            }
            self.consumed = consumed;
            let token = self.tokens.drain(count..).next().unwrap();
            return Ok(Placement::Rejected(token));
        }
        Ok(Placement::Rejected(token))
    }

    fn draw_ignore(
        &mut self,
        ntc: &mut NativeTestCase,
        candidates: &[TermId],
        from: usize,
        next: Option<&Token>,
    ) -> Result<(Token, Fit), Step> {
        let index = spanned(ntc, LABEL_INTEGER, |ntc| {
            ntc.draw_integer_uniform(0, candidates.len() as u64 - 1)
        })?;
        let id = candidates[index as usize];
        let text = match &self.spec.terminals[id] {
            TerminalGen::Explicit(spec) | TerminalGen::Pattern(spec) => generate_string(ntc, spec)?,
        };
        let text: Vec<char> = text.chars().collect();
        let reading = self.spec.reading(id, &text);
        let token = Token {
            id,
            text,
            reading,
            droppable: false,
        };
        let fit = self.ignore_fit(from, &token, next);
        Ok((token, fit))
    }

    /// Decide whether an ignorable goes before `next` (or at the end of the
    /// string when `next` is `None`): always when a lexer would otherwise
    /// read `next` and the previous token differently, otherwise at the
    /// string's ignore rate. A separator the basic lexer needs but none
    /// of the ignorables can provide is left out; one the dynamic lexer
    /// needs too makes the test case invalid, since the string would not
    /// be in the grammar.
    fn maybe_ignore(&mut self, ntc: &mut NativeTestCase, next: Option<&Token>) -> Result<(), Step> {
        if self.spec.ignore.is_empty() {
            return Ok(());
        }
        let pair = match (self.tokens.last(), next) {
            (Some(prev), Some(next)) => self.spec.fit(&[prev, next]),
            _ => Fit::Exact,
        };
        let forced = if pair < Fit::Exact { Some(true) } else { None };
        if ntc.weighted(self.ignore_rate, forced)?
            && !self.insert_ignore(ntc, next)?
            && pair == Fit::Wrong
        {
            ntc.conclude(Status::Invalid, None);
            return Err(Step::Engine(EngineError::InvalidTestCase));
        }
        Ok(())
    }

    fn emit(&mut self, ntc: &mut NativeTestCase, token: Token) -> Result<(), Step> {
        self.maybe_ignore(ntc, Some(&token))?;
        self.push(token)
    }

    fn push(&mut self, token: Token) -> Result<(), Step> {
        self.consume(token.text.len())?;
        self.tokens.push(token);
        Ok(())
    }

    fn run(&mut self, ntc: &mut NativeTestCase, budget: u64) -> Result<(), Step> {
        self.rule(ntc, self.spec.start, budget, 0)?;
        self.maybe_ignore(ntc, None)
    }
}

/// Draw a string from `spec`.
pub fn generate_grammar(
    ntc: &mut NativeTestCase,
    spec: &GrammarSpec,
) -> Result<String, EngineError> {
    let base_span_depth = ntc.span_depth();
    let min = spec.min_size();
    let mut target = if spec.max_size > min
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
    let mut attempt = 0;
    loop {
        let mut generation = Generation {
            spec,
            tokens: Vec::new(),
            consumed: 0,
            ignore_rate,
        };
        match generation.run(ntc, target) {
            Ok(()) => {
                let mut out = String::new();
                for token in &generation.tokens {
                    out.extend(token.text.iter());
                }
                return Ok(out);
            }
            Err(Step::Engine(e)) => return Err(e),
            Err(Step::TooLong) => {
                while ntc.span_depth() > base_span_depth {
                    ntc.stop_span(true);
                }
                attempt += 1;
                if attempt >= MAX_ATTEMPTS {
                    ntc.conclude(Status::Invalid, None);
                    return Err(EngineError::InvalidTestCase);
                }
                target = min.max(target / 2);
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/generate_tests.rs"]
mod tests;
