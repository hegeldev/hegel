# Getting Started with Hegel for OCaml

This guide walks you through the basics of installing Hegel and writing your first tests.

## Install Hegel

Add `hegel` to your opam environment:

```bash
opam install hegel
```

Hegel calls the native `libhegel` shared library and locates it automatically
at runtime; the opam release bundles it, so there is no separate install step.
See the [README](../README.md#install-hegel) for how the library is resolved
and the supported platforms.

## Write your first test

You're now ready to write your first test. Hegel works with whatever test 
framework your project already uses. This guide uses [Alcotest](https://github.com/mirage/alcotest).

Add `hegel` and `alcotest` to your dune test stanza. This guide also uses
`ppx_sexp_conv` to write value printers with `[%sexp_of: t]`, but it is optional.

```
(test
 (name my_tests)
 (libraries hegel alcotest)
 (preprocess (pps ppx_hegel_test ppx_sexp_conv)))
```

Then create a test file:

```ocaml
open Hegel

let%hegel_test integer_self_equality tc =
  let n = draw tc (integers ()) in
  assert (n = n)
;;

let () =
  Alcotest.run
    "my_tests"
    [ "properties", [ Alcotest.test_case "integer self equality" `Quick integer_self_equality ] ]
;;
```

Run it with `dune runtest`. You should see Alcotest report the test as `OK`.

Let's look at what's happening in more detail. The `let%hegel_test name tc =
body` extension defines `name` as a `unit -> unit` function that runs `body`
up to 100 times by default. The `tc` parameter is a test case, which represents the 
state of the Hegel test. The example above draws a random integer and checks 
that it is equal to itself.

Next, try a test that fails:

```ocaml
let%hegel_test integer_under_fifty tc =
  let n = draw tc (integers ()) in
  assert (n < 50)  (* this will fail! *)
;;
```

This test asserts that any integer is less than 50, which is obviously incorrect.
Hegel will find a test case that makes this assertion fail, and then shrink it
to find the smallest counterexample (`n = 50`). Add it to the Alcotest list
alongside the first test:

```ocaml
let () =
  Alcotest.run
    "my_tests"
    [ ( "properties"
      , [ Alcotest.test_case "integer self equality" `Quick integer_self_equality
        ; Alcotest.test_case "integer under fifty" `Quick integer_under_fifty
        ] )
    ]
;;
```

`dune runtest` will now report `integer under fifty` as failed, print the
drawn value as `n = 50` (named after the `let` binding), and exit non-zero.

To fix this test, you can constrain the integers you generate with `min_value`
and `max_value`:

```ocaml
let%hegel_test integer_under_fifty tc =
  let n = draw tc (integers ~min_value:0 ~max_value:49 ()) in
  assert (n < 50)
;;
```

Run `dune runtest` again. It should now pass.

## Use generators

Hegel provides a rich library of generators that you can use out of the box.
There are primitive generators, such as `integers`, `floats`, and `text`, and
generators for collections, such as `lists` and `tuples`, and generator combinators,
such as `map` and `flat_map`. All of these are documented in the `Generators`
module, and available unqualified after `open Hegel`.

For example, you can use `lists` to generate a list of integers:

```ocaml
let%hegel_test prepend_increases_length tc =
  let lst = draw tc (lists (integers ()) ()) in
  let initial_length = List.length lst in
  let extended = draw tc (integers ()) :: lst in
  assert (List.length extended > initial_length)
;;
```

This test checks that prepending an element to a random list of integers should
always increase its length.

You can also build up compound values by writing a plain function that takes the
test case and draws its parts. For example, say you have a `person` record that
you want to generate:

```ocaml
type person = { age : int; name : string }

let generate_person tc =
  let age = draw tc (integers ~min_value:0 ~max_value:120 ()) in
  let name = draw tc (text ()) in
  { age; name }
```

`generate_person` has type `test_case -> person`. It is *not* a `generator`
value, so you do not pass it to `draw`. You call it directly with the same `tc`,
and it draws its fields for you:

```ocaml
let%hegel_test person_has_nonnegative_age tc =
  let p = generate_person tc in
  assert (p.age >= 0)
;;
```

If you instead want a first-class `generator` value wrap the function with `composite`:

```ocaml
let person_generator = composite generate_person
(* person_generator : (person, unprintable) generator *)

let%hegel_test people_are_generatable tc =
  let p = draw_silent tc person_generator in
  assert (p.age >= 0)
;;
```

`composite` carries no printer (the value type is yours), so draw it with
`draw_silent`. To print it on a failing replay or to feed it into combinators
like `lists` that expect a *printable* element generator, write it with
`printable_composite` instead, which prints the value as it draws it (see
[Debugging failures](#debug-your-failing-test-cases)).

## Debug your failing test cases

When a test fails, Hegel replays the minimal failing example and prints a
report: the values you
drew (as OCaml syntax, named after their `let` binding), the exception, and a
copy-pasteable line that replays the exact case.

```ocaml
let%hegel_test reverse_is_identity tc =
  let xs = draw tc (lists (integers ()) ()) in
  assert (xs = List.rev xs)
;;
```

On failure this prints:

```
let xs = [0; 1] in
Exception: File "test/my_tests.ml", line 3, characters 2-8: Assertion failed

To reproduce this failure, add this attribute to the let%hegel_test:
    [@@failure_blobs [ "AXic..." ]]
```

The final lines show how to replay the exact case. Under `let%hegel_test` it is
a `[@@failure_blobs [ "..." ]]`
attribute you paste onto the test (one per failure; stacked attributes replay
every blob), while a plain `run_hegel_test` caller gets a
`~failure_blobs:[ "..." ]` argument to pass and a `Hegel.main` binary a
`--failure-blob` flag. Set `print_blob = false` in your `[@@settings ...]` to
omit them. A run that finds several distinct failures starts its report with
`Property-based test failed with N distinct failures.` and reports each one.

A value that is shadowed or drawn inside a loop is numbered (`x_1`, `x_2`, …):

```ocaml
let%hegel_test all_draws_below_ten tc =
  for _ = 1 to 3 do
    let x = draw tc (integers ()) in
    assert (x < 10)
  done
;;

(* On failure, prints:
     let x_1 = … in
     let x_2 = … in
     let x_3 = 10 in  *)
```

The same `let x` runs on each iteration, so Hegel disambiguates the draws as
`x_1`, `x_2`, `x_3` in draw order. You can override the name with `~label`:
`draw ~label:"y" tc (integers ())`.

Values print as OCaml syntax: lists as `[1; 2; 3]`, tuples as `(1, "a")`,
options as `Some 3`, records and variants of derived types as
`{ x = 1; y = 2 }` and `Circle 1.5`, and strings always quoted.

Printing is the generator's job: a generator prints each value as it draws
it, so a list prints element by element, each element as its own generator
prints it, and a value a `filter` rejects is never printed.

Some combinators hand the result type to your own code and so carry no printer:
`map`, `sampled_from`, `just` and `composite`. (`flat_map` prints whenever the
generators its function returns do.) Either draw it with
`draw_silent` (which prints nothing):

```ocaml
let parity = draw_silent tc (map (fun n -> n mod 2) (integers ()))
```

or make it printable. `map_as_call` prints a mapped value as a call on its
printed input, and `printable_composite` writes a generator that prints its
value as it draws its parts, through `Printer.Out`:

```ocaml
let key = draw tc (map_as_call "Key.of_int" Key.of_int (integers ()))
(* prints: let key = Key.of_int 3 in *)

let bags =
  printable_composite (fun tc printer ->
    Printer.Out.parens_if_argument printer (fun printer ->
      Printer.Out.group printer ~indent:2 "" "" (fun printer ->
        Printer.Out.text printer "Bag.of_list";
        Printer.Out.breakable printer " ";
        let items = draw_and_print tc printer (lists (integers ()) ()) in
        Bag.of_list items)))
(* a list of bags prints as: [Bag.of_list [1; 2]; Bag.of_list []] *)
```

When the value is easier to describe once it is built, attach a printer for
the finished value: `with_ocaml_printer` takes a `Printer` (OCaml syntax), and
`with_printer` takes an `'a -> Sexplib0.Sexp.t` and prints the s-expressions it
returns.

```ocaml
let pair =
  draw
    tc
    (with_ocaml_printer
       Printer.(pair int string)
       (map (fun n -> n, string_of_int n) (integers ())))

let parity =
  draw tc (with_printer [%sexp_of: int] (map (fun n -> n mod 2) (integers ())))
```

You can also attach your own debug information with `note`:

```ocaml
let%hegel_test remainder_below_divisor tc =
  let n = draw tc (integers ~min_value:0 ~max_value:1000 ()) in
  let r = n mod 7 in
  note tc (Printf.sprintf "n mod 7 = %d" r);
  assert (r < 7)
;;
```

## Assert with `require`

For a check with a custom message, use `require`, which raises `Failure msg`
when the condition is false:

```ocaml
require tc ~msg:"list must stay sorted" (is_sorted xs)
```

## Stateful testing

`Stateful` applies a random sequence of rules to the SUT and checks invariants
on it. First, create a state machine with `module%hegel_state_machine`. Mark
rules with `[@@rule]` and invariants with `[@@invariant]`. A rule takes the
test case and the state and updates the state in place. Rules may be weighted
with `[@@rule { weight = <weight> }]`. Every invariant is checked on the initial
and final states and sampled after intermediate steps, or after every step when
marked `[@@invariant { always_check = true }]`. Pass `?sexp_of_state` to 
`Stateful.run` to trace the model state. Draws are also printed with their 
`let`-bound name.

```ocaml
module%hegel_state_machine Stack = struct
  type state = int list ref

  let sexp_of_state (stack : state) = sexp_of_list sexp_of_int !stack

  let push tc stack =
    let n = draw tc (integers ~min_value:0 ~max_value:9 ()) in
    stack := n :: !stack
  [@@rule]

  let stack_stays_small _tc stack = assert (List.length !stack <= 2)
  [@@invariant { always_check = true }]
end

let%hegel_test stack_model tc = Stack.run tc ~init:(ref [])
```

When a sequence fails, the report shows each step, the draws it made, the state
after it, and which step broke the invariant:

```
state = ()
Step 1: push
  let n = 0 in
state = (0)
Step 2: push
  let n = 0 in
state = (0 0)
Step 3: push
  let n = 0 in
state = (0 0 0)
Invariant stack_stays_small violated after step 3.
```

State machines can also be created without the PPX. Create the rules and
invariants with `Stateful.Rule.create` and `Stateful.Invariant.create`, put
them in a module of type `Stateful.State_machine`, and pass it to
`Stateful.run`. This is the same `Stack` as above, written by hand:

```ocaml
module Stack = struct
  type state = int list ref

  let push tc stack =
    let n = draw ~label:"n" tc (integers ~min_value:0 ~max_value:9 ()) in
    stack := n :: !stack

  let stack_stays_small _tc stack = assert (List.length !stack <= 2)
  let rules = [ Stateful.Rule.create ~name:"push" ~step:push () ]

  let invariants =
    [ Stateful.Invariant.create
        ~name:"stack_stays_small"
        ~inv:stack_stays_small
        ~always_check:true
        ()
    ]
end

let%hegel_test stack_model tc =
  Stateful.run
    tc
    (module Stack)
    ~init:(ref [])
    ~sexp_of_state:(fun stack -> [%sexp_of: int list] !stack)
```

In state machines not written with the PPX, draws print as `draw_1`, `draw_2`, ... unless given a `~label`.

One rule runs at a time. To run rules on several workers at once, write a
`module%hegel_concurrent_state_machine` and pass `~max_concurrency:n` to its
`run`. Rules marked `[@@rule { group = "group" }]` run concurrently only with
rules in the same group, and `[@@rule { weight = <weight> }]` hints at the 
proportion of times a rule should be executed. 

See the `Hegel.Stateful` API docs for concurrent testing, for invariants across
multiple rules, and for value pools that let one rule act on data an earlier
rule produced.

## Change the number of test cases

By default Hegel runs 100 test cases. To override this, attach a
`[@@settings ...]` attribute to the test:

```ocaml
let%hegel_test integer_self_equality tc =
  let n = draw tc (integers ()) in
  assert (n = n)
[@@settings Settings.create ~test_cases:500 ()]
;;
```

## Learning more

- Run `just docs` to build the full odoc API documentation.
- Browse the [`examples/`](../examples/) directory for runnable tests.
