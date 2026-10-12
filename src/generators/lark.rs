use std::panic::{RefUnwindSafe, UnwindSafe};
use std::path::PathBuf;
use std::sync::OnceLock;

use super::generators::draw_and_print_value;
use super::strings::{CharactersGenerator, TerminalGenerator};
use super::{Generator, PrintableGenerator, TestCase, combine_labels, label_from_name};
use crate::ffi;
use crate::pretty::PrettyPrinter;
use crate::test_case::invalid_argument;

const LARK_LABEL: u64 = label_from_name("hegel.from_lark");

type ExplicitTerminal = dyn TerminalGenerator + Send + Sync + UnwindSafe + RefUnwindSafe;

/// Generator for strings matching a grammar written in Lark's syntax.
/// Created by [`from_lark()`].
///
/// The grammar is checked and compiled on the first draw; a grammar Lark
/// would reject, an undefined start rule, a cap below the smallest string
/// the grammar can produce, or an `explicit` generator for a terminal the
/// grammar does not use is reported then as an invalid argument.
pub struct LarkGenerator {
    text: String,
    start: Option<String>,
    explicit: Vec<(String, Box<ExplicitTerminal>)>,
    alphabet: Option<CharactersGenerator>,
    max_size: Option<usize>,
    max_depth: Option<u32>,
    import_paths: Vec<PathBuf>,
    label: u64,
    handle: OnceLock<ffi::Grammar>,
}

fn lark_label(text: &str, start: Option<&str>) -> u64 {
    combine_labels(&[
        LARK_LABEL,
        label_from_name(text),
        label_from_name(start.unwrap_or("start")),
    ])
}

impl LarkGenerator {
    /// Generate strings of the rule named `start` rather than of Lark's
    /// default start rule, `start`.
    pub fn start(mut self, start: &str) -> Self {
        self.handle = OnceLock::new();
        self.start = Some(start.to_string());
        self.label = lark_label(&self.text, self.start.as_deref());
        self
    }

    /// Draw the terminal `name` from `generator` instead of from the
    /// terminal's own pattern. Required for a terminal the grammar
    /// `%declare`s without defining; for any other terminal the strings
    /// drawn should match its pattern, or the result may not be in the
    /// grammar.
    pub fn explicit<G>(mut self, name: &str, generator: G) -> Self
    where
        G: TerminalGenerator + Send + Sync + UnwindSafe + RefUnwindSafe + 'static,
    {
        self.handle = OnceLock::new();
        self.explicit.push((name.to_string(), Box::new(generator)));
        self
    }

    /// Constrain which characters terminals drawn from their patterns may
    /// contain, as [`RegexGenerator::alphabet`](super::RegexGenerator::alphabet)
    /// does for a single pattern.
    pub fn alphabet(mut self, alphabet: CharactersGenerator) -> Self {
        self.handle = OnceLock::new();
        self.alphabet = Some(alphabet);
        self
    }

    /// Set the maximum length in characters of the generated strings. The
    /// default is 8192.
    pub fn max_size(mut self, max_size: usize) -> Self {
        self.handle = OnceLock::new();
        self.max_size = Some(max_size);
        self
    }

    /// Set how deeply rules may nest in a generated string. The default is
    /// 200.
    pub fn max_depth(mut self, max_depth: u32) -> Self {
        self.handle = OnceLock::new();
        self.max_depth = Some(max_depth);
        self
    }

    /// Add a directory to search for `%import`ed grammars, as Lark's
    /// `import_paths` does. Directories are searched in the order added, and
    /// an import a grammar file makes relative to itself (`%import .lib.X`)
    /// is also looked for next to that file. Lark's bundled `common`,
    /// `python`, `unicode` and `lark` grammars are always available.
    pub fn import_path(mut self, dir: impl Into<PathBuf>) -> Self {
        self.handle = OnceLock::new();
        self.import_paths.push(dir.into());
        self
    }

    fn handle(&self) -> &ffi::Grammar {
        self.handle.get_or_init(|| {
            if self.max_size == Some(0) {
                invalid_argument!("max_size must be at least 1");
            }
            if self.max_depth == Some(0) {
                invalid_argument!("max_depth must be at least 1");
            }
            let alphabet = self.alphabet.as_ref().map(|a| a.build_alphabet_handle());
            let explicit: Vec<(String, &ffi::StringGenerator)> = self
                .explicit
                .iter()
                .map(|(name, g)| (name.clone(), g.terminal_handle()))
                .collect();
            ffi::Grammar::new(
                &self.text,
                self.start.as_deref(),
                self.max_size.map_or(0, |n| n as u64),
                self.max_depth.unwrap_or(0),
                alphabet.as_ref(),
                &explicit,
                &self.import_paths,
            )
            .unwrap_or_else(|msg| invalid_argument!("{msg}"))
        })
    }
}

impl Generator<String> for LarkGenerator {
    fn label(&self) -> u64 {
        self.label
    }

    fn do_draw(&self, tc: &TestCase) -> String {
        tc.generate_grammar(self.handle())
    }
}

impl PrintableGenerator<String> for LarkGenerator {
    fn do_draw_and_print(&self, tc: &TestCase, printer: &mut PrettyPrinter) -> String {
        draw_and_print_value(self, tc, printer)
    }
}

/// Generate strings matching a grammar written in the syntax of the
/// [Lark](https://lark-parser.readthedocs.io/en/stable/grammar.html) parsing
/// library, starting from its `start` rule.
///
/// The whole grammar language is supported: rules, terminals with string
/// and regex patterns and priorities, `%import` (including Lark's bundled
/// `common`, `python`, `unicode` and `lark` grammars), `%ignore`,
/// `%declare`, `%override`, `%extend` and templates. Generated strings are
/// ones Lark's Earley parser accepts for the grammar, with `%ignore`d
/// terminals such as whitespace and comments inserted wherever a lexer
/// would skip them; a string Lark's basic lexer would read differently is
/// avoided where the grammar allows it. A draw that cannot produce such
/// a string after several attempts rejects the test case, as a failed
/// [`assume`](crate::TestCase::assume) does, so a grammar that often
/// needs that fails the filter health check.
///
/// Strings range up to 8192 characters by default, biased toward the
/// larger end; use [`max_size`](LarkGenerator::max_size) and
/// [`max_depth`](LarkGenerator::max_depth) to keep them smaller. Keep
/// them small for an ambiguous grammar: checking a string against one,
/// when a saved failure is replayed or a string is shrunk, is an Earley
/// parse cubic in the string's length, as it is in Lark itself, and a
/// parse that grows past the engine's budget (some hundreds of tokens of
/// a highly ambiguous grammar) treats the string as outside the grammar.
///
/// A drawn string is a single choice holding the string itself, which
/// the engine checks against the grammar by parsing it when replaying a
/// saved failure — so the failure survives edits to the grammar as long
/// as the string still parses — and shrinks through its derivation:
/// repetitions are deleted, a nested rule is hoisted over the rule
/// around it, ignored text is cut, and each terminal's text is shortened
/// and simplified, every step a string of the grammar.
///
/// ```no_run
/// use hegel::generators as gs;
///
/// const GRAMMAR: &str = r#"
///     ?start: sum
///     ?sum: product | sum "+" product -> add
///     ?product: atom | product "*" atom -> mul
///     ?atom: NUMBER | "(" sum ")"
///     %import common.NUMBER
///     %import common.WS
///     %ignore WS
/// "#;
///
/// #[hegel::test]
/// fn parses_every_expression(tc: hegel::TestCase) {
///     let expression = tc.draw(gs::from_lark(GRAMMAR));
///     assert!(my_parser::parse(&expression).is_ok());
/// }
/// # mod my_parser { pub fn parse(_: &str) -> Result<(), ()> { Ok(()) } }
/// ```
///
/// See [`LarkGenerator`] for builder methods.
pub fn from_lark(grammar: &str) -> LarkGenerator {
    LarkGenerator {
        text: grammar.to_string(),
        start: None,
        explicit: Vec::new(),
        alphabet: None,
        max_size: None,
        max_depth: None,
        import_paths: Vec::new(),
        label: lark_label(grammar, None),
        handle: OnceLock::new(),
    }
}
