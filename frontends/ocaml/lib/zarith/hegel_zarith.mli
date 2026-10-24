(** Generators for the arbitrary-precision integers of
    {{:https://github.com/ocaml/Zarith} zarith}. Link [hegel.zarith] to use
    them; [hegel] itself does not depend on zarith. *)

(** [integers ?min_value ?max_value ()] creates a generator for integers in
    [[min_value, max_value]], inclusive, of any magnitude.

    A missing bound is taken to be [2{^127}] values beyond the other bound, or
    beyond zero when that is further. Values shrink toward zero, or toward the
    bound nearest zero when zero is out of range. [min_value > max_value]
    raises [Hegel.Usage_error].

    {[
    let%hegel_test big_integers_example tc =
      let n =
        Hegel.draw tc (Hegel_zarith.integers ~min_value:(Z.shift_left Z.one 100) ())
      in
      assert (Z.numbits n >= 101)
    ;;
    ]} *)
val integers : ?min_value:Z.t -> ?max_value:Z.t -> unit -> Z.t Hegel.generator

(**/**)

(** [to_bytes n] is [n] as non-empty two's-complement little-endian bytes. *)
val to_bytes : Z.t -> string

(** [of_bytes s] is the integer the two's-complement little-endian bytes [s]
    encode. *)
val of_bytes : string -> Z.t
