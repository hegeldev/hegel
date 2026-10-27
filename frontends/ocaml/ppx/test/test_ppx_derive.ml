(** Tests for the [@@deriving hegel_generator] PPX.

    These tests exercise the PPX-generated code for:
    - Record types (product types)
    - Variant types (sum types)
    - Type aliases
    - Single-field records
    - Nested derived types
    - Variants with tuple arguments
    - quickcheck-convention naming ([hegel_generator] for [t],
      [hegel_generator_foo] otherwise; [M.t] fields resolve to
      [M.hegel_generator])
    - [char] fields (resolved from [Hegel.Derive], like every primitive)
    - [@hegel.generator] overrides on any type occurrence
    - [@hegel.do_not_generate] on variant constructors
    - always-printable derived generators: the deriver also derives
      [sexp_of_<t>] (via ppx_sexp_conv's expander) and makes it the
      generator's printer; [[@sexp.opaque]] is the escape hatch for un-sexpable
      fields
    - printing: a derived generator prints what it draws as OCaml syntax,
      each part as that part's generator prints it
    - [~portable], which draws the same values as the plain derivation *)

open! Core
open Hegel

(** A record with two primitive fields. *)
type point =
  { x : int
  ; y : int
  }
[@@deriving hegel_generator]

(** A record with three different-typed fields. *)
type person =
  { name : string
  ; age : int
  ; active : bool
  }
[@@deriving hegel_generator]

(** A variant with no-arg constructors. *)
type color =
  | Red
  | Green
  | Blue
[@@deriving hegel_generator]

(** A variant with arguments. *)
type shape =
  | Circle of float
  | Rectangle of int * int
  | Point
[@@deriving hegel_generator]

(** A type alias to int. *)
type score = int [@@deriving hegel_generator]

(** A single-field record. *)
type wrapper = { value : int } [@@deriving hegel_generator]

(** A type with an option field. *)
type maybe_int = { data : int option } [@@deriving hegel_generator]

(** A nested derived type: record containing another derived record. *)
type line_segment =
  { start_pt : point
  ; end_pt : point
  }
[@@deriving hegel_generator]

(** A variant with a tuple argument. *)
type pair_or_single =
  | Pair of int * int
  | Single of int
[@@deriving hegel_generator]

type measured =
  | Dimensions of
      { width : int
      ; height : int
      }
  | Unmeasured
[@@deriving hegel_generator]

(** A type alias to bool. *)
type flag = bool [@@deriving hegel_generator]

(** A type alias to float. *)
type temperature = float [@@deriving hegel_generator]

(** A type alias to string. *)
type label = string [@@deriving hegel_generator]

(** A type with a list field. *)
type int_list_wrapper = { items : int list } [@@deriving hegel_generator]

(** A record with an int field, used to check that derived ints span the full
    native-int range (regression for the old ±2³⁰−1 clamp). *)
type full_range = { n : int } [@@deriving hegel_generator]

type char_and_float =
  { a : char
  ; b : float
  }
[@@deriving hegel_generator]

(** Deriving in a module signature. *)
module Signature_deriving : sig
  type t [@@deriving hegel_generator]
  type named [@@deriving hegel_generator]
end = struct
  type t = { value : int } [@@deriving hegel_generator]
  type named = Named of string [@@deriving hegel_generator]
end

let (_ : (Signature_deriving.t, Hegel.printable) Hegel.generator) =
  Signature_deriving.hegel_generator
;;

let (_ : (Signature_deriving.named, Hegel.printable) Hegel.generator) =
  Signature_deriving.hegel_generator_named
;;

(** A module whose main type is [t]: derives a value named plain
    [hegel_generator] (no [_t] suffix), following the quickcheck convention. *)
module Temperature_reading = struct
  type t = { celsius : float } [@@deriving hegel_generator]
end

(** A record with a qualified [M.t] field: the deriver resolves it to
    [Temperature_reading.hegel_generator]. *)
type weather =
  { reading : Temperature_reading.t
  ; humidity : int
  }
[@@deriving hegel_generator]

(** A record-field override: limit an int to [[3, 5]] by swapping in a custom
    generator (the quickcheck idiom for range-limiting a field). *)
type ranked =
  { name : string
  ; level : (int[@hegel.generator integers ~min_value:3 ~max_value:5 ()])
  }
[@@deriving hegel_generator]

(** A constructor-argument override *)
type aged =
  | Age of (int[@hegel.generator integers ~min_value:18 ~max_value:99 ()])
  | Unknown
[@@deriving hegel_generator]

(** A type with no generator anywhere in scope (and none derivable: it holds a
    function). Excluded constructors must not require one. *)
type ungeneratable = { thunk : unit -> unit }

(** An all-nullary variant with an excluded constructor: the generator samples
    only from the remaining constructors. *)
type compass =
  | North
  | South
  | Broken [@hegel.do_not_generate]
[@@deriving hegel_generator]

(** A data-carrying variant with an excluded constructor whose argument type has
    no generator. [[@hegel.do_not_generate]] implies opaque printing for the
    excluded arguments (the deriver wraps them in [[@sexp.opaque]] before
    invoking ppx_sexp_conv's expander). *)
type task_result =
  | Finished of int
  | Blocked of ungeneratable [@hegel.do_not_generate]
  | Cancelled
[@@deriving hegel_generator]

type printed_point =
  { px : int
  ; py : int
  }
[@@deriving hegel_generator]

type with_opaque =
  { id : int
  ; handle :
      (ungeneratable
      [@sexp.opaque]
      [@hegel.generator
        with_ocaml_printer
          (fun _ -> Hegel.Printer.opaque)
          (composite (fun _ -> { thunk = Fun.id }))])
  }
[@@deriving hegel_generator]

(** Test: derived [int] fields use the same full default range as [integers ()],
    not the old 30-bit clamp. The engine over-weights boundary values
    (empirically ~36% of unbounded draws exceed 2³⁰−1 in magnitude), so 200
    cases see one with near-certainty. *)
let test_int_full_range_e2e () =
  let clamp = 1073741823 in
  let saw_beyond_clamp = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:200 ~seed:0 ())
    (fun tc ->
       let r = Hegel.draw_silent tc hegel_generator_full_range in
       if r.n > clamp || r.n < -clamp then saw_beyond_clamp := true);
  assert !saw_beyond_clamp
;;

(** Test: derived point generator produces valid points. *)
let%hegel_test test_point_e2e tc =
  let p = Hegel.draw_silent tc hegel_generator_point in
  ignore ((p.x, p.y) : int * int)
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: derived person generator produces valid persons. *)
let%hegel_test test_person_e2e tc =
  let p = Hegel.draw_silent tc hegel_generator_person in
  ignore ((p.name, p.age, p.active) : string * int * bool)
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: derived score (type alias to int) generates integers. *)
let%hegel_test test_score_e2e tc =
  let _v : score = Hegel.draw_silent tc hegel_generator_score in
  ()
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: derived wrapper (single-field record) generates values. *)
let%hegel_test test_wrapper_e2e tc =
  let w = Hegel.draw_silent tc hegel_generator_wrapper in
  ignore w.value
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: derived line_segment (nested record) generates values, resolving the
    unqualified [point] fields to [hegel_generator_point]. *)
let%hegel_test test_line_segment_e2e tc =
  let ls = Hegel.draw_silent tc hegel_generator_line_segment in
  ignore (ls.start_pt.x, ls.start_pt.y, ls.end_pt.x, ls.end_pt.y)
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: derived temperature (type alias to float) generates floats. *)
let%hegel_test test_temperature_e2e tc =
  let f : temperature = Hegel.draw_silent tc hegel_generator_temperature in
  assert (Float.is_finite f)
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: derived label (type alias to string) generates strings. *)
let%hegel_test test_label_e2e tc =
  let s : label = Hegel.draw_silent tc hegel_generator_label in
  ignore (String.length s)
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: derived int_list_wrapper (list field) generates values. *)
let%hegel_test test_int_list_wrapper_e2e tc =
  let w = Hegel.draw_silent tc hegel_generator_int_list_wrapper in
  ignore (List.length w.items)
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: derived color generator covers all constructors. *)
let test_color_e2e () =
  let saw_red = ref false in
  let saw_green = ref false in
  let saw_blue = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:50 ~seed:0 ())
    (fun tc ->
       match Hegel.draw_silent tc hegel_generator_color with
       | Red -> saw_red := true
       | Green -> saw_green := true
       | Blue -> saw_blue := true);
  assert !saw_red;
  assert !saw_green;
  assert !saw_blue
;;

(** Test: derived shape generator covers all constructors. *)
let test_shape_e2e () =
  let saw_circle = ref false in
  let saw_rectangle = ref false in
  let saw_point = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:50 ~seed:0 ())
    (fun tc ->
       match Hegel.draw_silent tc hegel_generator_shape with
       | Circle f ->
         assert (Float.is_finite f);
         saw_circle := true
       | Rectangle (w, h) ->
         ignore (w, h);
         saw_rectangle := true
       | Point -> saw_point := true);
  assert !saw_circle;
  assert !saw_rectangle;
  assert !saw_point
;;

(** Test: derived maybe_int (option field) generates both Some and None. *)
let test_maybe_int_e2e () =
  let saw_some = ref false in
  let saw_none = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:50 ~seed:0 ())
    (fun tc ->
       match (Hegel.draw_silent tc hegel_generator_maybe_int).data with
       | Some _ -> saw_some := true
       | None -> saw_none := true);
  assert !saw_some;
  assert !saw_none
;;

(** Test: derived pair_or_single covers both constructors. *)
let test_pair_or_single_e2e () =
  let saw_pair = ref false in
  let saw_single = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:50 ~seed:0 ())
    (fun tc ->
       match Hegel.draw_silent tc hegel_generator_pair_or_single with
       | Pair (a, b) ->
         ignore (a, b);
         saw_pair := true
       | Single n ->
         ignore n;
         saw_single := true);
  assert !saw_pair;
  assert !saw_single
;;

(** Test: derived measured (inline-record constructor, [Pcstr_record]) covers
    both constructors and draws the inline fields. *)
let test_inline_record_e2e () =
  let saw_dimensions = ref false in
  let saw_unmeasured = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:50 ~seed:0 ())
    (fun tc ->
       match Hegel.draw_silent tc hegel_generator_measured with
       | Dimensions { width; height } ->
         ignore ((width, height) : int * int);
         saw_dimensions := true
       | Unmeasured -> saw_unmeasured := true);
  assert !saw_dimensions;
  assert !saw_unmeasured
;;

(** Test: derived flag covers both true and false. *)
let test_flag_e2e () =
  let saw_true = ref false in
  let saw_false = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:50 ~seed:0 ())
    (fun tc ->
       let b : flag = Hegel.draw_silent tc hegel_generator_flag in
       if b then saw_true := true else saw_false := true);
  assert !saw_true;
  assert !saw_false
;;

let%hegel_test test_char_and_float_e2e tc =
  let r = Hegel.draw_silent tc hegel_generator_char_and_float in
  assert (Char.to_int r.a >= 0 && Char.to_int r.a <= 255);
  assert (Float.is_finite r.b)
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: a module's [t] derives a value named plain [hegel_generator]. *)
let%hegel_test test_module_t_naming_e2e tc =
  let r = Hegel.draw_silent tc Temperature_reading.hegel_generator in
  assert (Float.is_finite r.Temperature_reading.celsius)
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: a qualified [M.t] field resolves to [M.hegel_generator]. *)
let%hegel_test test_qualified_field_e2e tc =
  let w = Hegel.draw_silent tc hegel_generator_weather in
  assert (Float.is_finite w.reading.Temperature_reading.celsius);
  ignore (w.humidity : int)
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: a [@hegel.generator] field override pins the field to the custom
    generator's distribution — every draw lands in [[3, 5]] *)
let test_field_override_range_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:200 ()) (fun tc ->
    let r = Hegel.draw_silent tc hegel_generator_ranked in
    assert (r.level >= 3 && r.level <= 5))
;;

(** Test: a [@hegel.generator] override on a constructor argument. *)
let test_constructor_arg_override_e2e () =
  let saw_age = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:100 ~seed:0 ())
    (fun tc ->
       match Hegel.draw_silent tc hegel_generator_aged with
       | Age n ->
         assert (n >= 18 && n <= 99);
         saw_age := true
       | Unknown -> ());
  assert !saw_age
;;

(** Test: [@hegel.do_not_generate] on an all-nullary variant — the excluded
    constructor never appears, the others still do. *)
let test_do_not_generate_nullary_e2e () =
  let saw_north = ref false in
  let saw_south = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:50 ~seed:0 ())
    (fun tc ->
       match Hegel.draw_silent tc hegel_generator_compass with
       | North -> saw_north := true
       | South -> saw_south := true
       | Broken -> failwith "Broken is marked [@hegel.do_not_generate]");
  assert !saw_north;
  assert !saw_south
;;

(** Test: [@hegel.do_not_generate] on a data-carrying constructor whose argument
    type has no generator — the type still derives (the deriver never references
    [hegel_generator_ungeneratable]) and the constructor never appears. *)
let test_do_not_generate_data_e2e () =
  let saw_finished = ref false in
  let saw_cancelled = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:50 ~seed:0 ())
    (fun tc ->
       match Hegel.draw_silent tc hegel_generator_task_result with
       | Finished _ -> saw_finished := true
       | Blocked _ -> failwith "Blocked is marked [@hegel.do_not_generate]"
       | Cancelled -> saw_cancelled := true);
  assert !saw_finished;
  assert !saw_cancelled;
  let blocked_sexp = Sexp.to_string (sexp_of_task_result (Blocked { thunk = Fun.id })) in
  assert (String.is_substring blocked_sexp ~substring:"opaque")
;;

(* The same types derived twice. The type names are the same, so the labels are
   the same, and one seed gives the same draws. *)
module Plain_derived = struct
  type leaf = { a : int } [@@deriving hegel_generator]

  type record =
    { xs : int list
    ; o : string option
    ; l : leaf
    }
  [@@deriving hegel_generator]

  type enum =
    | E1
    | E2
  [@@deriving hegel_generator]

  type variant =
    | V1 of float
    | V2 of int * char
    | V3 of { w : bool }
  [@@deriving hegel_generator]

  type alias = int * bool [@@deriving hegel_generator]
end

module Portable_derived = struct
  type leaf = { a : int } [@@deriving hegel_generator ~portable]

  type record =
    { xs : int list
    ; o : string option
    ; l : leaf
    }
  [@@deriving hegel_generator ~portable]

  type enum =
    | E1
    | E2
  [@@deriving hegel_generator ~portable]

  type variant =
    | V1 of float
    | V2 of int * char
    | V3 of { w : bool }
  [@@deriving hegel_generator ~portable]

  type alias = int * bool [@@deriving hegel_generator ~portable]

  type nat =
    | Z
    | S of nat
  [@@deriving hegel_generator ~portable]
end

module Plain_nat = struct
  type nat =
    | Z
    | S of nat
  [@@deriving hegel_generator]
end

(** A recursive variant: [Leaf] is the leaf, [Node] the branch. *)
type tree =
  | Leaf of int
  | Node of tree * tree
[@@deriving hegel_generator]

(** A recursive variant whose generated constructors all refer to it. *)
type rose = Rose of int * rose list [@@deriving hegel_generator]

(** A recursive record. *)
type chain =
  { link : int
  ; next : chain option
  }
[@@deriving hegel_generator]

(** Mutually recursive types. *)
type expr =
  | Num of int
  | Add of expr * expr
  | Neg of negation

and negation = { inner : expr } [@@deriving hegel_generator]

(** A recursive variant with an excluded constructor. *)
type guarded =
  | Stop
  | Step of guarded
  | Broken_step of guarded [@hegel.do_not_generate]
[@@deriving hegel_generator]

let rec tree_size = function
  | Leaf _ -> 1
  | Node (l, r) -> tree_size l + tree_size r
;;

let rec rose_depth (Rose (_, children)) =
  1 + List.fold children ~init:0 ~f:(fun acc c -> max acc (rose_depth c))
;;

let rec chain_length c = 1 + Option.value_map c.next ~default:0 ~f:chain_length

let rec expr_size = function
  | Num _ -> 1
  | Add (a, b) -> expr_size a + expr_size b
  | Neg { inner } -> 1 + expr_size inner
;;

let rec guarded_ok = function
  | Stop -> true
  | Step g -> guarded_ok g
  | Broken_step _ -> false
;;

(** Test: recursive types derive generators that terminate and branch. *)
let test_recursive_types_e2e () =
  let saw_node = ref false
  and saw_deep_rose = ref false
  and saw_long_chain = ref false
  and saw_neg = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:100 ~seed:0 ())
    (fun tc ->
       if tree_size (Hegel.draw tc hegel_generator_tree) > 1 then saw_node := true;
       if rose_depth (Hegel.draw tc hegel_generator_rose) > 1 then saw_deep_rose := true;
       if chain_length (Hegel.draw tc hegel_generator_chain) > 1
       then saw_long_chain := true;
       (match Hegel.draw tc hegel_generator_expr with
        | Neg _ -> saw_neg := true
        | Num _ | Add _ -> ());
       ignore (expr_size (Hegel.draw tc hegel_generator_negation).inner : int);
       assert (guarded_ok (Hegel.draw tc hegel_generator_guarded));
       ignore (Hegel.draw tc Plain_nat.hegel_generator_nat : Plain_nat.nat));
  Alcotest.(check bool) "tree branches" true !saw_node;
  Alcotest.(check bool) "rose nests" true !saw_deep_rose;
  Alcotest.(check bool) "chain links" true !saw_long_chain;
  Alcotest.(check bool) "mutual recursion reaches the other type" true !saw_neg
;;

(** Test: [~portable] draws the same values as the plain derivation. *)
let test_portable_matches_plain_e2e () =
  let collect gen =
    let seen = ref [] in
    Hegel.run_hegel_test
      ~settings:(Hegel.Settings.create ~test_cases:30 ~seed:0 ())
      (fun tc ->
         let value, shown = Hegel.Generators.Private.render_draw tc gen in
         seen
         := (shown ^ " " ^ Sexp.to_string ((Hegel.Generators.printer gen) value)) :: !seen);
    List.rev !seen
  in
  let same name plain portable =
    Alcotest.(check (list string)) name (collect plain) (collect portable)
  in
  same
    "record"
    Plain_derived.hegel_generator_record
    Portable_derived.hegel_generator_record;
  same "enum" Plain_derived.hegel_generator_enum Portable_derived.hegel_generator_enum;
  same
    "variant"
    Plain_derived.hegel_generator_variant
    Portable_derived.hegel_generator_variant;
  same "alias" Plain_derived.hegel_generator_alias Portable_derived.hegel_generator_alias;
  same "recursive" Plain_nat.hegel_generator_nat Portable_derived.hegel_generator_nat
;;

(** [shown ?n gen] draws from [gen] in each of [n] test cases, returning each
    value with the rendering of its display. *)
let shown ?(n = 30) gen =
  let seen = ref [] in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:n ~seed:0 ())
    (fun tc -> seen := Hegel.Generators.Private.render_draw tc gen :: !seen);
  List.rev !seen
;;

(** [check_shown name gen expected] checks that every value [gen] draws
    displays as [expected value] describes it. *)
let check_shown ?n name gen expected =
  List.iter (shown ?n gen) ~f:(fun (value, rendered) ->
    Alcotest.(check string) name (Hegel.Printer.to_string (expected value)) rendered)
;;

(** Test: bare deriving yields a printable generator *)
let%hegel_test test_sexp_printer_e2e tc =
  let p = Hegel.draw tc hegel_generator_printed_point in
  ignore ((p.px, p.py) : int * int);
  Alcotest.(check string)
    "derived printer matches the derived sexp converter"
    (Sexp.to_string (sexp_of_printed_point p))
    (Sexp.to_string ((Hegel.Generators.printer hegel_generator_printed_point) p))
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

(** Test: a derived generator prints what it draws as OCaml syntax. *)
let test_printer_e2e () =
  let open Hegel.Printer in
  check_shown "record" hegel_generator_printed_point (fun p ->
    record [ "px", int p.px; "py", int p.py ]);
  check_shown "nested record" hegel_generator_line_segment (fun l ->
    let point (p : point) = record [ "x", int p.x; "y", int p.y ] in
    record [ "start_pt", point l.start_pt; "end_pt", point l.end_pt ]);
  check_shown "option field" hegel_generator_maybe_int (fun m ->
    record [ "data", option int m.data ]);
  check_shown "list field" hegel_generator_int_list_wrapper (fun w ->
    record [ "items", list int w.items ]);
  check_shown "nullary variant" hegel_generator_color (function
    | Red -> constructor "Red" []
    | Green -> constructor "Green" []
    | Blue -> constructor "Blue" []);
  check_shown
    "nullary variant with an excluded constructor"
    hegel_generator_compass
    (function
    | North -> text "North"
    | South -> text "South"
    | Broken -> opaque);
  check_shown "variant" hegel_generator_shape (function
    | Circle f -> constructor "Circle" [ float f ]
    | Rectangle (w, h) -> constructor "Rectangle" [ int w; int h ]
    | Point -> constructor "Point" []);
  check_shown "inline record" hegel_generator_measured (function
    | Dimensions { width; height } ->
      constructor "Dimensions" [ record [ "width", int width; "height", int height ] ]
    | Unmeasured -> constructor "Unmeasured" []);
  check_shown "alias of a tuple" Plain_derived.hegel_generator_alias (fun (n, b) ->
    tuple [ int n; bool b ]);
  check_shown "alias" hegel_generator_label string;
  let rec print_tree = function
    | Leaf n -> constructor "Leaf" [ int n ]
    | Node (l, r) -> constructor "Node" [ print_tree l; print_tree r ]
  in
  check_shown "recursive variant" hegel_generator_tree print_tree;
  let rec print_nat = function
    | Plain_nat.Z -> text "Z"
    | S n -> constructor "S" [ print_nat n ]
  in
  check_shown
    "recursive variant with nullary leaves"
    Plain_nat.hegel_generator_nat
    print_nat;
  let rec print_chain c =
    record [ "link", int c.link; "next", option print_chain c.next ]
  in
  check_shown "recursive record" hegel_generator_chain print_chain
;;

(** Test: an [[@sexp.opaque]] field is opaque in [sexp_of], and prints as
    its [[@hegel.generator]] prints. *)
let%hegel_test test_opaque_field_e2e tc =
  let w = Hegel.draw tc hegel_generator_with_opaque in
  ignore (w.id : int);
  w.handle.thunk ();
  let rendered =
    Sexp.to_string ((Hegel.Generators.printer hegel_generator_with_opaque) w)
  in
  assert (String.is_substring rendered ~substring:"opaque");
  let _, rendered = Hegel.Generators.Private.render_draw tc hegel_generator_with_opaque in
  assert (String.is_substring rendered ~substring:"handle = <abstr>")
[@@settings Hegel.Settings.create ~test_cases:20 ~seed:0 ()]
;;

let () =
  Alcotest.run
    "hegel-ppx-derive"
    [ ( "ppx_derive"
      , [ Alcotest.test_case "derived point" `Quick test_point_e2e
        ; Alcotest.test_case "derived int full range" `Quick test_int_full_range_e2e
        ; Alcotest.test_case "derived person" `Quick test_person_e2e
        ; Alcotest.test_case "derived color covers all" `Quick test_color_e2e
        ; Alcotest.test_case "derived shape covers all" `Quick test_shape_e2e
        ; Alcotest.test_case "derived score (alias)" `Quick test_score_e2e
        ; Alcotest.test_case "derived wrapper (single field)" `Quick test_wrapper_e2e
        ; Alcotest.test_case "derived maybe_int (option)" `Quick test_maybe_int_e2e
        ; Alcotest.test_case "derived line_segment (nested)" `Quick test_line_segment_e2e
        ; Alcotest.test_case
            "derived pair_or_single (tuple)"
            `Quick
            test_pair_or_single_e2e
        ; Alcotest.test_case "derived flag (bool alias)" `Quick test_flag_e2e
        ; Alcotest.test_case
            "derived temperature (float alias)"
            `Quick
            test_temperature_e2e
        ; Alcotest.test_case "derived label (string alias)" `Quick test_label_e2e
        ; Alcotest.test_case
            "derived int_list_wrapper (list)"
            `Quick
            test_int_list_wrapper_e2e
        ; Alcotest.test_case "derived char record" `Quick test_char_and_float_e2e
        ; Alcotest.test_case
            "derived measured (inline record)"
            `Quick
            test_inline_record_e2e
        ; Alcotest.test_case "module t naming" `Quick test_module_t_naming_e2e
        ; Alcotest.test_case "qualified M.t field" `Quick test_qualified_field_e2e
        ; Alcotest.test_case "field override range" `Quick test_field_override_range_e2e
        ; Alcotest.test_case
            "constructor arg override"
            `Quick
            test_constructor_arg_override_e2e
        ; Alcotest.test_case
            "do_not_generate (nullary)"
            `Quick
            test_do_not_generate_nullary_e2e
        ; Alcotest.test_case "do_not_generate (data)" `Quick test_do_not_generate_data_e2e
        ; Alcotest.test_case "recursive types" `Quick test_recursive_types_e2e
        ; Alcotest.test_case "default printer" `Quick test_sexp_printer_e2e
        ; Alcotest.test_case "display" `Quick test_printer_e2e
        ; Alcotest.test_case "opaque field escape hatch" `Quick test_opaque_field_e2e
        ; Alcotest.test_case
            "portable matches plain"
            `Quick
            test_portable_matches_plain_e2e
        ] )
    ]
;;
