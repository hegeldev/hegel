(** [program ?ox ?effect_syntax tc] is the source of a complete program that
    prints its values; with [~ox:true] it also uses OxCaml-only features, and
    with [~effect_syntax:true] OCaml 5.3's effect handler syntax. *)
val program : ?ox:bool -> ?effect_syntax:bool -> Hegel.test_case -> string

(** The line at which a program splits into two modules. *)
val module_boundary : string
