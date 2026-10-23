open! Core
open Hegel
module Unix = Core_unix

let parse_with ~default_settings args =
  let buf = Buffer.create 64 in
  let err = Format.formatter_of_buffer buf in
  let cmd =
    Cmdliner.Cmd.v (Cmdliner.Cmd.info "prog") (Cli.settings_term ~default_settings)
  in
  match Cmdliner.Cmd.eval_value ~err ~argv:(Array.of_list ("prog" :: args)) cmd with
  | Ok (`Ok resolved) -> Ok resolved
  | Ok (`Help | `Version) -> Alcotest.fail "unexpected help"
  | Error _ ->
    Format.pp_print_flush err ();
    Error (Buffer.contents buf)
;;

let parse args = parse_with ~default_settings:(fun () -> Settings.create ~seed:3 ()) args

let parse_exn args =
  match parse args with
  | Ok resolved -> resolved
  | Error message -> Alcotest.failf "%s: %s" (String.concat ~sep:" " args) message
;;

let expect_error args substring =
  match parse args with
  | Ok _ -> Alcotest.failf "%s: expected an error" (String.concat ~sep:" " args)
  | Error message ->
    Alcotest.(check bool)
      (Printf.sprintf "%S contains %S" message substring)
      true
      (String.is_substring message ~substring)
;;

let test_no_flags () =
  let settings, blob = parse_exn [] in
  Alcotest.(check (option int)) "default settings" (Some 3) settings.seed;
  Alcotest.(check (option string)) "no blob" None blob
;;

let test_seed () =
  Alcotest.(check (option int)) "int" (Some 42) (fst (parse_exn [ "--seed"; "42" ])).seed;
  Alcotest.(check (option int)) "none" None (fst (parse_exn [ "--seed"; "none" ])).seed;
  expect_error [ "--seed"; "x" ] "--seed expects an integer or 'none'"
;;

let test_enums () =
  let settings, _ =
    parse_exn [ "--verbosity"; "quiet"; "--derandomize"; "yes"; "--backend"; "urandom" ]
  in
  Alcotest.(check bool) "verbosity" true (Poly.equal settings.verbosity Settings.Quiet);
  Alcotest.(check bool) "derandomize" true settings.derandomize;
  Alcotest.(check bool) "backend" true (Poly.equal settings.backend Settings.Urandom);
  expect_error [ "--verbosity"; "loud" ] "--verbosity"
;;

let test_database () =
  let database args = (fst (parse_exn ("--database" :: args))).database in
  Alcotest.(check bool)
    "disabled"
    true
    (Poly.equal (database [ "disabled" ]) Settings.Disabled);
  Alcotest.(check bool)
    "default"
    true
    (Poly.equal (database [ "default" ]) Settings.Unset);
  Alcotest.(check bool) "path" true (Poly.equal (database [ "db" ]) (Settings.Path "db"))
;;

let test_suppress_health_check () =
  let checks value =
    (fst (parse_exn [ "--suppress-health-check"; value ])).suppress_health_check
  in
  Alcotest.(check int) "all" 4 (List.length (checks "all"));
  Alcotest.(check bool)
    "list"
    true
    (Poly.equal
       (checks "filter_too_much, too_slow,test_cases_too_large,large_initial_test_case")
       [ Settings.Filter_too_much
       ; Too_slow
       ; Test_cases_too_large
       ; Large_initial_test_case
       ]);
  expect_error [ "--suppress-health-check"; "too_fast" ] "got \"too_fast\""
;;

let test_failure_blob () =
  Alcotest.(check (option string))
    "blob"
    (Some "abc")
    (snd (parse_exn [ "--failure-blob"; "abc" ]))
;;

let test_unknown_flag () = expect_error [ "--bogus" ] "unknown option"

let test_profile () =
  Exn.protect
    ~finally:(fun () -> Settings.set_default_profile None)
    ~f:(fun () ->
      let settings, _ =
        parse_with ~default_settings:Settings.default [ "--profile"; "ci" ]
        |> Result.ok_or_failwith
      in
      Alcotest.(check bool) "ci profile applies" true settings.derandomize;
      Settings.set_default_profile None;
      expect_error [ "--profile"; "no_such_profile" ] "no_such_profile";
      expect_error [ "--profile"; "bad name!" ] "bad name!";
      (* The rejected names are not left as the default profile. *)
      ignore (Settings.default () : Settings.t))
;;

let test_single_test_case () =
  let settings =
    Cli.single_test_case
      { (Settings.create ~test_cases:100 ()) with
        suppress_health_check = [ Settings.Filter_too_much ]
      }
  in
  Alcotest.(check int) "one test case" 1 settings.test_cases;
  Alcotest.(check bool)
    "suppressions added"
    true
    (List.for_all
       [ Settings.Too_slow; Test_cases_too_large; Filter_too_much ]
       ~f:(fun check -> List.mem settings.suppress_health_check check ~equal:Poly.equal))
;;

(* [Hegel.main] exits, so it runs in a child: this binary re-executed with
   [HEGEL_TEST_MAIN_CHILD] set, which makes [test_hegel.ml] call [main_child]. *)
let main_child mode =
  let log line =
    Out_channel.with_file
      (Sys.getenv_exn "HEGEL_TEST_MAIN_LOG")
      ~append:true
      ~f:(fun oc -> Out_channel.output_string oc (line ^ "\n"))
  in
  match mode with
  | "pass" ->
    Hegel.main (fun tc ->
      let valid = draw tc (booleans ()) in
      log (if valid then "valid" else "rejected");
      assume tc valid)
  | _ ->
    Hegel.main (fun tc ->
      let n = draw tc (integers ~min_value:0 ~max_value:100 ()) in
      if n >= 0 then failwith "deliberate failure")
;;

let run_child ~mode ~log args =
  let process =
    Unix.create_process_env
      ~prog:Stdlib.Sys.executable_name
      ~args:("--database" :: "disabled" :: args)
      ~env:(`Extend [ "HEGEL_TEST_MAIN_CHILD", mode; "HEGEL_TEST_MAIN_LOG", log ])
      ()
  in
  Unix.close process.stdin;
  let stdout = In_channel.input_all (Unix.in_channel_of_descr process.stdout) in
  let stderr = In_channel.input_all (Unix.in_channel_of_descr process.stderr) in
  Unix.waitpid process.pid, stdout ^ stderr
;;

let test_main_passes () =
  Test_helpers.with_tempdir ~prefix:"hegel-main" ~f:(fun dir ->
    let log = Filename.concat dir "log" in
    let status, output = run_child ~mode:"pass" ~log [] in
    (match status with
     | Ok () -> ()
     | Error _ -> Alcotest.failf "main failed: %s" output);
    let lines = In_channel.read_lines log in
    Alcotest.(check int)
      "exactly one valid case"
      1
      (List.count lines ~f:(String.equal "valid"));
    Alcotest.(check string) "the valid case is the last" "valid" (List.last_exn lines))
;;

let test_main_rejects_bad_flags () =
  Test_helpers.with_tempdir ~prefix:"hegel-main" ~f:(fun dir ->
    let status, output =
      run_child ~mode:"pass" ~log:(Filename.concat dir "log") [ "--bogus" ]
    in
    Alcotest.(check bool)
      "exit status 124"
      true
      (Poly.equal status (Error (`Exit_non_zero 124)));
    Alcotest.(check bool)
      "names the flag"
      true
      (String.is_substring output ~substring:"--bogus"))
;;

let test_main_failure_replays_from_blob () =
  Test_helpers.with_tempdir ~prefix:"hegel-main" ~f:(fun dir ->
    let log = Filename.concat dir "log" in
    let status, output = run_child ~mode:"fail" ~log [] in
    Alcotest.(check bool) "fails" true (Result.is_error status);
    let prefix = "rerun with: --failure-blob " in
    let blob =
      match List.find_map (String.split_lines output) ~f:(String.chop_prefix ~prefix) with
      | Some blob -> blob
      | None -> Alcotest.failf "no %S line in:\n%s" prefix output
    in
    let status, output = run_child ~mode:"fail" ~log [ "--failure-blob"; blob ] in
    Alcotest.(check bool) "replay fails" true (Result.is_error status);
    Alcotest.(check bool)
      "replay reports the failure"
      true
      (String.is_substring output ~substring:"deliberate failure"))
;;

let tests =
  [ Alcotest.test_case "no flags" `Quick test_no_flags
  ; Alcotest.test_case "--seed" `Quick test_seed
  ; Alcotest.test_case "enum flags" `Quick test_enums
  ; Alcotest.test_case "--database" `Quick test_database
  ; Alcotest.test_case "--suppress-health-check" `Quick test_suppress_health_check
  ; Alcotest.test_case "--failure-blob" `Quick test_failure_blob
  ; Alcotest.test_case "unknown flag" `Quick test_unknown_flag
  ; Alcotest.test_case "--profile" `Quick test_profile
  ; Alcotest.test_case "single_test_case" `Quick test_single_test_case
  ; Alcotest.test_case "main passes" `Quick test_main_passes
  ; Alcotest.test_case "main rejects bad flags" `Quick test_main_rejects_bad_flags
  ; Alcotest.test_case
      "main failure replays from blob"
      `Quick
      test_main_failure_replays_from_blob
  ]
;;
