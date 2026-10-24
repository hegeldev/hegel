open Hegel
open Generators

let settings ?(test_cases = 100) () = Settings.create ~test_cases ~seed:0 ()

let quiet ?test_cases () =
  { (settings ?test_cases ()) with verbosity = Settings.Quiet; database = Disabled }
;;

let check_all ?test_cases gen check =
  run_hegel_test ~settings:(settings ?test_cases ()) (fun tc -> check (draw tc gen))
;;

let shown gen =
  let result = ref "" in
  run_hegel_test ~settings:(settings ~test_cases:1 ()) (fun tc ->
    let _, show = Ppx_internal.draw tc gen in
    result := Printer.to_string (show ()));
  !result
;;

let minimal ?(test_cases = 1000) gen condition =
  let found = ref None in
  (match
     run_hegel_test ~settings:(quiet ~test_cases ()) (fun tc ->
       let value = draw_silent tc gen in
       if condition value
       then (
         found := Some value;
         failwith "found"))
   with
   | () -> Alcotest.fail "no value satisfied the condition"
   | exception Failure _ -> ());
  Option.get !found
;;

let expect_usage_error ?(substring = "") f =
  match f () with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Usage_error msg ->
    Alcotest.(check bool)
      (Printf.sprintf "%S mentions %S" msg substring)
      true
      (Test_helpers.contains_substring msg substring)
;;

let expect_draw_usage_error ?substring gen =
  expect_usage_error ?substring (fun () ->
    run_hegel_test ~settings:(settings ~test_cases:5 ()) (fun tc -> ignore (draw tc gen)))
;;

let is_float32 f = Int32.float_of_bits (Int32.bits_of_float f) = f

let test_floats_width_32 () =
  check_all (floats ~width:`W32 ()) (fun f ->
    Alcotest.(check bool) "representable" true (Float.is_nan f || is_float32 f));
  check_all (floats ~width:`W32 ~min_value:0.0 ~max_value:1.0 ()) (fun f ->
    Alcotest.(check bool) "in range" true (f >= 0.0 && f <= 1.0 && is_float32 f));
  let tiny =
    minimal (floats ~width:`W32 ~min_value:0.0 ~exclude_min:true ()) (fun f -> f < 1e-40)
  in
  Alcotest.(check bool) "a subnormal" true (tiny > 0.0 && is_float32 tiny);
  expect_draw_usage_error (floats ~width:`W32 ~min_value:0.1 ())
;;

let test_floats_width_64 () =
  check_all (floats ~width:`W64 ~min_value:0.0 ~max_value:1.0 ()) (fun f ->
    Alcotest.(check bool) "in range" true (f >= 0.0 && f <= 1.0))
;;

let is_hex c = (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')

let check_uuid u =
  Alcotest.(check int) "length" 36 (String.length u);
  String.iteri
    (fun i c ->
       if i = 8 || i = 13 || i = 18 || i = 23
       then Alcotest.(check char) "hyphen" '-' c
       else Alcotest.(check bool) (Printf.sprintf "%C is hex" c) true (is_hex c))
    u;
  Alcotest.(check bool) "not nil" true (u <> "00000000-0000-0000-0000-000000000000")
;;

let test_uuids () =
  check_all (uuids ()) check_uuid;
  List.iter
    (fun version ->
       check_all (uuids ~version ()) (fun u ->
         check_uuid u;
         Alcotest.(check char) "version" (Char.chr (Char.code '0' + version)) u.[14];
         Alcotest.(check bool) "variant" true (String.contains "89ab" u.[19])))
    [ 1; 2; 3; 4; 5 ];
  Alcotest.(check bool) "shown as a string" true (String.length (shown (uuids ())) = 38);
  expect_usage_error ~substring:"version=0" (fun () -> ignore (uuids ~version:0 ()));
  expect_usage_error ~substring:"version=6" (fun () -> ignore (uuids ~version:6 ()))
;;

let test_from_regex_alphabet () =
  check_all (from_regex ~fullmatch:false ~alphabet:"abc" "b" ()) (fun s ->
    Alcotest.(check bool) "contains b" true (String.contains s 'b');
    String.iter
      (fun c -> Alcotest.(check bool) "in alphabet" true (String.contains "abc" c))
      s);
  check_all
    (from_regex ~codec:"ascii" ~min_codepoint:48 ~max_codepoint:57 "\\w+" ())
    (fun s ->
       String.iter
         (fun c -> Alcotest.(check bool) "a digit" true (c >= '0' && c <= '9'))
         s);
  check_all
    (from_regex
       ~categories:[ "Lu" ]
       ~exclude_characters:"A"
       ~include_characters:"z"
       "."
       ())
    (fun s -> Alcotest.(check bool) "not A" true (s <> "A"));
  check_all (from_regex ~exclude_categories:[ "L" ] ~max_codepoint:127 "." ()) (fun s ->
    Alcotest.(check bool)
      "not a letter"
      true
      (not ((s >= "a" && s <= "z") || (s >= "A" && s <= "Z"))));
  expect_usage_error ~substring:"mutually exclusive" (fun () ->
    ignore (from_regex ~alphabet:"a" ~codec:"ascii" "a" ()));
  expect_usage_error ~substring:"mutually exclusive" (fun () ->
    ignore (from_regex ~categories:[ "L" ] ~exclude_categories:[ "N" ] "a" ()));
  expect_draw_usage_error (from_regex ~alphabet:"abc" "x" ())
;;

let distinct xs = List.length (List.sort_uniq compare xs) = List.length xs

let test_unique_by () =
  check_all
    (lists
       (tuples2 (integers ~min_value:0 ~max_value:20 ()) (booleans ()))
       ~unique_by:fst
       ())
    (fun pairs ->
       Alcotest.(check bool) "distinct keys" true (distinct (List.map fst pairs)));
  check_all
    (lists (integers ~min_value:0 ~max_value:20 ()) ~unique:true ())
    (fun xs -> Alcotest.(check bool) "distinct" true (distinct xs));
  let xs =
    minimal
      (lists (integers ~min_value:0 ~max_value:20 ()) ~unique_by:(fun x -> x mod 5) ())
      (fun xs -> List.length xs >= 3)
  in
  Alcotest.(check (list int)) "minimal" [ 0; 1; 2 ] xs;
  Alcotest.(check bool)
    "shown"
    true
    (List.mem
       (shown
          (lists
             (integers ~min_value:0 ~max_value:1 ())
             ~min_size:2
             ~max_size:2
             ~unique_by:Fun.id
             ()))
       [ "[0; 1]"; "[1; 0]" ]);
  expect_usage_error ~substring:"mutually exclusive" (fun () ->
    ignore (lists (integers ()) ~unique:true ~unique_by:Fun.id ()))
;;

let test_arrays () =
  check_all
    (arrays (integers ~min_value:0 ~max_value:9 ()) ~min_size:2 ~max_size:5 ())
    (fun a ->
       Alcotest.(check bool) "size" true (Array.length a >= 2 && Array.length a <= 5));
  Alcotest.(check string)
    "shown"
    "[|3; 3|]"
    (shown (arrays (integers ~min_value:3 ~max_value:3 ()) ~min_size:2 ~max_size:2 ()));
  let a =
    minimal
      (arrays (integers ~min_value:0 ~max_value:9 ()) ())
      (fun a -> Array.length a >= 2)
  in
  Alcotest.(check (array int)) "minimal" [| 0; 0 |] a;
  expect_usage_error (fun () -> ignore (arrays (integers ()) ~min_size:(-1) ()))
;;

module Int_set = Set.Make (Int)

let test_sets () =
  check_all
    (sets
       (module Int_set)
       (integers ~min_value:0 ~max_value:9 ())
       ~min_size:3
       ~max_size:6
       ())
    (fun s ->
       Alcotest.(check bool)
         "size"
         true
         (Int_set.cardinal s >= 3 && Int_set.cardinal s <= 6));
  let s =
    minimal
      (sets (module Int_set) (integers ~min_value:0 ~max_value:9 ()) ())
      (fun s -> Int_set.cardinal s >= 2)
  in
  Alcotest.(check (list int)) "minimal" [ 0; 1 ] (Int_set.elements s);
  Alcotest.(check string)
    "shown"
    "Set.of_list [5]"
    (shown
       (sets
          (module Int_set)
          (integers ~min_value:5 ~max_value:5 ())
          ~min_size:1
          ~max_size:1
          ()));
  expect_usage_error (fun () ->
    ignore (sets (module Int_set) (integers ()) ~max_size:(-1) ()))
;;

let test_permutations () =
  let values = [ 1; 2; 3; 4; 5 ] in
  check_all (permutations values) (fun xs ->
    Alcotest.(check (list int)) "a permutation" values (List.sort compare xs));
  check_all (permutations []) (fun xs -> Alcotest.(check (list int)) "empty" [] xs);
  check_all (permutations [ 7 ]) (fun xs -> Alcotest.(check (list int)) "single" [ 7 ] xs);
  let shuffled = minimal (permutations values) (fun xs -> xs <> values) in
  Alcotest.(check (list int)) "one swap" [ 1; 2; 3; 5; 4 ] shuffled;
  Alcotest.(check string) "shown" "<abstr>" (shown (permutations values))
;;

let test_tuples () =
  let i n = integers ~min_value:n ~max_value:n () in
  check_all
    (tuples5 (i 1) (i 2) (i 3) (i 4) (i 5))
    (fun t -> Alcotest.(check bool) "5" true (t = (1, 2, 3, 4, 5)));
  check_all
    (tuples6 (i 1) (i 2) (i 3) (i 4) (i 5) (i 6))
    (fun t -> Alcotest.(check bool) "6" true (t = (1, 2, 3, 4, 5, 6)));
  check_all
    (tuples7 (i 1) (i 2) (i 3) (i 4) (i 5) (i 6) (i 7))
    (fun t -> Alcotest.(check bool) "7" true (t = (1, 2, 3, 4, 5, 6, 7)));
  check_all
    (tuples8 (i 1) (i 2) (i 3) (i 4) (i 5) (i 6) (i 7) (i 8))
    (fun t -> Alcotest.(check bool) "8" true (t = (1, 2, 3, 4, 5, 6, 7, 8)));
  Alcotest.(check string)
    "shown"
    "(1, 2, 3, 4, 5, 6, 7, 8)"
    (shown (tuples8 (i 1) (i 2) (i 3) (i 4) (i 5) (i 6) (i 7) (i 8)));
  Alcotest.(check string)
    "shown"
    "(1, 2, 3, 4, 5)"
    (shown (tuples5 (i 1) (i 2) (i 3) (i 4) (i 5)));
  Alcotest.(check string)
    "shown"
    "(1, 2, 3, 4, 5, 6)"
    (shown (tuples6 (i 1) (i 2) (i 3) (i 4) (i 5) (i 6)));
  Alcotest.(check string)
    "shown"
    "(1, 2, 3, 4, 5, 6, 7)"
    (shown (tuples7 (i 1) (i 2) (i 3) (i 4) (i 5) (i 6) (i 7)))
;;

let tests =
  [ Alcotest.test_case "32-bit floats" `Quick test_floats_width_32
  ; Alcotest.test_case "64-bit floats" `Quick test_floats_width_64
  ; Alcotest.test_case "uuids" `Quick test_uuids
  ; Alcotest.test_case "from_regex with an alphabet" `Quick test_from_regex_alphabet
  ; Alcotest.test_case "lists with unique_by" `Quick test_unique_by
  ; Alcotest.test_case "arrays" `Quick test_arrays
  ; Alcotest.test_case "sets" `Quick test_sets
  ; Alcotest.test_case "permutations" `Quick test_permutations
  ; Alcotest.test_case "tuples of 5 to 8" `Quick test_tuples
  ]
;;
