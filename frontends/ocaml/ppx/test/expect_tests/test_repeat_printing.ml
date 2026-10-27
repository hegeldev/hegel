open! Core
open Hegel
open Generators

let run_failing body =
  try
    Hegel.run_hegel_test
      ~settings:
        { (Settings.create ~test_cases:100 ~seed:0 ()) with verbosity = Settings.Normal }
      body
  with
  | _ -> ()
;;

let%expect_test "repeat prints each iteration under a repetition note" =
  run_failing (fun tc ->
    let total = ref 0 in
    repeat tc (fun tc ->
      let n = draw ~label:"n" tc (integers ~min_value:0 ~max_value:6 ()) in
      total := !total + n;
      assert (!total < 10)));
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    (* Repetition #1 *)
      let n = 4 in
    (* Repetition #2 *)
      let n = 6 in
    Exception: File "ppx/test/expect_tests/test_repeat_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

let%expect_test "set_worker attributes a clone's output to the worker" =
  run_failing (fun tc ->
    let worker = clone tc in
    set_worker worker 3;
    let n = draw ~label:"n" worker (integers ~min_value:5 ~max_value:5 ()) in
    assert (n <> 5));
  print_string (Expect_scrub.scrub_concurrent_report [%expect.output]);
  [%expect
    {|
    [worker 3 +time] let n = 5 in
    Exception: File "ppx/test/expect_tests/test_repeat_printing.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;
