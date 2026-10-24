open Cmdliner

let single_test_case (settings : Settings.t) =
  { settings with
    test_cases = 1
  ; suppress_health_check = Too_slow :: settings.suppress_health_check
  ; unbounded_choices = true
  }
;;

let set_default_profile name =
  match
    Settings.set_default_profile (Some name);
    Settings.default ()
  with
  | _ -> ()
  | exception (Hegel_ffi.Ffi.Usage_error _ as error) ->
    Settings.set_default_profile None;
    raise error
;;

let flag ?docv name kind doc =
  Arg.(value & opt (some kind) None & info [ name ] ?docv ~doc)
;;

let settings_flags =
  [ "seed", "N", "Seed for randomization, or $(b,none) for unset."
  ; ( "verbosity"
    , "LEVEL"
    , "How much Hegel prints: $(b,quiet), $(b,normal), $(b,verbose), or $(b,debug)." )
  ; "derandomize", "BOOL", "Use the same seed on every run."
  ; ( "database"
    , "PATH"
    , "Where failing examples are stored: a path, $(b,disabled), or $(b,default)." )
  ; "database_key", "KEY", "The key failing examples are stored under."
  ; ( "phases"
    , "NAMES"
    , "Comma-separated list of the phases to run: $(b,explicit), $(b,reuse), \
       $(b,generate), $(b,target), $(b,shrink)." )
  ; ( "suppress_health_check"
    , "NAMES"
    , "Comma-separated list of health checks to disable: ($(b,filter_too_much), \
       $(b,too_slow), $(b,test_cases_too_large), $(b,large_initial_test_case)), or \
       $(b,all)." )
  ; ( "report_multiple_failures"
    , "BOOL"
    , "Report every distinct failure rather than only the first." )
  ; "stop_on_first_failure", "BOOL", "Stop generating at the first failure."
  ; "show_statistics", "BOOL", "Print statistics about the run."
  ; "print_blob", "BOOL", "Print the blob that reproduces a failure."
  ; ( "backend"
    , "BACKEND"
    , "Source of randomness: $(b,default) (a seeded generator) or $(b,urandom) \
       (fresh entropy for every choice)." )
  ; ( "nondeterminism_strictness"
    , "LEVEL"
    , "What a nondeterministic test does: $(b,quiet), $(b,warn), or $(b,error)." )
  ]
;;

let settings_term ~default_settings =
  let resolve profile overrides failure_blobs =
    match
      Option.iter set_default_profile profile;
      let settings = Settings.set_from_strings (default_settings ()) overrides in
      let database_key =
        List.fold_left
          (fun found (key, value) -> if key = "database_key" then Some value else found)
          None
          overrides
      in
      settings, database_key
    with
    | settings, database_key -> Ok (settings, database_key, failure_blobs)
    | exception Hegel_ffi.Ffi.Usage_error message -> Error (`Msg message)
  in
  let overrides =
    List.fold_right
      (fun (key, docv, doc) rest ->
         let option = String.map (fun c -> if c = '_' then '-' else c) key in
         Term.(
           const (fun value rest ->
             Option.fold ~none:rest ~some:(fun value -> (key, value) :: rest) value)
           $ flag option Arg.string ~docv doc
           $ rest))
      settings_flags
      (Term.const [])
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
       $ overrides
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
  let run (settings, flag_database_key, failure_blobs) =
    Internal.run_test
      ~settings:(single_test_case settings)
      ?test_location
      ~rerun_hint:Command_line
      ~on_failure:(fun () -> exit 1)
      ?database_key:(Option.fold ~none:database_key ~some:Option.some flag_database_key)
      ~failure_blobs
      test_fn
  in
  let info = Cmd.info (Filename.basename Sys.executable_name) in
  let term = Term.(const run $ settings_term ~default_settings:settings) in
  exit (Cmd.eval ~catch:false (Cmd.v info term))
;;
