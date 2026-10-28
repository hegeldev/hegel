open Hegel
open Generators

type tree =
  | Leaf of int
  | Node of tree list

let rec depth = function
  | Leaf _ -> 0
  | Node children -> 1 + List.fold_left (fun acc t -> max acc (depth t)) 0 children
;;

let rec sexp_of_tree : tree -> Sexplib0.Sexp.t = function
  | Leaf n -> List [ Atom "Leaf"; Atom (string_of_int n) ]
  | Node children -> List [ Atom "Node"; List (List.map sexp_of_tree children) ]
;;

let tree_printer f gen = with_printer sexp_of_tree (map f gen)

let rec leaves = function
  | Leaf _ -> 1
  | Node children -> List.fold_left (fun acc t -> acc + leaves t) 0 children
;;

let trees ?max_depth ?max_leaves () =
  recursive
    ?max_depth
    ?max_leaves
    (tree_printer (fun n -> Leaf n) (integers ~min_value:0 ~max_value:9 ()))
    (fun subtrees -> tree_printer (fun children -> Node children) (lists subtrees ()))
;;

let settings ?(test_cases = 100) () = Settings.create ~test_cases ~seed:0 ()

let test_recursive_respects_bounds () =
  let saw_node = ref false in
  Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
    let t = draw tc (trees ~max_depth:4 ~max_leaves:10 ()) in
    (match t with
     | Node _ -> saw_node := true
     | Leaf _ -> ());
    assert (depth t <= 4);
    assert (leaves t <= 10));
  Alcotest.(check bool) "branches are generated" true !saw_node
;;

let test_recursive_max_depth_zero () =
  Hegel.run_hegel_test ~settings:(settings ~test_cases:30 ()) (fun tc ->
    match draw tc (trees ~max_depth:0 ()) with
    | Leaf _ -> ()
    | Node _ -> Alcotest.fail "max_depth 0 generates only leaves")
;;

let test_recursive_tiny_leaf_budget () =
  Hegel.run_hegel_test ~settings:(settings ~test_cases:200 ()) (fun tc ->
    let t =
      draw
        tc
        (recursive
           ~max_leaves:1
           (tree_printer (fun n -> Leaf n) (integers ()))
           (fun subtrees ->
              tree_printer (fun children -> Node children) (lists subtrees ~min_size:2 ())))
    in
    assert (leaves t <= 1))
;;

let test_recursive_shrinks_to_a_leaf () =
  match
    Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
      let t = draw tc (trees ()) in
      assert (leaves t < 3))
  with
  | () -> Alcotest.fail "expected a failure"
  | exception Assert_failure _ -> ()
;;

let test_recursive_invalid_arguments () =
  let check name f =
    match f () with
    | exception Hegel.Usage_error _ -> ()
    | (_ : (tree, printable) generator) -> Alcotest.fail (name ^ ": expected Usage_error")
  in
  check "negative max_depth" (fun () -> trees ~max_depth:(-1) ());
  check "zero max_leaves" (fun () -> trees ~max_leaves:0 ())
;;

let deferred_trees () =
  let trees, define = deferred () in
  define
    (one_of
       [ tree_printer (fun n -> Leaf n) (integers ~min_value:0 ~max_value:9 ())
       ; tree_printer (fun children -> Node children) (lists trees ~max_size:2 ())
       ]);
  trees
;;

let test_deferred_draws () =
  let saw_node = ref false in
  Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
    match draw tc (deferred_trees ()) with
    | Node _ -> saw_node := true
    | Leaf _ -> ());
  Alcotest.(check bool) "branches are generated" true !saw_node
;;

let test_deferred_mutual_recursion () =
  let evens, define_evens = deferred () in
  let odds, define_odds = deferred () in
  let succ gen = with_printer Sexplib0.Sexp_conv.sexp_of_int (map (fun n -> n + 1) gen) in
  define_evens
    (one_of [ with_printer Sexplib0.Sexp_conv.sexp_of_int (just 0); succ odds ]);
  define_odds (succ evens);
  Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
    assert (draw tc evens mod 2 = 0);
    assert (draw tc odds mod 2 = 1))
;;

let test_deferred_printer () =
  let gen, define = deferred () in
  (match printer gen 3 with
   | (_ : Sexplib0.Sexp.t) -> Alcotest.fail "expected Usage_error"
   | exception Hegel.Usage_error _ -> ());
  define (integers ());
  Alcotest.(check string) "printer" "3" (Sexplib0.Sexp.to_string (printer gen 3))
;;

let test_deferred_undefined () =
  let undefined, _define = deferred () in
  match
    Hegel.run_hegel_test ~settings:(settings ~test_cases:1 ()) (fun tc ->
      ignore (draw tc (lists undefined ~min_size:1 ()) : int list))
  with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Hegel.Usage_error _ -> ()
;;

let test_deferred_defined_twice () =
  let gen, define = deferred () in
  define (integers ());
  match define (integers ()) with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Hegel.Usage_error _ -> ignore (gen : (int, printable) generator)
;;

let test_deferred_defined_as_itself () =
  let expect_error f =
    match f () with
    | () -> Alcotest.fail "expected Usage_error"
    | exception Hegel.Usage_error _ -> ()
  in
  let a, define_a = deferred () in
  expect_error (fun () -> define_a a);
  let b, define_b = deferred () in
  let c, define_c = deferred () in
  define_b c;
  expect_error (fun () ->
    define_c (with_printer Sexplib0.Sexp_conv.sexp_of_int (filter (fun _ -> true) b)))
;;

let test_deferred_defined_as_a_map_of_itself () =
  let gen, define = deferred () in
  match define (with_printer Sexplib0.Sexp_conv.sexp_of_int (map Fun.id gen)) with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Hegel.Usage_error _ -> ignore (gen : (int, printable) generator)
;;

let test_deferred_drawn_deep_inside_spans () =
  let ints, define = deferred () in
  define (integers ~min_value:0 ~max_value:9 ());
  let rec nest n gen =
    if n = 0
    then gen
    else
      nest
        (n - 1)
        (with_printer
           Sexplib0.Sexp_conv.sexp_of_int
           (map
              snd
              (tuples2 (with_printer Sexplib0.Sexp_conv.sexp_of_unit (just ())) gen)))
  in
  Hegel.run_hegel_test ~settings:(settings ~test_cases:5 ()) (fun tc ->
    assert (draw tc (nest 100 ints) < 10))
;;

let tests =
  [ Alcotest.test_case "recursive respects bounds" `Quick test_recursive_respects_bounds
  ; Alcotest.test_case "recursive max_depth 0" `Quick test_recursive_max_depth_zero
  ; Alcotest.test_case "recursive tiny leaf budget" `Quick test_recursive_tiny_leaf_budget
  ; Alcotest.test_case "recursive shrinks" `Quick test_recursive_shrinks_to_a_leaf
  ; Alcotest.test_case
      "recursive invalid arguments"
      `Quick
      test_recursive_invalid_arguments
  ; Alcotest.test_case "deferred draws" `Quick test_deferred_draws
  ; Alcotest.test_case "deferred mutual recursion" `Quick test_deferred_mutual_recursion
  ; Alcotest.test_case "deferred printer" `Quick test_deferred_printer
  ; Alcotest.test_case "deferred undefined" `Quick test_deferred_undefined
  ; Alcotest.test_case "deferred defined twice" `Quick test_deferred_defined_twice
  ; Alcotest.test_case "deferred defined as itself" `Quick test_deferred_defined_as_itself
  ; Alcotest.test_case
      "deferred defined as a map of itself"
      `Quick
      test_deferred_defined_as_a_map_of_itself
  ; Alcotest.test_case
      "deferred drawn deep inside spans"
      `Quick
      test_deferred_drawn_deep_inside_spans
  ]
;;
