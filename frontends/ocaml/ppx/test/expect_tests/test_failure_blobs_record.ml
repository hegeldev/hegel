(** Snapshot tests for the [@@failure_blobs ...] recording and replay flows. *)

let prop tc = if Hegel.draw tc (Hegel.booleans ()) then failwith "deliberate failure"

let settings () =
  { (Hegel.Settings.create ~test_cases:50 ~seed:0 ()) with
    database = Hegel.Settings.Disabled
  }
;;

let expect_failure message f =
  match f () with
  | () -> failwith "expected the run to raise Failure"
  | exception Failure actual -> assert (String.equal actual message)
;;

(* Pull the blob out of the report's reproduce line: the substring
   between the first quote after [failure_blobs] and the next quote. Works for
   both the [[@@failure_blobs [ "..." ]]] and [~failure_blobs:[ "..." ]]
   forms. *)
let extract_blob out =
  (* A draw's source filename may itself contain "failure_blobs". *)
  let marker = "To reproduce this failure" in
  let ml = String.length marker in
  let rec find i =
    if String.equal (String.sub out i ml) marker then i + ml else find (i + 1)
  in
  let after = find 0 in
  let q1 = String.index_from out after '"' in
  let q2 = String.index_from out (q1 + 1) '"' in
  String.sub out (q1 + 1) (q2 - q1 - 1)
;;

let replace_all ~sub ~by s =
  let sl = String.length s in
  let subl = String.length sub in
  let buf = Buffer.create sl in
  let rec go i =
    if i + subl <= sl && String.equal (String.sub s i subl) sub
    then (
      Buffer.add_string buf by;
      go (i + subl))
    else if i < sl
    then (
      Buffer.add_char buf s.[i];
      go (i + 1))
  in
  go 0;
  Buffer.contents buf
;;

(* The stock OCaml compiler renders the wrapped-library path with a dot
   ([Expect_tests.Test_failure_blobs_record.A]); the oxcaml compiler keeps the
   mangled form ([Expect_tests__Test_failure_blobs_record.A]). *)
let normalize out = replace_all ~sub:"__" ~by:"." out

(* Round-trip: recording mode prints a blob on failure; that exact blob, fed
   back through replay mode, must reproduce the original failure. *)
let%expect_test "recording then replay round-trips the failure blob" =
  expect_failure "deliberate failure" (fun () ->
    Hegel.run_hegel_test ~settings:(settings ()) ~failure_blobs:[] prop);
  let recorded = [%expect.output] in
  let blob = extract_blob recorded in
  print_string (Expect_scrub.scrub_report recorded);
  [%expect
    {|
    draw_1 = true
    Exception: Failure("deliberate failure")

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}];
  expect_failure "deliberate failure" (fun () ->
    Hegel.run_hegel_test ~settings:(settings ()) ~failure_blobs:[ blob ] prop);
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    draw_1 = true
    Exception: Failure("deliberate failure")
    |}]
;;

let%expect_test "blob replay preserves the original failure backtrace" =
  let recording = Printexc.backtrace_status () in
  Printexc.record_backtrace true;
  Fun.protect
    ~finally:(fun () -> Printexc.record_backtrace recording)
    (fun () ->
       let original = ref "" in
       let replay_failure _tc =
         try failwith "replay backtrace" with
         | exn ->
           let bt = Printexc.get_raw_backtrace () in
           original := Printexc.raw_backtrace_to_string bt;
           Printexc.raise_with_backtrace exn bt
       in
       expect_failure "replay backtrace" (fun () ->
         Hegel.run_hegel_test ~settings:(settings ()) replay_failure);
       let recorded = [%expect.output] in
       let blob = extract_blob recorded in
       print_string (Expect_scrub.scrub_report recorded);
       [%expect
         {|
         Exception: Failure("replay backtrace")

         To reproduce this failure, pass this argument to run_hegel_test:
             ~failure_blobs:[ "<BLOB>" ]
         |}];
       let backtrace =
         match
           Hegel.run_hegel_test
             ~settings:(settings ())
             ~failure_blobs:[ blob ]
             replay_failure
         with
         | () -> assert false
         | exception Failure msg ->
           let bt = Printexc.get_raw_backtrace () in
           assert (String.equal msg "replay backtrace");
           Printexc.raw_backtrace_to_string bt
       in
       assert (not (String.equal !original ""));
       assert (String.starts_with backtrace ~prefix:!original);
       print_string (Expect_scrub.scrub_report [%expect.output]);
       [%expect {| Exception: Failure("replay backtrace") |}])
;;

let%expect_test "usage errors from a replayed body stay usage errors" =
  expect_failure "deliberate failure" (fun () ->
    Hegel.run_hegel_test ~settings:(settings ()) prop);
  let recorded = [%expect.output] in
  let blob = extract_blob recorded in
  print_string (Expect_scrub.scrub_report recorded);
  [%expect
    {|
    draw_1 = true
    Exception: Failure("deliberate failure")

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}];
  (match
     Hegel.run_hegel_test ~settings:(settings ()) ~failure_blobs:[ blob ] (fun _tc ->
       raise (Hegel.Usage_error "invalid replay argument"))
   with
   | () -> assert false
   | exception Hegel.Usage_error msg ->
     assert (String.equal msg "invalid replay argument"));
  [%expect {||}]
;;

let%expect_test "stale blobs that pass, reject, or overrun do not reproduce an error" =
  List.iter
    (fun body ->
       match
         Hegel.run_hegel_test
           ~settings:(settings ())
           ~failure_blobs:[ "AAEAAAABAQ==" ]
           body
       with
       | () -> failwith "expected stale blob failure"
       | exception Hegel.Did_not_reproduce msg -> print_endline msg)
    [ (fun _ -> ())
    ; (fun tc -> Hegel.assume tc false)
    ; (fun _ -> raise Hegel.Internal.Stop_test)
    ];
  [%expect
    {|
    The failure blob did not reproduce an error. The failure is fixed, or a nondeterministic failure did not occur again in the replays.
    The failure blob did not reproduce an error. The failure is fixed, or a nondeterministic failure did not occur again in the replays.
    The failure blob did not reproduce an error. The failure is fixed, or a nondeterministic failure did not occur again in the replays.
    |}]
;;

let%hegel_test invalid_blob = prop [@@failure_blobs [ "INVALID_BLOB" ]]

let%expect_test "an invalid supplied blob raises a usage error" =
  match invalid_blob () with
  | () -> failwith "expected invalid blob failure"
  | exception Hegel.Usage_error msg ->
    print_endline msg;
    [%expect
      {| hegel_run_start_blob: the supplied failure blob could not be decoded. It may be corrupt or from an incompatible Hegel version. |}]
;;

let%expect_test "every blob is replayed" =
  expect_failure "deliberate failure" (fun () ->
    Hegel.run_hegel_test ~settings:(settings ()) prop);
  let blob = extract_blob [%expect.output] in
  (match
     Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
       if not (Hegel.draw tc (Hegel.booleans ())) then failwith "false drawn")
   with
   | () -> assert false
   | exception Failure _ -> ());
  let stale = extract_blob [%expect.output] in
  expect_failure "deliberate failure" (fun () ->
    Hegel.run_hegel_test ~settings:(settings ()) ~failure_blobs:[ stale; blob ] prop);
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    draw_1 = true
    Exception: Failure("deliberate failure")
    |}];
  (match
     Hegel.run_hegel_test ~settings:(settings ()) ~failure_blobs:[ blob; blob ] prop
   with
   | () -> assert false
   | exception Hegel.Multiple_failures exns ->
     Printf.printf "%d failures\n" (List.length exns));
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    draw_1 = true
    Exception: Failure("deliberate failure")
    draw_1 = true
    Exception: Failure("deliberate failure")
    2 failures
    |}]
;;

exception A
exception B

let multi_prop tc =
  let v = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:100 ()) in
  if v >= 60 then raise A;
  if v <= 30 then raise B
;;

let%hegel_test multi_fail_test tc =
  let v = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:100 ()) in
  if v >= 60 then raise A;
  if v <= 30 then raise B
[@@settings
  { (Hegel.Settings.create ~test_cases:300 ~seed:9 ()) with
    report_multiple_failures = true
  }]
;;

let%expect_test "recording groups each failure's draws with its diagnostic" =
  (match multi_fail_test () with
   | () -> assert false
   | exception Hegel.Multiple_failures exns ->
     Printf.printf "%d failures raised" (List.length exns));
  Printf.printf "%s" (Expect_scrub.scrub_report (normalize [%expect.output]));
  [%expect
    {|
    Property-based test failed with 2 distinct failures.

    v = 60
    Exception: Expect_tests.Test_failure_blobs_record.A

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]

    v = 0
    Exception: Expect_tests.Test_failure_blobs_record.B

    To reproduce this failure, add this attribute to the let%hegel_test:
        [@@failure_blobs [ "<BLOB>" ]]
    2 failures raised
    |}]
;;

(* With [print_blob] off the per-failure blocks still print, but without the
   trailing blob line. *)
let%expect_test "the multi-failure report omits blobs when print_blob is off" =
  let settings =
    { (Hegel.Settings.create ~test_cases:300 ~seed:9 ()) with
      report_multiple_failures = true
    ; print_blob = false
    }
  in
  (match Hegel.run_hegel_test ~settings multi_prop with
   | () -> assert false
   | exception Hegel.Multiple_failures exns ->
     Printf.printf "%d failures raised" (List.length exns));
  Printf.printf "%s" (Expect_scrub.scrub_report (normalize [%expect.output]));
  [%expect
    {|
    Property-based test failed with 2 distinct failures.

    draw_1 = 60
    Exception: Expect_tests.Test_failure_blobs_record.A

    draw_1 = 0
    Exception: Expect_tests.Test_failure_blobs_record.B
    2 failures raised
    |}]
;;

let%expect_test "a quiet multi-failure report prints only the reproduce lines" =
  let settings =
    { (Hegel.Settings.create ~test_cases:300 ~seed:9 ()) with
      report_multiple_failures = true
    ; verbosity = Hegel.Settings.Quiet
    }
  in
  (match Hegel.run_hegel_test ~settings multi_prop with
   | () -> assert false
   | exception Hegel.Multiple_failures exns ->
     Printf.printf "%d failures raised" (List.length exns));
  Printf.printf "%s" (Expect_scrub.scrub_report (normalize [%expect.output]));
  [%expect
    {|
    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    2 failures raised
    |}]
;;

let%expect_test "blob scrubbing preserves quoted failure diagnostics" =
  print_string
    (Expect_scrub.scrub_blobs
       {|Exception: Failure("failure_blobs should not hide this message")

To reproduce this failure, pass this argument to run_hegel_test:
    ~failure_blobs:[ "encoded choices" ]
|});
  [%expect
    {|
    Exception: Failure("failure_blobs should not hide this message")

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;
