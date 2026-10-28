open! Core
open Hegel

(* Deterministic, database-disabled run so the failure blob is stable; swallow
   the failure so the expect block only sees the report. *)
let run_failing body =
  let settings =
    { (Settings.create ~test_cases:20 ~seed:0 ()) with
      verbosity = Settings.Normal
    ; database = Settings.Disabled
    }
  in
  try Hegel.run_hegel_test ~settings body with
  | _ -> ()
;;

(* A state machine module: [inc] and [my_inv] are collected into its [rules]
   and [invariants]. *)
module%hegel_state_machine Counter = struct
  type state = int ref

  let inc _tc n = incr n [@@rule]

  let my_inv tc n =
    Hegel.note tc (sprintf "checking n = %d" !n);
    assert (!n <= 1)
  [@@invariant { always_check = true }]
  ;;
end

let%expect_test "an invariant failure names the invariant after the steps" =
  run_failing (fun tc -> Counter.run tc ~init:(ref 0));
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Checking invariants on the initial state.
      checking n = 0
    Step 1: inc {
    }
      checking n = 1
    Step 2: inc {
    }
      checking n = 2
    Invariant my_inv failed:
    Exception: File "ppx/test/expect_tests/test_stateful_trace.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

let%expect_test "invariant failure in the initial state" =
  let module M = struct
    type state = unit

    let rules = [ Stateful.Rule.create ~name:"noop" ~step:(fun _tc () -> ()) () ]

    let invariants =
      [ Stateful.Invariant.create ~name:"silly_inv" ~inv:(fun _tc () -> assert false) () ]
    ;;
  end
  in
  run_failing (fun tc -> Stateful.run tc (module M) ~init:());
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Checking invariants on the initial state.
    Invariant silly_inv failed:
    Exception: File "ppx/test/expect_tests/test_stateful_trace.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

module%hegel_state_machine Stack = struct
  type state = int list ref

  let push tc stack =
    let n = Hegel.draw tc (integers ~min_value:0 ~max_value:100 ()) in
    stack := n :: !stack
  [@@rule]
  ;;

  let pop tc stack =
    match !stack with
    | [] -> Hegel.assume tc false
    | top :: rest ->
      assert (top < 50);
      stack := rest
  [@@rule]
  ;;
end

let%expect_test "a rule failure names the rule across multiple steps" =
  run_failing (fun tc -> Stack.run tc ~init:(ref []));
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Step 1: push {
      let n = 50 in
    }
    Step 2: pop {
    }
    Rule pop failed:
    Exception: File "ppx/test/expect_tests/test_stateful_trace.ml", line LINE, characters C1-C2: Assertion failed

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

(* The only rule rejects its first application, so swarm testing cannot hide
   the rejection. *)
module%hegel_state_machine Rejecting = struct
  type state = int ref

  let step tc n =
    incr n;
    Hegel.assume tc (!n > 1)
  [@@rule]
  ;;
end

let%hegel_test rejecting_steps tc = Rejecting.run tc ~init:(ref 0) ~step_count:3
[@@settings
  { (Settings.create ~test_cases:1 ~seed:0 ()) with
    verbosity = Settings.Verbose
  ; phases = [ Settings.Generate ]
  }]
;;

let%expect_test "a rejected rule notes the rejection under its heading" =
  rejecting_steps ();
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Starting phase: Generate
    Running test case
    Step 1: step {
      Rule stopped early due to violated assumption.
    }
    Step 2: step {
    }
    Step 3: step {
    }
    Step 4: step {
    }
    Ending phase: Generate
    |}]
;;

(* The rule raises from a tail call, so its own frame is not in the backtrace. *)
module%hegel_state_machine Tail_boom = struct
  type state = unit

  let boom tc () =
    ignore (Hegel.draw tc (integers ()) : int);
    failwith "tail boom"
  [@@rule]
  ;;
end

let%expect_test "a rule failure keeps its origin between replays" =
  let recording = Printexc.backtrace_status () in
  Printexc.record_backtrace true;
  Exn.protect
    ~finally:(fun () -> Printexc.record_backtrace recording)
    ~f:(fun () ->
      match
        Hegel.run_hegel_test
          ~settings:
            { (Settings.create ~test_cases:20 ~seed:0 ()) with
              database = Settings.Disabled
            ; verbosity = Settings.Normal
            ; nondeterminism_strictness = Error
            }
          (fun tc -> Tail_boom.run tc ~step_count:3 ~init:())
      with
      | () -> print_endline "passed"
      | exception Failure message -> print_endline message);
  print_string (Expect_scrub.scrub_report [%expect.output]);
  [%expect
    {|
    Step 1: boom {
      let draw_1 = 0 in
    }
    Rule boom failed:
    Exception: Failure("tail boom")

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    tail boom
    |}]
;;

module%hegel_concurrent_state_machine Concurrent_boom = struct
  type state = unit

  let boom tc () () =
    ignore (Hegel.draw tc (integers ()) : int);
    failwith "concurrent boom"
  [@@rule]
  ;;
end

let%expect_test "one concurrent worker remains deterministic" =
  (try
     Hegel.run_hegel_test
       ~settings:
         { (Settings.create ~test_cases:20 ~seed:0 ()) with
           database = Settings.Disabled
         ; verbosity = Settings.Normal
         }
       (fun tc ->
          Concurrent_boom.run
            tc
            ~step_count:5
            ~init:()
            ~min_concurrency:1
            ~max_concurrency:1)
   with
   | Failure message as exn ->
     if String.equal message "concurrent boom" then () else raise exn
   | exn -> raise exn);
  print_string (Expect_scrub.scrub_concurrent_report [%expect.output]);
  [%expect
    {|
    Concurrency level: 1
    ---------------- Round 1: group "<anonymous>" ----------------
    [worker 0 +time] Rule: boom {
    [worker 0 +time]   let draw_1 = 0 in
    [worker 0 +time] }
    [worker 0 +time] Rule boom failed:
    Exception: Failure("concurrent boom")

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;

module%hegel_concurrent_state_machine Concurrent_counter = struct
  type state = int Atomic.t

  let increment _tc () state = Atomic.incr state [@@rule { group = "writes" }]

  let stays_zero _tc state = if Atomic.get state <> 0 then failwith "invariant boom"
  [@@invariant { always_check = true }]
  ;;
end

let%expect_test "concurrent invariant failures retain their names" =
  (try
     Hegel.run_hegel_test
       ~settings:
         { (Settings.create ~test_cases:1 ~seed:0 ()) with database = Settings.Disabled }
       (fun tc ->
          Concurrent_counter.run
            tc
            ~init:(Atomic.make 0)
            ~min_concurrency:1
            ~max_concurrency:3)
   with
   | Failure message when String.equal message "invariant boom" -> ());
  print_string (Expect_scrub.scrub_concurrent_report [%expect.output]);
  [%expect
    {|
    Concurrency level: 1
    Checking invariants on the initial state.
    ---------------- Round 1: group "writes" ----------------
    [worker 0 +time] Rule: increment {
    [worker 0 +time] }
    Invariant stays_zero failed:
    Exception: Failure("invariant boom")

    To reproduce this failure, pass this argument to run_hegel_test:
        ~failure_blobs:[ "<BLOB>" ]
    |}]
;;
