(** Snapshot tests for [Hegel.require]: the failure messages and the
    caller-derived failure origins. Tests call [run_hegel_test] directly with the database
    disabled to avoid persisted examples affecting the report. *)

let settings () =
  { (Hegel.Settings.create ~test_cases:10 ()) with database = Hegel.Settings.Disabled }
;;

let%expect_test "require fails with the default message" =
  (try
     Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
       Hegel.require tc true;
       Hegel.require tc false)
   with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Exception: Failure("require: condition was false")

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

let%expect_test "require fails with a custom message" =
  (try
     Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
       Hegel.require tc ~msg:"the invariant broke" false)
   with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Exception: Failure("the invariant broke")

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

(* Two require calls at distinct source lines produce two distinct failures: the
   origin is derived from the caller's line, not from require's raise site
   inside the library (which would collapse every require failure in a run into
   one origin). *)
let%expect_test "require failures get caller-derived origins" =
  (try
     Hegel.run_hegel_test
       ~settings:
         { (Hegel.Settings.create ~test_cases:300 ~seed:9 ()) with
           database = Hegel.Settings.Disabled
         ; report_multiple_failures = true
         ; print_blob = false
         }
       (fun tc ->
          let v = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:100 ()) in
          Hegel.require tc ~msg:"too big" (v < 60);
          Hegel.require tc ~msg:"too small" (v > 30))
   with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Property-based test failed with 2 distinct failures.

    let draw_1 = 60 in
    Exception: Failure("too big")

    let draw_1 = 0 in
    Exception: Failure("too small")
    |}]
;;
