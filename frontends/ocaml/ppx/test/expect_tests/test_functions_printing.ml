open! Core
open Hegel
open Generators

(* swallow the failure the property raises so the expect block only sees what we
   printed. *)
let run_failing body =
  try
    Hegel.run_hegel_test
      ~settings:
        { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }
      body
  with
  | _ -> ()
;;

let%hegel_test binding_name tc =
  let f_gen =
    functions
      ~sexp_of_arg:sexp_of_int
      ~returns:(integers ~min_value:0 ~max_value:1000 ())
      ()
  in
  let f = draw_silent tc f_gen in
  assert (f 42 < 10)
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test "a function is named from its binding" =
  (try binding_name () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    f 42 = 10
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let%hegel_test drawn_function_prints_applications tc =
  let f =
    draw_silent
      tc
      (functions2
         ~sexp_of_arg1:sexp_of_int
         ~sexp_of_arg2:sexp_of_string
         ~returns:
           (lists (integers ~min_value:3 ~max_value:3 ()) ~min_size:2 ~max_size:2 ())
         ())
  in
  assert (List.is_empty (f (-1) "a b"))
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test
    "an application prints its arguments as sexps and its result as OCaml syntax"
  =
  (try drawn_function_prints_applications () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    f (-1) "a b" = [3; 3]
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let%hegel_test binding_name_inline tc =
  let f =
    draw_silent
      tc
      (functions
         ~sexp_of_arg:sexp_of_int
         ~returns:(integers ~min_value:0 ~max_value:1000 ())
         ())
  in
  assert (f 42 < 10)
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test "a function drawn inline is also named from its binding" =
  (try binding_name_inline () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    f 42 = 10
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let%hegel_test explicit_name_beats_binding tc =
  let f =
    draw_silent
      tc
      (functions
         ~name:"chosen"
         ~sexp_of_arg:sexp_of_int
         ~returns:(integers ~min_value:0 ~max_value:1000 ())
         ())
  in
  assert (f 42 < 10)
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test "an explicit ~name wins over the draw-site binding name" =
  (try explicit_name_beats_binding () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    chosen 42 = 10
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let%expect_test "without a binding the function falls back to its default name" =
  run_failing (fun tc ->
    let f =
      draw_silent
        tc
        (functions
           ~sexp_of_arg:sexp_of_int
           ~returns:(integers ~min_value:0 ~max_value:1000 ())
           ())
    in
    assert (f 42 < 10));
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    function 42 = 10
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

let%expect_test "an explicit ~name is used when there is no binding name" =
  run_failing (fun tc ->
    let f =
      draw_silent
        tc
        (functions
           ~name:"myfun"
           ~sexp_of_arg:sexp_of_int
           ~returns:(integers ~min_value:0 ~max_value:1000 ())
           ())
    in
    assert (f 42 < 10));
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    myfun 42 = 10
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

let%hegel_test functions2_binding tc =
  let g =
    draw_silent
      tc
      (functions2
         ~sexp_of_arg1:sexp_of_int
         ~sexp_of_arg2:sexp_of_bool
         ~returns:(integers ~min_value:0 ~max_value:1000 ())
         ())
  in
  assert (g 3 true < 10)
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test "functions2 shows its applications curried, named from its binding" =
  (try functions2_binding () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    g 3 true = 10
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let%hegel_test functions3_binding tc =
  let h =
    draw_silent
      tc
      (functions3
         ~sexp_of_arg1:sexp_of_int
         ~sexp_of_arg2:sexp_of_bool
         ~sexp_of_arg3:sexp_of_int
         ~returns:(integers ~min_value:0 ~max_value:1000 ())
         ())
  in
  assert (h 1 true 2 < 10)
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test "functions3 shows its applications curried, named from its binding" =
  (try functions3_binding () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    h 1 true 2 = 10
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

(* The PPX injects [~name] into every [draw_silent] binding, but it is ignored
   for non-function generators, which stay silent. *)
let%hegel_test draw_silent_scalar_stays_silent tc =
  let n = draw_silent tc (integers ~min_value:5 ~max_value:5 ()) in
  ignore (n : int);
  assert false
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test "a scalar drawn with draw_silent prints nothing even when named" =
  (try draw_silent_scalar_stays_silent () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let func_call_loop verbosity =
  Hegel.run_hegel_test
    ~settings:
      { (Settings.create ~test_cases:1 ~seed:0 ()) with
        verbosity
      ; phases = [ Settings.Generate ]
      }
    (fun tc ->
       let f =
         draw_silent
           tc
           (functions
              ~sexp_of_arg:sexp_of_int
              ~returns:(integers ~min_value:0 ~max_value:1000 ())
              ())
       in
       for _ = 0 to 2 do
         ignore (f 10 : int)
       done;
       assert false)
;;

let%expect_test "function call only prints the first time in normal verbosity" =
  (try func_call_loop Settings.Normal with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    function 10 = 586
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

let%expect_test "function call prints every time in verbose/debug verbosity" =
  (try func_call_loop Settings.Verbose with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Starting phase: Generate
    Running test case
    function 10 = 586
    function 10 = 586
    function 10 = 586
    Running test case
    function 10 = 586
    function 10 = 586
    function 10 = 586
    Running test case
    function 10 = 586
    function 10 = 586
    function 10 = 586
    Running test case
    function 10 = 586
    function 10 = 586
    function 10 = 586
    Running test case
    function 10 = 586
    function 10 = 586
    function 10 = 586
    Ending phase: Generate
    Running test case
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}];
  (try func_call_loop Settings.Debug with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    no config file loaded
    Starting phase: Generate
    function 10 = 586
    function 10 = 586
    function 10 = 586
    test case #1: status = Interesting, choices = 1
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    function 10 = 586
    Ending phase: Generate
    function 10 = 586
    function 10 = 586
    function 10 = 586
    Test done. interesting_test_cases=1
    function 10 = 586
    function 10 = 586
    function 10 = 586
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

let%hegel_test printable_function_draw tc =
  let f =
    draw
      tc
      (with_printer
         (fun _ -> Sexp.Atom "<fun>")
         (functions
            ~sexp_of_arg:sexp_of_int
            ~returns:(integers ~min_value:0 ~max_value:1000 ())
            ()))
  in
  assert (f 42 < 10)
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test "a printable function generator prints with its sexp_of" =
  (try printable_function_draw () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    let f = <fun> in
    f 42 = 10
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

(* Drawn nested (inside a span, draw depth > 0) a function still gets its label
   threaded into its pairs — here the explicit name [f] — but its [<fun>]
   value line is suppressed like any nested draw. It is
   applied at the top level, so its pair still shows. *)
let%hegel_test printable_function_drawn_nested tc =
  let f =
    Private.group Private.Labels.list tc (fun () ->
      draw
        tc
        (with_printer
           (fun _ -> Sexp.Atom "<fun>")
           (functions
              ~name:"f"
              ~sexp_of_arg:sexp_of_int
              ~returns:(integers ~min_value:0 ~max_value:1000 ())
              ())))
  in
  assert (f 42 < 10)
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test "a function drawn nested keeps its label but suppresses its value line" =
  (try printable_function_drawn_nested () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    f 42 = 10
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

(* A function applied inside a span (draw depth > 0) does not print its pair,
   like any nested draw. Here [f] is applied only inside the [group], so nothing
   is shown even though the property fails. *)
let%hegel_test application_inside_span_is_suppressed tc =
  let f =
    draw_silent
      tc
      (functions
         ~sexp_of_arg:sexp_of_int
         ~returns:(integers ~min_value:0 ~max_value:1000 ())
         ())
  in
  let r = Private.group Private.Labels.list tc (fun () -> f 42) in
  ignore (r : int);
  assert false
[@@settings Settings.create ~test_cases:100 ~seed:0 ()]
;;

let%expect_test "a function applied inside a span prints nothing" =
  (try application_inside_span_is_suppressed () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let%hegel_test partially_printable_args_and_ret tc =
  let f =
    draw_silent
      tc
      (functions
         ~sexp_of_arg:sexp_of_int
         ~returns:(functions ~returns:(integers ()) ())
         ())
  in
  let n = f 1 () in
  assert (n = 0)
[@@settings Settings.create ~test_cases:100 ~seed:0 ()]
;;

let%expect_test "partially printable applications" =
  (try partially_printable_args_and_ret () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    f 1 = <fun>
    function <abstr> = 1
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let%hegel_test unprintable_args_and_ret tc =
  let f = draw_silent tc (functions ~returns:(functions ~returns:(integers ()) ()) ()) in
  let n = f () () in
  assert (n = 0)
[@@settings Settings.create ~test_cases:100 ~seed:0 ()]
;;

let%expect_test "unprintable applications" =
  (try unprintable_args_and_ret () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    f <abstr> = <fun>
    function <abstr> = 1
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let%hegel_test results_with_no_printer tc =
  let f = draw_silent tc (functions ~returns:(just 3) ()) in
  let g =
    draw_silent
      tc
      (functions ~returns:(map succ (integers ~min_value:0 ~max_value:0 ())) ())
  in
  let h = draw_silent tc (functions ~returns:(composite (fun _ -> 5)) ()) in
  assert (f () + g () + h () = 0)
[@@settings Settings.create ~test_cases:100 ~seed:0 ()]
;;

let%expect_test "a result with no printer shows as <abstr>" =
  (try results_with_no_printer () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    h <abstr> = <abstr>
    g <abstr> = <abstr>
    f <abstr> = <abstr>
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let%hegel_test functions_print_arg tc =
  let f =
    draw_silent
      tc
      (functions
         ~print_arg:(Printer.list Printer.int)
         ~sexp_of_arg:(fun _ -> Sexplib0.Sexp.Atom "unused")
         ~returns:(integers ~min_value:0 ~max_value:1000 ())
         ())
  in
  let g =
    draw_silent
      tc
      (functions2
         ~print_arg1:Printer.int
         ~print_arg2:Printer.string
         ~returns:(integers ~min_value:0 ~max_value:1000 ())
         ())
  in
  let h =
    draw_silent
      tc
      (functions3
         ~print_arg1:Printer.int
         ~print_arg2:Printer.bool
         ~print_arg3:(Printer.option Printer.int)
         ~returns:(integers ~min_value:0 ~max_value:1000 ())
         ())
  in
  assert (f [ 1; 2 ] + g (-1) "a" + h 1 true (Some 2) < 10)
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test "print_arg renders arguments as OCaml syntax" =
  (try functions_print_arg () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    h 1 true (Some 2) = 0
    g (-1) "a" = 0
    f [1; 2] = 10
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;

let%hegel_test function_of_flat_map tc =
  let f =
    draw_silent
      tc
      (functions
         ~sexp_of_arg:sexp_of_int
         ~returns:
           (flat_map
              (fun n -> map Fn.id (integers ~min_value:n ~max_value:n ()))
              (integers ~min_value:5 ~max_value:5 ()))
         ())
  in
  assert (f 1 < 5)
[@@settings
  { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }]
;;

let%expect_test "a function returning an unprintable flat_map prints <abstr>" =
  (try function_of_flat_map () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    f 1 = <abstr>
    Exception: File "ppx/test/expect_tests/test_functions_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    |}]
;;
