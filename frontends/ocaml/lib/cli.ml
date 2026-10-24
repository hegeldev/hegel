open Cmdliner

exception Parse of string

let single_test_case (settings : Settings.t) =
  { settings with
    test_cases = 1
  ; suppress_health_check = Too_slow :: settings.suppress_health_check
  ; unbounded_choices = true
  }
;;

let parse_seed = function
  | "none" -> None
  | value ->
    (match int_of_string_opt value with
     | Some n -> Some n
     | None ->
       raise (Parse (Printf.sprintf "--seed expects an integer or 'none', got %S" value)))
;;

let parse_database : string -> Settings.database = function
  | "disabled" -> Disabled
  | "default" -> Unset
  | path -> Path path
;;

let parse_health_check : string -> Settings.health_check = function
  | "filter_too_much" -> Filter_too_much
  | "too_slow" -> Too_slow
  | "test_cases_too_large" -> Test_cases_too_large
  | "large_initial_test_case" -> Large_initial_test_case
  | other ->
    raise
      (Parse
         (Printf.sprintf
            "--suppress-health-check expects any of: all, filter_too_much,too_slow, \
             test_cases_too_large, or large_initial_test_case, got %S"
            other))
;;

let parse_health_checks = function
  | "all" ->
    [ Settings.Filter_too_much; Too_slow; Test_cases_too_large; Large_initial_test_case ]
  | value ->
    List.map
      (fun part -> parse_health_check (String.trim part))
      (String.split_on_char ',' value)
;;

let set_default_profile name =
  match
    Settings.set_default_profile (Some name);
    Settings.default ()
  with
  | _ -> ()
  | exception Hegel_ffi.Ffi.Usage_error message ->
    Settings.set_default_profile None;
    raise (Parse message)
;;

let flag ?docv name kind doc =
  Arg.(value & opt (some kind) None & info [ name ] ?docv ~doc)
;;

let settings_term ~default_settings =
  let bool_values =
    [ "true", true; "1", true; "yes", true; "false", false; "0", false; "no", false ]
  in
  let resolve
        profile
        seed
        verbosity
        derandomize
        database
        suppress_health_check
        backend
        failure_blobs
    =
    match
      Option.iter set_default_profile profile;
      let s : Settings.t = default_settings () in
      let override flag default = Option.value flag ~default in
      { s with
        seed = override (Option.map parse_seed seed) s.seed
      ; verbosity = override verbosity s.verbosity
      ; derandomize = override derandomize s.derandomize
      ; database = override (Option.map parse_database database) s.database
      ; suppress_health_check =
          override
            (Option.map parse_health_checks suppress_health_check)
            s.suppress_health_check
      ; backend = override backend s.backend
      }
    with
    | settings -> Ok (settings, failure_blobs)
    | exception Parse message -> Error (`Msg message)
  in
  Term.(
    term_result
      ~usage:true
      (const resolve
       $ flag
           "profile"
           Arg.string
           ~docv:"NAME"
           "Set the default settings profile. The other flags apply on top of it."
       $ flag
           "seed"
           Arg.string
           ~docv:"N"
           "Seed for randomization, or $(b,none) for unset."
       $ flag
           "verbosity"
           (Arg.enum
              [ "quiet", Settings.Quiet
              ; "normal", Settings.Normal
              ; "verbose", Settings.Verbose
              ; "debug", Settings.Debug
              ])
           ~docv:"LEVEL"
           "How much Hegel prints. $(docv) is $(b,quiet), $(b,normal), $(b,verbose), or \
            $(b,debug)."
       $ flag
           "derandomize"
           (Arg.enum bool_values)
           ~docv:"BOOL"
           "Use the same seed on every run."
       $ flag
           "database"
           Arg.string
           ~docv:"PATH"
           "Where failing examples are stored: a path, $(b,disabled), or $(b,default)."
       $ flag
           "suppress-health-check"
           Arg.string
           ~docv:"NAMES"
           "Comma-separated list of health checks to disable: ($(b,filter_too_much), \
            $(b,too_slow), $(b,test_cases_too_large), $(b,large_initial_test_case)), or \
            $(b,all)."
       $ flag
           "backend"
           (Arg.enum [ "default", Settings.Default; "urandom", Settings.Urandom ])
           ~docv:"BACKEND"
           "Source of randomness: $(b,default) (a seeded generator) or $(b,urandom) \
            (fresh entropy for every choice)."
       $ Arg.(
           value
           & opt_all string []
           & info
               [ "failure-blob" ]
               ~docv:"BLOB"
               ~doc:
                 "Replay a failure from the blob instead of generating a test case. \
                  Repeat it to replay several blobs.")))
;;

let main ?(settings = Settings.default) ?test_location ?database_key test_fn =
  let run (settings, failure_blobs) =
    Internal.run_test
      ~settings:(single_test_case settings)
      ?test_location
      ~rerun_hint:Command_line
      ~on_failure:(fun () -> exit 1)
      ?database_key
      ~failure_blobs
      test_fn
  in
  let info = Cmd.info (Filename.basename Sys.executable_name) in
  let term = Term.(const run $ settings_term ~default_settings:settings) in
  exit (Cmd.eval ~catch:false (Cmd.v info term))
;;
