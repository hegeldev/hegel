open! Core
open Hegel
module Unix = Core_unix

(* ==== Settings and profiles ==== *)

let all_ci_vars =
  [ "CI"
  ; "TF_BUILD"
  ; "BUILDKITE"
  ; "CIRCLECI"
  ; "CIRRUS_CI"
  ; "CODEBUILD_BUILD_ID"
  ; "GITHUB_ACTIONS"
  ; "GITLAB_CI"
  ; "HEROKU_TEST_RUN_ID"
  ; "TEAMCITY_VERSION"
  ; "bamboo.buildKey"
  ; "HEGEL_DEFAULT_PROFILE"
  ]
;;

let with_ci_vars_cleared f =
  let saved = List.map all_ci_vars ~f:(fun v -> v, Sys.getenv v) in
  List.iter all_ci_vars ~f:Test_helpers.unsetenv;
  Exn.protect
    ~finally:(fun () ->
      List.iter saved ~f:(fun (k, v) ->
        match v with
        | Some v -> Unix.putenv ~key:k ~data:v
        | None -> Test_helpers.unsetenv k))
    ~f
;;

(* Outside CI the default profile is [development]: the base settings. *)
let test_default_settings_not_ci () =
  with_ci_vars_cleared (fun () ->
    let s = Settings.default () in
    Alcotest.(check int) "test_cases" 100 s.test_cases;
    Alcotest.(check bool) "verbosity normal" true (Poly.equal s.verbosity Settings.Normal);
    Alcotest.(check (option int)) "no seed" None s.seed;
    Alcotest.(check bool) "derandomize off" false s.derandomize;
    Alcotest.(check bool) "database unset" true (Poly.equal s.database Settings.Unset);
    Alcotest.(check bool)
      "no health checks suppressed"
      true
      (List.is_empty s.suppress_health_check);
    Alcotest.(check int) "all phases" 5 (List.length s.phases);
    Alcotest.(check bool) "print_blob on" true s.print_blob;
    Alcotest.(check bool) "report_multiple_failures off" false s.report_multiple_failures;
    Alcotest.(check bool) "stop_on_first_failure on" true s.stop_on_first_failure;
    Alcotest.(check bool) "show_statistics off" false s.show_statistics;
    Alcotest.(check bool)
      "development is the base settings"
      true
      (Poly.equal s (Settings.from_profile "base"));
    Alcotest.(check bool)
      "same as from_profile development"
      true
      (Poly.equal s (Settings.from_profile "development"));
    Alcotest.(check bool) "backend default" true (Poly.equal s.backend Settings.Default);
    Alcotest.(check bool)
      "nondeterminism_strictness quiet"
      true
      (Poly.equal s.nondeterminism_strictness Settings.Nondeterminism_strictness.Quiet);
    Alcotest.(check bool)
      "workload backend urandom"
      true
      (Poly.equal (Settings.from_profile "workload").backend Settings.Urandom))
;;

(* On a CI server the engine selects the shipped [ci] profile. *)
let test_default_settings_ci () =
  with_ci_vars_cleared (fun () ->
    Unix.putenv ~key:"CI" ~data:"1";
    let s = Settings.default () in
    Alcotest.(check bool) "derandomize on" true s.derandomize;
    Alcotest.(check bool)
      "database disabled"
      true
      (Poly.equal s.database Settings.Disabled);
    Alcotest.(check bool)
      "too_slow suppressed"
      true
      (Poly.equal s.suppress_health_check [ Settings.Too_slow ]);
    Alcotest.(check bool) "print_blob on" true s.print_blob;
    Alcotest.(check bool)
      "same as from_profile ci"
      true
      (Poly.equal s (Settings.from_profile "ci")))
;;

let test_from_profile_unknown () =
  match Settings.from_profile "hegel_ocaml_no_such_profile" with
  | _ -> Alcotest.fail "expected Usage_error"
  | exception Usage_error msg ->
    Alcotest.(check bool)
      "message names the profile"
      true
      (Test_helpers.contains_substring msg "hegel_ocaml_no_such_profile")
;;

let test_register_profile_round_trip () =
  List.iteri
    Settings.[ Quiet; Normal; Verbose; Debug ]
    ~f:(fun i verbosity ->
      let name = sprintf "hegel_ocaml_test_round_trip_%d" i in
      let s =
        { Settings.test_cases = 7 + i
        ; verbosity
        ; seed = Some (40 + i)
        ; derandomize = true
        ; database =
            (match i with
             | 0 -> Settings.Unset
             | 1 -> Settings.Disabled
             | _ -> Settings.Path "/tmp/hegel-ocaml-profile-db")
        ; suppress_health_check =
            (if i mod 2 = 0
             then
               Settings.
                 [ Filter_too_much
                 ; Too_slow
                 ; Test_cases_too_large
                 ; Large_initial_test_case
                 ]
             else [ Settings.Filter_too_much; Settings.Large_initial_test_case ])
        ; phases =
            (if i mod 2 = 0
             then Settings.[ Explicit; Reuse; Generate; Target; Shrink ]
             else [ Settings.Generate; Settings.Shrink ])
        ; print_blob = i mod 2 = 0
        ; report_multiple_failures = true
        ; stop_on_first_failure = i mod 2 = 1
        ; show_statistics = true
        ; backend = (if i mod 2 = 0 then Settings.Default else Settings.Urandom)
        ; nondeterminism_strictness =
            (match i mod 3 with
             | 0 -> Quiet
             | 1 -> Warn
             | _ -> Error)
        }
      in
      Settings.register_profile name s;
      Alcotest.(check bool) name true (Poly.equal s (Settings.from_profile name)))
;;

let test_set_default_profile () =
  let name = "hegel_ocaml_test_default" in
  Settings.register_profile name { (Settings.default ()) with test_cases = 7 };
  Settings.set_default_profile (Some name);
  Exn.protect
    ~finally:(fun () -> Settings.set_default_profile None)
    ~f:(fun () ->
      Alcotest.(check int)
        "default follows the override"
        7
        (Settings.default ()).test_cases);
  Alcotest.(check int) "cleared" 100 (Settings.default ()).test_cases
;;

(* [hegel.toml] is loaded once per process, so it is exercised in a child: the
   parent writes a config, points [HEGEL_CONFIG] at it, and re-executes this
   binary with [HEGEL_TEST_CONFIG_CHILD] set, which makes the runner call
   [config_child] and exit. *)
let config_child () =
  let n = (Settings.default ()).test_cases in
  if n <> 7 then failwithf "default: expected 7 test cases from hegel.toml, got %d" n ();
  let n = (Settings.from_profile "nightly").test_cases in
  if n <> 7 then failwithf "nightly: expected 7 test cases from hegel.toml, got %d" n ();
  let n = (Settings.from_profile "base").test_cases in
  if n <> 100 then failwithf "base: expected the base 100 test cases, got %d" n ();
  if not (Settings.default ()).print_blob then failwith "default: expected print_blob on";
  if (Settings.from_profile "noblob").print_blob
  then failwith "noblob: expected print_blob off from hegel.toml";
  if
    not
      (Poly.equal
         (Settings.from_profile "noblob").nondeterminism_strictness
         Settings.Nondeterminism_strictness.Warn)
  then failwith "noblob: expected nondeterminism_strictness warn from hegel.toml"
;;

let test_hegel_toml_config () =
  Test_helpers.with_tempdir ~prefix:"hegel-toml" ~f:(fun dir ->
    let path = Filename.concat dir "hegel.toml" in
    Out_channel.write_all
      path
      ~data:
        "default = \"nightly\"\n\n\
         [profiles.nightly]\n\
         test_cases = 7\n\n\
         [profiles.noblob]\n\
         print_blob = false\n\
         nondeterminism_strictness = \"warn\"\n";
    let pid =
      Unix.create_process_env
        ~prog:Stdlib.Sys.executable_name
        ~args:[]
        ~env:(`Extend [ "HEGEL_CONFIG", path; "HEGEL_TEST_CONFIG_CHILD", "1" ])
        ()
    in
    match Unix.waitpid pid.pid with
    | Ok () -> ()
    | Error _ as status ->
      Alcotest.failf
        "child reading hegel.toml failed: %s"
        (Unix.Exit_or_signal.to_string_hum status))
;;

let test_settings_create () =
  let s = Settings.create ~test_cases:42 ~seed:7 () in
  Alcotest.(check int) "test_cases" 42 s.test_cases;
  Alcotest.(check (option int)) "seed" (Some 7) s.seed;
  let d = Settings.create () in
  Alcotest.(check int) "default test_cases" 100 d.test_cases;
  Alcotest.(check (option int)) "no seed" None d.seed
;;

let test_health_check_to_string () =
  Alcotest.(check string)
    "filter"
    "filter_too_much"
    (Settings.health_check_to_string Settings.Filter_too_much);
  Alcotest.(check string)
    "slow"
    "too_slow"
    (Settings.health_check_to_string Settings.Too_slow);
  Alcotest.(check string)
    "large"
    "test_cases_too_large"
    (Settings.health_check_to_string Settings.Test_cases_too_large);
  Alcotest.(check string)
    "initial"
    "large_initial_test_case"
    (Settings.health_check_to_string Settings.Large_initial_test_case)
;;

let test_phase_to_string () =
  Alcotest.(check string)
    "explicit"
    "explicit"
    (Settings.phase_to_string Settings.Explicit);
  Alcotest.(check string) "reuse" "reuse" (Settings.phase_to_string Settings.Reuse);
  Alcotest.(check string)
    "generate"
    "generate"
    (Settings.phase_to_string Settings.Generate);
  Alcotest.(check string) "target" "target" (Settings.phase_to_string Settings.Target);
  Alcotest.(check string) "shrink" "shrink" (Settings.phase_to_string Settings.Shrink)
;;

(* With backtrace recording off the failure has no location, and the run
   still reports it. *)
let test_failure_without_backtrace () =
  let was = Stdlib.Printexc.backtrace_status () in
  Stdlib.Printexc.record_backtrace false;
  Exn.protect
    ~finally:(fun () -> Stdlib.Printexc.record_backtrace was)
    ~f:(fun () ->
      match
        run_hegel_test
          ~settings:
            { (Hegel.Settings.create ~test_cases:10 ()) with
              database = Settings.Disabled
            ; verbosity = Settings.Quiet
            }
          (fun _tc -> failwith "boom")
      with
      | () -> Alcotest.fail "expected a failure"
      | exception Failure msg -> Alcotest.(check string) "the failure" "boom" msg)
;;

(* [Lazy.force] raises through [CamlinternalLazy] frames, which the failure
   origin skips, so failures forced at different lines stay distinct. *)
let test_lazy_failures_at_different_lines_are_distinct () =
  match
    run_hegel_test
      ~settings:
        { (Hegel.Settings.create ~test_cases:300 ~seed:9 ()) with
          report_multiple_failures = true
        ; database = Settings.Disabled
        ; verbosity = Settings.Quiet
        }
      (fun tc ->
         let v = Hegel.draw tc (Hegel.integers ~min_value:0 ~max_value:100 ()) in
         if v >= 60 then Lazy.force (lazy (failwith "a"));
         if v <= 30 then Lazy.force (lazy (failwith "b")))
  with
  | () -> Alcotest.fail "expected failures"
  | exception Failure msg ->
    Alcotest.(check string) "two distinct failures" "2 failures found!" msg
;;

(* ==== Sexp renderer tests ==== *)

let render_to_string ~max_width sexp =
  let module Ffi = Hegel_ffi.Ffi in
  let ctx = Ffi.context_new () in
  Exn.protect
    ~finally:(fun () -> Ffi.context_free ctx)
    ~f:(fun () ->
      let options = Ffi.printer_options_new ctx in
      Ffi.printer_options_set_max_width ctx options max_width;
      let p = Ffi.printer_new ctx (Some options) in
      Ffi.printer_options_free ctx options;
      Exn.protect
        ~finally:(fun () -> Ffi.printer_free ctx p)
        ~f:(fun () ->
          Internal.render_sexp ctx p sexp;
          Ffi.printer_value ctx p))
;;

let test_render_sexp_atoms () =
  Alcotest.(check string) "bare" "foo" (render_to_string ~max_width:79 (Sexp.Atom "foo"));
  Alcotest.(check string)
    "escaped"
    "\"needs quoting\""
    (render_to_string ~max_width:79 (Sexp.Atom "needs quoting"));
  Alcotest.(check string)
    "newline never literal"
    "\"a\\nb\""
    (render_to_string ~max_width:79 (Sexp.Atom "a\nb"))
;;

let test_render_sexp_fits_matches_to_string_hum () =
  let sexps =
    [ Sexp.List []
    ; Sexp.List [ Sexp.Atom "a" ]
    ; Sexp.List
        [ Sexp.Atom "a"; Sexp.List []; Sexp.List [ Sexp.Atom "b"; Sexp.Atom "c" ] ]
    ]
  in
  List.iter sexps ~f:(fun sexp ->
    Alcotest.(check string)
      "one-line rendering agrees with to_string_hum"
      (Sexp.to_string_hum sexp)
      (render_to_string ~max_width:79 sexp))
;;

let test_render_sexp_breaks_when_narrow () =
  let sexp =
    Sexp.List [ Sexp.Atom "a"; Sexp.List []; Sexp.List [ Sexp.Atom "b"; Sexp.Atom "c" ] ]
  in
  Alcotest.(check string)
    "outer breaks, inner stays inline"
    "(a\n ()\n (b c))"
    (render_to_string ~max_width:7 sexp);
  Alcotest.(check string)
    "trailing paren glues, forcing the inner group to break too"
    "(a\n ()\n (b\n  c))"
    (render_to_string ~max_width:6 sexp);
  Alcotest.(check string)
    "nested break indents past both parens"
    "(a\n (bbbbb\n  ccccc))"
    (render_to_string
       ~max_width:8
       (Sexp.List [ Sexp.Atom "a"; Sexp.List [ Sexp.Atom "bbbbb"; Sexp.Atom "ccccc" ] ]))
;;

(* ==== Real-engine run tests ==== *)

let int_gen = integers ~min_value:0 ~max_value:100 ()

exception Too_big of int

let test_run_quiet_raises_shrunk_failure () =
  match
    run_hegel_test
      ~settings:
        { (Hegel.Settings.create ~test_cases:200 ~seed:1 ()) with
          verbosity = Hegel.Settings.Quiet
        ; database = Hegel.Settings.Disabled
        }
      (fun tc ->
         let v = Hegel.draw tc int_gen in
         if v >= 10 then raise (Too_big v))
  with
  | () -> Alcotest.fail "expected a failure"
  | exception Too_big v -> Alcotest.(check int) "the shrunk value" 10 v
;;

let test_run_usage_error_propagates () =
  match
    run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:1 ()) (fun _tc ->
      raise (Usage_error "usage sentinel"))
  with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Usage_error message ->
    Alcotest.(check string) "usage error message" "usage sentinel" message
  | exception exn -> raise exn
;;

let test_run_internal_error_propagates () =
  match
    run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:1 ()) (fun _tc ->
      raise (Internal.Internal_error "internal sentinel"))
  with
  | () -> Alcotest.fail "expected Internal_error"
  | exception Internal.Internal_error message ->
    Alcotest.(check string) "internal error message" "internal sentinel" message
  | exception exn -> raise exn
;;

(** [assume false] rejects cases without failing the run. *)
let test_run_assume_rejects () =
  let rejected = ref false in
  let accepted = ref false in
  run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:20 ~seed:0 ()) (fun tc ->
    let v = Hegel.draw tc int_gen in
    if v < 50 then rejected := true;
    assume tc (v >= 50);
    assert (v >= 50);
    accepted := true);
  Alcotest.(check bool) "attempted rejected cases" true !rejected;
  Alcotest.(check bool) "accepted cases still run" true !accepted
;;

(** Nesting test cases is rejected. *)
let test_run_nested_guard () =
  let got_failure = ref false in
  (try
     run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:5 ()) (fun _tc ->
       run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:1 ()) (fun _ -> ()))
   with
   | Failure msg when Test_helpers.contains_substring msg "nest" -> got_failure := true
   | _ -> ());
  Alcotest.(check bool) "nested run rejected" true !got_failure
;;

(** A non-finite [target] observation is an engine-side argument error. *)
let test_target_non_finite () =
  match
    run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:5 ()) (fun tc ->
      target tc ~label:"x" ~value:Float.nan)
  with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Hegel.Usage_error _ -> ()
;;

(** A non-finite [event_value] observation is an engine-side argument error *)
let test_event_value_non_finite () =
  match
    run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:5 ()) (fun tc ->
      event_value tc ~label:"x" ~value:Float.nan)
  with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Hegel.Usage_error _ -> ()
;;

(** A non-UTF-8 [event] label is an engine-side argument error. *)
let test_event_bad_label () =
  match
    run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:5 ()) (fun tc ->
      event tc ~label:"\xff")
  with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Hegel.Usage_error _ -> ()
;;

(* A failure saved under a [database_key] is replayed by a later run with the
   same key, and not by one with a different key. *)
let test_database_key () =
  Test_helpers.with_tempdir ~prefix:"hegel-db" ~f:(fun dir ->
    let run ~phases ~key =
      run_hegel_test
        ~settings:
          { (Hegel.Settings.create ~test_cases:100 ~seed:1 ()) with
            database = Settings.Path dir
          ; verbosity = Settings.Quiet
          ; phases
          }
        ~database_key:key
        (fun tc -> if Hegel.draw tc int_gen >= 50 then failwith "big")
    in
    (match
       run ~phases:Settings.[ Explicit; Reuse; Generate; Target; Shrink ] ~key:"k"
     with
     | () -> Alcotest.fail "expected the first run to fail"
     | exception Failure _ -> ());
    (match run ~phases:[ Settings.Reuse ] ~key:"k" with
     | () -> Alcotest.fail "expected the saved failure to be replayed"
     | exception Failure _ -> ());
    run ~phases:[ Settings.Reuse ] ~key:"other")
;;

(* ==== Test location (the engine's Antithesis reporting) ==== *)

let antithesis_env = "ANTITHESIS_OUTPUT_DIR"

(* Run [f] with [ANTITHESIS_OUTPUT_DIR] pointing at a fresh tempdir, restoring
   the previous value afterwards. *)
let with_antithesis_dir f =
  Test_helpers.with_tempdir ~prefix:"hegel-antithesis" ~f:(fun dir ->
    let prev = Sys.getenv antithesis_env in
    Unix.putenv ~key:antithesis_env ~data:dir;
    Exn.protect
      ~finally:(fun () ->
        match prev with
        | Some v -> Unix.putenv ~key:antithesis_env ~data:v
        | None -> Test_helpers.unsetenv antithesis_env)
      ~f:(fun () -> f dir))
;;

let sample_location =
  { function_name = "my_test"; file = "tests/test_basic.ml"; begin_line = 42 }
;;

(* Inside Antithesis the engine reports a run started with a [test_location] as
   an [always] assertion in [sdk.jsonl], identified by module and function. *)
let test_test_location_reports_result () =
  with_antithesis_dir (fun dir ->
    let settings = Hegel.Settings.create ~test_cases:3 () in
    run_hegel_test ~settings ~test_location:sample_location (fun tc ->
      ignore (Hegel.draw tc int_gen : int));
    let lines () =
      In_channel.read_all (Filename.concat dir "sdk.jsonl") |> String.split_lines
    in
    (* The SDK format: a declaration line ([hit:false]) then the result. *)
    let passed = lines () in
    Alcotest.(check int) "declaration + result" 2 (List.length passed);
    let result = List.last_exn passed in
    let has s = String.is_substring result ~substring:s in
    Alcotest.(check bool)
      "assertion id"
      true
      (has "\"id\":\"tests/test_basic::my_test passes properties\"");
    Alcotest.(check bool) "always assertion" true (has "\"assert_type\":\"always\"");
    Alcotest.(check bool) "hit" true (has "\"hit\":true");
    Alcotest.(check bool) "passing result" true (has "\"condition\":true");
    Alcotest.(check bool)
      "location"
      true
      (has
         "\"location\":{\"class\":\"tests/test_basic\",\"function\":\"my_test\",\"file\":\"tests/test_basic.ml\",\"begin_line\":42");
    (match
       run_hegel_test ~settings ~test_location:sample_location (fun _tc ->
         failwith "deliberate failure")
     with
     | () -> Alcotest.fail "expected the failing run to raise"
     | exception Failure _ -> ());
    (* The failing run reports twice: the run's result and the final replay's
       (a test case replayed from a blob). Every new result is a failure. *)
    let results =
      List.drop (lines ()) 2
      |> List.filter ~f:(String.is_substring ~substring:"\"hit\":true")
    in
    Alcotest.(check bool) "at least one failing result" false (List.is_empty results);
    List.iter results ~f:(fun v ->
      Alcotest.(check bool)
        "failing result"
        true
        (String.is_substring v ~substring:"\"condition\":false")))
;;

(* Without a location nothing is written *)
let test_no_test_location_reports_nothing () =
  with_antithesis_dir (fun dir ->
    run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:3 ()) (fun tc ->
      ignore (Hegel.draw tc int_gen : int));
    Alcotest.(check bool)
      "no sdk.jsonl"
      false
      (Stdlib.Sys.file_exists (Filename.concat dir "sdk.jsonl")))
;;

(* rejects a location string that is not valid UTF-8. *)
let test_test_location_invalid_utf8 () =
  match
    run_hegel_test
      ~settings:(Hegel.Settings.create ~test_cases:1 ())
      ~test_location:{ sample_location with file = "\xff.ml" }
      (fun _tc -> ())
  with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Usage_error _ -> ()
;;

(** A health-check failure is a run-level error (no counterexample), surfaced as
    a [Failure] carrying the engine's error message. *)
let test_run_health_check_failure () =
  let raised =
    try
      run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:50 ()) (fun tc ->
        let v = Hegel.draw tc int_gen in
        (* Always-false precondition: every case is invalid →
           FilterTooMuch. *)
        assume tc (v > 1_000_000));
      false
    with
    | Failure msg -> String.is_substring msg ~substring:"FailedHealthCheck: FilterTooMuch"
    | _ -> false
  in
  Alcotest.(check bool) "filter health-check diagnostic surfaced" true raised
;;

let tests =
  [ Alcotest.test_case "default settings non-ci" `Quick test_default_settings_not_ci
  ; Alcotest.test_case "default settings ci" `Quick test_default_settings_ci
  ; Alcotest.test_case "from_profile unknown" `Quick test_from_profile_unknown
  ; Alcotest.test_case
      "register_profile round trip"
      `Quick
      test_register_profile_round_trip
  ; Alcotest.test_case "set_default_profile" `Quick test_set_default_profile
  ; Alcotest.test_case "hegel.toml via HEGEL_CONFIG" `Quick test_hegel_toml_config
  ; Alcotest.test_case "Settings.create" `Quick test_settings_create
  ; Alcotest.test_case "health_check_to_string" `Quick test_health_check_to_string
  ; Alcotest.test_case "phase_to_string" `Quick test_phase_to_string
  ; Alcotest.test_case
      "test_location reports result"
      `Quick
      test_test_location_reports_result
  ; Alcotest.test_case
      "no test_location reports nothing"
      `Quick
      test_no_test_location_reports_nothing
  ; Alcotest.test_case "test_location invalid utf8" `Quick test_test_location_invalid_utf8
  ; Alcotest.test_case
      "run quiet raises the shrunk failure"
      `Quick
      test_run_quiet_raises_shrunk_failure
  ; Alcotest.test_case "run Usage_error propagates" `Quick test_run_usage_error_propagates
  ; Alcotest.test_case
      "run Internal_error propagates"
      `Quick
      test_run_internal_error_propagates
  ; Alcotest.test_case "run assume rejects" `Quick test_run_assume_rejects
  ; Alcotest.test_case "run nested guard" `Quick test_run_nested_guard
  ; Alcotest.test_case "render_sexp atoms" `Quick test_render_sexp_atoms
  ; Alcotest.test_case
      "render_sexp fits"
      `Quick
      test_render_sexp_fits_matches_to_string_hum
  ; Alcotest.test_case "render_sexp breaks" `Quick test_render_sexp_breaks_when_narrow
  ; Alcotest.test_case "event_value non-finite" `Quick test_event_value_non_finite
  ; Alcotest.test_case "event bad label" `Quick test_event_bad_label
  ; Alcotest.test_case "run health-check failure" `Quick test_run_health_check_failure
  ; Alcotest.test_case "failure without backtrace" `Quick test_failure_without_backtrace
  ; Alcotest.test_case
      "lazy failures at different lines are distinct"
      `Quick
      test_lazy_failures_at_different_lines_are_distinct
  ; Alcotest.test_case "target non-finite" `Quick test_target_non_finite
  ; Alcotest.test_case "database_key" `Quick test_database_key
  ]
;;
