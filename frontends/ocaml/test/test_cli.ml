open! Core
open Hegel
module Unix = Core_unix

(* cmdliner styles its errors with ANSI escapes on some terminals *)
let strip_ansi s =
  let b = Buffer.create (String.length s) in
  let rec go i =
    if i < String.length s
    then
      if Char.equal s.[i] '\027'
      then Option.iter (String.index_from s i 'm') ~f:(fun j -> go (j + 1))
      else (
        Buffer.add_char b s.[i];
        go (i + 1))
  in
  go 0;
  Buffer.contents b
;;

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
    Error (strip_ansi (Buffer.contents buf))
;;

let parse args = parse_with ~default_settings:(fun () -> Settings.create ~seed:3 ()) args

let expect_error args substring =
  match parse args with
  | Ok _ -> Alcotest.failf "%s: expected an error" (String.concat ~sep:" " args)
  | Error message ->
    Alcotest.(check bool)
      (Printf.sprintf "%S contains %S" message substring)
      true
      (String.is_substring message ~substring)
;;

type parsed = Settings.t * string option * string list

(* Each flag draws its argument and the change it should make to the parsed
   settings, database key and failure blobs. *)
let flags : (test_case -> string * (parsed -> parsed)) list =
  let choose tc values = draw_silent tc (sampled_from values) in
  let setting (f : Settings.t -> Settings.t) ((s, key, blobs) : parsed) : parsed =
    f s, key, blobs
  in
  let boolean name (set : Settings.t -> bool -> Settings.t) tc =
    let value, b =
      choose
        tc
        [ "true", true; "1", true; "yes", true; "false", false; "0", false; "no", false ]
    in
    sprintf "--%s=%s" name value, setting (fun s -> set s b)
  in
  let subset tc all =
    let picked =
      draw_silent
        tc
        (lists (with_printer (fun (name, _) -> Sexp.Atom name) (sampled_from all)) ())
    in
    String.concat ~sep:"," (List.map picked ~f:fst), List.map picked ~f:snd
  in
  let health_checks =
    [ "filter_too_much", Settings.Filter_too_much
    ; "too_slow", Too_slow
    ; "test_cases_too_large", Test_cases_too_large
    ; "large_initial_test_case", Large_initial_test_case
    ]
  in
  let phases =
    [ "explicit", Settings.Explicit
    ; "reuse", Reuse
    ; "generate", Generate
    ; "target", Target
    ; "shrink", Shrink
    ]
  in
  let in_order all picked =
    List.filter (List.map all ~f:snd) ~f:(fun x -> List.mem picked x ~equal:Poly.equal)
  in
  [ (fun tc ->
      let half = integers ~min_value:0 ~max_value:0xFFFF_FFFF () in
      match draw_silent tc (optional (tuples2 half half)) with
      | None -> "--seed=none", setting (fun s -> { s with seed = None })
      | Some (hi, lo) ->
        let n = Unsigned.UInt64.(logor (shift_left (of_int hi) 32) (of_int lo)) in
        ( sprintf "--seed=%s" (Unsigned.UInt64.to_string n)
        , setting (fun s -> { s with seed = Some n }) ))
  ; (fun tc ->
      let name, verbosity =
        choose
          tc
          [ "quiet", Settings.Quiet
          ; "normal", Normal
          ; "verbose", Verbose
          ; "debug", Debug
          ]
      in
      "--verbosity=" ^ name, setting (fun s -> { s with verbosity }))
  ; boolean "derandomize" (fun s derandomize -> { s with derandomize })
  ; (fun tc ->
      let name, database =
        match draw_silent tc (integers ~min_value:0 ~max_value:2 ()) with
        | 0 -> "disabled", Settings.Disabled
        | 1 -> "default", Unset
        | _ ->
          let path =
            draw_silent
              tc
              (filter
                 (fun path ->
                    not (List.mem [ "disabled"; "default" ] path ~equal:String.equal))
                 (text ~min_size:1 ()))
          in
          path, Path path
      in
      "--database=" ^ name, setting (fun s -> { s with database }))
  ; (fun tc ->
      let key = draw_silent tc (text ~min_size:1 ()) in
      "--database-key=" ^ key, fun (s, _, blobs) -> s, Some key, blobs)
  ; (fun tc ->
      let value, picked = subset tc phases in
      "--phases=" ^ value, setting (fun s -> { s with phases = in_order phases picked }))
  ; (fun tc ->
      let value, checks =
        if draw_silent tc (booleans ())
        then "all", List.map health_checks ~f:snd
        else (
          let value, picked = subset tc health_checks in
          value, in_order health_checks picked)
      in
      ( "--suppress-health-check=" ^ value
      , setting (fun s -> { s with suppress_health_check = checks }) ))
  ; boolean "report-multiple-failures" (fun s report_multiple_failures ->
      { s with report_multiple_failures })
  ; boolean "stop-on-first-failure" (fun s stop_on_first_failure ->
      { s with stop_on_first_failure })
  ; boolean "show-statistics" (fun s show_statistics -> { s with show_statistics })
  ; boolean "print-blob" (fun s print_blob -> { s with print_blob })
  ; (fun tc ->
      let name, backend = choose tc [ "default", Settings.Default; "urandom", Urandom ] in
      "--backend=" ^ name, setting (fun s -> { s with backend }))
  ; (fun tc ->
      let name, nondeterminism_strictness =
        choose
          tc
          [ "quiet", Settings.Nondeterminism_strictness.Quiet
          ; "warn", Warn
          ; "error", Error
          ]
      in
      ( "--nondeterminism-strictness=" ^ name
      , setting (fun s -> { s with nondeterminism_strictness }) ))
  ; (fun tc ->
      let blob = draw_silent tc (text ~min_size:1 ()) in
      "--failure-blob=" ^ blob, fun (s, key, blobs) -> s, key, blobs @ [ blob ])
  ]
;;

let test_flags_select_settings () =
  run_hegel_test ~settings:(Settings.create ~test_cases:200 ()) (fun tc ->
    let present =
      List.filter_map flags ~f:(fun draw_flag ->
        if draw_silent tc (booleans ())
        then Some (draw_silent tc (integers ()), draw_flag tc)
        else None)
    in
    (* Sorting by drawn keys puts the flags in an arbitrary order. *)
    let present =
      List.sort present ~compare:(fun (a, _) (b, _) -> Int.compare a b) |> List.map ~f:snd
    in
    let args = List.map present ~f:fst in
    note tc (String.concat ~sep:" " args);
    let expected =
      List.fold
        present
        ~init:(Settings.create ~seed:3 (), None, [])
        ~f:(fun acc (_, apply) -> apply acc)
    in
    match parse args with
    | Ok actual -> if not (Poly.equal actual expected) then failwith "settings differ"
    | Error message -> failwith message)
;;

(* Every setting a run of one test case leaves the user has a flag: all of
   them but [test_cases] and [unbounded_choices], which [single_test_case]
   fixes. *)
let test_every_setting_has_a_flag () =
  Alcotest.(check (list string))
    "flags"
    (List.sort
       ~compare:String.compare
       [ "seed"
       ; "derandomize"
       ; "database"
       ; "database_key"
       ; "verbosity"
       ; "phases"
       ; "suppress_health_check"
       ; "report_multiple_failures"
       ; "stop_on_first_failure"
       ; "show_statistics"
       ; "print_blob"
       ; "backend"
       ; "nondeterminism_strictness"
       ])
    (List.sort
       ~compare:String.compare
       (List.map Cli.settings_flags ~f:(fun (key, _, _) -> key)))
;;

let test_large_seed () =
  match parse [ "--seed"; "9223372036854775813" ] with
  | Ok (settings, _, _) ->
    Alcotest.(check (option Test_helpers.uint64))
      "seed"
      (Some (Unsigned.UInt64.of_string "9223372036854775813"))
      settings.seed
  | Error message -> Alcotest.fail message
;;

let test_bad_values () =
  expect_error [ "--seed"; "x" ] "seed";
  expect_error [ "--verbosity"; "loud" ] "verbosity";
  expect_error [ "--derandomize"; "maybe" ] "derandomize";
  expect_error [ "--phases"; "explore" ] "explore";
  expect_error [ "--suppress-health-check"; "too_fast" ] "too_fast";
  expect_error [ "--bogus" ] "unknown option"
;;

let test_profile () =
  Exn.protect
    ~finally:(fun () -> Settings.set_default_profile None)
    ~f:(fun () ->
      let settings, _, _ =
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
  Alcotest.(check bool) "unbounded choices" true settings.unbounded_choices;
  Alcotest.(check bool)
    "suppressions added"
    true
    (Poly.equal settings.suppress_health_check [ Settings.Too_slow; Filter_too_much ])
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
    Alcotest.(check bool)
      "exits with status 1"
      true
      (Poly.equal status (Error (`Exit_non_zero 1)));
    Alcotest.(check bool)
      "the exception is not raised again"
      false
      (String.is_substring output ~substring:"Fatal error");
    let prefix = "    --failure-blob " in
    let blob =
      match List.find_map (String.split_lines output) ~f:(String.chop_prefix ~prefix) with
      | Some blob -> blob
      | None -> Alcotest.failf "no %S line in:\n%s" prefix output
    in
    let status, output = run_child ~mode:"fail" ~log [ "--failure-blob"; blob ] in
    Alcotest.(check bool)
      "replay exits with status 1"
      true
      (Poly.equal status (Error (`Exit_non_zero 1)));
    Alcotest.(check bool)
      "replay reports the failure"
      true
      (String.is_substring output ~substring:"deliberate failure");
    let status, output =
      run_child ~mode:"fail" ~log [ "--failure-blob"; blob; "--failure-blob"; blob ]
    in
    Alcotest.(check bool)
      "replaying two blobs exits with status 1"
      true
      (Poly.equal status (Error (`Exit_non_zero 1)));
    Alcotest.(check int)
      "both blobs reproduce"
      2
      (List.count (String.split_lines output) ~f:(fun line ->
         String.is_substring line ~substring:"Exception:")))
;;

let tests =
  [ Alcotest.test_case "flags select settings" `Quick test_flags_select_settings
  ; Alcotest.test_case "every setting has a flag" `Quick test_every_setting_has_a_flag
  ; Alcotest.test_case "large seed" `Quick test_large_seed
  ; Alcotest.test_case "bad values" `Quick test_bad_values
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
