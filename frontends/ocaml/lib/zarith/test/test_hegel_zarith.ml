let settings ?(test_cases = 100) () = Hegel.Settings.create ~test_cases ~seed:0 ()
let pow2 n = Z.shift_left Z.one n

let check_in_range ?min_value ?max_value ~lo ~hi () =
  Hegel.run_hegel_test ~settings:(settings ()) (fun tc ->
    let n = Hegel.draw tc (Hegel_zarith.integers ?min_value ?max_value ()) in
    Alcotest.(check bool)
      (Printf.sprintf "%s in range" (Z.to_string n))
      true
      (Z.leq lo n && Z.leq n hi))
;;

let test_bounded () =
  let lo = Z.neg (pow2 200)
  and hi = pow2 100 in
  check_in_range ~min_value:lo ~max_value:hi ~lo ~hi ()
;;

let test_unbounded () =
  check_in_range ~lo:(Z.neg (pow2 127)) ~hi:(Z.pred (pow2 127)) ();
  check_in_range
    ~min_value:(pow2 300)
    ~lo:(pow2 300)
    ~hi:(Z.pred (Z.add (pow2 300) (pow2 127)))
    ();
  check_in_range
    ~max_value:(Z.neg (pow2 300))
    ~lo:(Z.succ (Z.sub (Z.neg (pow2 300)) (pow2 127)))
    ~hi:(Z.neg (pow2 300))
    ();
  check_in_range ~min_value:Z.zero ~lo:Z.zero ~hi:(Z.pred (pow2 127)) ();
  check_in_range ~max_value:Z.zero ~lo:(Z.neg (pow2 127)) ~hi:Z.zero ()
;;

let test_finds_large_values () =
  match
    Hegel.run_hegel_test
      ~settings:{ (settings ~test_cases:1000 ()) with verbosity = Hegel.Settings.Quiet }
      (fun tc ->
         let n = Hegel.draw tc (Hegel_zarith.integers ()) in
         if Z.geq n (pow2 80) then failwith (Z.to_string n))
  with
  | () -> Alcotest.fail "expected a value of at least 2^80"
  | exception Failure s ->
    Alcotest.(check bool) "a large value" true (Z.geq (Z.of_string s) (pow2 80))
;;

let test_negative_values () =
  match
    Hegel.run_hegel_test
      ~settings:{ (settings ~test_cases:1000 ()) with verbosity = Hegel.Settings.Quiet }
      (fun tc ->
         let n = Hegel.draw tc (Hegel_zarith.integers ()) in
         if Z.lt n (Z.of_int (-1000)) then failwith (Z.to_string n))
  with
  | () -> Alcotest.fail "expected a value below -1000"
  | exception Failure s -> Alcotest.(check string) "the shrunk value" "-1001" s
;;

let test_round_trip () =
  List.iter
    (fun n ->
       Alcotest.(check string)
         "round trip"
         (Z.to_string n)
         (Z.to_string (Hegel_zarith.of_bytes (Hegel_zarith.to_bytes n))))
    [ Z.zero
    ; Z.one
    ; Z.minus_one
    ; Z.of_int 127
    ; Z.of_int 128
    ; Z.of_int (-128)
    ; Z.of_int (-129)
    ; Z.of_int 255
    ; Z.of_int (-256)
    ; pow2 64
    ; Z.neg (pow2 64)
    ; Z.pred (pow2 127)
    ; Z.neg (pow2 127)
    ]
;;

let test_invalid_bounds () =
  match Hegel_zarith.integers ~min_value:Z.one ~max_value:Z.zero () with
  | _ -> Alcotest.fail "expected Usage_error"
  | exception Hegel.Usage_error msg ->
    Alcotest.(check string) "message" "Cannot have max_value=0 < min_value=1" msg
;;

let test_prints_decimal () =
  Hegel.run_hegel_test ~settings:(settings ~test_cases:10 ()) (fun tc ->
    let gen = Hegel_zarith.integers ~min_value:(pow2 70) ~max_value:(pow2 70) () in
    let n, show =
      Hegel.Generators.Ppx_internal.draw tc gen ~sexp_of:(Hegel.Generators.printer gen)
    in
    Alcotest.(check string) "shown" (Z.to_string n) (Hegel.Printer.to_string (show ()));
    Alcotest.(check string)
      "printer"
      (Z.to_string n)
      (Sexplib0.Sexp.to_string (Hegel.Generators.printer gen n)))
;;

let () =
  Alcotest.run
    "hegel-zarith"
    [ ( "integers"
      , [ Alcotest.test_case "bounded" `Quick test_bounded
        ; Alcotest.test_case "unbounded" `Quick test_unbounded
        ; Alcotest.test_case "finds large values" `Quick test_finds_large_values
        ; Alcotest.test_case "finds negative values" `Quick test_negative_values
        ; Alcotest.test_case "byte round trip" `Quick test_round_trip
        ; Alcotest.test_case "invalid bounds" `Quick test_invalid_bounds
        ; Alcotest.test_case "prints in decimal" `Quick test_prints_decimal
        ] )
    ]
;;
