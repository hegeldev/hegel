//! Matching the terminals a grammar's user supplies a generator for (the
//! `%declare`d ones, and any overridden) when a string is parsed back
//! (see [`super::parse`]). Such a terminal has no pattern of its own, so
//! what it matches is what its generator could have drawn. A pattern
//! terminal matches at a position as CPython's `pattern.match` does, one
//! end; an explicit terminal matches every prefix its generator could
//! produce, since nothing ties the generator to one length: a text
//! generator any run of its alphabet within its lengths, a regex
//! generator every prefix the pattern matches whole (or, when it
//! searches, every prefix containing a match), and the email, URL and
//! domain generators every prefix matching a pattern that admits all
//! they draw, and a little more, since what they draw is not regular.

use super::compile::{Grammar, TermId};
use super::lexsim::Lexer;
use super::parse::Terminals;
use crate::control::{InternalError, hegel_internal_unwrap};
use crate::native::draws::StringSpec;
use crate::native::intervalsets::IntervalSet;
use crate::native::re::matcher::Matcher;
use crate::native::re::parser::{ParsedPattern, parse_pattern};
use crate::native::re::vm::Program;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// A pattern admitting every domain name the domain generator draws:
/// labels of a letter and up to 62 more letters, digits and hyphens,
/// then a top-level domain of the same shape, at least two characters.
const DOMAIN: &str = r"(?:[A-Za-z][A-Za-z0-9-]{0,62}\.)+[A-Za-z][A-Za-z0-9-]{1,62}";

/// A pattern admitting every address the email generator draws: up to
/// 64 characters of the RFC 5322 atext set, `@`, and a domain.
const EMAIL: &str =
    r"[!#-'*+\-/-9=A-Z^-`a-z{-~]{1,64}@(?:[A-Za-z][A-Za-z0-9-]{0,62}\.)+[A-Za-z][A-Za-z0-9-]{1,62}";

/// A pattern admitting every URL the URL generator draws: a scheme, a
/// domain, an optional port, a path of URL-safe and percent-encoded
/// characters and an optional fragment of the same with `?` and `/`.
const URL: &str = r"https?://(?:[A-Za-z][A-Za-z0-9-]{0,62}\.)+[A-Za-z][A-Za-z0-9-]{1,62}(?::[0-9]{1,5})?/(?:[A-Za-z0-9$\-_.+!*'(),~/]|%[0-9A-F]{2})*(?:#(?:[A-Za-z0-9$\-_.+!*'(),~?/]|%[0-9A-F]{2})*)?";

/// What the strings an explicit terminal's generator draws look like,
/// for the parser to match the terminal with.
pub struct ExplicitMatcher {
    kind: Kind,
}

enum Kind {
    Text {
        intervals: Arc<IntervalSet>,
        min_size: usize,
        max_size: usize,
    },
    Pattern {
        parsed: ParsedPattern,
        /// The pattern compiled for the thread simulation, when it can
        /// express it; the backtracking matcher stands in otherwise.
        program: Option<Program>,
        fullmatch: bool,
    },
}

impl Kind {
    fn pattern(parsed: ParsedPattern, fullmatch: bool) -> Kind {
        let program = Program::compile(&parsed).ok();
        Kind::Pattern {
            parsed,
            program,
            fullmatch,
        }
    }
}

fn approximate(pattern: &str) -> Result<ParsedPattern, InternalError> {
    Ok(hegel_internal_unwrap!(
        parse_pattern(pattern, 0).ok(),
        "the pattern approximating an internet generator does not parse: {pattern}"
    ))
}

impl ExplicitMatcher {
    /// The matcher for a terminal drawn from `spec`.
    pub fn new(spec: &StringSpec) -> Result<ExplicitMatcher, InternalError> {
        let kind = match spec {
            StringSpec::Text {
                intervals,
                min_size,
                max_size,
            } => Kind::Text {
                intervals: Arc::clone(intervals),
                min_size: *min_size,
                max_size: *max_size,
            },
            StringSpec::Regex {
                compiled,
                fullmatch,
                ..
            } => Kind::pattern(compiled.parsed().clone(), *fullmatch),
            StringSpec::Email => Kind::pattern(approximate(EMAIL)?, true),
            StringSpec::Url => Kind::pattern(approximate(URL)?, true),
            StringSpec::Domain(_) => Kind::pattern(approximate(DOMAIN)?, true),
        };
        Ok(ExplicitMatcher { kind })
    }

    /// The positions where a token of the terminal starting at `pos` may
    /// end, in increasing order, appended to `out`; `pos` itself when the
    /// generator can draw the empty string.
    pub fn ends(&self, text: &[char], pos: usize, out: &mut Vec<usize>) {
        match &self.kind {
            Kind::Text {
                intervals,
                min_size,
                max_size,
            } => {
                let run = text[pos..]
                    .iter()
                    .take(*max_size)
                    .take_while(|&&c| intervals.contains(u32::from(c)))
                    .count();
                out.extend(pos + min_size..=pos + run);
            }
            Kind::Pattern {
                parsed,
                program,
                fullmatch: true,
            } => whole_ends(parsed, program, text, pos, out),
            Kind::Pattern {
                parsed,
                program,
                fullmatch: false,
            } => {
                let mut first: Option<usize> = None;
                let mut found = Vec::new();
                for start in pos..=text.len() {
                    if first.is_some_and(|first| start >= first) {
                        break;
                    }
                    found.clear();
                    whole_ends(parsed, program, text, start, &mut found);
                    if let Some(&end) = found.first() {
                        first = Some(first.map_or(end, |first| first.min(end)));
                    }
                }
                if let Some(first) = first {
                    out.extend(first..=text.len());
                }
            }
        }
    }
}

/// The positions where `parsed`, matched from `pos`, can end, through
/// `program` when there is one and by trying the backtracking matcher
/// on every prefix otherwise.
fn whole_ends(
    parsed: &ParsedPattern,
    program: &Option<Program>,
    text: &[char],
    pos: usize,
    out: &mut Vec<usize>,
) {
    match program {
        Some(program) => program.ends(text, pos, out),
        None => {
            let matcher = Matcher::new(parsed);
            for end in pos..=text.len() {
                if matches!(matcher.fullmatch(&text[pos..end]), Ok(true)) {
                    out.push(end);
                }
            }
        }
    }
}

/// A grammar's terminals as the parser matches them: the pattern
/// terminals through the lexer simulation, as Lark's dynamic lexer
/// does, and the explicit ones through their generators' matchers.
pub struct GrammarTerminals {
    lexer: Lexer,
    explicit: Vec<Option<ExplicitMatcher>>,
}

impl GrammarTerminals {
    /// The terminals of `grammar`, with those in `explicit` matched by
    /// the generator given for them instead of any pattern.
    pub fn new(
        grammar: &Grammar,
        explicit: &[(TermId, Arc<StringSpec>)],
    ) -> Result<GrammarTerminals, InternalError> {
        let mut matchers: Vec<Option<ExplicitMatcher>> = Vec::new();
        matchers.resize_with(grammar.terminals.len(), || None);
        for (id, spec) in explicit {
            matchers[*id] = Some(ExplicitMatcher::new(spec)?);
        }
        Ok(GrammarTerminals {
            lexer: Lexer::new(&grammar.terminals),
            explicit: matchers,
        })
    }

    /// The lexer simulation over the pattern terminals.
    #[cfg(test)]
    pub fn lexer(&self) -> &Lexer {
        &self.lexer
    }
}

impl Terminals for GrammarTerminals {
    fn ends(&self, id: TermId, text: &[char], pos: usize, out: &mut Vec<usize>) {
        match &self.explicit[id] {
            Some(matcher) => matcher.ends(text, pos, out),
            None => self.lexer.ends(id, text, pos, out),
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/explicit_tests.rs"]
mod tests;
