(** The command line of a {!Hegel.main} binary. Not for direct use. *)

(** Why the command line did not select settings to run with. *)
type error =
  | Help of string (** [--help] or [-h] was given; the text is the usage. *)
  | Parse_error of string
  (** An unknown flag, a missing or invalid value, or a flag for a setting
      the binary fixes. *)

(** [single_test_case settings] is [settings] for a run of exactly one test
    case: [test_cases = 1], with [Too_slow] suppressed and
    [unbounded_choices] set. *)
val single_test_case : Settings.t -> Settings.t

(** The usage text [--help] prints. *)
val usage : string

(** [parse ~default_settings args] reads the flags [args] (without the
    program name): the settings they select on top of [default_settings ()],
    the database key a [--database-key] flag gives, and the failure blobs.

    [--profile NAME] is applied first, by setting the process's default
    profile before [default_settings] runs. [--failure-blob BLOB] may repeat.
    Every other flag [--some-setting VALUE] (or [--some-setting=VALUE]) sets
    the [hegel.toml] key [some_setting], the engine parsing the value, so a
    flag exists for every setting the engine has, except [--test-cases] and
    [--unbounded-choices], which {!single_test_case} fixes and which are
    rejected. *)
val parse
  :  default_settings:(unit -> Settings.t)
  -> string list
  -> (Settings.t * string option * string list, error) result

(** Same as [Hegel.main]. *)
val main
  :  ?settings:(unit -> Settings.t)
  -> ?test_location:Internal.test_location
  -> ?database_key:string
  -> (Internal.test_case -> unit)
  -> 'a
