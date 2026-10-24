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
