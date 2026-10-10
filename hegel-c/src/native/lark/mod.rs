//! Port of Lark's grammar language (`lark/load_grammar.py`) for generating
//! strings from a grammar.
//!
//! Lark grammars are parsed by [`parser`] into the [`ast`] types, assembled
//! by [`builder`] (imports, templates, directives, validation, exactly as
//! Lark's `GrammarBuilder` does) and compiled by [`compile`] into the
//! [`compile::Grammar`] the generator walks: terminals as single regexes,
//! rules as trees of alternatives, sequences and repetitions. Unlike Lark,
//! repetition operators are kept as repetition nodes rather than desugared
//! into recursive rules, so the generator can size them directly. [`lower`]
//! desugars them for [`parse`], an Earley parser with a dynamic lexer that
//! checks strings against the grammar and recovers their derivations,
//! matching the terminals the user supplies generators for through
//! [`explicit`].

pub mod analysis;
pub mod ast;
pub mod builder;
pub mod compile;
/// The generator and the grammar shrink pass will seed the memo and read
/// derivations back; until they do, only validation is reached.
#[cfg_attr(not(test), allow(dead_code))]
pub mod constraint;
pub mod escape;
/// Matched by [`constraint`] alone until the generator wires it in.
#[cfg_attr(not(test), allow(dead_code))]
pub mod explicit;
pub mod generate;
pub mod lexer;
pub mod lexsim;
/// Nothing in the engine parses yet: database validation and the grammar
/// shrink pass will, and lift the allowance.
#[cfg_attr(not(test), allow(dead_code))]
pub mod lower;
#[cfg_attr(not(test), allow(dead_code))]
pub mod parse;
pub mod parser;

use alloc::string::String;
use core::fmt;

/// An error in a grammar: a syntax error, an unsatisfied reference, a
/// misuse of a directive, or a grammar that can never produce a finite
/// string. The message is the diagnostic to report to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarError {
    pub message: String,
}

impl GrammarError {
    pub fn new(message: impl Into<String>) -> Self {
        GrammarError {
            message: message.into(),
        }
    }
}

impl fmt::Display for GrammarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

pub type GrammarResult<T> = Result<T, GrammarError>;

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/mod_tests.rs"]
mod tests;
