RELEASE_TYPE: minor

This release changes how failing test cases print drawn values. A drawn value now prints as a `let` binding in OCaml syntax, instead of as `name = <s-expression>`, with the location of the draw as a `(* file:line *)` comment when it has one (always on OxCaml, and with `~loc` elsewhere):

```
(* before *)
xs = (1 2 3)
pair = (7 hello)
p = ((x 1) (y 2))

(* after *)
let xs = [1; 2; 3] in
let pair = (7, "hello") in
let p = { x = 1; y = 2 } in
```

Printing is now the generator's job: a printable generator prints each value while it draws it, through the engine's printer. A container prints element by element, each element as its own generator prints it, so a custom-printed element prints the same inside a list, an option, a tuple or a derived record as it does alone. A value that `filter`, a unique list, a set or an association list rejects is never printed, and neither is anything a draw printed before it raised. Strings are always quoted, options print as `Some x`, derived records and variants print as `{ x = 1 }` and `Node (Leaf 1, Leaf 2)`, and a value too long for one line wraps one element per line. Steps of a state machine print their draws the same way, indented under `Step N: name`.

New ways to make a generator printable:

- `map_as_call name f gen` maps `gen` with `f` and prints the result as a call on the value `gen` drew, `Key.of_int 3`, parenthesized where it is an argument: `Some (Key.of_int 3)`.
- `printable_composite ?label ?sexp_of body` is a hand-written printable generator: `body tc printer` draws the value's parts and prints it to `printer` as it goes, drawing a part that should appear in the output with the new `draw_and_print tc printer gen`.
- `flat_map f gen` is now printable whenever the generators `f` returns are, and prints the value the inner generator draws: its type is `('a -> ('b, 'q) generator) -> ('a, 'p) generator -> ('b, 'q) generator`. It used to always return an `unprintable` generator, so code that annotates a `flat_map` result as `unprintable` needs updating (code that wraps it in `with_printer` keeps working, and still prints the s-expression).

The new `Printer.Out` module is the printer these generators print to, a handle on the engine's printer for the value being drawn: `text`, `breakable`, `if_break`, `hard_break`, `begin_group` / `end_group` and the scoped `group`, `separator` and `seq`, `shift_indent`, `comment`, `reflow`, `doc` and `value` to print a `Printer.doc`, `sexp`, `argument` / `parens_if_argument` for argument position, speculation (`speculate`, `begin_speculation`, `commit_speculation`, `abort_speculation`) to discard output, `deferred` for a region filled in later (such as a value that draws as it is used), `should_print`, `noop`, and `with_document` to lay a sub-document out to a string. A write to a region whose test case has ended does nothing. `Printer` also gains the `if_break`, `hard_break`, `comment` and `shift_indent` documents.

```ocaml
let keys = map_as_call "Key.of_int" Key.of_int (integers ~min_value:0 ())
(* draw tc (lists keys ()) prints: let draw_1 = [Key.of_int 3; Key.of_int 0] in *)

let bags =
  printable_composite (fun tc printer ->
    Printer.Out.parens_if_argument printer (fun printer ->
      Printer.Out.group printer ~indent:2 "" "" (fun printer ->
        Printer.Out.text printer "Bag.of_list";
        Printer.Out.breakable printer " ";
        Bag.of_list (draw_and_print tc printer (lists (integers ()) ())))))
(* prints: let draw_2 = Bag.of_list [1; -2] in *)
```

Printing the finished value remains available as an escape hatch for a generator whose value is easier to describe once it is built: `with_ocaml_printer` attaches a printer that returns a `Printer.doc`, printed as OCaml syntax, and `with_printer` (or `~sexp_of_state` for a state machine's state) prints the s-expressions it returns, as before. A state machine run takes `?print_state` to print its state as a `Printer.doc` instead; a `module%hegel_state_machine` that binds `print_state` uses it.

```ocaml
draw tc (with_ocaml_printer Printer.(list int) (map (fun n -> [ n ]) (integers ())))
(* prints: let draw_1 = [3] in *)

draw tc (with_printer [%sexp_of: int list] (map (fun n -> [ n ]) (integers ())))
(* prints: let draw_1 = (3) in *)
```

Generators keep their `printable` / `unprintable` parameter, `Generators.printer` still returns a generator's s-expression printer (`<abstr>` for a `map_as_call`, a printable `flat_map` or a `printable_composite` without `~sexp_of`), and `[@@deriving hegel_generator]` still defines `sexp_of_<t>`, but no longer prints with it. A derived generator prints each field as the field's generator prints it, so every field's generator must now be printable: a type whose `[@hegel.generator]` override is unprintable (built with `map`, say, or a `flat_map` returning one) no longer compiles, and neither does a derived type alias of an unprintable generator. Make the override printable with `with_ocaml_printer`, `map_as_call` or `with_printer`. `Printer.to_string` lays a `Printer.doc` out to a string.

A few lower-level interfaces change with this. The doc-hidden `make_hash_tables_printed` takes `~wrap`, the names of the functions a drawn table's association list is wrapped in (`~wrap:[ "Hashtbl.of_seq"; "List.to_seq" ]`), instead of `~print_of_pairs`. The doc-hidden `Generators.Ppx_internal.draw` and `Generators.Ppx_internal.with_sexp_of` are removed (derived generators draw their fields with `draw_and_print`), and the body given to `Ppx_internal.composite_with_label` takes the printer after the test case, so code generated by an older `ppx_hegel_generator` needs rebuilding.

Floats print as the shortest literal that reads back as the same float, without an exponent from `1e-5` up to `1e16` (`100.`, `1000000.`), and with one beyond (`1e+16`). Strings escape ASCII control characters as `%S` does (`"\b"`), and print the characters Rust's `Debug` escapes as `\u{...}`: control and format characters such as the bidirectional override U+202E, line and paragraph separators, combining marks, and unassigned code points. A negative atom of an s-expression, or a `Printer.of_to_string` text that is not a single token, is parenthesized as an argument (`Some (-5)`), and a single token is not (`Some 42`).

An application of a generated function prints its arguments, followed by its result as `returns` prints it: `f 3 = [1; 2]`, or `f 3 = <fun>` when the result is itself a function. The new `?print_arg` (`?print_arg1`, ... for `functions2` and `functions3`) prints an argument as a `Printer.doc`; `~sexp_of_arg` still prints it as an s-expression, and an argument with neither prints as `<abstr>`, as every value without a printer does (it used to be `<opaque>`). The arguments of `functions2` and `functions3` print curried, `g 3 true = 10`, rather than as one list, `g (3 true) = 10`.

`text` and `characters` now leave character-set validation to the engine. `~categories` may name a category that contains the surrogates, such as `"C"`, and the engine drops the surrogates from it; constraints that leave no characters at all (`~categories:["Cs"]`) raise `Usage_error` when the generator is drawn from rather than when it is built.

The failure report is laid out as in hegel-rust: the drawn values and notes, the exception, `note: <caveat>` when the engine attaches one, and how to replay the failure, with no `--- Failure ---` frame and no color:

```
let n = 50 in
Exception: File "my_tests.ml", line 5, characters 2-8: Assertion failed

To reproduce this failure, add this attribute to the let%hegel_test:
    [@@failure_blobs [ "AAEAAAAACgEAAAAy" ]]
```

A plain `run_hegel_test` caller is told the `~failure_blobs` argument to pass instead, and a `Hegel.main` binary the `--failure-blob` flag. A run with several distinct failures starts with `Property-based test failed with N distinct failures.` and reports each after a blank line. Under `Quiet` verbosity only the replay lines print. `HEGEL_COLOR` no longer has any effect.

A run that ends without a verdict on the property now raises an exception saying why, chosen by the engine's classification of the error, instead of `Failure` with the engine's message:

- `Failed_health_check { check; message }` when a health check failed, naming the check.
- `Flaky` when the test behaved nondeterministically under the `Error` `nondeterminism_strictness`.
- `Unsatisfiable` when no valid test case was generated.
- `Run_error` for any other run error.

A run with several distinct failures raises `Multiple_failures` with their exceptions instead of `Failure "N failures found!"`, a `~failure_blobs` replay that reproduces nothing raises `Did_not_reproduce` instead of `Failure`, and calling `run_hegel_test` from inside a test raises `Usage_error`. Code matching `Failure` for these cases needs to match the new exceptions instead:

```ocaml
(* before *)
match run_hegel_test test with
| () -> ()
| exception Failure msg when String.starts_with ~prefix:"FailedHealthCheck" msg -> ...

(* after *)
match run_hegel_test test with
| () -> ()
| exception Failed_health_check { check; message } -> ...
```

`~failure_blobs` now replays every blob in the list, in turn, and reports every failure they reproduce; it used to replay only the first. `let%hegel_test` accepts several stacked `[@@failure_blobs ...]` attributes, which it used to reject as duplicates, and replays the blobs of all of them, so the attribute printed for each failure of a run can be pasted onto the test as it is. `Hegel.main` binaries accept `--failure-blob` more than once.

`Settings.t`'s `seed` field is now an `Unsigned.UInt64.t option` instead of an `int option`, so it holds any seed libhegel accepts, up to 2^64 - 1. A seed of 2^62 or more from `--seed`, `HEGEL_SEED` or `hegel.toml` used to wrap around (2^63 + 5 became 5). `Settings.create ~seed` still takes an `int`, and raises `Usage_error` for a negative one. Code that builds or reads the field needs to convert:

```ocaml
(* before *)
{ s with seed = Some 42 }

(* after *)
{ s with seed = Some (Unsigned.UInt64.of_int 42) }
```

`Settings.database` has a new `Default_location` case, the database at `.hegel` whatever the profile says (`database = "default"` in `hegel.toml`), and `Unset` now keeps the default profile's database rather than forcing `.hegel`: under the `ci` and `workload` profiles, which disable the database, a test whose settings say `Unset` now has none, as with `Database::unset()` in C++ and `Database.unset()` in Java. Settings read from libhegel (`Settings.default ()`, `Settings.from_profile`) report the default database as `Default_location` rather than `Unset`, and `--database default` on a `Hegel.main` binary selects it.

The default database key of a test with a source location is now `file::function_name`, as in the C++ and TypeScript frontends, instead of `file:function_name`. Failing examples saved under the old key are not replayed; they are found again by generation.

`Settings.t` has a new `unbounded_choices` field, also set by `HEGEL_UNBOUNDED_CHOICES`, the `unbounded_choices` key of `hegel.toml` and the `--unbounded-choices` flag of a `Hegel.main` binary. Setting it lets a test case make any number of choices, rather than stopping it as an overrun once it has made 2^20. A `Hegel.main` binary runs its one test case with the `Too_slow` and `Test_cases_too_large` health checks suppressed, as hegel-rust's `#[hegel::main]` binaries do, and no longer lifts the choice limit on its own.

`Hegel.engine_version ()` returns the version of the libhegel engine the library drives. The settings documentation now lists every `HEGEL_*` settings variable the engine reads, including `HEGEL_VERBOSITY`, `HEGEL_PHASES`, `HEGEL_SUPPRESS_HEALTH_CHECK`, `HEGEL_REPORT_MULTIPLE_FAILURES` and `HEGEL_BACKEND`, which libhegel now applies.

New test-case operations:

- `reject tc` discards the current test case, like `assume tc false`, and has any result type, so it can end a branch of a `match`.
- `repeat tc body` runs `body` for as many iterations as the engine wants and then ends the test case, printing each iteration's output under a `// Repetition #n` note.
- `set_worker tc i` attributes a test case's output to concurrent worker `i`, so a test that draws from several threads through `clone` can label each thread's lines in the failure report.
- `clone` now works in an explicit example (`[@@example ...]`), where it used to raise `Usage_error`: the clone draws the example's values, in sequence with the test case it was cloned from, so a test that draws from several threads runs on its examples too. `set_worker` does nothing there, as an example has no output to attribute.

New generators for recursive data:

- `recursive ?max_depth ?max_leaves leaf branch` generates trees and other recursive values. `branch` receives the generator of sub-values and builds one level of structure from it; the engine decides where the value branches, bounds its depth and size, and shrinks it toward a single leaf.
- `deferred ()` returns a printable generator together with the function that defines it later, so generators can refer to each other, or to themselves, before they exist.

```ocaml
type tree =
  | Leaf of int
  | Node of tree list

let trees =
  recursive
    (map_as_call "Leaf" (fun n -> Leaf n) (integers ()))
    (fun subtrees -> map_as_call "Node" (fun children -> Node children) (lists subtrees ()))
```

`[@@deriving hegel_generator]` now supports recursive and mutually recursive types. A variant whose constructors refer to the type itself, such as `type tree = Leaf of int | Node of tree * tree`, is generated with `recursive`, its non-recursive constructors as the leaves; other recursive types are generated through `deferred`.

`let%hegel_test` takes explicit examples: each `[@@example { n = 0; s = "" }]` attribute runs the test once, before generation, with every `let n = draw tc gen` binding taking the example's value for `n`. A failing example is reported with the values it gave. An example must not be rejected: one that `assume` or `reject` rejects raises `Usage_error` ("Explicit test cases must not be rejected, ..."). In an example, `repeat` runs its body once. Examples run in the `Explicit` phase, which used to do nothing, and do not run when the test replays failure blobs.

```ocaml
let%hegel_test round_trips tc =
  let n = draw tc (integers ()) in
  let s = draw tc (text ()) in
  assert (parse (print n s) = (n, s))
[@@example { n = 0; s = "" }]
```

New generators:

- `uuids ?version ()` generates UUIDs as lowercase hyphenated strings, of the given RFC 4122 version when one is given.
- ``floats ~width:`W32`` generates floats that are exactly representable as 32-bit floats.
- `from_regex` takes the character filtering options of `text` (`?codec`, `?min_codepoint`, `?max_codepoint`, `?categories`, `?exclude_categories`, `?include_characters`, `?exclude_characters`, or `?alphabet`), which restrict every character of the drawn string.
- `lists ~unique_by:key` generates lists whose elements have distinct keys.
- `arrays` generates arrays, which print as `[|1; 2|]`.
- `sets (module S) elements` generates sets of a `Set.S` module. A drawn set prints as `Set.of_list [1; 2]`, or as `Int_set.of_list [1; 2]` given `~module_name:"Int_set"`, as a first-class module does not carry its name.
- `permutations values` generates permutations of a list, shrinking toward its original order.
- `subsequences ?min_size ?max_size values` generates sublists of a list that keep its order, and `samples ?min_size ?max_size ?replacement values` lists of elements chosen from it, with replacement by default or each element at most once with `~replacement:false`. Both shrink toward fewer elements from earlier in the list.
- `unit ()` generates `()`, printed as `()`, and `[@@deriving hegel_generator]` now derives a `unit` field from it.
- `tuples5` to `tuples8` generate tuples of 5 to 8 elements.
- The new optional `hegel.zarith` library, built when zarith is installed, has `Hegel_zarith.integers` for zarith integers of any magnitude. A missing bound is taken to be 2^127 values beyond the other bound, or beyond zero when that is further.

`text` and `binary` without `~max_size` now take libhegel's default maximum, 100 characters or bytes (or `min_size + 100` when `~min_size` is above 100), as every other Hegel frontend does. They used to have no maximum at all. Pass `~max_size` for longer strings.

`filter` now takes its number of attempts before rejecting the test case (3) from libhegel, so it rejects as often as a filter in any other frontend.

Tuples, arrays, permutations, unique lists, association lists, hash tables and `filter` attempts now mark their draws for libhegel with the same labels as in the other frontends, so the engine shrinks and mutates them the same way.

`Hegel.main` binaries now parse their command line as hegel-rust's `#[hegel::main]` binaries do: `--some-setting VALUE` (or `--some-setting=VALUE`) sets the `hegel.toml` key `some_setting`, and libhegel parses the value, so every setting libhegel has gets a flag and the flags and `hegel.toml` always agree. That adds `--phases`, `--report-multiple-failures`, `--stop-on-first-failure`, `--show-statistics`, `--print-blob`, `--nondeterminism-strictness` and `--database-key` to the existing flags. `--test-cases` is rejected with an error saying why, as a `Hegel.main` binary always runs one test case. An unknown flag or a bad value now prints the error and the usage and exits with status 2 (it was cmdliner's 124), and the `hegel` package no longer depends on cmdliner. `--seed` no longer accepts a negative number, as libhegel's seeds are unsigned, and `--suppress-health-check ""` now suppresses none.

New generator `from_lark grammar ()` generates strings of a grammar written in the syntax of the [Lark](https://lark-parser.readthedocs.io/en/stable/grammar.html) parsing library, as hegel-rust's `from_lark` does. `?start` picks the start rule, `?max_size` and `?max_depth` bound the strings, `?import_path` is the directory `%import`ed grammars are read from (Lark's bundled `common`, `python`, `unicode` and `lark` grammars are always available), the character filtering options of `text` restrict the characters the terminals draw, and `?explicit` draws a terminal's values from a generator of your own:

```ocaml
let%hegel_test greets tc =
  let s =
    draw tc
      (from_lark
         {|
           start: "hello " NAME
           %declare NAME
         |}
         ~explicit:[ "NAME", from_regex "[A-Z][a-z]+" () ]
         ())
  in
  assert (String.starts_with ~prefix:"hello " s)
```

With `Settings.Verbose` or `Settings.Debug`, a test case that stops short now says why, as the other Hegel frontends do: `Test case stopped: failed assumption` or `Test case stopped: out of data`.

This release also replaces the `?exclude_min` and `?exclude_max` flags of `floats` with `?min_value_exclusive` and `?max_value_exclusive`, which take the bound directly, as the other Hegel frontends do:

```ocaml
(* before *)
floats ~min_value:0.0 ~max_value:1.0 ~exclude_min:true ~exclude_max:true ()

(* after *)
floats ~min_value_exclusive:0.0 ~max_value_exclusive:1.0 ()
```

Giving both `~min_value` and `~min_value_exclusive` (or both `~max_value` and `~max_value_exclusive`) raises `Usage_error`. `floats` also gains `?allow_subnormal`: subnormal ("denormalised") values are drawn whenever the bounds admit any, as before, and `~allow_subnormal:false` keeps them out, for code that may run with flush-to-zero floating point. Asking for subnormals with bounds that exclude them all, or excluding them from a range that holds nothing else, raises `Usage_error`.

`repeat` now introduces each iteration with a `(* Repetition #n *)` note, an OCaml comment, where it used to print `// Repetition #n`. The engine now also decides how many iterations run and is told which one failed (libhegel's new `hegel_repeat_t` functions) rather than the frontend drawing those choices itself; the choices are the same, so an existing failure blob still replays.

`Stateful.run`, `Stateful.run_concurrent` and the `run` of a `module%hegel_state_machine` or `module%hegel_concurrent_state_machine` no longer take `?print_state` or `?sexp_of_state`, and a module's `print_state` or `sexp_of_state` binding no longer has any effect: the `state = …` lines are gone from the failure report, as in the other Hegel frontends. Note what matters from a rule or invariant instead. The report now reads as it does elsewhere: each step prints as `Step 1: push {` with its draws and notes indented beneath it and a closing `}` (a concurrent worker's as `Rule: push {`), a rule that stops on a failed assumption notes `Rule stopped early due to violated assumption.` indented under its heading, the rule or invariant a failure escaped from is named right before the failure (`Rule pop failed:`, `Invariant short failed:`) instead of `Invariant short violated after step 2.`, the invariant checks on the initial and final state are announced (`Checking invariants on the initial state.`, `Checking invariants on the final state.`) when the machine has invariants, and a concurrent machine always reports its `Concurrency level: N`, even at one worker.

A draw given a `~label` now prints numbered from its first use, `let x_1 = ... in` then `let x_2 = ... in`, where it printed `let x = ... in` for every use. A name the PPX captures from a single `let x = [%draw ...]` still prints bare, as does the first use of a name the PPX sees reused; an unnamed draw still prints as `draw_1`, `draw_2`. Each name counts on its own, in the order the draws run, and a number already taken by another name is skipped: a bare `x_2` followed by a reused `x` prints `x_2` then `x_3`. The counts start afresh in each repetition of `repeat` and each step of a stateful test, and a clone shares its parent's counts. `Hegel.Internal.draw_display_name` follows the same rule.

Every draw made through a test case, printed or silent, is now wrapped in an engine span labelled with its generator's label, and the compound generators (`map`, `bind`, `tuple2`, `lists`, `composite`, `printable_composite` and the rest) open no spans of their own around their component draws, where each used to open one; `filter` still opens a span around every attempt. This makes the choice sequence line up with every other Hegel frontend, so `composite` and `printable_composite` are now labelled `hegel.composite` rather than `hegel.fixed_dicts`, and a generator derived by `ppx_hegel_generator` keeps its type-name label. A `reproduce_blob` recorded by an earlier release no longer reproduces its failure, and failures saved in the database replay as fresh test cases.
