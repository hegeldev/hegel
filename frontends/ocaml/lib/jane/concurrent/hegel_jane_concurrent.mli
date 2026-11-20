@@ portable

(** Runs Hegel's concurrent state machines on Jane Street's [Concurrent]
    library. See {{!page-hegel_jane_concurrent}Hegel_jane_concurrent} for a
    guide, examples, and the interface. *)

type 'a ctx =
  { context : 'a
  ; concurrent : 'a Concurrent.t
  }

(** [create_local ~spawn_join_n] creates a custom backend for a scheduler
    whose context belongs to the calling scope. Use the backend within that
    scope. The callback follows the contract of {!Hegel.Concurrency.create}. *)
val create_local
  :  spawn_join_n:
       (n:int
        -> (f:('ctx @ local -> int -> Hegel.Concurrency.outcome) @ portable
            -> Hegel.Concurrency.outcome list @ contended)
           @ local)
     @ local
  -> 'ctx Hegel.Concurrency.t @ local

val of_concurrent : 'a Concurrent.t @ local -> 'a ctx Hegel.Concurrency.t @ local
