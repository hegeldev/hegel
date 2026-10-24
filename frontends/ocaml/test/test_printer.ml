(** Tests for [Hegel.Printer]: values render as OCaml syntax through the
    engine's layout. *)

open Hegel

let check name expected doc =
  Alcotest.(check string) name expected (Printer.to_string doc)
;;

let check_narrow name ~max_width expected doc =
  Alcotest.(check string) name expected (Printer.to_string ~max_width doc)
;;

let test_integers () =
  check "int" "42" (Printer.int 42);
  check "negative int" "-3" (Printer.int (-3));
  check "int32" "7l" (Printer.int32 7l);
  check "negative int32" "-7l" (Printer.int32 (-7l));
  check "int64" "7L" (Printer.int64 7L);
  check "negative int64" "-7L" (Printer.int64 (-7L));
  check "nativeint" "7n" (Printer.nativeint 7n);
  check "negative nativeint" "-7n" (Printer.nativeint (-7n))
;;

let test_floats () =
  check "integral" "1." (Printer.float 1.0);
  check "fraction" "0.1" (Printer.float 0.1);
  check "negative" "-2.5" (Printer.float (-2.5));
  check "negative zero" "-0." (Printer.float (-0.0));
  check "exponent" "1e+300" (Printer.float 1e300);
  check "smallest subnormal" "5e-324" (Printer.float 5e-324);
  check "needs all digits" "0.30000000000000004" (Printer.float (0.1 +. 0.2));
  check "nan" "nan" (Printer.float Float.nan);
  check "infinity" "infinity" (Printer.float Float.infinity);
  check "neg_infinity" "neg_infinity" (Printer.float Float.neg_infinity);
  check
    "negative float argument"
    "Some (-2.5)"
    (Printer.option Printer.float (Some (-2.5)));
  check "nan argument" "Some nan" (Printer.option Printer.float (Some Float.nan));
  check
    "neg_infinity argument"
    "Some neg_infinity"
    (Printer.option Printer.float (Some Float.neg_infinity))
;;

let test_scalars () =
  check "true" "true" (Printer.bool true);
  check "unit" "()" (Printer.unit ());
  check "char" "'a'" (Printer.char 'a');
  check "escaped char" "'\\n'" (Printer.char '\n')
;;

let test_strings () =
  check "plain" "\"hello\"" (Printer.string "hello");
  check "with space" "\"hello world\"" (Printer.string "hello world");
  check "empty" "\"\"" (Printer.string "");
  check "escapes" "\"q\\\"b\\\\n\\nt\\tr\\r\"" (Printer.string "q\"b\\n\nt\tr\r");
  check "control bytes" "\"\\001\\127\"" (Printer.string "\001\127");
  check "UTF-8 kept" "\"h\xc3\xa9llo\"" (Printer.string "h\xc3\xa9llo");
  check "invalid UTF-8 escaped" "\"\\255a\"" (Printer.string "\xffa");
  check "bytes escape non-ASCII" "\"h\\195\\169\"" (Printer.bytes "h\xc3\xa9")
;;

let test_structures () =
  check "list" "[1; 2; 3]" (Printer.list Printer.int [ 1; 2; 3 ]);
  check "empty list" "[]" (Printer.list Printer.int []);
  check "list of negatives" "[-1; -2]" (Printer.list Printer.int [ -1; -2 ]);
  check "array" "[|1; 2|]" (Printer.array Printer.int [| 1; 2 |]);
  check "none" "None" (Printer.option Printer.int None);
  check "some" "Some 1" (Printer.option Printer.int (Some 1));
  check "some negative" "Some (-1)" (Printer.option Printer.int (Some (-1)));
  check
    "nested some"
    "Some (Some 1)"
    (Printer.option (Printer.option Printer.int) (Some (Some 1)));
  check "ok" "Ok 1" (Printer.result Printer.int Printer.string (Ok 1));
  check "error" "Error \"no\"" (Printer.result Printer.int Printer.string (Error "no"));
  check "pair" "(1, \"a\")" (Printer.pair Printer.int Printer.string (1, "a"));
  check
    "triple"
    "(1, true, 'c')"
    (Printer.triple Printer.int Printer.bool Printer.char (1, true, 'c'));
  check "empty tuple" "()" (Printer.tuple []);
  check "singleton tuple" "1" (Printer.tuple [ Printer.int 1 ]);
  check
    "record"
    "{ x = 1; y = Some 2 }"
    (Printer.record [ "x", Printer.int 1; "y", Printer.option Printer.int (Some 2) ]);
  check "nullary constructor" "Leaf" (Printer.constructor "Leaf" []);
  check
    "constructor with arguments"
    "Node (Leaf, -1)"
    (Printer.constructor "Node" [ Printer.constructor "Leaf" []; Printer.int (-1) ]);
  check
    "constructor with a record"
    "Point { x = 1 }"
    (Printer.constructor "Point" [ Printer.record [ "x", Printer.int 1 ] ]);
  check "apply without arguments" "f" (Printer.apply (Printer.text "f") []);
  check
    "apply"
    "f 1 (-2) (Some 3)"
    (Printer.apply
       (Printer.text "f")
       [ Printer.int 1; Printer.int (-2); Printer.option Printer.int (Some 3) ])
;;

let test_layout () =
  check_narrow
    "a list breaks one element per line"
    ~max_width:8
    "[100;\n 200;\n 300]"
    (Printer.list Printer.int [ 100; 200; 300 ]);
  check_narrow
    "a record breaks one field per line"
    ~max_width:12
    "{\n  x = 100;\n  y = 200 }"
    (Printer.record [ "x", Printer.int 100; "y", Printer.int 200 ]);
  check_narrow
    "a group indents by its opening delimiter"
    ~max_width:5
    "<<a\n  b>>"
    (Printer.group
       "<<"
       [ Printer.text "a"; Printer.breakable " "; Printer.text "b" ]
       ">>");
  check_narrow
    "an explicit indent"
    ~max_width:3
    "(a\n    b)"
    (Printer.group
       ~indent:4
       "("
       [ Printer.text "a"; Printer.breakable " "; Printer.text "b" ]
       ")");
  check "concat" "ab" (Printer.concat [ Printer.text "a"; Printer.text "b" ]);
  check "text newlines are hard breaks" "a\nb" (Printer.text "a\nb");
  check "opaque" "<abstr>" Printer.opaque;
  check "reflow fits" "Foo(1, 2)" (Printer.reflow "Foo(1, 2)");
  check_narrow
    "reflow breaks"
    ~max_width:8
    "Foo(111,\n    222)"
    (Printer.reflow "Foo(111, 222)");
  check
    "of_to_string"
    "Some (0x2a)"
    (Printer.option (Printer.of_to_string (Printf.sprintf "0x%x")) (Some 42))
;;

let tests =
  [ Alcotest.test_case "integers" `Quick test_integers
  ; Alcotest.test_case "floats" `Quick test_floats
  ; Alcotest.test_case "scalars" `Quick test_scalars
  ; Alcotest.test_case "strings" `Quick test_strings
  ; Alcotest.test_case "structures" `Quick test_structures
  ; Alcotest.test_case "layout" `Quick test_layout
  ]
;;
