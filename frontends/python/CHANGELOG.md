# Changelog

## 0.5.3 - 2026-11-25

This patch adds wheels for Python 3.15, and wheels for Windows on ARM64.

## 0.5.2 - 2026-11-24

This release updates the bundled libhegel engine to 0.53.2.

## 0.5.1 - 2026-11-20

This patch sets a lower bound for the probability of checking a sampled invariant.

## 0.5.0 - 2026-11-19

This release moves the date and time generators into a `gs.temporal` namespace, under the names every Hegel frontend uses for them, moves date and time generation into the engine, which now bundles a pinned IANA time zone database, and adds `zoned_datetimes()`, `instants()`, `spans()`, `zone_names()`, `offsets()` and `tzdb_version()`.

The released top-level names are deprecated. They keep working, with a `DeprecationWarning`, and take the arguments they were released with; the arguments this release adds are on the `gs.temporal` names only:

```python
# before
gs.dates()
gs.times(timezones=gs.timezones())
gs.datetimes(timezones=gs.timezones())
gs.timedeltas()
gs.timezones()

# after
gs.temporal.dates()
gs.temporal.times(zones=gs.temporal.zones())
gs.temporal.datetimes(zones=gs.temporal.zones())
gs.temporal.durations()
gs.temporal.zones()
```

The module `hegel.generators.time` is now `hegel.generators.temporal`, with no alias under the old name, and `from_type` resolves `datetime.timedelta` to `durations()`, `datetime.tzinfo` and `zoneinfo.ZoneInfo` to `zones()`, `datetime.timezone` to `offsets()` and `gs.temporal.Span` to `spans()`.

Every bound of every generator may be omitted or passed as `None`, and the `repr` of a date or time generator shows only the arguments it was given.

`zones()` and the new `zone_names()` draw IANA zones and their names from the names `zoneinfo` can load. Half the draws are weighted toward zones with a rare feature (a fractional offset, negative daylight saving, a skipped day), and a name shrinks down a ladder from `UTC` through the zones a reader recognises rather than alphabetically. Link names such as `Asia/Calcutta` are included and legacy names such as `EST5EDT` excluded by default; `allow_links` and `allow_legacy` switch each. `names` restricts the draw to an explicit list instead, and cannot be combined with either.

`datetimes()` with a `zones` generator now draws the wall-clock time in the engine when the zone is a `zoneinfo.ZoneInfo`, against the transitions `zoneinfo` itself reads for it: one draw in twenty on average lands within a second, an hour or a day of one of the zone's daylight-saving or offset transitions, or of a well-known instant such as the Unix epoch or the end of the signed 32-bit epoch. A repeated wall time is drawn with either value of `fold`, where `fold` used to be always 0, or with the one `ambiguous="earlier"` or `"later"` asks for, and a skipped wall time is reached deliberately; `allow_imaginary=False` now makes the engine draw a real wall time instead of rejecting the test case. A `tzinfo` of another kind (pytz, dateutil, a `ZoneInfo` with no key) is drawn without that targeting, and `fold` and `ambiguous` go by what the `tzinfo` itself reports.

`datetimes()` also accepts timezone-aware bounds, as Hypothesis does: both bounds are then moments in time, and every datetime is aware and lies between them in a zone drawn from `zones`, which defaults to every zone of the zone source and must not generate `None`. One naive and one aware bound is an `InvalidArgument`. The new `zoned_datetimes()` takes the same arguments but always draws aware datetimes.

The new `instants()` draws moments in time as UTC datetimes between two aware bounds, targeting instants such as the Unix epoch, the 32-bit boundaries and the transitions of every zone in the database. The new `spans()` draws calendar spans such as "1 month and 2 days", which a `timedelta` cannot represent, as `gs.temporal.Span` objects with a field per unit from years to nanoseconds; each unit takes a `(min, max)` bound, naming any unit draws only the named ones, and `sign` chooses one sign for the whole span (`"uniform"`, the default), a sign per unit (`"mixed"`) or magnitudes only (`"non_negative"`):

```python
span = tc.draw(gs.temporal.spans(months=(0, 11), days=(0, 30)))
str(span)  # "3mo 12d", as every Hegel frontend prints it
```

A span with no positive unit prints one leading `-` (`"-3d 4h"` is minus three days and minus four hours), and one with units of both signs prints a sign on each (`"-3d +4h"`).

`dates()` takes `weekdays` (numbered as `date.weekday()` numbers them) and `months` to draw only, say, weekends or Februaries, moving a drawn date to the nearest allowed one rather than filtering. `times()`, `datetimes()`, `zoned_datetimes()`, `instants()` and `durations()` take a `precision`, one of the `timespec` names `isoformat` uses (`"milliseconds"`, `"seconds"`, `"minutes"` or `"hours"`), so every drawn time or delta is a whole number of that unit. `tzdb_version()` reports the IANA release of the engine's bundled database. `offsets()` draws fixed offsets as `datetime.timezone` objects, within bounds and at a `granularity` of hours, quarter-hours, minutes or seconds, targeting the offsets real zones have used.

`times()`, `datetimes()` and `durations()` draw whole microseconds, Python's resolution, rather than nanoseconds truncated afterwards. `times()` now raises `InvalidArgument` when its `zones` generator produces something that is not a `tzinfo`. `dates()` targets the leap days nearest the bounds, the year boundaries, the 2038 and 2106 overflow days and ISO week 53 in one draw of twenty on average, and `durations()` round durations such as a second, an hour, a day or a week; a `timedelta` now shrinks toward zero as a whole, so the simplest negative delta is a microsecond below zero rather than a whole day.

Zoned draws take their time zone data from a `gs.temporal.ZoneSource`, so the gaps and folds they aim at are those of the database the code under test reads. The default, `ZoneSource.zoneinfo()`, is `zoneinfo`'s own database: the names `zoneinfo.available_timezones()` lists, and each zone read from the TZif file `zoneinfo.ZoneInfo` would load for it (from `zoneinfo.TZPATH`, then the `tzdata` package) the first time a draw needs it. `ZoneSource.bundled()` is the engine's database. `zones()`, `zone_names()`, `datetimes()` and `zoned_datetimes()` take a `source`; on the last two it is the data every drawn `ZoneInfo` is read against, and the zones drawn by default. `default_zone_source()` returns the default and `set_default_zone_source()` replaces it, returning the source it replaced. A generator reads its source, and the names the source knows, when it is built:

```python
BUNDLED = gs.temporal.ZoneSource.bundled()
gs.temporal.zoned_datetimes(source=BUNDLED)
gs.temporal.datetimes(zones=gs.temporal.zones(source=BUNDLED))
```

Where `zoneinfo` finds no time zone database at all (Windows without the `tzdata` package), a generator that needs the default source's zones (`zones()`, `zone_names()`, `zoned_datetimes()`, `datetimes()` with aware bounds, `from_type(datetime.tzinfo)`) raises `InvalidArgument` when it is built, saying to install `tzdata`; `zone_names(source=ZoneSource.bundled())` draws names without it. The deprecated `gs.timezones()` still generates only `datetime.UTC` there, as it always has.

An integration with another date and time library can fill an empty `ZoneSource()` with that library's data, as TZif bytes (`add_tzif`), fields (`add_zone` with a `ZoneData`, built from `ZoneType`, `ZoneRule` and `RuleDate`), an offset oracle the engine probes (`add_oracle` with a `ZoneOracle`) or links (`add_link`), up front or from a loader the first time a draw needs a zone (`declare` and `set_loader`), and label it with a `version`. An exception the loader raises fails the draw with an `InvalidArgument` caused by it. `source.info(name)`, `name in source`, `source.resolve(name, wall_time)` and `source.offset_at(name, instant)` read back what the engine made of the data, to check it against the library's own answers.

## 0.4.9 - 2026-11-18

This patch makes verbose output (`verbosity="verbose"` or `"debug"`) stream as the test runs. A test case's drawn values and notes are printed as soon as they are final, rather than all at once when the test case ends, so a long-running test shows its progress as it goes and a stateful test prints each step, or each round of a concurrent state machine, as it happens. Hegel also no longer holds a test case's output in memory until it finishes, so memory use stays flat however long a test case runs. Failure reports are unchanged.

An output sink installed with `with_output_override` may now be called while a test case is running, including from a test's own threads, and an exception it raises is reported through `sys.unraisablehook` rather than propagating into the test.

A failing test case in a verbose run now ends with one line, the traceback's line for the frame in your test that raised the exception, followed by its type and the first line of its message (`File "tests/test_x.py", line 12, in test_x: AssertionError: x too big: 10`), instead of its full traceback, which is printed at `verbosity="debug"` instead. Hegel only formats a traceback when it will be printed or reported.

`with_output_override(None)` sends Hegel's output nowhere: nothing is printed, and verbose output costs nothing, while a failure is still reported on its exception as usual.

## 0.4.8 - 2026-11-16

This release updates the bundled libhegel engine to 0.51.6.

## 0.4.7 - 2026-11-13

This patch adds a `pause_faults` flag to stateful invariants, next to `always_run`: on `@hegel.invariant(...)`, `Machine.invariant(...)` and the `Invariant` dataclass. An invariant declared with `pause_faults=True` has fault injection paused for the duration of each check, so that inside [Antithesis](https://antithesis.com/) the fault injector stops while the invariant runs and the check observes the system under test without a fault landing in the middle of it.

```python
class Cluster(hegel.StateMachine):
    @hegel.invariant(pause_faults=True)
    def replicas_agree(self, tc):
        assert len({node.state() for node in self.nodes}) == 1
```

Outside Antithesis the flag has no effect.

## 0.4.6 - 2026-11-13

This release updates the bundled libhegel engine to 0.51.4.

## 0.4.5 - 2026-11-13

This release updates the bundled libhegel engine to 0.51.3.

## 0.4.4 - 2026-11-13

This release updates the bundled libhegel engine to 0.51.2.

## 0.4.3 - 2026-11-10

This release updates the bundled libhegel engine to 0.51.1.

## 0.4.2 - 2026-11-09

This patch lets a long state-machine run explore more than one configuration. Between rounds, a state machine now occasionally redraws its value distribution, the rules each worker has enabled, and the concurrency level. Previously these were fixed for the whole test case, so a single long-running test case kept one configuration for its whole run.

A concurrent machine now always starts `max_concurrency` worker threads, and only the current concurrency level's worth of them run rules in a round. A failure report notes `Concurrency level: n` at the first round and whenever the level changes.

## 0.4.1 - 2026-11-06

This patch adds `@hegel.main`, which turns a function that takes a test case into the entry point of a program. Calling the decorated function reads the command line and runs one test case. Each setting except `test_cases` is a command-line option named after its `hegel.toml` key (`--seed`, `--phases`, `--backend` and so on), and `--profile` selects a settings profile.

```python
@hegel.main
def workload(tc):
    n = tc.draw(gs.integers())
    assert n + 0 == n


if __name__ == "__main__":
    workload()
```

## 0.4.0 - 2026-11-03

This release lets a rule of a concurrent state machine belong to several concurrency groups. The `group` argument of `@hegel.rule` and `Machine.rule` is replaced by `groups`, a list of group names; a rule runs in the rounds of every group it names, so it may overlap the rules of each of them. `Rule.group` becomes `Rule.groups`, a tuple. Omitting `groups` still puts the rule in the anonymous group.

Before:

```python
@hegel.rule(group="write")
def write(self, tc): ...
```

After:

```python
@hegel.rule(groups=["write"])
def write(self, tc): ...

@hegel.rule(groups=["write", "flush"])
def read(self, tc): ...
```

Here `read` may run concurrently with `write` in one round and with `flush` in another, while `write` and `flush` never overlap each other, which is how a two-tier store whose flush briefly hides a key can be caught by a read that overlaps it.

The list must name at least one group and must not repeat one: an empty list or a repeated group is an `InvalidArgument` reported by the engine when the machine runs.

## 0.3.0 - 2026-10-29

This release changes how deeply nested draws are handled, which can make an existing test fail a health check. A test case used to be abandoned once its draws nested 100 deep, which a `recursive()` generator at its default `max_depth` of 32 could reach, and a draw from a `deferred()` generator nested more than 64 deep was rejected as if by a filter. Both fixed limits are gone: draws may now nest as deeply as Python's recursion limit allows, and a test case is abandoned as too large only when the interpreter's recursion limit (`sys.getrecursionlimit()`) is within 200 frames, rather than failing with a `RecursionError`. Abandoned test cases count towards the `test_cases_too_large` health check, so:

- a self-referential generator whose values routinely nest that deeply now fails with `FailedHealthCheck` (`test_cases_too_large`), where such draws used to be quietly rejected;
- a `deferred()` definition that only refers back to itself now fails with `FailedHealthCheck` rather than `Unsatisfiable`.

To keep the old behaviour of discarding the too-deep test cases, suppress the health check; or raise the recursion limit to let the draws nest deeper. The `unbounded_choices` setting does not help here: it lifts the limit on choices, not on nesting.

```python
@hegel.test(suppress_health_check=[hegel.HealthCheck.test_cases_too_large])
def test_trees(tc):
    tc.draw(trees)
```

It also adds `from_lark()`, which generates strings of a grammar written in the syntax of the [Lark](https://lark-parser.readthedocs.io/en/stable/grammar.html) parsing library, as the Rust library's `from_lark` does. The grammar is parsed and its strings generated by Hegel's engine, so Lark need not be installed. `start` picks the rule to generate, `explicit` maps terminal names to generators of strings that supply those terminals' values (required for a terminal the grammar `%declare`s), `alphabet` restricts the characters the terminals' patterns produce, `max_size` and `max_depth` cap the length and nesting of the strings, and `import_path` is the directory `%import`ed grammar files are read from. The 0.2.0 changelog described `from_lark()` as following this Python's Unicode version, though no `from_lark()` existed then; it now exists, and does.

```python
expression = tc.draw(
    gs.from_lark(
        """
        ?start: sum
        ?sum: product | sum "+" product
        ?product: atom | product "*" atom
        ?atom: NUMBER | "(" sum ")"
        %import common.NUMBER
        %import common.WS
        %ignore WS
        """
    )
)
```

It gives `Settings` an `unbounded_choices` field (default `False`), the engine setting that lifts the limit of 2**20 choices per test case, for long-running test cases such as a concurrent state machine driven for hours. `Settings` now mirrors every engine setting, so a profile that sets `unbounded_choices` (one registered by another Hegel library in the same process, say) is no longer reset to the default when a Python test runs with it.

A `Settings` value now holds only the fields it was given, and takes every other one from the default profile each time it is used, rather than copying every field from the profile when it was created. A `Settings` made at import time, such as `FAST = hegel.Settings(test_cases=3)` in a `conftest.py`, therefore now follows the pytest options (`--hegel-seed`, `--hegel-setting` and the rest) and any later `Settings.set_default_profile()`, where it used to keep the defaults of the moment it was made. Reading a field (`FAST.seed`) resolves it in the same way, at the time of reading. `Settings.from_profile()` still pins its profile. `Settings` is no longer a dataclass, so `dataclasses.fields()` and `dataclasses.replace()` no longer apply to it; use `Settings.replace()`. `Settings` values can still be copied and pickled, and keep their unresolved form when they are, so an unpickled value follows the default profile of the process that uses it.

`TestCase.clone()` and `TestCase.set_worker()` are now public, so a test can draw from several threads: give each thread its own clone, which draws from an independent choice stream of the same test case, and optionally attribute its output to a numbered worker. They were previously private, used only by the concurrent stateful runner. A clone now shares draw names with the test case it was cloned from, rather than numbering its draws afresh: a draw named `x` on each prints as `x_1` and `x_2`, where each used to number its draws from 1, so that both printed as `x_1`. This changes the names in the failure reports of concurrent stateful runs, whose workers are clones.

A draw assigned inside a function nested in a `@hegel.test` function, such as the `step` of `tc.repeat(step)`, now prints numbered from its first use (`x_1` under `# Repetition #1`, `x_2` under `# Repetition #2`), as a draw inside a loop does and as the Rust library prints a draw in a closure, where its first use printed bare (`x`, then `x_2`). The test function itself is the top level: a draw assigned directly in it still prints bare, whether or not the test is defined inside another function.

`text()`, whatever its alphabet, and `binary()` without a `max_size` now leave the maximum to the engine, which applies the default every Hegel library shares (100, or `min_size + 100` when `min_size` is larger), rather than computing it in Python. Their reprs no longer show a `max_size` that was not given. `filter()` and the filtered `values_consumed()` of stateful pools take the number of attempts before rejecting from the engine too.

The pytest plugin gains `--hegel-setting KEY=VALUE`, which sets any setting by its `hegel.toml` key with the value spelled as in `hegel.toml` (`--hegel-setting test_cases=1000`, `--hegel-setting suppress_health_check=too_slow,filter_too_much`), and may be repeated. The engine parses every option's value, including those of `--hegel-seed`, `--hegel-verbosity`, `--hegel-derandomize` and `--hegel-database`, so a bad value is a usage error naming what the setting accepts. This changes what some values mean: `--hegel-seed` takes a seed from 0 to 2**64 - 1, or `none`, so a negative seed such as `--hegel-seed=-1`, or one of 2**64 or more, is now a usage error, where it used to be reduced modulo 2**64; and `--hegel-database=default` and `--hegel-database=disabled` now select the engine's default database and turn the database off, as in `hegel.toml`, rather than naming directories called `default` and `disabled` (`--hegel-database=./default` still names one). `--hegel-verbosity` therefore no longer lists its levels as argparse choices: a bad level is still rejected before any test runs, but by the engine, with the engine's message, and the levels are named in the option's help text.

Stacked `@hegel.reproduce_failure` decorators now replay every blob, where only the topmost was replayed before. Every failure the blobs reproduce is raised, together as a `MultipleFailures` when there is more than one, and `DidNotReproduce` is raised only when none of them fails, so the decorators printed for each failure of a `report_multiple_failures` run can all be pasted onto the test at once. `hegel.run()`'s `reproduce_blob` likewise takes a collection of blobs as well as one, and the decorator now records its blobs in a list, `hegel_reproduce_blobs`, in place of the `hegel_reproduce_blob` attribute.

`FailedHealthCheck` now says which health check failed: its new `health_check` attribute is the `HealthCheck` member, and its message ends by naming it as Python spells it, with the `suppress_health_check` value that suppresses it.

A `tc.assume(False)` or `tc.reject()` in an explicit test case (`@hegel.explicit_test_case`) still raises `InvalidArgument`, now with the message `Explicit test cases must not be rejected`, shared with the other Hegel libraries. `tc.repeat()` in an explicit test case now runs its body once, as in the other Hegel libraries, where it used to raise `InvalidArgument` saying `draw_silent` is not supported, so a rejection inside it gets that message too. Under `Verbosity.quiet` a failing explicit test case no longer attaches its drawn values and notes to the exception as a `Falsifying example:` note, as a generated failure never did.

A failure's caveat (such as `unconfirmed failure ...` for a failure that did not reproduce on every replay) is now attached to the exception as a note starting `note: `, as the other Hegel libraries print it.

Printing a drawn value is now a capability of the generator that drew it, as in the Rust library, rather than a `repr` applied to whatever value comes out. `Generator.do_draw_and_print` no longer defaults to drawing with `do_draw` and printing the value's `repr`: a `Generator` subclass that implements only `do_draw` cannot print, and drawing it with `tc.draw`, directly or inside a structure such as `gs.lists(...)`, raises the new `hegel.NotPrintable` (an `InvalidArgument`, so it ends the run as a usage error). `tc.draw` now runs the generator's printing on every test case, not only on the replay of a failure, so this is raised on the first test case that draws one, and a draw nested inside a composite needs a printable generator too. To fix such a generator, implement `do_draw_and_print` (a subclass that does gets `do_draw` for free), append `.print_as_repr()` for the old behaviour, use `.print_with(...)`, or draw it with `tc.draw_silent`, which still accepts any generator:

```python
class Coords(gs.Generator[tuple[int, int]]):
    def do_draw_and_print(self, tc, printer):
        with printer.group("Coord(", ")"):
            x = tc.draw_and_print(gs.integers(0, 9), printer)
            printer.separator()
            y = tc.draw_and_print(gs.integers(0, 9), printer)
        return (x, y)
```

`tc.draw(gs.lists(Coords(), min_size=1))` reports `xs = [Coord(0, 0)]`. Every built-in generator is printable and prints as before, except that an empty set now prints as `set()` and an empty frozenset as `frozenset()`, where they printed as `{}` and `frozenset({})`, and the containers `from_type()` builds by converting a drawn value now print their elements with the elements' own generators, as a call: `tuple[Foo, ...]` prints as `tuple([...])`, `deque[Foo]` as `deque([...])`, `OrderedDict[str, Foo]` as `OrderedDict({...})`, and a `TypedDict` named `Movie` as `Movie({...})`, where they printed the `repr` of the finished value, losing a `print_with` registered for `Foo`. A draw that raises now prints nothing at all: the half-drawn value and the draws made inside it are discarded, where the report used to list the inner draws under a `<KeyError raised while drawing from ...>` line.

`PrettyPrinter` gains `repr(value)` (the preferred spelling of `pretty_print(value, printer)`), a `group(open, close)` context manager, `seq`, `separator`, and `deferred()`, which opens a region anchored at the current position that can be printed into later, from any thread, so a generator can describe a value that is only known after it is drawn. `PrettyPrinter.comment(text)` now renders `  # text`, where it used to render the text with no comment marker. `PrettyPrinter.value()` is no longer public: render a `Document` with `Document.finish()`. `Document` and `Speculation` are now exported from `hegel`.

The notes a failing test's exception carries no longer begin with a `Falsifying example:` line. The first note is now the drawn values and notes themselves, one per line, as the other Hegel frontends print them; it is followed, as before, by a `note:` line with the engine's caveat when there is one and by the `reproduce_failure` reproducer when there is one. A test that matched the notes on that heading needs updating.

Under `verbosity="verbose"`, a test case that stops short now says why in the same words as the other Hegel frontends: `Test case stopped: failed assumption` or `Test case stopped: out of data`. Cases that pass or fail print no line of their own after their drawn values (a failing one still prints its traceback), where they printed `Test case passed` and `Test case failed: <origin>`, and an explicit test case is no longer introduced with a `Running explicit test case:` line.

This release also replaces the `exclude_min` and `exclude_max` flags of `gs.floats()` with `min_value_exclusive` and `max_value_exclusive`, which take the bound directly, as the other Hegel frontends do:

```python
# before
gs.floats(min_value=0.0, max_value=1.0, exclude_min=True, exclude_max=True)

# after
gs.floats(min_value_exclusive=0.0, max_value_exclusive=1.0)
```

Giving both `min_value` and `min_value_exclusive` (or both `max_value` and `max_value_exclusive`) is an `InvalidArgument`. An exclusive bound can no longer be set without a bound value, so that error no longer exists; the remaining validation (an exclusive `inf` minimum, an exclusive `-inf` maximum, or exclusive bounds on a single-point range) is unchanged, and its messages now name the new arguments. The `repr` of such a generator shows the exclusive form, `floats(min_value_exclusive=0.0)`.

`tc.repeat()` now introduces each repetition with a `# Repetition #N` note, a Python comment, where it used to print `// Repetition #N`. The engine now also decides how many repetitions run and is told which one failed (libhegel's new `hegel_repeat_t` functions) rather than the frontend drawing those choices itself; the choices are the same, so an existing reproduce blob still replays.

`StateMachine.teardown()` is removed. No other Hegel frontend has a per-test-case teardown hook on a state machine, and a fresh instance is made for every test case anyway: do any cleanup a machine needs in a rule or invariant, or run the machine from a `@hegel.test` body with `hegel.run_state_machine(tc, Machine)` and clean up after it there.

A concurrent machine's round header now quotes the group's name with double quotes (`---------------- Round 1: group "deposit" ----------------`), as the other Hegel frontends do, instead of Python's `repr` form.

`text(alphabet=...)` now accepts only what the engine's string generator accepts: a string or sequence of characters, or a `characters()` generator. Passing any other generator, such as `gs.sampled_from("abc") | gs.just("d")`, used to build the string in Python one character at a time; it now raises `InvalidArgument`. To build text from an arbitrary generator of characters, draw a list of them and join it, which draws and shrinks the same way:

```python
# before
gs.text(alphabet=gs.sampled_from("abc") | gs.just("d"))

# after
gs.lists(gs.sampled_from("abc") | gs.just("d")).map("".join)
```

A generator class of your own is now labelled with its qualified name (`mymodule.Coords`) instead of the shared `hegel.generator` label, so the engine can tell its draws apart from every other generator's when it shrinks and mutates them. A class that sets `label` itself keeps it. Failures saved by an earlier release under the shared label replay from the database as fresh test cases, and a `reproduce_failure` blob recorded for a test that draws such a generator no longer reproduces it.

## 0.2.2 - 2026-10-28

This release updates the bundled libhegel engine to 0.49.0.

## 0.2.1 - 2026-10-20

This release fixes the source distribution. PyPI rejected `hegeltest-0.2.0.tar.gz` because its metadata declared a `LICENSE` file at the archive's root while maturin, which builds a Cargo workspace member's sdist with the whole workspace as its root, had put the file under `frontends/python/`, so 0.2.0 is on PyPI as wheels only. `pyproject.toml` now lists the license file explicitly in `license-files` and gives the license as the SPDX expression `MIT`, which puts `LICENSE` at the root of the sdist and the license in the metadata's `License-Expression` field. Building from the sdist now requires maturin 1.9.3 or newer.

## 0.2.0 - 2026-10-20

This release makes `text()`, `characters()`, `from_regex()` and `from_lark()` follow this Python's Unicode version (`unicodedata.unidata_version`) rather than the engine's newer tables. A `characters()` or `text()` generator with category constraints no longer draws a character this Python does not know yet or puts in another category, and a `from_regex()` draw, or a grammar terminal's, always matches the pattern as this Python's `re` reads it: `\w`, `\d` and `\s`, word boundaries and `re.IGNORECASE` all follow the host's tables.

`from_regex()` also accepts the `\z` end-of-string anchor that `re` gained in Python 3.14.

The API's docstrings are much expanded, for the new documentation site at <https://hegel.readthedocs.io>: every generator documents its arguments, its defaults, how its values shrink and what is an invalid argument. Three descriptions were also wrong and are corrected: `uuids()` without `version=` generates arbitrary 128-bit values, most of which have no RFC 4122 version; invariants run on the initial and final state and only sometimes in between, unless `always_run=True`; and `Settings()` resolves the default profile from `set_default_profile` first, then `HEGEL_DEFAULT_PROFILE`, then `hegel.toml`.

`Settings` gains `stop_on_first_failure` (default `True`), the engine setting the Rust library already exposes: with it off a run uses its whole `test_cases` budget before reporting. A `hegel.toml` profile setting it was previously ignored by Python runs.

`label_from_name` and `combine_labels` are no longer exported from `hegel.generators`. They are implementation details, not part of the public API.

## 0.1.0 - 2026-10-19

Initial release of Hegel for Python: bindings to libhegel (`hegeltest-c` 0.45.13)
through a pyo3 extension module, the `hegel` API (`@hegel.test`, generators,
explicit test cases, failure reproduction, sequential and concurrent stateful
testing) and a pytest plugin. Published with wheels for Apple-silicon macOS
only, ahead of the release automation that builds every platform.
