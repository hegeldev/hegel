open Hegel
open Generators
open Generators.Private

(* [Labels.from_name] / [Labels.combine] are the engine's own hashes. *)
let test_labels_match_engine () =
  let ctx = Hegel_ffi.Ffi.context_new () in
  Fun.protect
    ~finally:(fun () -> Hegel_ffi.Ffi.context_free ctx)
    (fun () ->
       List.iter
         (fun name ->
            Alcotest.(check int64)
              name
              (Hegel_ffi.Ffi.label_from_name ctx name)
              (Labels.from_name name))
         [ ""; "hegel_ocaml.list"; "hegel.integer"; "h\xc3\xa9llo" ];
       List.iter
         (fun labels ->
            Alcotest.(check int64)
              "combine"
              (Hegel_ffi.Ffi.label_combine ctx labels)
              (Labels.combine labels))
         [ []
         ; [ Labels.list ]
         ; [ Labels.list; Labels.from_name "x"; 0L; -1L; Int64.max_int ]
         ])
;;

(* [with_tc f] runs [f] with a real per-test-case handle from the native
   engine. Used by the collection-record tests, which exercise the OCaml-side
   collection bookkeeping. *)
let with_tc f = Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:1 ()) f

(* A label reflects the generator's kind and its components'. *)
let test_labels_structural () =
  let same name a b = Alcotest.(check int64) name (label_of a) (label_of b) in
  let differ name a b =
    Alcotest.(check bool) name false (Int64.equal (label_of a) (label_of b))
  in
  same "same shape" (lists (integers ()) ()) (lists (integers ()) ());
  differ "element kind" (lists (integers ()) ()) (lists (text ()) ());
  differ "unique" (lists (integers ()) ()) (lists (integers ()) ~unique:true ());
  differ "map on a leaf" (map succ (integers ())) (integers ());
  differ
    "map on a composite"
    (map fst (tuples2 (integers ()) (text ())))
    (tuples2 (integers ()) (text ()));
  same
    "printer is not part of the label"
    (with_printer Printer.int (map succ (integers ())))
    (map succ (integers ()));
  differ "flat_map" (flat_map (fun _ -> integers ()) (integers ())) (integers ());
  differ "filter" (filter (fun _ -> true) (integers ())) (integers ());
  differ "tuple order" (tuples2 (integers ()) (text ())) (tuples2 (text ()) (integers ()));
  differ "one_of alternatives" (one_of [ integers () ]) (one_of [ text () ]);
  differ "optional" (optional (integers ())) (optional (text ()));
  differ
    "assoc_list vs hash_table"
    (assoc_lists (integers ()) (text ()) ())
    (hash_tables (integers ()) (text ()) ());
  differ
    "functions"
    (functions ~returns:(integers ()) ())
    (functions ~returns:(text ()) ());
  Alcotest.(check int64) "just" (Labels.from_name "hegel_ocaml.just") (label_of (just 1));
  with_tc (fun tc ->
    let pool = Stateful.Pool.create tc in
    Alcotest.(check int64)
      "pool"
      Labels.pool
      (label_of (Stateful.Pool.values_reusable pool)))
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

(** [shown gen] draws from [gen] once in each of [n] test cases and returns
    the rendered displays of the drawn values. *)
let shown ?(n = 1) gen =
  let out = ref [] in
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:n ()) (fun tc ->
    let _, show = Ppx_internal.draw tc gen in
    out := Printer.to_string (show ()) :: !out);
  List.rev !out
;;

let check_shown name gen expected =
  Alcotest.(check (list string)) name [ expected ] (shown gen)
;;

let int_gen n = integers ~min_value:n ~max_value:n ()

let test_printer_primitives () =
  check_shown "int" (int_gen 42) "42";
  check_shown "negative int" (int_gen (-42)) "-42";
  check_shown "bool" (booleans ~p:1.0 ()) "true";
  check_shown "text" (text ~alphabet:"a" ~min_size:2 ~max_size:2 ()) "\"aa\"";
  check_shown "chars" (map (fun _ -> 'a') (int_gen 0) |> with_printer Printer.char) "'a'";
  check_shown
    "binary"
    (binary ~min_size:1 ~max_size:1 ()
     |> map (fun _ -> "\xff")
     |> with_printer Printer.bytes)
    "\"\\255\""
;;

(* [filter] is type-preserving, so it keeps the source's display. *)
let test_printer_filter_delegates () =
  check_shown "filter" (filter (fun _ -> true) (int_gen 5)) "5";
  check_shown
    "filter keeps an attached printer"
    (filter (fun _ -> true) (with_printer Printer.int (map succ (int_gen 4))))
    "5";
  check_shown
    "filter keeps a printer attached to a composite"
    (filter
       (fun _ -> true)
       (with_printer
          (fun (a, b) -> Printer.int (a + b))
          (tuples2 (int_gen 1) (int_gen 2))))
    "3"
;;

(* A value whose type is the caller's shows as [<abstr>] until [with_printer]
   gives it a printer, whatever kind of generator it came from. *)
let test_opaque_values () =
  check_shown "map" (map succ (int_gen 1)) "<abstr>";
  check_shown "map on a composite" (map fst (tuples2 (int_gen 1) (int_gen 2))) "<abstr>";
  check_shown "just" (just 3) "<abstr>";
  check_shown "sampled_from" (sampled_from [ 3 ]) "<abstr>";
  check_shown "composite" (composite (fun _ -> 3)) "<abstr>";
  check_shown "function" (functions ~returns:(int_gen 1) ()) "<fun>"
;;

let test_with_printer () =
  check_shown
    "on a leaf"
    (with_printer Printer.int (map (fun v -> v * 2) (int_gen 21)))
    "42";
  check_shown
    "on a composite"
    (with_printer (fun (a, b) -> Printer.int (a + b)) (tuples2 (int_gen 1) (int_gen 2)))
    "3";
  check_shown
    "replacing a printer"
    (with_printer
       (fun n -> Printer.text (Printf.sprintf "0x%x" n))
       (with_printer Printer.int (composite (fun _ -> 255))))
    "0xff";
  check_shown
    "map over a printed composite"
    (map
       (fun (a, b) -> a + b)
       (with_printer
          (fun (a, b) -> Printer.int (a + b))
          (tuples2 (int_gen 1) (int_gen 2))))
    "<abstr>"
;;

(* [filter] over a generator whose values show as [<abstr>] keeps that. *)
let test_filter_on_opaque () =
  check_shown "filter" (filter (fun _ -> true) (sampled_from [ 1 ])) "<abstr>"
;;

let test_printer_structures () =
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
    (flat_map (fun n -> lists (int_gen n) ~min_size:1 ~max_size:1 ()) (int_gen 7))
    "[7]"
;;

(* [one_of] shows each value as the branch it came from shows it. *)
let test_printer_one_of () =
  let gen =
    one_of
      [ with_printer (fun i -> Printer.text ("one:" ^ string_of_int i)) (just 1)
      ; with_printer (fun i -> Printer.text ("two:" ^ string_of_int i)) (just 2)
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

let test_printer_optional () =
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
  ; Alcotest.test_case "printer primitives" `Quick test_printer_primitives
  ; Alcotest.test_case "printer filter delegates" `Quick test_printer_filter_delegates
  ; Alcotest.test_case "opaque values" `Quick test_opaque_values
  ; Alcotest.test_case "with_printer" `Quick test_with_printer
  ; Alcotest.test_case "filter on opaque" `Quick test_filter_on_opaque
  ; Alcotest.test_case "printer structures" `Quick test_printer_structures
  ; Alcotest.test_case "printer one_of" `Quick test_printer_one_of
  ; Alcotest.test_case "printer optional" `Quick test_printer_optional
  ]
;;
