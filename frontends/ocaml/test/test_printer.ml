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
  check "round hundred" "100." (Printer.float 100.);
  check "a million" "1000000." (Printer.float 1e6);
  check "large integral" "1234567890123456." (Printer.float 1234567890123456.);
  check "first exponent form" "1e+16" (Printer.float 1e16);
  check "small fraction" "0.00001" (Printer.float 1e-5);
  check "smaller fraction" "1e-06" (Printer.float 1e-6);
  check "negative round" "-250." (Printer.float (-250.));
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
  check "bytes escape non-ASCII" "\"h\\195\\169\"" (Printer.bytes "h\xc3\xa9");
  check "backspace" "\"\\b\"" (Printer.string "\b");
  check "right-to-left override" "\"a\\u{202e}b\"" (Printer.string "a\u{202e}b");
  check "leading combining mark" "\"\\u{301}a\"" (Printer.string "\u{301}a");
  check "line separator" "\"\\u{2028}\"" (Printer.string "\u{2028}");
  check "paragraph separator" "\"\\u{2029}\"" (Printer.string "\u{2029}");
  check "zero width joiner" "\"\\u{200d}\"" (Printer.string "\u{200d}");
  check "C1 control" "\"\\u{85}\"" (Printer.string "\u{85}");
  check "no-break space" "\"\\u{a0}\"" (Printer.string "\u{a0}");
  check "highest escaped code point" "\"\\u{10ffff}\"" (Printer.string "\u{10ffff}");
  check "emoji kept" "\"\u{1f600}\"" (Printer.string "\u{1f600}");
  check "CJK kept" "\"\u{4e2d}\"" (Printer.string "\u{4e2d}")
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
    "of_to_string token"
    "Some 0x2a"
    (Printer.option (Printer.of_to_string (Printf.sprintf "0x%x")) (Some 42));
  check
    "of_to_string negative token"
    "Some (-5)"
    (Printer.option (Printer.of_to_string string_of_int) (Some (-5)));
  check
    "of_to_string with spaces"
    "Some (a b)"
    (Printer.option (Printer.of_to_string Fun.id) (Some "a b"));
  check
    "of_to_string with a comma"
    "Some (a,b)"
    (Printer.option (Printer.of_to_string Fun.id) (Some "a,b"));
  check
    "of_to_string empty"
    "Some ()"
    (Printer.option (Printer.of_to_string Fun.id) (Some ""));
  check
    "of_sexp negative atom"
    "Some (-5)"
    (Printer.option (Printer.of_sexp Sexplib0.Sexp_conv.sexp_of_int) (Some (-5)));
  check
    "of_sexp positive atom"
    "Some 5"
    (Printer.option (Printer.of_sexp Sexplib0.Sexp_conv.sexp_of_int) (Some 5));
  check
    "of_sexp lone minus"
    "Some -"
    (Printer.option (Printer.of_sexp (fun () -> Sexplib0.Sexp.Atom "-")) (Some ()))
;;

let render ?max_width f = Printer.Out.with_document ?max_width f

let check_out ?max_width name expected f =
  Alcotest.(check string) name expected (render ?max_width f)
;;

let raises_usage_error name f =
  match f () with
  | _ -> Alcotest.failf "%s: expected Usage_error" name
  | exception Usage_error _ -> ()
;;

let numbers p =
  Printer.Out.seq p "[" "]" [ 1000; 2000 ] (fun p n -> Printer.Out.value p Printer.int n)
;;

let test_doc_primitives () =
  let trailing =
    Printer.group
      "["
      [ Printer.text "1000;"
      ; Printer.breakable " "
      ; Printer.text "2000"
      ; Printer.if_break ";"
      ]
      "]"
  in
  check "if_break in a group that fits" "[1000; 2000]" trailing;
  check_narrow "if_break in a broken group" ~max_width:8 "[1000;\n 2000;]" trailing;
  check
    "hard_break"
    "a\nb"
    (Printer.concat [ Printer.text "a"; Printer.hard_break; Printer.text "b" ]);
  check
    "comment"
    "[1;  (* one *)\n 2\n]"
    (Printer.group
       "["
       [ Printer.text "1;"
       ; Printer.comment "one"
       ; Printer.breakable " "
       ; Printer.text "2"
       ]
       "]");
  check_narrow
    "shift_indent"
    ~max_width:3
    "(a\n   b)"
    (Printer.group
       "("
       [ Printer.text "a"
       ; Printer.shift_indent 2
       ; Printer.breakable " "
       ; Printer.text "b"
       ]
       ")");
  raises_usage_error "comment with a newline" (fun () -> Printer.comment "a\nb");
  raises_usage_error "comment with a terminator" (fun () -> Printer.comment "a *) b")
;;

let test_out_primitives () =
  let open Printer.Out in
  check_out "text" "a\nb\n" (fun p -> text p "a\nb\n");
  check_out ~max_width:5 "breakable" "[aaa\n bbb]" (fun p ->
    group p "[" "]" (fun p ->
      text p "aaa";
      breakable p " ";
      text p "bbb"));
  let trailing p =
    group p "[" "]" (fun p ->
      text p "1000;";
      breakable p " ";
      text p "2000";
      if_break p ";")
  in
  check_out "if_break in a group that fits" "[1000; 2000]" trailing;
  check_out ~max_width:8 "if_break in a broken group" "[1000;\n 2000;]" trailing;
  check_out "hard_break" "a\nb" (fun p ->
    text p "a";
    hard_break p;
    text p "b");
  check_out ~max_width:3 "begin_group and end_group" "<<a\n    b>>" (fun p ->
    begin_group p ~indent:4 "<<";
    text p "a";
    breakable p " ";
    text p "b";
    end_group p ">>");
  raises_usage_error "an unbalanced end_group" (fun () ->
    render (fun p -> end_group p "]"));
  check_out ~max_width:3 "shift_indent" "(a\n   b)" (fun p ->
    group p "(" ")" (fun p ->
      text p "a";
      shift_indent p 2;
      breakable p " ";
      text p "b"));
  check_out "comment" "[1;  (* one *)\n 2\n]" (fun p ->
    group p "[" "]" (fun p ->
      text p "1;";
      comment p "one";
      breakable p " ";
      text p "2"));
  raises_usage_error "comment with a newline" (fun () ->
    render (fun p -> comment p "a\nb"));
  raises_usage_error "comment with a terminator" (fun () ->
    render (fun p -> comment p "*)"));
  check_out ~max_width:8 "reflow" "Foo(111,\n    222)" (fun p -> reflow p "Foo(111, 222)");
  check_out "seq" "[1000; 2000]" numbers;
  check_out ~max_width:8 "seq breaks" "[1000;\n 2000]" numbers;
  check_out "seq with a separator" "(1, 2)" (fun p ->
    seq p ~sep:"," "(" ")" [ 1; 2 ] (fun p n -> value p Printer.int n));
  check_out "separator" "a; b" (fun p ->
    text p "a";
    separator p ";";
    text p "b");
  check_out "doc" "Some 1" (fun p -> doc p (Printer.option Printer.int (Some 1)));
  check_out "sexp" "(1 2)" (fun p ->
    sexp p Sexplib0.Sexp_conv.(sexp_of_list sexp_of_int) [ 1; 2 ]);
  check_out "group closes when its body raises" "[a]" (fun p ->
    match
      group p "[" "]" (fun p ->
        text p "a";
        failwith "body")
    with
    | () -> ()
    | exception Failure _ -> ())
;;

let test_out_argument () =
  let open Printer.Out in
  check_out "a negative argument is parenthesized" "(-5)" (fun p ->
    value (argument p) Printer.int (-5));
  check_out "an atomic argument is not" "5" (fun p -> value (argument p) Printer.int 5);
  check_out "a top-position document is not" "-5" (fun p -> value p Printer.int (-5));
  check_out "parens_if_argument" "(f x)" (fun p ->
    parens_if_argument (argument p) (fun p ->
      Alcotest.(check bool) "inside is top position" false (in_argument p);
      text p "f x"));
  check_out "parens_if_argument at top position" "f x" (fun p ->
    parens_if_argument p (fun p -> text p "f x"));
  check_out "a group resets the position" "[-5]" (fun p ->
    let p = argument p in
    Alcotest.(check bool) "argument" true (in_argument p);
    group p "[" "]" (fun p -> value p Printer.int (-5)));
  Alcotest.(check bool) "noop is never an argument" false (in_argument (argument noop))
;;

let test_out_speculation () =
  let open Printer.Out in
  check_out "commit keeps" "ab" (fun p ->
    text p "a";
    speculate p (fun p ->
      text p "b";
      (), `Commit));
  check_out "abort discards" "a" (fun p ->
    text p "a";
    speculate p (fun p ->
      text p "b";
      (), `Abort));
  check_out "raising discards" "a" (fun p ->
    text p "a";
    match
      speculate p (fun p ->
        text p "b";
        failwith "attempt")
    with
    | () -> ()
    | exception Failure _ -> ());
  check_out "begin and commit" "ab" (fun p ->
    text p "a";
    begin_speculation p;
    text p "b";
    commit_speculation p);
  check_out "begin and abort" "a" (fun p ->
    text p "a";
    begin_speculation p;
    text p "b";
    abort_speculation p)
;;

let test_out_deferred () =
  let open Printer.Out in
  check_out "a deferred region fills in at its anchor" "[1; 2; 3]" (fun p ->
    group p "[" "]" (fun p ->
      text p "1; ";
      let hole = deferred p in
      text p "; 3";
      Alcotest.(check bool) "live" true (should_print hole);
      text hole "2"));
  check_out "a deferred region keeps its parent's position" "Some (-1)" (fun p ->
    text p "Some ";
    value (deferred (argument p)) Printer.int (-1));
  let escaped = ref noop in
  check_out "a deferred region in an aborted speculation dies" "a" (fun p ->
    text p "a";
    let hole =
      speculate p (fun p ->
        let hole = deferred p in
        hole, `Abort)
    in
    Alcotest.(check bool) "dead" false (should_print hole);
    text hole "b";
    Alcotest.(check bool) "no region in a dead one" false (should_print (deferred hole));
    escaped := hole);
  Alcotest.(check bool) "freed with its document" false (should_print !escaped);
  text !escaped "late";
  Alcotest.(check bool)
    "no region in a freed one"
    false
    (should_print (deferred !escaped))
;;

let test_out_noop () =
  let open Printer.Out in
  Alcotest.(check bool) "noop prints nothing" false (should_print noop);
  text noop "a";
  value noop (fun _ -> Alcotest.fail "formatted for noop") 1;
  Alcotest.(check bool) "deferred noop" false (should_print (deferred noop));
  Alcotest.(check int)
    "speculate on noop"
    3
    (speculate noop (fun p ->
       text p "a";
       3, `Abort));
  Printer.release noop
;;

let tests =
  [ Alcotest.test_case "integers" `Quick test_integers
  ; Alcotest.test_case "floats" `Quick test_floats
  ; Alcotest.test_case "scalars" `Quick test_scalars
  ; Alcotest.test_case "strings" `Quick test_strings
  ; Alcotest.test_case "structures" `Quick test_structures
  ; Alcotest.test_case "layout" `Quick test_layout
  ; Alcotest.test_case "document primitives" `Quick test_doc_primitives
  ; Alcotest.test_case "printer primitives" `Quick test_out_primitives
  ; Alcotest.test_case "argument position" `Quick test_out_argument
  ; Alcotest.test_case "speculation" `Quick test_out_speculation
  ; Alcotest.test_case "deferred regions" `Quick test_out_deferred
  ; Alcotest.test_case "noop printer" `Quick test_out_noop
  ]
;;
