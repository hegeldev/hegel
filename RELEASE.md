RELEASE_TYPE: patch

This patch adds `gs::from_lark`, which generates strings matching a grammar written in the syntax of the [Lark](https://lark-parser.readthedocs.io/en/stable/grammar.html) parsing library, the way `hypothesis.extra.lark.from_lark` does:

```rust
use hegel::generators as gs;

const GRAMMAR: &str = r#"
    ?start: sum
    ?sum: product | sum "+" product
    ?product: atom | product "*" atom
    ?atom: NUMBER | "(" sum ")"
    %import common.NUMBER
    %import common.WS
    %ignore WS
"#;

#[hegel::test]
fn parses_every_expression(tc: hegel::TestCase) {
    let expression = tc.draw(gs::from_lark(GRAMMAR));
    assert!(my_parser::parse(&expression).is_ok());
}
```

The grammar is compiled by libhegel, which supports Lark's whole grammar language — rules, string and regex terminals with priorities, `%import` (with Lark's bundled `common`, `python`, `unicode` and `lark` grammars built in, and `.import_path(dir)` for your own), `%ignore`, `%declare`, `%override`, `%extend` and templates — and generates strings Lark's Earley parser accepts, with `%ignore`d whitespace and comments inserted where a lexer would skip them. Grammar errors are reported at the first draw with Lark's diagnostics, or better ones where they exist.

Strings range up to 8192 characters by default and are biased toward the larger end; `.max_size(n)` and `.max_depth(n)` cap them (keep an ambiguous grammar's strings small, since checking one on replay or shrinking is an Earley parse cubic in its length, as in Lark itself), `.start(name)` picks the start rule, and `.alphabet(characters)` constrains the characters regex terminals draw. `.explicit(name, generator)` draws a terminal from any `Generator<String>` instead, and is required for `%declare`d terminals: the grammar string is drawn with a marker standing for each occurrence, a value is drawn per marker and spliced in, an `%ignore`d terminal goes on either side of each where the grammar has one, and `max_size` counts the values like any other characters (a string they push over it is drawn again with less of the cap for the grammar's own text).

A grammar string is a single choice holding the string itself, so a saved failure survives edits to the grammar as long as the string still parses, and it shrinks through its derivation — repetitions deleted, nested rules hoisted, ignored text cut, terminals shortened — rather than by re-running the generator.

`gs::from_regex` strings are recorded the same way: one choice holding the string, constrained by the pattern. A saved failure replays as long as the string still matches the pattern, and shrinking edits the string within the pattern instead of re-running the generator, so a `\d` shrinks to `0` and a `\w` to `0` rather than to the smallest digit or letter the old choice order happened to reach. Failures saved by earlier versions were recorded as the generator's individual draws and no longer replay; they are dropped from the database the way any stale entry is. A pattern that matches no string, such as `(?i)x(?![a-z])Y`, now generates nothing instead of strings Python's `re` would reject, and a lookbehind of variable width, such as `(?<=a*)b`, is rejected as Python's `re` rejects it.

The simplest string of a `gs::from_regex` pattern — the string a saved failure the pattern no longer matches is replaced by on replay, and the floor the shrinker works towards — is now the least string of the pattern in the shrinker's order (shorter first, then character by character), rather than the string the first alternative and the first character of each class happen to give: `(?:bbbb|a)` has the simplest string `a` where it had `bbbb`, so a failure on that pattern now shrinks to `a` instead of stopping at `bbbb`, and `(?i)abc` has `ABC`.
