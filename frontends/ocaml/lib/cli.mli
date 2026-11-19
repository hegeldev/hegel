(** The command line of a {!Hegel.main} binary. Not for direct use. *)

(** [settings_term ~default_settings] parses the Hegel flags into the settings
    they select on top of [default_settings ()]. [--profile] is applied first
    by setting the process's default profile before [default_settings] runs. *)
val settings_term
  :  default_settings:(unit -> Settings.t)
  -> (Settings.t * string option) Cmdliner.Term.t

(** Same as [Hegel.main]. *)
val main
  :  ?settings:(unit -> Settings.t)
  -> ?test_location:Internal.test_location
  -> ?database_key:string
  -> (Internal.test_case -> unit)
  -> 'a
