open Hegel
open Generators
open Generators.Private

(* [Labels.from_name] / [Labels.combine] are computed by the engine. *)
let test_labels_match_engine () =
  let ctx = Hegel_ffi.Ffi.context_new () in
  Fun.protect
    ~finally:(fun () -> Hegel_ffi.Ffi.context_free ctx)
    (fun () ->
       let resolve = Internal.Label.resolve ctx in
       List.iter
         (fun name ->
            Alcotest.(check int64)
              name
              (Hegel_ffi.Ffi.label_from_name ctx name)
              (resolve (Labels.from_name name)))
         [ ""; "hegel.lists"; "hegel.integers"; "h\xc3\xa9llo" ];
       List.iter
         (fun labels ->
            Alcotest.(check int64)
              "combine"
              (Hegel_ffi.Ffi.label_combine ctx (List.map resolve labels))
              (resolve (Labels.combine labels)))
         [ []
         ; [ Labels.list ]
         ; [ Labels.list; Labels.from_name "x"; Labels.combine [ Labels.tuple ] ]
         ];
       let label = Labels.combine [ Labels.list; Labels.from_name "y" ] in
       Alcotest.(check int64) "resolved once" (resolve label) (resolve label))
;;

(* [with_tc f] runs [f] with a real per-test-case handle from the native
   engine. Used by the collection-record tests, which exercise the OCaml-side
   collection bookkeeping. *)
let with_tc f = Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:1 ()) f

(* A label reflects the generator's kind and its components'. *)
let test_labels_structural () =
  let ctx = Hegel_ffi.Ffi.context_new () in
  let value label = Internal.Label.resolve ctx label in
  let same name a b =
    Alcotest.(check int64) name (value (label_of a)) (value (label_of b))
  in
  let differ name a b =
    Alcotest.(check bool)
      name
      false
      (Int64.equal (value (label_of a)) (value (label_of b)))
  in
  same "same shape" (lists (integers ()) ()) (lists (integers ()) ());
  differ "element kind" (lists (integers ()) ()) (lists (text ()) ());
  same "unique" (lists (integers ()) ()) (lists (integers ()) ~unique:true ());
  differ "map on a leaf" (map succ (integers ())) (integers ());
  differ
    "map on a composite"
    (map fst (tuples2 (integers ()) (text ())))
    (tuples2 (integers ()) (text ()));
  same
    "printer is not part of the label"
    (with_printer Core.Int.sexp_of_t (map succ (integers ())))
    (map succ (integers ()));
  differ "flat_map" (flat_map (fun _ -> integers ()) (integers ())) (integers ());
  differ "filter" (filter (fun _ -> true) (integers ())) (integers ());
  differ "tuple order" (tuples2 (integers ()) (text ())) (tuples2 (text ()) (integers ()));
  differ "one_of alternatives" (one_of [ integers () ]) (one_of [ text () ]);
  differ "optional" (optional (integers ())) (optional (text ()));
  same
    "assoc_list and hash_table"
    (assoc_lists (integers ()) (text ()) ())
    (hash_tables (integers ()) (text ()) ());
  differ
    "functions"
    (functions ~returns:(integers ()) ())
    (functions ~returns:(text ()) ());
  Alcotest.(check int64)
    "just"
    (value (Labels.from_name "hegel.just"))
    (value (label_of (just 1)));
  List.iter
    (fun (name, label) ->
       Alcotest.(check int64) name (value (Labels.from_name name)) (value label))
    [ "hegel.lists", Labels.list
    ; "hegel.sets", Labels.set
    ; "hegel.dicts", Labels.dict
    ; "hegel.tuple", Labels.tuple
    ; "hegel.array", Labels.array
    ; "hegel.permutation", Labels.permutation
    ; "hegel.filter", Labels.filter
    ; "hegel.filter.attempt", Labels.filter_attempt
    ; "hegel.one_of", Labels.one_of
    ; "hegel.optional", Labels.optional
    ];
  with_tc (fun tc ->
    let pool = Stateful.Pool.create tc in
    Alcotest.(check int64)
      "pool"
      (value Labels.pool)
      (value (label_of (Stateful.Pool.values_reusable pool))));
  Hegel_ffi.Ffi.context_free ctx
;;

let test_collection_new () =
  with_tc (fun data ->
    with_collection ~min_size:0 ~max_size:5 data (fun coll ->
      Alcotest.(check bool) "not finished" false coll.finished))
;;

let test_collection_new_no_max () =
  with_tc (fun data ->
    with_collection ~min_size:0 data (fun coll ->
      Alcotest.(check bool) "not finished" false coll.finished;
      Alcotest.(check bool) "max_size is None" true (coll.max_size = None)))
;;

let test_collection_reject_when_finished () =
  with_tc (fun data ->
    with_collection ~min_size:0 data (fun coll ->
      coll.finished <- true;
      collection_reject coll data))
;;

(** Test: collection_more returns false when already finished. *)
let test_collection_more_when_finished () =
  with_tc (fun data ->
    with_collection ~min_size:0 data (fun coll ->
      coll.finished <- true;
      let result = collection_more coll data in
      Alcotest.(check bool) "returns false" false result))
;;

(** Test: discardable_group exception path — stop_span skipped when aborted. *)
let test_discardable_group_exception () =
  with_tc (fun data ->
    Internal.set_test_aborted data true;
    let raised = ref false in
    (try ignore (discardable_group Labels.flat_map data (fun () -> raise Exit) : _) with
     | Exit -> raised := true);
    Alcotest.(check bool) "raised Exit" true !raised)
;;

(* ==== E2E tests ==== *)

(** Test: map doubles values correctly. *)
let test_map_doubles_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:10 ()) (fun tc ->
    let gen = integers ~min_value:1 ~max_value:5 () |> map (fun v -> v * 2) in
    let v = Hegel.draw_silent tc gen in
    assert (v >= 2 && v <= 10);
    assert (v mod 2 = 0))
;;

(** Test: double map composes correctly (Leaf draw-closure composition). *)
let test_double_map_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:10 ()) (fun tc ->
    let gen =
      integers ~min_value:1 ~max_value:5 ()
      |> map (fun v -> v * 2)
      |> map (fun v -> v + 1)
    in
    let v = Hegel.draw_silent tc gen in
    assert (List.mem v [ 3; 5; 7; 9; 11 ]))
;;

(** Mapping a filtered generator preserves the predicate and transforms its values. *)
let test_map_on_filtered_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:10 ()) (fun tc ->
    let gen =
      filter (fun v -> v > 5) (integers ~min_value:0 ~max_value:10 ())
      |> map (fun v -> v * 2)
    in
    let v = Hegel.draw_silent tc gen in
    assert (v > 10 && v <= 20))
;;

(** Test: flat_map through engine. *)
let test_flat_map_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:10 ()) (fun tc ->
    let gen =
      flat_map
        (fun (n : int) -> map (fun v -> n, v) (integers ~min_value:0 ~max_value:n ()))
        (integers ~min_value:1 ~max_value:5 ())
    in
    let n, v = Hegel.draw_silent tc gen in
    assert (n >= 1 && n <= 5);
    assert (v >= 0 && v <= n))
;;

(** Test: filter through engine. *)
let test_filter_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:10 ()) (fun tc ->
    let gen = filter (fun v -> v mod 2 = 0) (integers ~min_value:0 ~max_value:100 ()) in
    let v = Hegel.draw tc gen in
    assert (v mod 2 = 0))
;;

(** Test: filter exhaustion through engine (always false → assume false). *)
let test_filter_exhaustion_e2e () =
  Test_helpers.expect_unsatisfiable (fun () ->
    Hegel.run_hegel_test
      ~settings:
        { (Hegel.Settings.create ~test_cases:10 ()) with
          suppress_health_check = [ Settings.Filter_too_much ]
        }
      (fun tc ->
         let gen = filter (fun _ -> false) (integers ~min_value:0 ~max_value:10 ()) in
         ignore (Hegel.draw tc gen : int);
         Alcotest.fail "an always-false filter returned a value"))
;;

(** Test: group helper through engine. *)
let test_group_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:5 ()) (fun tc ->
    let n =
      group Labels.list tc (fun () ->
        Internal.generate_integer tc ~min_value:0 ~max_value:10)
    in
    assert (n >= 0 && n <= 10))
;;

(** Test: discardable_group through engine — success path. *)
let test_discardable_group_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:5 ()) (fun tc ->
    let n =
      discardable_group Labels.tuple tc (fun () ->
        Internal.generate_integer tc ~min_value:0 ~max_value:10)
    in
    assert (n >= 0 && n <= 10))
;;

(** [printer gen] renders [value] to [expected]. ([gen] is printable, so its
    printer is total — no [option].) *)
let check_printer name gen value expected =
  Alcotest.(check string) name expected (Core.Sexp.to_string ((printer gen) value))
;;

let test_printer_int () = check_printer "int" (integers ()) 42 "42"
let test_printer_bool () = check_printer "bool" (booleans ()) true "true"
let test_printer_text () = check_printer "text" (text ()) "hi" "hi"
let test_printer_chars () = check_printer "chars" (chars ()) 'a' "a"
let test_printer_binary () = check_printer "binary" (binary ()) "ab" "ab"

let test_printer_filter_delegates () =
  check_printer "filter" (filter (fun _ -> true) (integers ())) 5 "5"
;;

let test_printer_with_printer () =
  check_printer
    "with_printer"
    (with_printer Core.Int.sexp_of_t (map (fun v -> v * 2) (integers ())))
    21
    "21"
;;

let test_filter_on_unprintable () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:5 ()) (fun tc ->
    let gen = filter (fun _ -> true) (sampled_from [ 1; 2; 3 ]) in
    let v = Hegel.draw_silent tc gen in
    assert (List.mem v [ 1; 2; 3 ]))
;;

let test_printer_list_primitive () =
  check_printer "list" (lists (integers ()) ()) [ 1; 2; 3 ] "(1 2 3)"
;;

let test_printer_tuple2 () =
  check_printer "tuple2" (tuples2 (integers ()) (integers ())) (1, 2) "(1 2)"
;;

let test_printer_tuple3 () =
  check_printer
    "tuple3"
    (tuples3 (integers ()) (integers ()) (integers ()))
    (1, 2, 3)
    "(1 2 3)"
;;

let test_printer_tuple4 () =
  check_printer
    "tuple4"
    (tuples4 (integers ()) (integers ()) (integers ()) (integers ()))
    (1, 2, 3, 4)
    "(1 2 3 4)"
;;

let test_printer_one_of_primitive () =
  check_printer
    "one_of primitive"
    (one_of
       [ integers ~min_value:0 ~max_value:5 (); integers ~min_value:6 ~max_value:9 () ])
    3
    "3"
;;

let test_printer_assoc_list_primitive () =
  check_printer
    "association list"
    (assoc_lists (integers ()) (integers ()) ())
    [ 1, 2; 3, 4 ]
    "((1 2)(3 4))"
;;

let test_printer_hash_table () =
  let table = Stdlib.Hashtbl.create 1 in
  Stdlib.Hashtbl.replace table 1 2;
  check_printer "hash table" (hash_tables (integers ()) (integers ()) ()) table "((1 2))"
;;

let test_printer_optional_some () =
  check_printer "optional some" (optional (integers ())) (Some 5) "(5)"
;;

let test_printer_optional_none () =
  check_printer "optional none" (optional (integers ())) None "()"
;;

(** [shown gen] draws from [gen] once in each of [n] test cases and returns
    the rendered displays of the drawn values. *)
let shown ?(n = 1) gen =
  let out = ref [] in
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:n ()) (fun tc ->
    out := snd (Private.render_draw tc gen) :: !out);
  List.rev !out
;;

let check_shown name gen expected =
  Alcotest.(check (list string)) name [ expected ] (shown gen)
;;

let int_gen n = integers ~min_value:n ~max_value:n ()

let test_display_primitives () =
  check_shown "int" (int_gen 42) "42";
  check_shown "negative int" (int_gen (-42)) "-42";
  check_shown "bool" (booleans ~p:1.0 ()) "true";
  check_shown "text" (text ~alphabet:"a" ~min_size:2 ~max_size:2 ()) "\"aa\"";
  Alcotest.(check bool)
    "chars"
    true
    (List.for_all
       (fun s -> String.length s >= 3 && s.[0] = '\'' && s.[String.length s - 1] = '\'')
       (shown ~n:20 (chars ())));
  Alcotest.(check bool)
    "binary"
    true
    (List.for_all
       (fun s -> String.length s >= 2 && s.[0] = '"' && s.[String.length s - 1] = '"')
       (shown ~n:20 (binary ~min_size:1 ~max_size:1 ())))
;;

let test_display_filter_delegates () =
  check_shown "filter" (filter (fun _ -> true) (int_gen 5)) "5";
  check_shown
    "filter keeps an attached printer"
    (filter (fun _ -> true) (with_printer Core.Int.sexp_of_t (map succ (int_gen 4))))
    "5";
  check_shown
    "filter keeps a printer attached to a composite"
    (filter
       (fun _ -> true)
       (with_printer
          (fun (a, b) -> Core.Int.sexp_of_t (a + b))
          (tuples2 (int_gen 1) (int_gen 2))))
    "3"
;;

let test_display_with_printer () =
  check_shown
    "on a leaf"
    (with_printer Core.Int.sexp_of_t (map (fun v -> v * 2) (int_gen 21)))
    "42";
  check_shown
    "as an s-expression"
    (with_printer
       Sexplib0.Sexp_conv.(sexp_of_pair sexp_of_int sexp_of_string)
       (map (fun n -> n, "a b") (int_gen 1)))
    "(1 \"a b\")";
  check_shown
    "replacing a printer"
    (with_printer
       (fun n -> Core.Sexp.Atom (Printf.sprintf "0x%x" n))
       (with_printer Core.Int.sexp_of_t (just 255)))
    "0xff";
  check_shown
    "on a printable generator"
    (with_printer
       (fun (a, b) -> Core.Int.sexp_of_t (a + b))
       (tuples2 (int_gen 1) (int_gen 2)))
    "3";
  check_shown
    "replacing a printer on a composite"
    (with_printer
       (fun (a, b) -> Core.Int.sexp_of_t (a * b))
       (with_printer
          (fun (a, b) -> Core.Int.sexp_of_t (a + b))
          (tuples2 (int_gen 2) (int_gen 3))))
    "6"
;;

let test_make_generators () =
  let chars = make_characters ~of_char:Char.code ~sexp_of:Core.Int.sexp_of_t () in
  Alcotest.(check string)
    "characters printer"
    "97"
    (Core.Sexp.to_string (printer chars 97));
  Alcotest.(check bool)
    "characters display"
    true
    (List.for_all
       (fun s -> int_of_string s >= 0 && int_of_string s <= 255)
       (shown ~n:20 chars));
  let tables =
    make_hash_tables
      ~of_pairs:Fun.id
      ~sexp_of_t:(fun sk sv pairs ->
        Sexplib0.Sexp_conv.(sexp_of_list (sexp_of_pair sk sv)) pairs)
      (int_gen 1)
      (int_gen 2)
      ~min_size:1
      ~max_size:1
      ()
  in
  Alcotest.(check string)
    "hash table printer"
    "((3 4))"
    (Core.Sexp.to_string (printer tables [ 3, 4 ]));
  check_shown "hash table display" tables "((1 2))"
;;

let test_display_structures () =
  check_shown "list" (lists (int_gen 1) ~min_size:3 ~max_size:3 ()) "[1; 1; 1]";
  check_shown
    "unique list"
    (lists (one_of [ int_gen 1 ]) ~unique:true ~min_size:1 ~max_size:1 ())
    "[1]";
  check_shown
    "tuple2"
    (tuples2 (int_gen 1) (text ~alphabet:"a" ~min_size:1 ~max_size:1 ()))
    "(1, \"a\")";
  check_shown "tuple3" (tuples3 (int_gen 1) (int_gen 2) (int_gen 3)) "(1, 2, 3)";
  check_shown
    "tuple4"
    (tuples4 (int_gen 1) (int_gen 2) (int_gen 3) (int_gen (-4)))
    "(1, 2, 3, -4)";
  check_shown
    "association list"
    (assoc_lists (int_gen 1) (int_gen 2) ~min_size:1 ~max_size:1 ())
    "[(1, 2)]";
  check_shown
    "hash table"
    (hash_tables (int_gen 1) (int_gen 2) ~min_size:1 ~max_size:1 ())
    "Hashtbl.of_seq (List.to_seq [(1, 2)])";
  check_shown
    "flat_map shows the second draw"
    (with_printer
       Sexplib0.Sexp_conv.(sexp_of_list sexp_of_int)
       (flat_map (fun n -> lists (int_gen n) ~min_size:1 ~max_size:1 ()) (int_gen 7)))
    "(7)"
;;

let test_display_one_of () =
  let gen =
    one_of
      [ with_printer (fun i -> Core.Sexp.Atom ("one:" ^ string_of_int i)) (just 1)
      ; with_printer (fun i -> Core.Sexp.Atom ("two:" ^ string_of_int i)) (just 2)
      ]
  in
  let seen = shown ~n:50 gen in
  Alcotest.(check bool)
    "every display is its branch's"
    true
    (List.for_all (fun s -> s = "one:1" || s = "two:2") seen);
  Alcotest.(check bool)
    "both branches drawn"
    true
    (List.mem "one:1" seen && List.mem "two:2" seen);
  let in_list = shown ~n:50 (lists gen ~min_size:2 ~max_size:2 ()) in
  Alcotest.(check bool)
    "a list shows each element by its own branch"
    true
    (List.mem "[one:1; two:2]" in_list || List.mem "[two:2; one:1]" in_list)
;;

let test_display_optional () =
  let seen = shown ~n:50 (optional (int_gen (-5))) in
  Alcotest.(check bool)
    "None or Some (-5)"
    true
    (List.for_all (fun s -> s = "None" || s = "Some (-5)") seen);
  Alcotest.(check bool)
    "both drawn"
    true
    (List.mem "None" seen && List.mem "Some (-5)" seen)
;;

let test_resolve_draw () =
  let tbl = Int_table.create 4 in
  Int_table.replace tbl 7 "v";
  (* consume:false keeps the entry *)
  Alcotest.(check string) "draw" "v" (resolve_draw tbl ~consume:false 7);
  Alcotest.(check int) "still present" 1 (Int_table.length tbl);
  (* consume:true removes it *)
  Alcotest.(check string) "consume" "v" (resolve_draw tbl ~consume:true 7);
  Alcotest.(check int) "removed" 0 (Int_table.length tbl);
  (* unknown id raises Flaky_strategy *)
  let raised =
    try
      ignore (resolve_draw tbl ~consume:false 99 : string);
      false
    with
    | Internal.Flaky_strategy -> true
  in
  Alcotest.(check bool) "unknown id raises Flaky_strategy" true raised
;;

(** Test: generators that print as they draw describe a value given to their
    printer as abstract, unless given a printer. *)
let test_print_as_drawn_printers () =
  let sexp gen v = Sexplib0.Sexp.to_string (printer gen v) in
  let composite = printable_composite (fun _ _ -> 0) in
  Alcotest.(check string) "composite" "<abstr>" (sexp composite 0);
  Alcotest.(check string)
    "composite with a printer"
    "0"
    (sexp (printable_composite ~sexp_of:Sexplib0.Sexp_conv.sexp_of_int (fun _ _ -> 0)) 0);
  Alcotest.(check string)
    "map_as_call"
    "<abstr>"
    (sexp (map_as_call "f" Fun.id (int_gen 0)) 0);
  Alcotest.(check string) "flat_map" "<abstr>" (sexp (flat_map int_gen (int_gen 0)) 0);
  check_shown
    "composite"
    (printable_composite ~label:"x" (fun _ p -> Printer.Out.text p "x"))
    "x"
;;

let tests =
  [ Alcotest.test_case "stateful: resolve_draw" `Quick test_resolve_draw
  ; Alcotest.test_case "labels match engine" `Quick test_labels_match_engine
  ; Alcotest.test_case "labels structural" `Quick test_labels_structural
  ; Alcotest.test_case "collection new" `Quick test_collection_new
  ; Alcotest.test_case "collection new no max" `Quick test_collection_new_no_max
  ; Alcotest.test_case
      "collection reject when finished"
      `Quick
      test_collection_reject_when_finished
  ; Alcotest.test_case
      "collection_more when finished"
      `Quick
      test_collection_more_when_finished
  ; Alcotest.test_case
      "discardable_group exception"
      `Quick
      test_discardable_group_exception
  ; Alcotest.test_case "map doubles e2e" `Quick test_map_doubles_e2e
  ; Alcotest.test_case "double map e2e" `Quick test_double_map_e2e
  ; Alcotest.test_case "map on filtered e2e" `Quick test_map_on_filtered_e2e
  ; Alcotest.test_case "flat_map e2e" `Quick test_flat_map_e2e
  ; Alcotest.test_case "filter e2e" `Quick test_filter_e2e
  ; Alcotest.test_case "filter exhaustion e2e" `Quick test_filter_exhaustion_e2e
  ; Alcotest.test_case "group e2e" `Quick test_group_e2e
  ; Alcotest.test_case "discardable_group e2e" `Quick test_discardable_group_e2e
  ; Alcotest.test_case "printer int" `Quick test_printer_int
  ; Alcotest.test_case "printer bool" `Quick test_printer_bool
  ; Alcotest.test_case "printer text" `Quick test_printer_text
  ; Alcotest.test_case "printer chars" `Quick test_printer_chars
  ; Alcotest.test_case "printer binary" `Quick test_printer_binary
  ; Alcotest.test_case "printer filter delegates" `Quick test_printer_filter_delegates
  ; Alcotest.test_case "printer with_printer" `Quick test_printer_with_printer
  ; Alcotest.test_case "filter on unprintable" `Quick test_filter_on_unprintable
  ; Alcotest.test_case "printer list primitive" `Quick test_printer_list_primitive
  ; Alcotest.test_case "printer tuple2" `Quick test_printer_tuple2
  ; Alcotest.test_case "printer tuple3" `Quick test_printer_tuple3
  ; Alcotest.test_case "printer tuple4" `Quick test_printer_tuple4
  ; Alcotest.test_case "printer one_of primitive" `Quick test_printer_one_of_primitive
  ; Alcotest.test_case
      "printer association list primitive"
      `Quick
      test_printer_assoc_list_primitive
  ; Alcotest.test_case "printer hash table" `Quick test_printer_hash_table
  ; Alcotest.test_case "printer optional some" `Quick test_printer_optional_some
  ; Alcotest.test_case "printer optional none" `Quick test_printer_optional_none
  ; Alcotest.test_case "display primitives" `Quick test_display_primitives
  ; Alcotest.test_case "display filter delegates" `Quick test_display_filter_delegates
  ; Alcotest.test_case "display with_printer" `Quick test_display_with_printer
  ; Alcotest.test_case "make_characters, make_hash_tables" `Quick test_make_generators
  ; Alcotest.test_case "display structures" `Quick test_display_structures
  ; Alcotest.test_case "display one_of" `Quick test_display_one_of
  ; Alcotest.test_case "display optional" `Quick test_display_optional
  ; Alcotest.test_case "print-as-drawn printers" `Quick test_print_as_drawn_printers
  ]
;;
