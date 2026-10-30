open! Core
module G = Hegel.Generators

let settings ?(test_cases = 20) () = Hegel.Settings.create ~test_cases ~seed:0 ()

let test_reject_skips_cases () =
  let accepted = ref 0 in
  Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
    if Hegel.draw_silent tc (G.booleans ()) then Hegel.reject tc else incr accepted);
  Alcotest.(check bool) "some cases accepted" true (!accepted > 0)
;;

let test_reject_everything_is_unsatisfiable () =
  Test_helpers.expect_unsatisfiable (fun () ->
    Hegel.run_hegel_test ~settings:(settings ()) (fun tc -> Hegel.reject tc))
;;

let test_repeat_runs_until_engine_stops () =
  let iterations = ref 0 in
  Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
    Hegel.repeat tc (fun tc ->
      incr iterations;
      ignore (Hegel.draw tc (G.integers ~min_value:0 ~max_value:10 ()) : int)));
  Alcotest.(check bool) "body ran" true (!iterations > 0)
;;

let test_repeat_continues_draw_names () =
  let names = ref [] in
  Hegel.run_hegel_test ~settings:(settings ~test_cases:1 ()) (fun tc ->
    let repetitions = ref 0 in
    names := [];
    Hegel.repeat tc (fun tc ->
      incr repetitions;
      if !repetitions <= 3
      then
        names := Hegel.Internal.draw_display_name tc ~label:"x" ~repeatable:true :: !names));
  Alcotest.(check (list string))
    "numbered across repetitions"
    [ "x_1"; "x_2"; "x_3" ]
    (List.rev !names)
;;

let test_repeat_skips_rejected_iterations () =
  let kept = ref 0 in
  Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
    Hegel.repeat tc (fun tc ->
      Hegel.assume tc (Hegel.draw_silent tc (G.booleans ()));
      incr kept));
  Alcotest.(check bool) "accepted iterations ran" true (!kept > 0)
;;

let test_repeat_failure_fails_the_test () =
  match
    Hegel.run_hegel_test ~settings:(settings ~test_cases:100 ()) (fun tc ->
      let total = ref 0 in
      Hegel.repeat tc (fun tc ->
        total := !total + Hegel.draw tc (G.integers ~min_value:0 ~max_value:10 ());
        if !total > 20 then failwith "total too large"))
  with
  | () -> Alcotest.fail "expected the repeated body to fail"
  | exception Failure msg ->
    Alcotest.(check string) "the body's failure" "total too large" msg
;;

let test_repeat_propagates_usage_errors () =
  match
    Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
      Hegel.repeat tc (fun tc ->
        ignore (Hegel.draw tc (G.text ~categories:[ "Cs" ] ~min_size:1 ()) : string)))
  with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Hegel.Usage_error _ -> ()
;;

let test_repeat_propagates_internal_errors () =
  match
    Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
      Hegel.repeat tc (fun _tc -> raise (Hegel.Internal.Internal_error "sentinel")))
  with
  | () -> Alcotest.fail "expected Internal_error"
  | exception Hegel.Internal.Internal_error message ->
    Alcotest.(check string) "the error" "sentinel" message
;;

let test_repeat_propagates_stop_test () =
  match
    Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
      Hegel.repeat tc (fun _tc -> raise Hegel.Internal.Stop_test))
  with
  | () -> Alcotest.fail "expected every case to overrun"
  | exception Hegel.Failed_health_check { check; _ } ->
    Alcotest.(check bool)
      "too large"
      true
      (Poly.equal check (Some Hegel.Settings.Test_cases_too_large))
;;

let test_example_propagates_internal_errors () =
  match
    Hegel.run_hegel_test_ppx
      ~settings:(settings ())
      ~examples:[ ([], fun _tc -> raise (Hegel.Internal.Internal_error "sentinel")) ]
      (fun _tc -> ())
  with
  | () -> Alcotest.fail "expected Internal_error"
  | exception Hegel.Internal.Internal_error message ->
    Alcotest.(check string) "the error" "sentinel" message
;;

let test_set_worker_rejects_negative_index () =
  match
    Hegel.run_hegel_test ~settings:(settings ~test_cases:1 ()) (fun tc ->
      Hegel.set_worker (Hegel.clone tc) (-1))
  with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Hegel.Usage_error _ -> ()
;;

let tests =
  [ Alcotest.test_case "reject skips cases" `Quick test_reject_skips_cases
  ; Alcotest.test_case
      "rejecting everything is unsatisfiable"
      `Quick
      test_reject_everything_is_unsatisfiable
  ; Alcotest.test_case
      "repeat runs until the engine stops"
      `Quick
      test_repeat_runs_until_engine_stops
  ; Alcotest.test_case
      "repeat continues draw names across repetitions"
      `Quick
      test_repeat_continues_draw_names
  ; Alcotest.test_case
      "repeat skips rejected iterations"
      `Quick
      test_repeat_skips_rejected_iterations
  ; Alcotest.test_case
      "a failing iteration fails the test"
      `Quick
      test_repeat_failure_fails_the_test
  ; Alcotest.test_case
      "repeat propagates usage errors"
      `Quick
      test_repeat_propagates_usage_errors
  ; Alcotest.test_case
      "repeat propagates internal errors"
      `Quick
      test_repeat_propagates_internal_errors
  ; Alcotest.test_case
      "repeat propagates Stop_test"
      `Quick
      test_repeat_propagates_stop_test
  ; Alcotest.test_case
      "an example propagates internal errors"
      `Quick
      test_example_propagates_internal_errors
  ; Alcotest.test_case
      "set_worker rejects a negative index"
      `Quick
      test_set_worker_rejects_negative_index
  ]
;;
