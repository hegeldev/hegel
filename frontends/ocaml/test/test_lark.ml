open Hegel

let settings ?(test_cases = 50) () = Settings.create ~test_cases ~seed:0 ()

let check_all ?test_cases gen check =
  run_hegel_test ~settings:(settings ?test_cases ()) (fun tc -> check (draw tc gen))
;;

let expect_usage_error ~substring gen =
  match check_all ~test_cases:1 gen ignore with
  | () -> Alcotest.fail "expected Usage_error"
  | exception Usage_error message ->
    if not (Test_helpers.contains_substring message substring)
    then Alcotest.failf "%S does not contain %S" message substring
;;

let arithmetic =
  {|
    ?start: sum
    ?sum: product | sum "+" product
    ?product: atom | product "*" atom
    ?atom: NUMBER | "(" sum ")"
    %import common.NUMBER
  |}
;;

let test_arithmetic () =
  check_all (from_lark arithmetic ()) (fun s ->
    assert (String.length s >= 1);
    assert (String.for_all (fun c -> String.contains "0123456789.eE+-*()" c) s))
;;

let test_start () =
  check_all
    (from_lark
       {|
      start: "a"
      other: "b"
    |}
       ~start:"other"
       ())
    (fun s -> Alcotest.(check string) "the start rule's string" "b" s)
;;

let test_explicit () =
  let names = from_regex "[a-z]{1,5}" () in
  check_all
    (from_lark
       {|
        start: "<" NAME ">"
        %declare NAME
      |}
       ~explicit:[ "NAME", names ]
       ())
    (fun s ->
       let n = String.length s in
       assert (n >= 3 && s.[0] = '<' && s.[n - 1] = '>');
       assert (String.for_all (fun c -> c >= 'a' && c <= 'z') (String.sub s 1 (n - 2))))
;;

let test_explicit_counts_toward_max_size () =
  let words = text ~min_size:1 ~max_size:3 ~alphabet:"xyz" () in
  check_all
    (from_lark
       {|
         start: WORD ("," WORD)*
         %declare WORD
       |}
       ~explicit:[ "WORD", words ]
       ~max_size:12
       ())
    (fun s -> assert (String.length s <= 12))
;;

let test_explicit_without_declare_is_rejected () =
  expect_usage_error
    ~substring:"NAME"
    (from_lark
       {|
        start: "x" NAME
        %declare NAME
      |}
       ())
;;

let test_explicit_value_must_be_utf_8 () =
  expect_usage_error
    ~substring:"UTF-8"
    (from_lark
       {|
         start: NAME
         %declare NAME
       |}
       ~explicit:[ "NAME", just "\xff" ]
       ())
;;

let test_alphabet () =
  check_all (from_lark {|start: /[a-z]+/|} ~alphabet:"ab" ()) (fun s ->
    assert (String.length s >= 1);
    assert (String.for_all (fun c -> c = 'a' || c = 'b') s))
;;

let test_max_size_and_depth () =
  check_all (from_lark arithmetic ~max_size:20 ~max_depth:30 ()) (fun s ->
    assert (String.length s <= 20))
;;

let test_import_path () =
  Test_helpers.with_tempdir ~prefix:"hegel-lark" ~f:(fun dir ->
    Out_channel.with_open_bin (Filename.concat dir "lib.lark") (fun oc ->
      Out_channel.output_string oc "WORD: \"hello\"\n");
    Sys.mkdir (Filename.concat dir "broken.lark") 0o755;
    check_all
      (from_lark
         {|
           start: WORD " " NUMBER
           %import lib.WORD
           %import common.NUMBER
         |}
         ~import_path:dir
         ())
      (fun s -> assert (String.starts_with ~prefix:"hello " s));
    expect_usage_error
      ~substring:"Could not read grammar"
      (from_lark
         {|
           start: X
           %import broken.X
         |}
         ~import_path:dir
         ()))
;;

let test_bad_grammar () =
  expect_usage_error ~substring:"" (from_lark "start: undefined_rule" ())
;;

let test_invalid_options () =
  let raises f substring =
    match f () with
    | (_ : string generator) -> Alcotest.fail "expected Usage_error"
    | exception Usage_error message ->
      if not (Test_helpers.contains_substring message substring)
      then Alcotest.failf "%S does not contain %S" message substring
  in
  raises (fun () -> from_lark arithmetic ~max_size:0 ()) "max_size";
  raises (fun () -> from_lark arithmetic ~max_depth:0 ()) "max_depth";
  raises
    (fun () ->
       from_lark arithmetic ~explicit:[ "A", text (); "B", text (); "A", text () ] ())
    "'A' is given more than one generator";
  raises (fun () -> from_lark arithmetic ~codec:"ascii" ~alphabet:"a" ()) "alphabet"
;;

let test_prints_as_a_string () =
  let shown = ref "" in
  run_hegel_test ~settings:(settings ~test_cases:1 ()) (fun tc ->
    let _, show = Generators.Ppx_internal.draw tc (from_lark {|start: "hi"|} ()) in
    shown := Printer.to_string (show ()));
  Alcotest.(check string) "quoted" {|"hi"|} !shown
;;

(* The compiled grammar is freed when the generator is collected. *)
let test_collected () =
  let draw_once () =
    check_all ~test_cases:1 (from_lark {|start: "x"|} ()) (fun s ->
      Alcotest.(check string) "drawn" "x" s)
  in
  draw_once ();
  ignore (from_lark {|start: "y"|} () : string generator);
  Gc.full_major ();
  Gc.full_major ()
;;

let tests =
  [ Alcotest.test_case "arithmetic" `Quick test_arithmetic
  ; Alcotest.test_case "start" `Quick test_start
  ; Alcotest.test_case "explicit" `Quick test_explicit
  ; Alcotest.test_case
      "explicit counts toward max_size"
      `Quick
      test_explicit_counts_toward_max_size
  ; Alcotest.test_case
      "declared terminal needs explicit"
      `Quick
      test_explicit_without_declare_is_rejected
  ; Alcotest.test_case
      "explicit value must be UTF-8"
      `Quick
      test_explicit_value_must_be_utf_8
  ; Alcotest.test_case "alphabet" `Quick test_alphabet
  ; Alcotest.test_case "max_size and max_depth" `Quick test_max_size_and_depth
  ; Alcotest.test_case "import_path" `Quick test_import_path
  ; Alcotest.test_case "bad grammar" `Quick test_bad_grammar
  ; Alcotest.test_case "invalid options" `Quick test_invalid_options
  ; Alcotest.test_case "prints as a string" `Quick test_prints_as_a_string
  ; Alcotest.test_case "collected" `Quick test_collected
  ]
;;
