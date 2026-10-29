use std::panic::{RefUnwindSafe, UnwindSafe};
use std::path::PathBuf;
use std::sync::{LazyLock, OnceLock};

use super::generators::draw_and_print_value;
use super::strings::CharactersGenerator;
use super::{Generator, PrintableGenerator, TestCase, combine_labels, label_from_name};
use crate::ffi;
use crate::pretty::PrettyPrinter;
use crate::test_case::invalid_argument;

static LARK_LABEL: LazyLock<u64> = LazyLock::new(|| label_from_name("hegel.from_lark"));

type ExplicitTerminal = dyn Generator<String> + Send + Sync + UnwindSafe + RefUnwindSafe;

/// A terminal whose values come from a generator of the user's rather
/// than from the grammar.
struct Explicit {
    name: String,
    generator: Box<ExplicitTerminal>,
}

/// One explicit terminal's generator as the test case draws it: a
/// forwarding wrapper, so the value gets the generator's own span.
struct Terminal<'a>(&'a ExplicitTerminal);

impl Generator<String> for Terminal<'_> {
    fn label(&self) -> u64 {
        self.0.label()
    }

    fn do_draw(&self, tc: &TestCase) -> String {
        self.0.do_draw(tc)
    }
}

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
    explicit: Vec<Explicit>,
    alphabet: Option<CharactersGenerator>,
    max_size: Option<usize>,
    max_depth: Option<u32>,
    import_dir: Option<PathBuf>,
    label: u64,
    handle: OnceLock<ffi::Grammar>,
}

impl LarkGenerator {
    /// Generate strings of the rule named `start` rather than of Lark's
    /// default start rule, `start`.
    pub fn start(mut self, start: &str) -> Self {
        self.handle = OnceLock::new();
        self.start = Some(start.to_string());
        self.relabel();
        self
    }

    /// Draw the terminal `name` from `generator` instead of from the
    /// terminal's own pattern. Required for a terminal the grammar
    /// `%declare`s without defining; any other terminal may be overridden.
    ///
    /// The grammar string is drawn with a marker standing for each
    /// occurrence of the terminal, then a value is drawn from `generator`
    /// for each marker, in order, and spliced in. The values are beyond
    /// the lexing checks the engine makes on the rest of the string: an
    /// `%ignore`d terminal goes on either side of each where the grammar
    /// has one, and it is the generator's part to draw values that read
    /// as the terminal where they land (a value under a quoted-string
    /// terminal should contain no quote, say). The values count toward
    /// [`max_size`](Self::max_size) like any other characters: a string
    /// they push over it is drawn again, with less of the cap for the
    /// grammar's own text, and a draw that cannot fit the values after a
    /// few tries rejects the test case.
    pub fn explicit<G>(mut self, name: &str, generator: G) -> Self
    where
        G: Generator<String> + Send + Sync + UnwindSafe + RefUnwindSafe + 'static,
    {
        self.handle = OnceLock::new();
        self.explicit.push(Explicit {
            name: name.to_string(),
            generator: Box::new(generator),
        });
        self.relabel();
        self
    }

    fn relabel(&mut self) {
        let mut labels = vec![
            *LARK_LABEL,
            label_from_name(&self.text),
            label_from_name(self.start.as_deref().unwrap_or("start")),
        ];
        for explicit in &self.explicit {
            labels.push(label_from_name(&explicit.name));
            labels.push(explicit.generator.label());
        }
        self.label = combine_labels(&labels);
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
    /// default is 8192, and the most allowed is 1048576.
    pub fn max_size(mut self, max_size: usize) -> Self {
        self.handle = OnceLock::new();
        self.max_size = Some(max_size);
        self
    }

    /// Set how deeply rules may nest in a generated string. The default is
    /// 200, and the most allowed is 500.
    pub fn max_depth(mut self, max_depth: u32) -> Self {
        self.handle = OnceLock::new();
        self.max_depth = Some(max_depth);
        self
    }

    /// Set the directory `%import`ed grammars are read from, replacing
    /// any set before. `%import a.b.X` reads `a/b.lark` from it, and Lark's
    /// bundled `common`, `python`, `unicode` and `lark` grammars stand in
    /// when it has no such file. A relative import (`%import .lib.X`) in
    /// `a/b.lark` reads `a/lib.lark`, and in the grammar text itself reads
    /// `lib.lark`; no bundled grammar stands in for it, as in Lark. Unlike
    /// Lark's `import_paths`, there is one directory and no search path.
    pub fn import_path(mut self, dir: impl Into<PathBuf>) -> Self {
        self.handle = OnceLock::new();
        self.import_dir = Some(dir.into());
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
            let names: Vec<String> = self.explicit.iter().map(|e| e.name.clone()).collect();
            if let Some(name) = names
                .iter()
                .find(|name| names.iter().filter(|n| n == name).count() > 1)
            {
                invalid_argument!("explicit terminal '{name}' is given more than one generator");
            }
            let alphabet = self.alphabet.as_ref().map(|a| a.build_alphabet_handle());
            ffi::Grammar::new(
                &self.text,
                self.start.as_deref(),
                self.max_size.map_or(0, |max_size| max_size as u64),
                self.max_depth.unwrap_or(0),
                alphabet.as_ref(),
                &names,
                self.import_dir.as_deref(),
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
        let grammar = self.handle();
        if self.explicit.is_empty() {
            return tc.generate_grammar(grammar, 0);
        }
        let draw = grammar.start_draw(0);
        while let Some(terminal) = tc.grammar_draw_next(&draw) {
            let value = tc.draw_silent(Terminal(&*self.explicit[terminal].generator));
            draw.provide(&value);
        }
        draw.result()
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
/// [`max_depth`](LarkGenerator::max_depth) to keep them smaller.
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
    let mut generator = LarkGenerator {
        text: grammar.to_string(),
        start: None,
        explicit: Vec::new(),
        alphabet: None,
        max_size: None,
        max_depth: None,
        import_dir: None,
        label: 0,
        handle: OnceLock::new(),
    };
    generator.relabel();
    generator
}
