RELEASE_TYPE: minor

This release changes how failing test cases print drawn values: they now print as OCaml syntax instead of s-expressions, and every generator can be drawn with `draw`, whether or not it has a printer.

```
(* before *)
xs = (1 2 3)
pair = (7 hello)
p = ((x 1) (y 2))

(* after *)
xs = [1; 2; 3]
pair = (7, "hello")
p = { x = 1; y = 2 }
```

Strings are always quoted, options print as `Some x`, derived records and variants print as `{ x = 1 }` and `Node (Leaf, 3)`, and a value too long for one line wraps one element per line. Structural generators (`lists`, `tuples2`, `optional`, `one_of`, `filter`, `flat_map`, derived generators, ...) print what they drew. Generators whose output type is yours (`map`, `just`, `sampled_from`, `composite`) have no way to show it, so their values print as `<abstr>`, as in the OCaml toplevel, until you give them a printer.

A printer is now an `'a -> Printer.doc`, built with the new `Hegel.Printer` module rather than returning a `Sexplib0.Sexp.t`. `Printer` has printers for the standard types (`Printer.int`, `Printer.(list string)`, ...), builders for your own types (`Printer.constructor`, `Printer.record`, `Printer.tuple`), `Printer.of_to_string` for a type that has a `to_string`, and `Printer.of_sexp` for one that has a `sexp_of_t`:

```ocaml
(* before *)
draw tc (with_printer [%sexp_of: int] (map succ (integers ())))

(* after *)
draw tc (with_printer Printer.int (map succ (integers ())))
draw tc (with_printer (Printer.of_sexp Core.Date.sexp_of_t) my_dates)
```

The other changes you may need to make:

- The generator type is now `'a generator`. The `printable` / `unprintable` phantom parameter is gone, so `(int, printable) generator` becomes `int generator`, and `Generators.printer` is removed.
- `functions`, `functions2` and `functions3` take `?print_arg` / `?print_arg1` / `?print_arg2` / `?print_arg3` (each an `'a -> Printer.doc`) instead of `?sexp_of_arg*`. Their applications print as `f 1 "a" = 3`.
- `Stateful.run`, `Stateful.run_concurrent`, the runners `module%hegel_state_machine` generates, and the `Hegel_jane_async` runners take `?print_state` instead of `?sexp_of_state`, and a state machine module defines `print_state` instead of `sexp_of_state`.
- `make_characters`, `make_dates`, `make_times` and `make_datetimes` take `~print` instead of `~sexp_of`, and `make_hash_tables` takes `~print_of_pairs` (turning the printed association list into the printed table) instead of `~sexp_of_t`.
- `[@@deriving hegel_generator]` no longer defines `sexp_of_<t>`, needs no sexp converter for its field types, and no longer needs `[@sexp.opaque]` on a field. If you used the derived `sexp_of_<t>`, derive it with `ppx_sexp_conv` instead. `Hegel.Derive` no longer re-exports the `Sexplib0.Sexp_conv` converters.

`text` and `characters` now leave character-set validation to the engine. `~categories` may name a category that contains the surrogates, such as `"C"`, and the engine drops the surrogates from it; constraints that leave no characters at all (`~categories:["Cs"]`) raise `Usage_error` when the generator is drawn from rather than when it is built.

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

`~failure_blobs` now replays every blob in the list, in turn, and reports every failure they reproduce; it used to replay only the first. `Hegel.main` binaries accept `--failure-blob` more than once.

`Settings.t` has a new `unbounded_choices` field. Setting it lets a test case make any number of choices, rather than stopping it as an overrun once it has made 2^20. `Hegel.main` now sets it instead of suppressing the `Test_cases_too_large` health check.

New test-case operations:

- `reject tc` discards the current test case, like `assume tc false`, and has any result type, so it can end a branch of a `match`.
- `repeat tc body` runs `body` for as many iterations as the engine wants and then ends the test case, printing each iteration's output under a `// Repetition #n` note.
- `set_worker tc i` attributes a test case's output to concurrent worker `i`, so a test that draws from several threads through `clone` can label each thread's lines in the failure report.

New generators for recursive data:

- `recursive ?max_depth ?max_leaves leaf branch` generates trees and other recursive values. `branch` receives the generator of sub-values and builds one level of structure from it; the engine decides where the value branches, bounds its depth and size, and shrinks it toward a single leaf.
- `deferred ()` returns a generator together with the function that defines it later, so generators can refer to each other, or to themselves, before they exist.

```ocaml
let trees =
  recursive
    (map (fun n -> Leaf n) (integers ()))
    (fun subtrees -> map (fun children -> Node children) (lists subtrees ()))
```

`[@@deriving hegel_generator]` now supports recursive and mutually recursive types. A variant whose constructors refer to the type itself, such as `type tree = Leaf of int | Node of tree * tree`, is generated with `recursive`, its non-recursive constructors as the leaves; other recursive types are generated through `deferred`.

`let%hegel_test` takes explicit examples: each `[@@example { n = 0; s = "" }]` attribute runs the test once, before generation, with every `let n = draw tc gen` binding taking the example's value for `n`. A failing example is reported with the values it gave. Examples run in the `Explicit` phase, which used to do nothing.

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
- `sets (module S) elements` generates sets of a `Set.S` module.
- `permutations values` generates permutations of a list, shrinking toward its original order.
- `tuples5` to `tuples8` generate tuples of 5 to 8 elements.
- The new optional `hegel.zarith` library, built when zarith is installed, has `Hegel_zarith.integers` for zarith integers of any magnitude. A missing bound is taken to be 2^127 values beyond the other bound, or beyond zero when that is further.

`text` and `binary` without `~max_size` now take libhegel's default maximum, 100 characters or bytes (or `min_size + 100` when `~min_size` is above 100), as every other Hegel frontend does. They used to have no maximum at all. Pass `~max_size` for longer strings.

`filter` now takes its number of attempts before rejecting the test case (3) from libhegel, so it rejects as often as a filter in any other frontend.
