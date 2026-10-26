let%hegel_test failing_example tc =
  let n = Hegel.draw tc (Hegel.integers ()) in
  let s = Hegel.draw tc (Hegel.text ()) in
  Hegel.note tc "checking";
  if n = 7 then failwith s
[@@example { n = 1; s = "fine" }]
[@@example { n = 7; s = "seven" }]
[@@settings Hegel.Settings.create ~test_cases:1 ()]
;;

let%expect_test "a failing explicit example prints the values it gave" =
  (try failing_example () with
   | _ -> ());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    n = 7
    s = "seven"
    checking
    Exception: Failure("seven")
    |}]
;;
