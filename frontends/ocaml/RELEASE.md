RELEASE_TYPE: minor

This release changes how failing test cases print drawn values: a value drawn from a printable generator now prints as OCaml syntax instead of as an s-expression.

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

Strings are always quoted, options print as `Some x`, derived records and variants print as `{ x = 1 }` and `Node (Leaf 1, Leaf 2)`, and a value too long for one line wraps one element per line. Structural generators (`lists`, `tuples2`, `optional`, `one_of`, `filter`, derived generators, ...) print what they drew, each part as its own generator prints it.

Floats print as the shortest literal that reads back as the same float, without an exponent from `1e-5` up to `1e16` (`100.`, `1000000.`), and with one beyond (`1e+16`). Strings escape ASCII control characters as `%S` does (`"\b"`), and print the characters Rust's `Debug` escapes as `\u{...}`: control and format characters such as the bidirectional override U+202E, line and paragraph separators, combining marks, and unassigned code points. A negative atom of an s-expression, or a `Printer.of_to_string` text that is not a single token, is parenthesized as an argument (`Some (-5)`), and a single token is not (`Some 42`).

An application of a generated function prints its arguments as before, through `~sexp_of_arg`, and its result as `returns` prints it: `f 3 = [1; 2]`, or `f 3 = <fun>` when the result is itself a function.

A printer you give with `with_printer`, or with `~sexp_of_arg` or `~sexp_of_state`, prints the s-expressions it returns, as before:

```ocaml
draw tc (with_printer [%sexp_of: int list] (map (fun n -> [ n ]) (integers ())))
(* prints: draw_1 = (3) *)
```

The printing API is unchanged: generators keep their `printable` / `unprintable` parameter, `Generators.printer` still returns a generator's s-expression printer, and `[@@deriving hegel_generator]` still defines `sexp_of_<t>`. A field of a derived type whose generator has no printer (an `[@hegel.generator]` override built with `map`, say) prints as that s-expression, inside the OCaml syntax of the rest of the value. The new `Hegel.Printer` module has the documents these values print as, and `Printer.to_string` to lay one out.

`text` and `characters` now leave character-set validation to the engine. `~categories` may name a category that contains the surrogates, such as `"C"`, and the engine drops the surrogates from it; constraints that leave no characters at all (`~categories:["Cs"]`) raise `Usage_error` when the generator is drawn from rather than when it is built.

The failure report is laid out as in hegel-rust: the drawn values and notes, the exception, `note: <caveat>` when the engine attaches one, and how to replay the failure, with no `--- Failure ---` frame and no color:

```
n = 50
Exception: File "my_tests.ml", line 5, characters 2-8: Assertion failed

To reproduce this failure, add this attribute to the let%hegel_test:
    [@@failure_blobs [ "AAEAAAAACgEAAAAy" ]]
```

A plain `run_hegel_test` caller is told the `~failure_blobs` argument to pass instead, and a `Hegel.main` binary the `--failure-blob` flag. A run with several distinct failures starts with `Property-based test failed with N distinct failures.` and reports each after a blank line. `HEGEL_COLOR` no longer has any effect.

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

`Settings.t`'s `seed` field is now an `Unsigned.UInt64.t option` instead of an `int option`, so it holds any seed libhegel accepts, up to 2^64 - 1. A seed of 2^62 or more from `--seed`, `HEGEL_SEED` or `hegel.toml` used to wrap around (2^63 + 5 became 5). `Settings.create ~seed` still takes an `int`, and raises `Usage_error` for a negative one. Code that builds or reads the field needs to convert:

```ocaml
(* before *)
{ s with seed = Some 42 }

(* after *)
{ s with seed = Some (Unsigned.UInt64.of_int 42) }
```

`Settings.t` has a new `unbounded_choices` field. Setting it lets a test case make any number of choices, rather than stopping it as an overrun once it has made 2^20. `Hegel.main` now sets it instead of suppressing the `Test_cases_too_large` health check.

New test-case operations:

- `reject tc` discards the current test case, like `assume tc false`, and has any result type, so it can end a branch of a `match`.
- `repeat tc body` runs `body` for as many iterations as the engine wants and then ends the test case, printing each iteration's output under a `// Repetition #n` note.
- `set_worker tc i` attributes a test case's output to concurrent worker `i`, so a test that draws from several threads through `clone` can label each thread's lines in the failure report.

New generators for recursive data:

- `recursive ?max_depth ?max_leaves leaf branch` generates trees and other recursive values. `branch` receives the generator of sub-values and builds one level of structure from it; the engine decides where the value branches, bounds its depth and size, and shrinks it toward a single leaf.
- `deferred ()` returns a printable generator together with the function that defines it later, so generators can refer to each other, or to themselves, before they exist.

```ocaml
type tree =
  | Leaf of int
  | Node of tree list
[@@deriving sexp_of]

let trees =
  recursive
    (with_printer sexp_of_tree (map (fun n -> Leaf n) (integers ())))
    (fun subtrees ->
      with_printer sexp_of_tree (map (fun children -> Node children) (lists subtrees ())))
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
- `sets (module S) elements` generates sets of a `Set.S` module.
- `permutations values` generates permutations of a list, shrinking toward its original order.
- `tuples5` to `tuples8` generate tuples of 5 to 8 elements.
- The new optional `hegel.zarith` library, built when zarith is installed, has `Hegel_zarith.integers` for zarith integers of any magnitude. A missing bound is taken to be 2^127 values beyond the other bound, or beyond zero when that is further.

`text` and `binary` without `~max_size` now take libhegel's default maximum, 100 characters or bytes (or `min_size + 100` when `~min_size` is above 100), as every other Hegel frontend does. They used to have no maximum at all. Pass `~max_size` for longer strings.

`filter` now takes its number of attempts before rejecting the test case (3) from libhegel, so it rejects as often as a filter in any other frontend.

Tuples, arrays, permutations, unique lists, association lists, hash tables and `filter` attempts now mark their draws for libhegel with the same labels as in the other frontends, so the engine shrinks and mutates them the same way.

`Hegel.main` binaries now have a flag for every setting: `--phases`, `--report-multiple-failures`, `--stop-on-first-failure`, `--show-statistics`, `--print-blob`, `--nondeterminism-strictness` and `--database-key` join the existing ones. Each flag is named after its `hegel.toml` key and takes the values `hegel.toml` takes, which libhegel parses, so the flags and `hegel.toml` always agree. `--seed` no longer accepts a negative number, as libhegel's seeds are unsigned, and `--suppress-health-check ""` now suppresses none.

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
