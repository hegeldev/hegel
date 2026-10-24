(** The command line of a {!Hegel.main} binary. Not for direct use. *)

(** [single_test_case settings] is [settings] for a run of exactly one test
    case: [test_cases = 1], with [Too_slow] suppressed and
    [unbounded_choices] set. *)
val single_test_case : Settings.t -> Settings.t

(** [settings_flags] is the setting each Hegel flag sets, by its [hegel.toml]
    key ([--suppress-health-check] for [suppress_health_check]), with the
    flag's value name and documentation. *)
val settings_flags : (string * string * string) list

(** [settings_term ~default_settings] parses the Hegel flags into the settings
    they select on top of [default_settings ()], the database key a
    [--database-key] flag gives, and the failure blobs. The engine parses each
    flag's value as [hegel.toml] does. [--profile] is applied first by setting
    the process's default profile before [default_settings] runs. *)
val settings_term
  :  default_settings:(unit -> Settings.t)
  -> (Settings.t * string option * string list) Cmdliner.Term.t

(** Same as [Hegel.main]. *)
val main
  :  ?settings:(unit -> Settings.t)
  -> ?test_location:Internal.test_location
  -> ?database_key:string
  -> (Internal.test_case -> unit)
  -> 'a
