# Strings under a grammar

How a string drawn from a Lark grammar (`from_lark`) lives in the choice
sequence: one string choice whose kind carries the grammar, generated
directly against the RNG, checked by parsing on replay and shrunk through
its derivation. A string drawn from a regex (`from_regex`) is one choice
on the same terms, with the pattern as its constraint; the last section
covers what differs. Module docs in `hegel-c/src/native/lark/` carry the
per-function detail; this document is the map. File names below are
relative to `hegel-c/src/native/`.

## The problem

The first `from_lark` recorded every decision of the grammar walk — which
alternative, how many repetitions, each terminal's characters — as a
choice of its own, the way any composite generator does. That made a
grammar string a long stretch of the choice sequence, and shrinking it
meant re-running the walk against edited choices: a shrinker pass that
deletes a few choices has no idea that it is deleting half a repetition,
and the walk's response to a change is whatever cascade the remaining
choices happen to produce. Database entries were as long as the walk,
and any change to the grammar (or to the walk's sizing) turned a stored
failure into noise.

## The model

A string drawn from a grammar is one `ChoiceData::String` node whose
`StringChoice` has `constraint: Some(Constraint::Grammar(..))`
(`core/choices.rs`). The
node's value is the text itself, its alphabet is every Unicode scalar
value and its only bounds are the empty string and the size cap: the
grammar alone decides which strings are in (`StringChoice::validate`
asks `GrammarConstraint::accepts`). Everything that works on string nodes
and does not know about grammars keeps working — the database stores the
text, a reproduce blob carries it, the execution cache keys on it, and a
stored failure survives an edit to the grammar as long as the text still
parses — while a string the grammar does not derive is an ordinary misfit
and is punned to the grammar's simplest string on replay
(`core/state.rs`, `resolve_choice_replayed`).

The pieces:

- **The constraint** (`lark/constraint.rs`): `GrammarConstraint` owns the
  compiled `Grammar`, its terminals as the parser matches them, a
  `Parser` from the start rule, and the size cap. It compares and hashes
  by an identity hashed from the compiled grammar, the start rule and the
  cap, so two runs that compile the same grammar afresh have the same
  kind (the `KindLedger` compares kinds across executions). It remembers
  the derivations it has found (up to `KNOWN_CAPACITY`, then forgets them
  all) so that a text known to derive is never parsed twice, and learns
  from every derivation the smallest text it has seen each rule and each
  terminal derive, for the shrinker. Its simplest string is set once, by
  the generator, at the first draw.
- **The generator** (`lark/generate.rs`): `generate_grammar` draws the
  string through `NativeTestCase::draw_string_from`, which hands the walk
  an `RngDraws` source over the test case's RNG (`draws/source.rs`) and
  records the text it returns as one node. On replay the stored text is
  validated instead, and the walk never runs. The walk is unchanged in
  what it produces — sizing, lexability checks and `%ignore` insertion as
  the module docs describe — but opens no spans within the `hegel.lark`
  span `generate_grammar` puts the choice in, and uses no collection
  protocol, since nothing it decides is a choice. A token of an
  `%ignore`d terminal that a rule itself draws is one the parser needs,
  and the lexing checks keep it apart from the ignorables they insert
  between tokens (`Token::needed`): only the inserted ones may be
  skipped or run through. The simplest string is
  the walk with every decision at its simplest (`SimplestDraws`), or, when
  that string does not lex back as its tokens, the first of a few seeded
  random walks that does, each candidate checked by parsing since the
  lexing checks only approximate the parser; a grammar with no string
  either way is reported at the first draw. A terminal the start rule
  reaches, or an ignorable, that the alphabet cannot produce is reported
  when the spec is built.
- **The parser** (`lark/parse.rs`, with `lark/lower.rs` desugaring
  repetitions): an Earley parser with a dynamic lexer, matching Lark's
  `parser="earley", lexer="dynamic"` on which strings it accepts, which is
  what `just check-parse` checks against Lark itself. It yields a `Tree`
  whose leaves cover the text exactly, `%ignore`d stretches included.
  An Earley parse is cubic in the text on an ambiguous grammar, so a
  parse that proposes more items than `PARSE_BUDGET` allows is given up
  and the text rejected, as one the grammar does not derive is, rather
  than take minutes.
  Terminals match through the lexer simulation with Lark's `match`
  semantics (one end per terminal, never empty); a terminal the user
  supplies the values of (`explicit`) is a string terminal in the
  compiled grammar whose text is its placeholder marker, so it matches
  like any literal.
- **The layout** (`lark/layout.rs`): `Layout` lays a `Tree` over its text,
  giving every node the range of text it derives (from its first to its
  last terminal, so ignored text at a node's edges belongs to no node but
  its own leaf) and answering the structural questions the shrinker asks:
  the hoists (a rule node containing a smaller node of the same rule),
  the repetitions of each repeat, the ignored stretches, the leaves and
  the rule nodes.

## Shrinking

The string passes (`shrinker/strings.rs`) skip a string under a grammar:
the strings they would propose are hardly ever in the grammar. The
grammar pass (`shrinker/grammar.rs`, `shrink_grammar_strings`, scheduled
just before them) edits the text where its derivation says the grammar
might still accept the result, and every candidate is a splice of the
text that `replace` then validates by parsing, so an edit the dynamic
lexer would read differently (two tokens that merge once the text between
them is gone, say) is simply rejected. For each node it tries, until a
round changes nothing:

1. the grammar's simplest string;
2. **hoists**: each rule node's text replaced by that of a smaller node of
   the same rule inside it (the 1st, 2nd, 4th, 8th, … such node below
   it, so a deep chain of one rule makes `d log d` pairs rather than
   `d²`), outermost first, starting over after each accepted hoist;
3. **repetitions**: for each repeat, all its repetitions deleted, then
   windows of 8, 4, 2 and 1 from the back;
4. **ignored text**: all of it cut at once, then each stretch from the
   back;
5. **smallest rule texts**: each rule node's text replaced by the smallest
   the constraint has seen that rule derive;
6. **leaves**: each leaf's text replaced by the smallest seen for its
   terminal, and otherwise deleted, cut to the shortest prefix that will
   do (a binary search), shortened one character at a time from the back,
   and lowered character by character down the alphabet's shrink order
   (binary search on the shrink key).

A candidate equal to the current text is not run, since the shrinker
would count it as accepted. The shrink order of the node is the string
kind's: shorter first, then character by character in the alphabet's
shrink order, so every edit above that is accepted is a strict
improvement and the pass terminates.

The nondeterminism graph shrinker (`graph_shrink.rs`) has a ladder per
kind rather than a searching pass; for a string under a grammar it
tries, after the simplest string, `Layout::coarse_candidates`: all the
ignored text cut, every repeat emptied, halved and shortened by one,
every hoist, and every leaf replaced by the smallest text seen for its
terminal, each once, and all of them again after one is accepted.

## Mutation

Span mutation (`test_runner.rs`, `try_span_mutation`) proposes new test
cases after a valid one by duplicating and swapping the choices under
same-label spans, which is how the engine reuses parts of one value in
another. A string under a grammar is one choice, in a span of its own
with none within it, so the same moves are made on its text through its
layout (`lark/mutate.rs`): the text of a rule node copied over another
node of the same rule (over a nested one to deepen a recursion, or over
the whole to cut it), or a repetition duplicated along with the text
that separates it from its neighbour. `try_grammar_mutation` runs after
span mutation on the same terms: a few random moves on a random string under
a grammar, each proposed through the execution cache, stopping at the
first interesting one. A move's result is parsed first, and one the
grammar does not derive or that is over the size cap is not run, since
replay would only pun it to the simplest string; nor is one whose
derivation nests rules deeper than the grammar's depth cap, which the
generator keeps to and the parser (which an ambiguous grammar may let
derive a generated string more deeply than the walk did) does not.

## What the design rules out, and what is left

- A derivation built by the shrinker is never remembered as a derivation
  of the edited text: the parser decides acceptance, because the
  grammar's language under a dynamic lexer is not the context-free
  language of its rules (a terminal's text must be what the lexer reads
  at that position in the new text). Parses are memoized by text, so the
  cost is one parse per new candidate.
- A terminal the user supplies the values of (`explicit`) stands in the
  string as a placeholder marker (`generate::placeholder`): the string
  holds the marker, the values are drawn as choices of their own after
  it and spliced in by the frontend, and the lexing checks never see
  them. A grammar with an ignorable has one between every marker and
  any token beside it (a marker at an edge of the string has nothing on
  that side): the generator insists on it, the constraint accepts no
  string without, and the shrinker never cuts one; whether a value reads as its
  terminal where it lands is the user's generator's affair. So is its
  size: a marker counts for nothing toward the grammar's size cap, which
  is on the characters outside the markers (the string choice's own
  length bound is left open for such a grammar), and the frontend keeps
  the spliced string under the cap the user set by drawing again under a
  smaller cap of its own (`hegel_generate_grammar`'s `max_size`) when
  the values pushed it over, in a span it discards, as a filter does.
- Mutation only reuses parts of the string being mutated, not of other
  strings under the same grammar seen earlier in the run.

## Strings under a regex

A string drawn from a regex is one string node whose kind has
`constraint: Some(Constraint::Regex(..))`: a `RegexConstraint`
(`re/constraint.rs`) over the parsed pattern, whose `accepts` runs the
backtracking matcher (`re/matcher.rs`) — the whole string must match, or
under `fullmatch(false)` some substring must — and whose identity is
hashed from the pattern, its flags, the match mode and the alphabet. The
alphabet is the one the generator was given, or every scalar value; the
size cap is `REGEX_MAX_SIZE` (`draws/regex.rs`), and a draw over it
concludes the test case invalid, so a recorded string always validates.
`generate_string` draws it through `draw_string_from` like a grammar
string, with the regex walk (`draws/regex.rs`) as the random source, and
sets the simplest string at the first draw the same way: the walk with
every decision at its simplest, or the first of a few seeded walks that
passes the pattern's deferred checks; a pattern no walk satisfies
concludes the test case invalid, as a draw that finds no string does.

There is no derivation pass: the string passes (`shrinker/strings.rs`)
shrink a string under a regex, every candidate checked against the
pattern when the node takes its new value, with two stages they run only
under a constraint — deleting runs of two to eight characters, which
removes an alternative the pattern only admits whole, and copying either
end of the simplest string over the current one, which reaches an
alternative that differs in several characters at once. A shrink that
needs several characters changed together anywhere else is out of reach.
Nor is there mutation within a regex string; span mutation still copies
one whole regex string over another drawn under the same label, as it
does any choice, and a copy the other pattern rejects is punned to that
pattern's simplest string on replay.
