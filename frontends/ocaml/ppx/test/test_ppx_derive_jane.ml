(** Tests for [@@deriving hegel_generator] with the Core generators from
    [Hegel_jane.Derive]. *)

open! Core
open Hegel_jane.Derive

(** A record mixing primitives (resolved to the jane-shadowed leaves) with
    Core-typed fields (resolved through the wrapper modules). *)
type event =
  { id : int
  ; initial : char
  ; day : Date.t
  ; elapsed : Time_ns.Span.t
  ; boundary : Time_ns.Ofday.t
  }
[@@deriving hegel_generator]

(** Test: the [event] record derives and generates Core-typed values with the
    full Core API available on them. *)
let%hegel_test test_event_e2e tc =
  let e = Hegel.draw_silent tc hegel_generator_event in
  assert (Date.year e.day >= 1 && Date.year e.day <= 9999);
  assert (
    Time_ns.Ofday.between
      e.boundary
      ~low:Time_ns.Ofday.start_of_day
      ~high:Time_ns.Ofday.start_of_next_day)
[@@settings Hegel.Settings.create ~test_cases:20 ()]
;;

let () =
  Alcotest.run
    "hegel-ppx-derive-jane"
    [ ( "ppx_derive_jane"
      , [ Alcotest.test_case "derived Core-typed record" `Quick test_event_e2e ] )
    ]
;;
