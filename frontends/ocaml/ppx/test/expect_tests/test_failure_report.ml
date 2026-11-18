let int_gen = Hegel.integers ~min_value:0 ~max_value:100 ()

let%expect_test "no-draw failure prints a report without drawn values" =
  (try
     Hegel.run_hegel_test
       ~settings:
         { (Hegel.Settings.create ~test_cases:10 ()) with
           database = Hegel.Settings.Disabled
         }
       (fun _tc -> failwith "boom")
   with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    --- Failure --------------------------------------------------------------------
    Exception: Failure("boom")
    rerun with: ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

let%expect_test "failure report prints the shrunk counterexample" =
  (try
     Hegel.run_hegel_test
       ~settings:
         { (Hegel.Settings.create ~test_cases:300 ~seed:9 ()) with
           database = Hegel.Settings.Disabled
         }
       (fun tc ->
          let v = Hegel.draw tc int_gen in
          if v >= 60 then failwith "large values are broken")
   with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    --- Failure --------------------------------------------------------------------

    draw_1 = 60

    Exception: Failure("large values are broken")
    rerun with: ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

(* The failure does not reproduce, so the report has a caveat. *)
let%expect_test "a flaky failure is reported with a caveat" =
  let calls = ref 0 in
  (match
     Hegel.run_hegel_test
       ~settings:
         { (Hegel.Settings.create ~test_cases:10 ~seed:0 ()) with
           database = Hegel.Settings.Disabled
         ; phases = [ Hegel.Settings.Generate ]
         }
       (fun tc ->
          ignore (Hegel.draw tc int_gen : int);
          let call = !calls in
          incr calls;
          if call = 0 then failwith "first call only")
   with
   | () -> print_endline "passed"
   | exception Failure message -> print_endline message);
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    --- Failure --------------------------------------------------------------------
    Exception: Failure("first call only")
    note: unconfirmed failure: failed 0 of 10 replays after the observed failure — a rare failure, or the environment changed between executions
    first call only
    |}]
;;

(* A fixed explicit position keeps wrapping identical across compilers.
   Continuation lines align under the sexp, including the location prefix. *)
let%expect_test "a multiline drawn value aligns under its name" =
  (try
     Hegel.run_hegel_test
       ~settings:
         { (Hegel.Settings.create ~test_cases:100 ~seed:0 ()) with
           database = Hegel.Settings.Disabled
         }
       (fun tc ->
          let l =
            Hegel.draw
              tc
              ~label:"l"
              ~loc:
                { Lexing.pos_fname = "draw.ml"; pos_lnum = 1; pos_bol = 0; pos_cnum = 0 }
              (Hegel.lists ~min_size:15 (Hegel.text ~min_size:5 ~max_size:10 ()) ())
          in
          assert (List.length l < 15))
   with
   | _ -> ());
  print_string (Expect_scrub.scrub_report ~hide_draw_positions:false [%expect.output]);
  [%expect
    {|
    --- Failure --------------------------------------------------------------------

    l @ draw.ml:<LINE> = (00000
     00000
     00000
     00000
     00000
     00000
     00000
     00000
     00000
     00000
     00000
     00000
     00000
     00000
     00000)

    Exception: File "ppx/test/expect_tests/test_failure_report.ml", line LINE, characters C1-C2: Assertion failed
    rerun with: ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

let run_boom verbosity body =
  match
    Hegel.run_hegel_test
      ~settings:
        { (Hegel.Settings.create ~test_cases:1 ~seed:0 ()) with
          database = Hegel.Settings.Disabled
        ; verbosity
        }
      body
  with
  | () -> print_endline "passed"
  | exception Failure message -> print_endline message
;;

let boom _tc =
  if Sys.opaque_identity true then failwith "boom";
  ()
;;

let%expect_test "a verbose run prints each failure's summary with its location" =
  run_boom Hegel.Settings.Verbose boom;
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Starting phase: Generate
    Running test case
    ppx/test/expect_tests/test_failure_report.ml:<LINE>LINE:COL: Exception: Failure("boom")
    Running test case
    ppx/test/expect_tests/test_failure_report.ml:<LINE>LINE:COL: Exception: Failure("boom")
    Running test case
    ppx/test/expect_tests/test_failure_report.ml:<LINE>LINE:COL: Exception: Failure("boom")
    Running test case
    ppx/test/expect_tests/test_failure_report.ml:<LINE>LINE:COL: Exception: Failure("boom")
    Running test case
    ppx/test/expect_tests/test_failure_report.ml:<LINE>LINE:COL: Exception: Failure("boom")
    Ending phase: Generate
    Starting phase: Shrink
    Running test case
    ppx/test/expect_tests/test_failure_report.ml:<LINE>LINE:COL: Exception: Failure("boom")
    Ending phase: Shrink
    Running test case
    ppx/test/expect_tests/test_failure_report.ml:<LINE>LINE:COL: Exception: Failure("boom")
    --- Failure --------------------------------------------------------------------
    Exception: Failure("boom")
    rerun with: ~failure_blobs:[ "<BLOB>" ]
    boom
    |}]
;;

let%expect_test "a debug run prints each failure's exception and backtrace" =
  run_boom Hegel.Settings.Debug boom;
  Expect_scrub.scrub_report [%expect.output]
  |> String.split_on_char '\n'
  |> List.filter (fun line ->
    List.exists
      (fun prefix -> String.starts_with ~prefix line)
      [ "Exception"; "<BACKTRACE>"; "---"; "test case #" ])
  |> List.iter print_endline;
  [%expect
    {|
    Exception: Failure("boom")
    <BACKTRACE>
    test case #1: status = Interesting, choices = 0
    Exception: Failure("boom")
    <BACKTRACE>
    Exception: Failure("boom")
    <BACKTRACE>
    Exception: Failure("boom")
    <BACKTRACE>
    Exception: Failure("boom")
    <BACKTRACE>
    Exception: Failure("boom")
    <BACKTRACE>
    ---------------------
    ---------------------
    Exception: Failure("boom")
    <BACKTRACE>
    --- Failure --------------------------------------------------------------------
    Exception: Failure("boom")
    |}]
;;

exception Two_lines

let () =
  Printexc.register_printer (function
    | Two_lines -> Some "first line\r\nsecond line"
    | _ -> None)
;;

let%expect_test "a summary keeps the first line of a multi-line message" =
  let recording = Printexc.backtrace_status () in
  Printexc.record_backtrace false;
  Fun.protect
    ~finally:(fun () -> Printexc.record_backtrace recording)
    (fun () ->
       try run_boom Hegel.Settings.Verbose (fun _tc -> raise Two_lines) with
       | Two_lines -> print_endline "raised");
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Starting phase: Generate
    Running test case
    Exception: first line …
    Running test case
    Exception: first line …
    Running test case
    Exception: first line …
    Running test case
    Exception: first line …
    Running test case
    Exception: first line …
    Ending phase: Generate
    Starting phase: Shrink
    Running test case
    Exception: first line …
    Ending phase: Shrink
    Running test case
    Exception: first line …
    --- Failure --------------------------------------------------------------------
    Exception: first line
    second line
    rerun with: ~failure_blobs:[ "<BLOB>" ]
    raised
    |}]
;;

let%expect_test "a quiet run reports the exception" =
  run_boom Hegel.Settings.Quiet (fun _tc -> failwith "boom");
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    --- Failure --------------------------------------------------------------------
    Exception: Failure("boom")
    rerun with: ~failure_blobs:[ "<BLOB>" ]
    boom
    |}]
;;

let%expect_test "a summary of an exception with its own location adds none" =
  (try
     run_boom Hegel.Settings.Verbose (fun tc ->
       match[@warning "-8"] Hegel.draw_silent tc (Hegel.just 1) with
       | 0 -> ())
   with
   | Match_failure _ -> print_endline "raised");
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Starting phase: Generate
    Running test case
    Exception: File "ppx/test/expect_tests/test_failure_report.ml", line LINE, characters C1-C2: Pattern matching failed
    Running test case
    Exception: File "ppx/test/expect_tests/test_failure_report.ml", line LINE, characters C1-C2: Pattern matching failed
    Running test case
    Exception: File "ppx/test/expect_tests/test_failure_report.ml", line LINE, characters C1-C2: Pattern matching failed
    Running test case
    Exception: File "ppx/test/expect_tests/test_failure_report.ml", line LINE, characters C1-C2: Pattern matching failed
    Running test case
    Exception: File "ppx/test/expect_tests/test_failure_report.ml", line LINE, characters C1-C2: Pattern matching failed
    Ending phase: Generate
    Starting phase: Shrink
    Running test case
    Exception: File "ppx/test/expect_tests/test_failure_report.ml", line LINE, characters C1-C2: Pattern matching failed
    Ending phase: Shrink
    Running test case
    Exception: File "ppx/test/expect_tests/test_failure_report.ml", line LINE, characters C1-C2: Pattern matching failed
    --- Failure --------------------------------------------------------------------
    Exception: File "ppx/test/expect_tests/test_failure_report.ml", line LINE, characters C1-C2: Pattern matching failed
    rerun with: ~failure_blobs:[ "<BLOB>" ]
    raised
    |}]
;;
