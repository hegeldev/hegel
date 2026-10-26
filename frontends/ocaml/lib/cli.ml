type error =
  | Help of string
  | Parse_error of string

exception Cli_error of error

let single_test_case (settings : Settings.t) =
  { settings with
    test_cases = 1
  ; suppress_health_check = Too_slow :: settings.suppress_health_check
  ; unbounded_choices = true
  }
;;

let usage =
  {|Usage: <program> [options]

Hegel property-based testing binary. Runs exactly one test case.

Options:
  --profile <NAME>                       Set the default settings profile, like
                                         the HEGEL_DEFAULT_PROFILE environment
                                         variable; other flags apply on top
  --failure-blob <BLOB>                  Replay a failure from the blob instead
                                         of generating a test case; repeat it
                                         to replay several blobs
  --seed <N|none>                        Seed for randomization ('none' for unset)
  --derandomize <BOOL>                   Use a deterministic derived seed
  --database <PATH|disabled|default>     Database path for failing-example storage
  --database-key <KEY>                   Key the failing examples are stored under
  --verbosity <LEVEL>                    quiet | normal | verbose | debug
  --phases <NAMES>                       Comma-separated phases to run: explicit,
                                         reuse, generate, target, shrink
  --suppress-health-check <NAMES>        Comma-separated health check names
                                         (filter_too_much, too_slow,
                                         test_cases_too_large,
                                         large_initial_test_case), or 'all'
  --report-multiple-failures <BOOL>      Report every distinct failure found
  --stop-on-first-failure <BOOL>         End generation once a failure is found
  --show-statistics <BOOL>               Print the run's statistics
  --print-blob <BOOL>                    Print the failure's reproduce blob
  --backend <default|urandom>            Randomness source
  --nondeterminism-strictness <LEVEL>    quiet | warn | error
  -h, --help                             Show this help and exit

Every setting in hegel.toml has a flag, named after its key with '-' for
'_', except test_cases and unbounded_choices: the binary runs exactly one
test case, with no limit on its choices. Every flag also takes the form
--flag=VALUE. A BOOL is true, false, 1, 0, yes or no. Values use the
vocabulary of the matching hegel.toml key.
|}
;;

let fixed_settings =
  [ "test-cases", "a Hegel.main binary runs exactly one test case"
  ; "unbounded-choices", "a Hegel.main binary always allows unbounded choices"
  ]
;;

let without_function_name message =
  let prefix = "hegel_settings_set_from_string: " in
  let n = String.length prefix * Bool.to_int (String.starts_with ~prefix message) in
  String.sub message n (String.length message - n)
;;

let is_unknown_setting message =
  String.starts_with ~prefix:"unknown setting" (without_function_name message)
;;

let split_flag arg =
  match String.index_opt arg '=' with
  | Some i ->
    String.sub arg 2 (i - 2), Some (String.sub arg (i + 1) (String.length arg - i - 1))
  | None -> String.sub arg 2 (String.length arg - 2), None
;;

let extract_profile args =
  let rec go profile kept = function
    | [] -> profile, List.rev kept
    | arg :: rest when String.starts_with ~prefix:"--profile" arg ->
      (match split_flag arg, rest with
       | ("profile", Some value), rest | ("profile", None), value :: rest ->
         if Option.is_some profile
         then raise (Cli_error (Parse_error "--profile may be given at most once"));
         go (Some value) kept rest
       | ("profile", None), [] ->
         raise (Cli_error (Parse_error "--profile requires a value"))
       | _ -> go profile (arg :: kept) rest)
    | arg :: rest -> go profile (arg :: kept) rest
  in
  go None [] args
;;

let set_default_profile name =
  match
    Settings.set_default_profile (Some name);
    Settings.default ()
  with
  | _ -> ()
  | exception Hegel_ffi.Ffi.Usage_error message ->
    Settings.set_default_profile None;
    raise (Cli_error (Parse_error message))
;;

let apply_flag settings (key, value) =
  match Settings.set_from_strings settings [ key, value ] with
  | settings -> settings
  | exception Hegel_ffi.Ffi.Usage_error message ->
    let flag = "--" ^ String.map (fun c -> if c = '_' then '-' else c) key in
    if is_unknown_setting message
    then raise (Cli_error (Parse_error ("Unknown argument: " ^ flag)))
    else raise (Cli_error (Parse_error (flag ^ ": " ^ without_function_name message)))
;;

let parse_exn ~default_settings args =
  let profile, args = extract_profile args in
  Option.iter set_default_profile profile;
  let settings = default_settings () in
  let rec go overrides blobs = function
    | [] -> List.rev overrides, List.rev blobs
    | ("--help" | "-h") :: _ -> raise (Cli_error (Help usage))
    | arg :: rest ->
      if String.length arg <= 2 || not (String.starts_with ~prefix:"--" arg)
      then raise (Cli_error (Parse_error ("Unknown argument: " ^ arg)));
      let name, value = split_flag arg in
      if String.contains name '_'
      then raise (Cli_error (Parse_error ("Unknown argument: --" ^ name)));
      Option.iter
        (fun reason ->
           raise
             (Cli_error
                (Parse_error (Printf.sprintf "--%s cannot be set: %s" name reason))))
        (List.assoc_opt name fixed_settings);
      let value, rest =
        match value, rest with
        | Some value, rest | None, value :: rest -> value, rest
        | None, [] ->
          if not (String.equal name "failure-blob")
          then (
            match
              Settings.set_from_strings
                settings
                [ String.map (fun c -> if c = '-' then '_' else c) name, "" ]
            with
            | _ -> ()
            | exception Hegel_ffi.Ffi.Usage_error message when is_unknown_setting message
              -> raise (Cli_error (Parse_error ("Unknown argument: " ^ arg)))
            | exception Hegel_ffi.Ffi.Usage_error _ -> ());
          raise (Cli_error (Parse_error (Printf.sprintf "--%s requires a value" name)))
      in
      if String.equal name "failure-blob"
      then go overrides (value :: blobs) rest
      else (
        let key = String.map (fun c -> if c = '-' then '_' else c) name in
        go ((key, value) :: overrides) blobs rest)
  in
  let overrides, blobs = go [] [] args in
  let settings = List.fold_left apply_flag settings overrides in
  let database_key = List.assoc_opt "database_key" (List.rev overrides) in
  settings, database_key, blobs
;;

let parse ~default_settings args =
  match parse_exn ~default_settings args with
  | parsed -> Ok parsed
  | exception Cli_error error -> Error error
;;

let main ?(settings = Settings.default) ?test_location ?database_key test_fn =
  match parse ~default_settings:settings (List.tl (Array.to_list Sys.argv)) with
  | Error (Help text) ->
    print_string text;
    exit 0
  | Error (Parse_error message) ->
    prerr_string (message ^ "\n\n" ^ usage);
    exit 2
  | Ok (settings, flag_database_key, failure_blobs) ->
    Internal.run_test
      ~settings:(single_test_case settings)
      ?test_location
      ~rerun_hint:Command_line
      ~on_failure:(fun () -> exit 1)
      ?database_key:(Option.fold ~none:database_key ~some:Option.some flag_database_key)
      ~failure_blobs
      test_fn;
    exit 0
;;
