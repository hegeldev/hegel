//! Drawing a grammar string whose explicit terminals take values the
//! caller draws.
//!
//! [`generate_grammar`] draws a string with a marker standing for each
//! occurrence of an explicit terminal. An [`ExplicitDraw`] walks that
//! string for the caller: [`ExplicitDraw::next`] names the terminal of the
//! next marker, the caller draws a value for it and hands it over with
//! [`ExplicitDraw::provide`], and the value is spliced in where the marker
//! stood. The values count toward the size cap like the grammar's own
//! characters: a string they push over it is drawn again, inside a
//! discarded span, with a smaller budget for the grammar's own text, and
//! after [`crate::HEGEL_FILTER_ATTEMPTS`] attempts that do not fit the test
//! case is rejected.

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;

use super::generate::{GrammarSpec, generate_grammar};
use crate::native::core::{EngineError, NativeTestCase, Status};
use crate::native::labels::label_from_name;

const LABEL_ATTEMPT: u64 = label_from_name("hegel.lark.attempt");

/// A drawn string being spliced: the string as the grammar drew it, how
/// far through it the splicing has got, and the result so far.
struct Splice {
    drawn: String,
    at: usize,
    out: String,
    /// Characters of `drawn` outside its markers, so far.
    own: u64,
    markers: u64,
    /// Characters of the values spliced in, so far.
    values: u64,
}

enum Stage {
    /// No attempt in progress.
    Idle,
    Splicing(Splice),
    /// A value is due for the explicit terminal at this index.
    Awaiting(Splice, usize),
    Done(String),
}

/// One draw of a grammar string with its explicit terminals' values
/// supplied by the caller. See the module documentation.
pub struct ExplicitDraw {
    spec: Arc<GrammarSpec>,
    max_size: u64,
    budget: u64,
    attempts: u32,
    stage: Stage,
}

impl ExplicitDraw {
    /// A draw of a string of `spec` of at most `max_size` characters,
    /// values included, or the grammar's own cap when `max_size` is 0.
    /// One below the grammar's smallest string, or above its cap, is read
    /// as that, as in [`generate_grammar`].
    pub fn new(spec: Arc<GrammarSpec>, max_size: u64) -> Self {
        let max_size = if max_size == 0 {
            spec.max_size()
        } else {
            max_size.max(spec.min_size()).min(spec.max_size())
        };
        ExplicitDraw {
            spec,
            max_size,
            budget: max_size,
            attempts: 0,
            stage: Stage::Idle,
        }
    }

    /// Splice on to the next marker: `Some(i)` when a value is due for the
    /// explicit terminal at index `i` of the grammar's explicit terminals,
    /// `None` when the string is complete (see [`Self::result`]). Draws
    /// the grammar string from `ntc` when an attempt starts, and rejects
    /// the test case when too many attempts in a row do not fit.
    pub fn next(&mut self, ntc: &mut NativeTestCase) -> Result<Option<usize>, EngineError> {
        loop {
            match core::mem::replace(&mut self.stage, Stage::Idle) {
                Stage::Idle => {
                    if self.spec.placeholders().is_empty() {
                        let text = generate_grammar(ntc, &self.spec, Some(self.max_size))?;
                        self.stage = Stage::Done(text);
                        return Ok(None);
                    }
                    ntc.start_span(LABEL_ATTEMPT);
                    let drawn = generate_grammar(ntc, &self.spec, Some(self.budget))?;
                    self.stage = Stage::Splicing(Splice {
                        out: String::with_capacity(drawn.len()),
                        drawn,
                        at: 0,
                        own: 0,
                        markers: 0,
                        values: 0,
                    });
                }
                Stage::Splicing(mut splice) => {
                    let rest = &splice.drawn[splice.at..];
                    let markers = self.spec.placeholders();
                    let found = rest.char_indices().find_map(|(at, _)| {
                        markers
                            .iter()
                            .position(|(_, marker)| rest[at..].starts_with(marker.as_str()))
                            .map(|i| (at, i))
                    });
                    if let Some((at, i)) = found {
                        let before = &rest[..at];
                        splice.own += before.chars().count() as u64;
                        splice.out.push_str(before);
                        splice.at += at + self.spec.placeholders()[i].1.len();
                        splice.markers += 1;
                        self.stage = Stage::Awaiting(splice, i);
                        return Ok(Some(i));
                    }
                    splice.own += rest.chars().count() as u64;
                    splice.out.push_str(rest);
                    let size = splice.own + splice.values;
                    if size <= self.max_size {
                        ntc.stop_span(false);
                        self.stage = Stage::Done(splice.out);
                        return Ok(None);
                    }
                    ntc.stop_span(true);
                    self.attempts += 1;
                    if self.attempts >= crate::HEGEL_FILTER_ATTEMPTS {
                        ntc.conclude(Status::Invalid, None);
                        return Err(EngineError::InvalidTestCase);
                    }
                    let scaled = u128::from(self.max_size)
                        * u128::from(splice.own + splice.markers)
                        / u128::from(size);
                    self.budget = (scaled as u64).min(self.budget.saturating_sub(1)).max(1);
                }
                Stage::Awaiting(splice, i) => {
                    let name = &self.spec.placeholders()[i].0;
                    self.stage = Stage::Awaiting(splice, i);
                    return Err(EngineError::InvalidArgument(format!(
                        "a value for explicit terminal '{name}' is due: provide it before \
                         asking for the next terminal"
                    )));
                }
                Stage::Done(text) => {
                    self.stage = Stage::Done(text);
                    return Ok(None);
                }
            }
        }
    }

    /// Splice in `value` for the explicit terminal [`Self::next`] named.
    pub fn provide(&mut self, value: &str) -> Result<(), String> {
        match core::mem::replace(&mut self.stage, Stage::Idle) {
            Stage::Awaiting(mut splice, _) => {
                splice.values += value.chars().count() as u64;
                splice.out.push_str(value);
                self.stage = Stage::Splicing(splice);
                Ok(())
            }
            other => {
                self.stage = other;
                Err(String::from(
                    "no explicit terminal is waiting for a value: call \
                     hegel_grammar_draw_next first",
                ))
            }
        }
    }

    /// The completed string, once [`Self::next`] has returned `None`.
    pub fn result(&self) -> Option<&str> {
        match &self.stage {
            Stage::Done(text) => Some(text),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/explicit_tests.rs"]
mod tests;
