open! Core
module G = Hegel.Generators

let one_case_settings () = Hegel.Settings.create ~test_cases:1 ()
let small_int = G.integers ~min_value:0 ~max_value:9 ()

(* Two handles — the parent and its clone — can be driven from two threads at
   once without tripping the engine's concurrent-use guard. *)
let test_clone_concurrent () =
  let worker_value = ref (-1) in
  Hegel.run_hegel_test ~settings:(one_case_settings ()) (fun tc ->
    let worker = Hegel.clone tc in
    let t =
      Caml_threads.Thread.create
        (fun () -> worker_value := Hegel.draw_silent worker small_int)
        ()
    in
    let main = Hegel.draw_silent tc small_int in
    Caml_threads.Thread.join t;
    Alcotest.(check bool) "main value in range" true (main >= 0 && main <= 9));
  Alcotest.(check bool)
    "worker value in range"
    true
    (!worker_value >= 0 && !worker_value <= 9)
;;

(* A clone's stream is deterministic: replaying the same seeded case reproduces
   both the parent's and the clone's drawn values. *)
let test_clone_reproducible () =
  let run () =
    let pair = ref (0, 0) in
    Hegel.run_hegel_test
      ~settings:{ (one_case_settings ()) with seed = Some 42 }
      (fun tc ->
         let p = Hegel.draw_silent tc small_int in
         let c = Hegel.draw_silent (Hegel.clone tc) small_int in
         pair := p, c);
    !pair
  in
  let a = run () in
  let b = run () in
  Alcotest.(check (pair int int)) "seeded clone draws reproduce" a b
;;

(* Cloning a clone yields a further independent stream that also draws
   cleanly. *)
let test_clone_of_clone () =
  let value = ref (-1) in
  Hegel.run_hegel_test ~settings:(one_case_settings ()) (fun tc ->
    let c1 = Hegel.clone tc in
    let c2 = Hegel.clone c1 in
    value := Hegel.draw_silent c2 small_int);
  Alcotest.(check bool) "nested clone value in range" true (!value >= 0 && !value <= 9)
;;

let tests =
  [ Alcotest.test_case "clone concurrent" `Quick test_clone_concurrent
  ; Alcotest.test_case "clone reproducible" `Quick test_clone_reproducible
  ; Alcotest.test_case "clone of clone" `Quick test_clone_of_clone
  ]
;;
