# How Hegel works

## The engine

Hegel is a family of property-based testing libraries, one per language, that
share one engine. The engine, `libhegel`, is written in Rust and does the work
that is the same in every language: deciding what to generate, running the
phases of a test, shrinking failures, keeping the example database, and
formatting the report. Each language's library, this one included, provides
the API you write tests with and translates between that language's values and
the engine.

Hegel for Python includes the engine in its compiled extension module, so
installing `hegeltest` installs everything Hegel needs.

## Choices

The engine never sees your values. When a test draws from a generator, the
generator asks the engine for a series of *choices*: an integer in a range, a
float, a boolean, a string from an alphabet, and so on. It builds its value
from them. A list generator, for example, repeatedly asks whether to add
another element and, each time it does, draws the element.

A test case is then entirely described by the sequence of choices made while
it ran. That has several consequences:

- **Shrinking works on choices.** To shrink a failure, the engine looks for a
  *simpler* sequence of choices (shorter, or with smaller values) that still
  makes the test fail. Because every generator is built from choices, this
  works through `map`, `filter`, `@hegel.composite`, state machines and your
  own code, without any generator needing to know how to shrink its values.
  Generators are written so that simpler choices produce simpler values: an
  integer shrinks toward zero, a list toward fewer and simpler elements.
- **Failures can be stored and replayed.** The example database stores the
  choice sequences of failing test cases, and the blob in
  `@hegel.reproduce_failure(...)` is an encoded choice sequence. Replaying the
  choices replays the test case, as long as the test and its generators still
  make the same requests.
- **Draws can depend on earlier draws.** A test can decide what to draw next
  based on what it has drawn so far, because the engine only ever answers one
  request at a time.

Choices are grouped into nested *spans*, one per draw, which tell the engine
which choices belong together. Shrinking uses them to, for example, delete a
whole list element at once.

## Phases

A run proceeds in phases, each of which can be turned off with the `phases`
setting:

`explicit`
: Run the test cases given with `@hegel.explicit_test_case`.

`reuse`
: Replay failures saved in the example database.

`generate`
: Generate new test cases, until the run has used its `test_cases` or found a
  failure.

`target`
: Once some test cases have run, steer generation toward inputs that
  maximise the scores given to {meth}`~hegel.TestCase.target`.

`shrink`
: Shrink each failure found to a minimal example.

Finally Hegel runs the test once more on the shrunk example. That run's
exception is the one the test raises, with the report attached as notes, and
its draws and notes are the ones the report shows.

## Draw names

To name the values in a report, Hegel looks at the source of the test and
finds the variable each `tc.draw(...)` call is assigned to. A name that is
assigned more than once, or is assigned inside a loop, is numbered: `x_1`,
`x_2`, and so on. A draw that isn't assigned to a variable is named `draw_1`,
`draw_2`. When the source isn't available, as in code typed at the
interactive prompt, every draw is numbered this way.

Draws made inside a composite generator are not reported separately: the
report shows the value the composite returned.

## Output

The report a failing test produces is a single document: the drawn values and
notes, in the order the test produced them. Hegel builds it during the final
run and attaches it to the exception afterwards, so it does not interleave
with anything the test prints. Values are printed with their `repr`, laid out
to fit the line width, with long collections broken across lines.
