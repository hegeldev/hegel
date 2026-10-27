(** E2E tests for the [ppx_hegel_test] PPX.

    These tests exercise the full expansion of [let%hegel_test] including the
    [@@settings ...] attribute. *)

open! Core
module Atomic = Stdlib.Atomic

let env_var = "ANTITHESIS_OUTPUT_DIR"

let with_env_dir dir ~f =
  let prev = Sys.getenv env_var in
  Core_unix.putenv ~key:env_var ~data:dir;
  Exn.protect
    ~finally:(fun () ->
      match prev with
      | Some v -> Core_unix.putenv ~key:env_var ~data:v
      | None -> Core_unix.unsetenv env_var)
    ~f
;;

let with_tempdir ~f =
  let dir =
    Core_unix.mkdtemp
      (Filename.concat (Stdlib.Filename.get_temp_dir_name ()) "hegel-ppx-test-")
  in
  Exn.protect
    ~finally:(fun () ->
      (try
         Stdlib.Sys.readdir dir
         |> Array.iter ~f:(fun name ->
           try Stdlib.Sys.remove (Filename.concat dir name) with
           | _ -> ())
       with
       | _ -> ());
      try Core_unix.rmdir dir with
      | _ -> ())
    ~f:(fun () -> f dir)
;;

(** A simple passing test, expected to expand into a unit -> unit wrapper. *)
let%hegel_test simple_pass (tc : Hegel.test_case) =
  let _ = Hegel.draw tc (Hegel.booleans ()) in
  ()
[@@settings Hegel.Settings.create ~test_cases:3 ()]
;;

(** A failing test. The generated wrapper will raise. *)
let%hegel_test simple_fail (tc : Hegel.test_case) =
  let _ = Hegel.draw tc (Hegel.booleans ()) in
  failwith "deliberate failure"
[@@settings Hegel.Settings.create ~test_cases:3 ()]
;;

(** A test without an explicit [@@settings] attribute — should fall back to
    default settings. *)
let%hegel_test no_settings (tc : Hegel.test_case) =
  let _ = Hegel.draw tc (Hegel.booleans ()) in
  ()
;;

let%hegel_main failing_main (tc : Hegel.test_case) =
  let n = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:100 ()) in
  if n >= 0 then failwith "deliberate failure"
[@@settings { (Hegel.Settings.default ()) with database = Hegel.Settings.Disabled }]
;;

let () =
  match Sys.getenv "HEGEL_TEST_MAIN_CHILD" with
  | Some _ -> failing_main ()
  | None -> ()
;;

let test_main_reports () =
  with_tempdir ~f:(fun dir ->
    let process =
      Core_unix.create_process_env
        ~prog:Stdlib.Sys.executable_name
        ~args:[]
        ~env:(`Extend [ "HEGEL_TEST_MAIN_CHILD", "1"; env_var, dir ])
        ()
    in
    Core_unix.close process.stdin;
    Alcotest.(check bool)
      "exits with status 1"
      true
      (Poly.equal (Core_unix.waitpid process.pid) (Error (`Exit_non_zero 1)));
    let sdk = In_channel.read_all (Filename.concat dir "sdk.jsonl") in
    Alcotest.(check bool)
      "the verdict names the binding"
      true
      (String.is_substring
         sdk
         ~substring:"test_ppx_hegel_test::failing_main passes properties"))
;;

let test_passing_writes_sdk_jsonl () =
  with_tempdir ~f:(fun dir ->
    with_env_dir dir ~f:(fun () ->
      simple_pass ();
      let path = Filename.concat dir "sdk.jsonl" in
      Alcotest.(check bool) "sdk.jsonl exists" true (Stdlib.Sys.file_exists path);
      let lines = In_channel.read_all path |> String.split_lines in
      Alcotest.(check int) "two lines (declaration + evaluation)" 2 (List.length lines);
      let decl = Yojson.Safe.from_string (List.nth_exn lines 0) in
      let eval = Yojson.Safe.from_string (List.nth_exn lines 1) in
      let decl_assoc =
        match decl with
        | `Assoc [ ("antithesis_assert", `Assoc fields) ] -> fields
        | _ -> Alcotest.fail "declaration not wrapped in antithesis_assert"
      in
      let eval_assoc =
        match eval with
        | `Assoc [ ("antithesis_assert", `Assoc fields) ] -> fields
        | _ -> Alcotest.fail "evaluation not wrapped in antithesis_assert"
      in
      Alcotest.(check string)
        "declaration hit"
        "false"
        (Yojson.Safe.to_string (List.Assoc.find_exn decl_assoc ~equal:String.equal "hit"));
      Alcotest.(check string)
        "evaluation hit"
        "true"
        (Yojson.Safe.to_string (List.Assoc.find_exn eval_assoc ~equal:String.equal "hit"));
      Alcotest.(check string)
        "evaluation condition is true (passing test)"
        "true"
        (Yojson.Safe.to_string
           (List.Assoc.find_exn eval_assoc ~equal:String.equal "condition"));
      let id =
        Yojson.Safe.to_string (List.Assoc.find_exn eval_assoc ~equal:String.equal "id")
      in
      Alcotest.(check bool)
        "id names the module and test"
        true
        (String.is_substring id ~substring:"test_ppx_hegel_test::simple_pass");
      (* location.file should mention the test file. *)
      let loc = List.Assoc.find_exn eval_assoc ~equal:String.equal "location" in
      let loc_assoc =
        match loc with
        | `Assoc f -> f
        | _ -> Alcotest.fail "location not an object"
      in
      let file =
        Yojson.Safe.to_string (List.Assoc.find_exn loc_assoc ~equal:String.equal "file")
      in
      Alcotest.(check bool)
        "location.file mentions test_ppx_hegel_test.ml"
        true
        (String.is_substring file ~substring:"test_ppx_hegel_test.ml")))
;;

let test_failing_writes_condition_false () =
  with_tempdir ~f:(fun dir ->
    with_env_dir dir ~f:(fun () ->
      (match simple_fail () with
       | () -> Alcotest.fail "expected the PPX wrapper to re-raise the failure"
       | exception Failure msg ->
         Alcotest.(check string) "original failure" "deliberate failure" msg);
      let path = Filename.concat dir "sdk.jsonl" in
      Alcotest.(check bool) "sdk.jsonl exists" true (Stdlib.Sys.file_exists path);
      let lines = In_channel.read_all path |> String.split_lines in
      let eval = Yojson.Safe.from_string (List.nth_exn lines 1) in
      let eval_assoc =
        match eval with
        | `Assoc [ ("antithesis_assert", `Assoc fields) ] -> fields
        | _ -> Alcotest.fail "evaluation not wrapped in antithesis_assert"
      in
      Alcotest.(check string)
        "evaluation condition is false (failing test)"
        "false"
        (Yojson.Safe.to_string
           (List.Assoc.find_exn eval_assoc ~equal:String.equal "condition"))))
;;

let invariant_checks = ref 0

module%hegel_state_machine Counter = struct
  let bump tc n =
    let by = Hegel.draw tc (Hegel.integers ~min_value:1 ~max_value:3 ()) in
    n := !n + by
  [@@rule]
  ;;

  let bump_twice tc n =
    let by = Hegel.draw tc (Hegel.integers ~min_value:1 ~max_value:3 ()) in
    n := !n + (2 * by)
  [@@rule { weight = 2.5 }]
  ;;

  let bump_thrice tc n =
    let by = Hegel.draw tc (Hegel.integers ~min_value:1 ~max_value:3 ()) in
    n := !n + (3 * by)
  [@@rule { weight = 3 }]
  ;;

  let positive _tc n =
    incr invariant_checks;
    assert (!n >= 0)
  [@@invariant { always_check = true }]
  ;;

  let helper = 42
end

let%hegel_test runs_machine (tc : Hegel.test_case) =
  invariant_checks := 0;
  Counter.run tc ~init:(ref 0) ~step_count:5;
  assert (!invariant_checks > 2)
[@@settings Hegel.Settings.create ~test_cases:3 ()]
;;

let test_state_machine_collects_marked_bindings () =
  Alcotest.(check (list string))
    "rule names"
    [ "bump"; "bump_twice"; "bump_thrice" ]
    (List.map Counter.rules ~f:Hegel.Stateful.Rule.name);
  Alcotest.(check (list (float 0.)))
    "rule weights"
    [ 1.0; 2.5; 3.0 ]
    (List.map Counter.rules ~f:Hegel.Stateful.Rule.weight);
  Alcotest.(check (list string))
    "invariant names"
    [ "positive" ]
    (List.map Counter.invariants ~f:Hegel.Stateful.Invariant.name);
  Alcotest.(check int) "unmarked items are kept" 42 Counter.helper;
  with_tempdir ~f:(fun dir -> with_env_dir dir ~f:runs_machine)
;;

module%hegel_concurrent_state_machine Ledger = struct
  let deposit _tc () (n : int Atomic.t) = Atomic.incr n [@@rule]
  let withdraw _tc () (n : int Atomic.t) = Atomic.decr n [@@rule { group = "money" }]

  let audit _tc () (n : int Atomic.t) = ignore (Atomic.get n : int)
  [@@rule { weight = 2.5 }]
  ;;

  let settle _tc () (n : int Atomic.t) = Atomic.incr n
  [@@rule { group = "money"; weight = 3 }]
  ;;

  let reconcile _tc () (n : int Atomic.t) = Atomic.decr n
  [@@rule { weight = 4.0; group = "money" }]
  ;;

  let nonnegative _tc (_n : int Atomic.t) = () [@@invariant]
end

let%hegel_test runs_ledger (tc : Hegel.test_case) =
  Ledger.run tc ~init:(Atomic.make 0) ~step_count:3
[@@settings Hegel.Settings.create ~test_cases:3 ()]
;;

let test_concurrent_state_machine_collects_groups_and_weights () =
  let module R = Hegel.Stateful.Concurrent_rule in
  Alcotest.(check (list string))
    "rule names"
    [ "deposit"; "withdraw"; "audit"; "settle"; "reconcile" ]
    (List.map Ledger.rules ~f:R.name);
  Alcotest.(check (list string))
    "rule groups"
    [ "<anonymous>"; "money"; "<anonymous>"; "money"; "money" ]
    (List.map Ledger.rules ~f:R.group);
  Alcotest.(check (list (float 0.)))
    "rule weights"
    [ 1.0; 1.0; 2.5; 3.0; 4.0 ]
    (List.map Ledger.rules ~f:R.weight);
  with_tempdir ~f:(fun dir -> with_env_dir dir ~f:runs_ledger)
;;

let test_no_settings_runs_with_defaults () =
  with_tempdir ~f:(fun dir ->
    with_env_dir dir ~f:(fun () ->
      no_settings ();
      let path = Filename.concat dir "sdk.jsonl" in
      Alcotest.(check bool) "sdk.jsonl exists" true (Stdlib.Sys.file_exists path)))
;;

let example_calls = ref []
let generated_calls = ref 0

let%hegel_test with_examples tc =
  let x = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:9 ()) in
  let s = Hegel.draw_silent tc (Hegel.text ~max_size:3 ()) in
  if x < 0 then example_calls := (x, s) :: !example_calls else Int.incr generated_calls
[@@example { x = -1; s = "four" }]
[@@example { s = "five!"; x = -2 }]
[@@settings Hegel.Settings.create ~test_cases:5 ()]
;;

let test_examples_run_first () =
  example_calls := [];
  generated_calls := 0;
  with_examples ();
  Alcotest.(check (list (pair int string)))
    "examples ran, in order"
    [ -2, "five!"; -1, "four" ]
    !example_calls;
  Alcotest.(check bool) "generation ran too" true (!generated_calls > 0)
;;

let%hegel_test failing_example tc =
  let n = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:9 ()) in
  Hegel.note tc "checking n";
  Hegel.target tc ~label:"n" ~value:(Float.of_int n);
  Hegel.event tc ~label:"seen";
  Hegel.event_value tc ~label:"n" ~value:(Float.of_int n);
  Int.incr generated_calls;
  assert (n <> 100)
[@@example { n = 100 }]
;;

let test_failing_example_ends_the_run () =
  generated_calls := 0;
  (match failing_example () with
   | () -> Alcotest.fail "expected the example to fail"
   | exception Assert_failure _ -> ());
  Alcotest.(check int) "only the example ran" 1 !generated_calls
;;

let%hegel_test example_and_generated_failures tc =
  let n = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:9 ()) in
  if n = 100 then failwith "the example";
  if n > 5 then invalid_arg "generated"
[@@example { n = 100 }]
[@@settings { (Hegel.Settings.create ~seed:0 ()) with report_multiple_failures = true }]
;;

let%hegel_test example_failure_only tc =
  let n = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:9 ()) in
  if n = 100 then failwith "the example"
[@@example { n = 100 }]
[@@settings
  { (Hegel.Settings.create ~test_cases:5 ()) with report_multiple_failures = true }]
;;

let%hegel_test examples_skipped tc =
  let n = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:9 ()) in
  assert (n <> 100)
[@@example { n = 100 }]
[@@settings
  { (Hegel.Settings.create ~test_cases:5 ()) with
    phases = [ Hegel.Settings.Generate ]
  ; verbosity = Hegel.Settings.Quiet
  }]
;;

let%hegel_test quiet_failing_example tc =
  let n = Hegel.draw tc (Hegel.integers ()) in
  assert (n <> 100)
[@@example { n = 100 }]
[@@settings { (Hegel.Settings.create ()) with verbosity = Hegel.Settings.Quiet }]
;;

let test_multiple_failures_with_examples () =
  (match example_and_generated_failures () with
   | () -> Alcotest.fail "expected failures"
   | exception Hegel.Multiple_failures exns ->
     Alcotest.(check int) "both failures" 2 (List.length exns));
  (match example_failure_only () with
   | () -> Alcotest.fail "expected the example to fail"
   | exception Failure msg -> Alcotest.(check string) "the example" "the example" msg);
  examples_skipped ();
  match quiet_failing_example () with
  | () -> Alcotest.fail "expected the example to fail"
  | exception Assert_failure _ -> ()
;;

let%hegel_test unused_example_value tc =
  let b = Hegel.draw tc (Hegel.booleans ()) in
  if b
  then (
    let n = Hegel.draw tc (Hegel.integers ()) in
    ignore (n : int))
[@@example { b = false; n = 3 }]
;;

let%hegel_test missing_example_value tc =
  let a = Hegel.draw tc (Hegel.integers ()) in
  let b = Hegel.draw tc (Hegel.integers ()) in
  ignore (a + b : int)
[@@example { a = 1 }]
;;

let%hegel_test silent_draw_in_example tc =
  let n = Hegel.draw tc (Hegel.integers ()) in
  ignore (n + Hegel.draw_silent tc (Hegel.integers ()) : int)
[@@example { n = 0 }]
;;

let%hegel_test function_in_example tc =
  let n = Hegel.draw tc (Hegel.integers ()) in
  let f = Hegel.draw_silent tc (Hegel.functions ~returns:(Hegel.integers ()) ()) in
  ignore (f n : int)
[@@example { n = 0 }]
;;

let test_example_usage_errors () =
  let expect name run substring =
    match run () with
    | () -> Alcotest.fail (name ^ ": expected Usage_error")
    | exception Hegel.Usage_error msg ->
      Alcotest.(check bool)
        (Printf.sprintf "%s: %S mentions %S" name msg substring)
        true
        (String.is_substring msg ~substring)
  in
  expect "unused value" unused_example_value "never drawn: n";
  expect "missing value" missing_example_value "no value for the draw b";
  expect "draw_silent" silent_draw_in_example "draw_silent";
  expect "silently drawn function" function_in_example "no value for the draw f";
  expect
    "explicit_draw outside an example"
    (fun () ->
       Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:1 ()) (fun tc ->
         ignore
           (Hegel.Generators.Ppx_internal.explicit_draw
              tc
              ~name:"x"
              ~source:"1"
              1
              (Hegel.integers ())
            : int)))
    "only be drawn in an explicit example"
;;

let test_clone_in_example () =
  let seen = ref [] in
  let explicit_draw tc name value =
    Hegel.Generators.Ppx_internal.explicit_draw
      tc
      ~name
      ~source:(string_of_int value)
      value
      (Hegel.integers ())
  in
  match
    Hegel.run_hegel_test_ppx
      ~settings:(Hegel.Settings.create ~test_cases:1 ())
      ~examples:
        [ ( [ "n"; "m" ]
          , fun tc ->
              let n = explicit_draw tc "n" 0 in
              let worker = Hegel.clone tc in
              Hegel.set_worker worker 1;
              let m = explicit_draw worker "m" 1 in
              Hegel.note worker "from the clone";
              seen := [ n; m ];
              failwith "boom" )
        ]
      (fun _ -> ())
  with
  | () -> Alcotest.fail "expected the example to fail"
  | exception Failure msg ->
    Alcotest.(check string) "the example" "boom" msg;
    Alcotest.(check (list int)) "the values" [ 0; 1 ] !seen
;;

let test_examples_without_location () =
  match
    Hegel.run_hegel_test_ppx
      ~examples:[ ([], fun _ -> failwith "no location") ]
      (fun _ -> ())
  with
  | () -> Alcotest.fail "expected the example to fail"
  | exception Failure msg -> Alcotest.(check string) "the example" "no location" msg
;;

let%hegel_test examples_and_blobs tc =
  let n = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:9 ()) in
  assert (n <> 100)
[@@example { n = 100 }] [@@failure_blobs [ "AAEAAAAACgEAAAAy" ]]
;;

let test_examples_with_failure_blobs () =
  match examples_and_blobs () with
  | () -> Alcotest.fail "expected the blob not to reproduce"
  | exception Hegel.Did_not_reproduce _ -> ()
;;

let%hegel_test rejecting_example tc =
  let n = Hegel.draw tc (Hegel.integers ()) in
  Hegel.assume tc (n <> 0);
  assert (n <> 0)
[@@example { n = 0 }]
[@@settings { (Hegel.Settings.create ()) with phases = [ Hegel.Settings.Explicit ] }]
;;

let%hegel_test explicit_reject_in_repeat tc =
  let n = Hegel.draw tc (Hegel.integers ()) in
  Hegel.repeat tc (fun tc ->
    Int.incr generated_calls;
    if n = 0 then Hegel.reject tc)
[@@example { n = 1 }]
[@@example { n = 0 }]
[@@settings { (Hegel.Settings.create ()) with phases = [ Hegel.Settings.Explicit ] }]
;;

let%hegel_test failing_repeat_example tc =
  let n = Hegel.draw tc (Hegel.integers ()) in
  Hegel.repeat tc (fun _ -> assert (n <> 7))
[@@example { n = 7 }]
[@@settings { (Hegel.Settings.create ()) with phases = [ Hegel.Settings.Explicit ] }]
;;

let check_rejected_example ~index test =
  match test () with
  | () -> Alcotest.fail "expected the rejected example to raise"
  | exception Hegel.Usage_error msg ->
    Alcotest.(check string)
      "message"
      (Printf.sprintf
         "Explicit test cases must not be rejected, but explicit example %d was rejected"
         index)
      msg
;;

let test_explicit_rejection_and_repeat () =
  check_rejected_example ~index:1 rejecting_example;
  generated_calls := 0;
  check_rejected_example ~index:2 explicit_reject_in_repeat;
  Alcotest.(check int) "both examples ran their iteration" 2 !generated_calls;
  match failing_repeat_example () with
  | () -> Alcotest.fail "expected the example to fail"
  | exception Assert_failure _ -> ()
;;

let () =
  Alcotest.run
    "hegel-ppx-hegel-test"
    [ ( "ppx_hegel_test"
      , [ Alcotest.test_case
            "passing test writes sdk.jsonl"
            `Quick
            test_passing_writes_sdk_jsonl
        ; Alcotest.test_case
            "failing test writes condition:false"
            `Quick
            test_failing_writes_condition_false
        ; Alcotest.test_case "let%hegel_main" `Quick test_main_reports
        ; Alcotest.test_case "examples run first" `Quick test_examples_run_first
        ; Alcotest.test_case
            "a failing example ends the run"
            `Quick
            test_failing_example_ends_the_run
        ; Alcotest.test_case
            "examples with multiple failures"
            `Quick
            test_multiple_failures_with_examples
        ; Alcotest.test_case "example usage errors" `Quick test_example_usage_errors
        ; Alcotest.test_case "clone in an example" `Quick test_clone_in_example
        ; Alcotest.test_case
            "examples without a location"
            `Quick
            test_examples_without_location
        ; Alcotest.test_case
            "examples with failure blobs"
            `Quick
            test_examples_with_failure_blobs
        ; Alcotest.test_case
            "explicit rejection and repeat"
            `Quick
            test_explicit_rejection_and_repeat
        ; Alcotest.test_case
            "no [@@settings] uses defaults"
            `Quick
            test_no_settings_runs_with_defaults
        ; Alcotest.test_case
            "module%hegel_state_machine collects [@@rule]/[@@invariant] bindings"
            `Quick
            test_state_machine_collects_marked_bindings
        ; Alcotest.test_case
            "module%hegel_concurrent_state_machine collects groups and weights"
            `Quick
            test_concurrent_state_machine_collects_groups_and_weights
        ] )
    ]
;;
