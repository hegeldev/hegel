(** Printing is the generator's job: a printable generator prints what it
    draws while it draws it, so an element's own printing shows up inside its
    container, a deferred region fills in as the value is used, and a
    rejected attempt leaves nothing behind. *)

open! Core
open Hegel
open Generators

let run_failing
      ?(settings =
        { (Settings.create ~test_cases:50 ~seed:0 ()) with verbosity = Settings.Normal })
      body
  =
  try Hegel.run_hegel_test ~settings body with
  | _ -> ()
;;

let print_report output =
  String.split_lines output
  |> List.take_while ~f:(fun line -> not (String.is_prefix line ~prefix:"Exception"))
  |> List.iter ~f:print_endline
;;

module Int_set = Stdlib.Set.Make (Int)

let int n = integers ~min_value:n ~max_value:n ()
let hex = with_ocaml_printer (fun n -> Printer.text (sprintf "0x%x" n)) (int 255)

let stacks elements =
  printable_composite ~label:"stack" (fun tc printer ->
    Printer.Out.parens_if_argument printer (fun printer ->
      Printer.Out.group printer ~indent:2 "" "" (fun printer ->
        Printer.Out.text printer "Stack.of_list";
        Printer.Out.breakable printer " ";
        Printer.Out.group printer "[" "]" (fun printer ->
          let n = draw_silent tc (integers ~min_value:1 ~max_value:1 ()) in
          List.init n ~f:(fun i ->
            if i > 0 then Printer.Out.separator printer ";";
            draw_and_print tc printer elements)))))
;;

type reading =
  { sensor : (int[@hegel.generator hex])
  ; ok : bool
  }
[@@deriving hegel_generator]

let%expect_test "an element's own printing shows up inside its container" =
  run_failing (fun tc ->
    let xs = draw tc (lists hex ~min_size:2 ~max_size:2 ()) in
    let stack = draw tc (lists (stacks (int 1)) ~min_size:2 ~max_size:2 ()) in
    let o =
      draw
        tc
        (filter Option.is_some (optional (with_ocaml_printer Printer.int (int (-3)))))
    in
    let r = draw tc hegel_generator_reading in
    ignore (xs, stack, o, r);
    assert false);
  print_report (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    let draw_1 = [0xff; 0xff] in
    let draw_2 = [Stack.of_list [1]; Stack.of_list [1]] in
    let draw_3 = Some (-3) in
    let draw_4 = { sensor = 0xff; ok = false } in
    |}]
;;

let tape_holes = ref []

(** A generator of functions that each draw a number when called: it prints
    as [Tape […]], the numbers filled in as the test body pulls them. *)
let tapes =
  printable_composite ~label:"tape" (fun tc printer ->
    Printer.Out.parens_if_argument printer (fun printer ->
      Printer.Out.group printer ~indent:2 "" "" (fun printer ->
        Printer.Out.text printer "Tape";
        Printer.Out.breakable printer " ";
        Printer.Out.group printer "[" "]" (fun printer ->
          let hole = Printer.Out.deferred printer in
          tape_holes := hole :: !tape_holes;
          let pulled = ref 0 in
          fun () ->
            let n = draw_silent tc (int 0) in
            if !pulled > 0 then Printer.Out.separator hole ";";
            Int.incr pulled;
            Printer.Out.value hole Printer.int n;
            n))))
;;

let%expect_test "a deferred value is printed as it is used" =
  run_failing (fun tc ->
    let t = draw tc tapes in
    let unused = draw tc tapes in
    let ts = draw tc (lists tapes ~min_size:1 ~max_size:1 ()) in
    ignore (t () + t () + List.hd_exn ts () : int);
    ignore unused;
    assert false);
  print_report (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    let draw_1 = Tape [0; 0] in
    let draw_2 = Tape [] in
    let draw_3 = [Tape [0]] in
    |}];
  List.iter !tape_holes ~f:(fun hole ->
    Printer.Out.text hole "late";
    assert (not (Printer.Out.should_print hole)))
;;

let%expect_test "a deferred region in a rejected attempt is dead" =
  tape_holes := [];
  run_failing (fun tc ->
    let attempts = ref 0 in
    let t =
      draw
        tc
        (filter
           (fun _ ->
              Int.incr attempts;
              !attempts % 2 = 0)
           tapes)
    in
    (match !tape_holes with
     | _ :: rejected :: _ ->
       assert (not (Printer.Out.should_print rejected));
       Printer.Out.text rejected "rejected"
     | _ -> ());
    ignore (t () : int);
    assert false);
  print_report (Expect_scrub.scrub_report [%expect.output]);
  [%expect {| let draw_1 = Tape [0] in |}]
;;

let%expect_test "a mapped value prints as its printer or call describes it" =
  run_failing (fun tc ->
    let d = draw tc (with_ocaml_printer Printer.int (map (fun n -> n * 2) (int 3))) in
    let s =
      draw tc (with_printer [%sexp_of: int list] (map (fun n -> [ n; n ]) (int 3)))
    in
    let k = draw tc (map_as_call "Key.of_int" Fn.id (int 3)) in
    let neg = draw tc (map_as_call "Key.of_int" Fn.id (int (-3))) in
    let ks =
      draw tc (lists (map_as_call "Key.of_int" Fn.id (int 0)) ~min_size:2 ~max_size:2 ())
    in
    let some =
      draw tc (filter Option.is_some (optional (map_as_call "Key.of_int" Fn.id (int 1))))
    in
    let wide =
      draw
        tc
        (map_as_call
           "Key.of_names"
           Fn.id
           (lists
              (just (String.make 20 'a') |> with_ocaml_printer Printer.string)
              ~min_size:4
              ~max_size:4
              ()))
    in
    ignore (d, s, k, neg, ks, some, wide);
    assert false);
  print_report (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    let draw_1 = 6 in
    let draw_2 = (3 3) in
    let draw_3 = Key.of_int 3 in
    let draw_4 = Key.of_int (-3) in
    let draw_5 = [Key.of_int 0; Key.of_int 0] in
    let draw_6 = Some (Key.of_int 1) in
    let draw_7 = Key.of_names
      ["aaaaaaaaaaaaaaaaaaaa";
       "aaaaaaaaaaaaaaaaaaaa";
       "aaaaaaaaaaaaaaaaaaaa";
       "aaaaaaaaaaaaaaaaaaaa"] in
    |}]
;;

type sonar =
  { active : bool
  ; depth : (int[@hegel.generator integers ~min_value:0 ~max_value:0 ()])
  }
[@@deriving hegel_generator]

let%expect_test "layout matches Rust's" =
  run_failing (fun tc ->
    let wide =
      draw
        tc
        (lists
           (with_ocaml_printer Printer.string (just (String.make 20 'a')))
           ~min_size:3
           ~max_size:3
           ())
    in
    let sonar = draw tc hegel_generator_sonar in
    let pair = draw tc (tuples2 (booleans ()) (int 0)) in
    let none = draw tc (filter Option.is_none (optional (int 0))) in
    ignore (wide, sonar, pair, none);
    assert false);
  print_report (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    let draw_1 = ["aaaaaaaaaaaaaaaaaaaa";
     "aaaaaaaaaaaaaaaaaaaa";
     "aaaaaaaaaaaaaaaaaaaa"] in
    let draw_2 = { active = false; depth = 0 } in
    let draw_3 = (false, 0) in
    let draw_4 = None in
    |}]
;;

let%expect_test "a narrow document breaks a record one field per line" =
  Hegel.run_hegel_test ~settings:(Settings.create ~test_cases:1 ~seed:0 ()) (fun tc ->
    print_endline
      (Printer.Out.with_document ~max_width:20 (fun printer ->
         ignore (draw_and_print tc printer hegel_generator_sonar : sonar))));
  [%expect
    {|
    {
      active = false;
      depth = 0 }
    |}]
;;

let verbose_lines ?(test_cases = 100) output body =
  let before = output () in
  Hegel.run_hegel_test
    ~settings:
      { (Settings.create ~test_cases ~seed:0 ()) with verbosity = Settings.Verbose }
    body;
  let lines = String.split_lines (output ()) in
  print_string before;
  lines
  |> List.filter ~f:(fun line ->
    not
      (List.mem
         ~equal:String.equal
         [ "Starting phase: Generate"; "Running test case"; "Ending phase: Generate" ]
         line))
;;

let check_lines name pattern lines =
  let re = Re.Perl.compile_pat pattern in
  match List.find lines ~f:(fun line -> not (Re.execp re line)) with
  | Some line -> printf "%s: unexpected line %S\n" name line
  | None -> printf "%s: %d lines ok\n" name (List.length lines)
;;

let distinct name pattern lines =
  let re = Re.Perl.compile_pat pattern in
  let elements line = List.map (Re.all re line) ~f:(fun group -> Re.Group.get group 1) in
  match
    List.find lines ~f:(fun line ->
      List.contains_dup ~compare:String.compare (elements line))
  with
  | Some line -> printf "%s: duplicate in %S\n" name line
  | None -> printf "%s: no duplicates\n" name
;;

let%expect_test "rejected attempts and duplicates are never printed" =
  verbose_lines
    (fun () -> [%expect.output])
    (fun tc ->
       ignore
         (draw tc (filter (fun n -> n = 0) (integers ~min_value:0 ~max_value:3 ())) : int))
  |> check_lines "filter" {|^let draw_\d+ = 0 in$|};
  let lists_lines =
    verbose_lines
      (fun () -> [%expect.output])
      (fun tc ->
         ignore (draw ~label:"xs" tc (lists (booleans ()) ~unique:true ()) : bool list))
  in
  check_lines
    "unique lists"
    {|^let xs = \[((true|false)(; (true|false))*)?\] in$|}
    lists_lines;
  distinct "unique lists" {|(true|false)|} lists_lines;
  let sets_lines =
    verbose_lines
      (fun () -> [%expect.output])
      (fun tc ->
         ignore
           (draw
              ~label:"xs"
              tc
              (sets (module Int_set) (integers ~min_value:0 ~max_value:2 ()) ())
            : Int_set.t))
  in
  check_lines "sets" {|^let xs = Set.of_list \[(\d(; \d)*)?\] in$|} sets_lines;
  distinct "sets" {|(\d)|} sets_lines;
  let assoc_lines =
    verbose_lines
      (fun () -> [%expect.output])
      (fun tc ->
         ignore
           (draw
              ~label:"xs"
              tc
              (assoc_lists (integers ~min_value:0 ~max_value:1 ()) (booleans ()) ())
            : (int * bool) list))
  in
  check_lines
    "assoc lists"
    {|^let xs = \[(\(\d, (true|false)\)(; \(\d, (true|false)\))*)?\] in$|}
    assoc_lines;
  distinct "assoc lists" {|\((\d),|} assoc_lines;
  [%expect
    {|
    filter: 100 lines ok
    unique lists: 100 lines ok
    unique lists: no duplicates
    sets: 100 lines ok
    sets: no duplicates
    assoc lists: 100 lines ok
    assoc lists: no duplicates
    |}]
;;

let%expect_test "a retried recursive value leaves no partial output" =
  verbose_lines
    ~test_cases:200
    (fun () -> [%expect.output])
    (fun tc ->
       ignore
         (draw
            ~label:"t"
            tc
            (recursive
               ~max_leaves:2
               (map_as_call "Leaf" Fn.id (int 0))
               (fun subtrees ->
                  map_as_call "Node" List.length (lists subtrees ~min_size:1 ())))
          : int))
  |> check_lines "recursive" {|^let t = (Leaf 0|Node \[[][LeafNod0; ]*\]) in$|};
  [%expect {| recursive: 200 lines ok |}]
;;

let%expect_test "combinators print what their components print" =
  run_failing (fun tc ->
    let either = draw tc (one_of [ hex ]) in
    let dependent =
      draw
        tc
        (flat_map_printable (fun n -> lists (int n) ~min_size:n ~max_size:n ()) (int 2))
    in
    let silent = draw_silent tc (flat_map (fun n -> int n) (int 2)) in
    ignore (either, dependent, silent);
    assert false);
  print_report (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    let draw_1 = 0xff in
    let draw_2 = [2; 2] in
    |}]
;;

let%expect_test "a draw that raises leaves no partial binding" =
  run_failing (fun tc ->
    ignore (draw tc (int 1) : int);
    ignore
      (draw
         tc
         (printable_composite (fun _ printer ->
            Printer.Out.text printer "Partial [";
            failwith "mid-draw"))
       : int));
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    let draw_1 = 1 in
    Exception: Failure("mid-draw")

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

let%expect_test "a note made while drawing prints after the binding" =
  run_failing (fun tc ->
    ignore
      (draw
         ~label:"n"
         tc
         (printable_composite (fun tc printer ->
            Hegel.note tc "drawing";
            let n = draw_silent tc (int 4) in
            Printer.Out.value printer Printer.int n;
            n))
       : int);
    assert false);
  print_report (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    let n = 4 in
    drawing
    |}]
;;

let%expect_test "a nested draw in a printable composite prints nothing" =
  run_failing (fun tc ->
    ignore
      (draw
         ~label:"n"
         tc
         (printable_composite (fun tc printer ->
            let n = draw ~label:"inner" tc (int 4) in
            Printer.Out.value printer Printer.int n;
            n))
       : int);
    assert false);
  print_report (Expect_scrub.scrub_report [%expect.output]);
  [%expect {| let n = 4 in |}]
;;
