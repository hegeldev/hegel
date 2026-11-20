module S = Hegel.Stateful
module J = Hegel_jane_concurrent

let rec try_decrement n =
  let v = Atomic.get n in
  v > 0 && (Atomic.compare_and_set n v (v - 1) || try_decrement n)
;;

let rules () =
  [ S.Concurrent_rule.create
      ~name:"increment"
      ~step:(fun _tc (_ : _ J.ctx) (n : int Atomic.t) -> Atomic.incr n)
      ()
  ; S.Concurrent_rule.create
      ~name:"decrement"
      ~step:(fun tc (_ : _ J.ctx) (n : int Atomic.t) -> Hegel.assume tc (try_decrement n))
      ()
  ; S.Concurrent_rule.create
      ~name:"nested"
      ~step:(fun _tc (ctx : _ J.ctx) (n : int Atomic.t) ->
        Concurrent.spawn_join ctx.concurrent () ~f:(fun _scope _ctx _concurrent ->
          Atomic.incr n))
      ()
  ]
;;

let invariants =
  [ S.Invariant.create
      ~name:"nonnegative"
      ~inv:(fun _tc n -> assert (Atomic.get n >= 0))
      ()
  ]
;;

module Counter = struct
  type ctx = unit J.ctx
  type state = int Atomic.t

  let rules = rules ()
  let invariants = invariants
end

module On_parallel = struct
  type ctx = Parallel_kernel.t J.ctx
  type state = int Atomic.t

  let rules =
    S.Concurrent_rule.create
      ~name:"bump_twice"
      ~step:(fun _tc (ctx : ctx) (n : int Atomic.t) ->
        let #((), ()) =
          Parallel_kernel.fork_join2
            ctx.context
            (fun _ -> Atomic.incr n)
            (fun _ -> Atomic.incr n)
        in
        ())
      ()
    :: rules ()
  ;;

  let invariants = invariants
end

let settings =
  { (Hegel.Settings.create ~test_cases:5 ~seed:0 ()) with
    database = Hegel.Settings.Disabled
  }
;;

type scheduler =
  { run : f:(unit Concurrent.t @ local portable -> unit) @ local unyielding -> unit }

let in_thread =
  { run = (fun ~f -> Concurrent_in_thread.with_blocking Await.Terminator.unkillable ~f) }
;;

let on_parallel =
  { run =
      (fun ~f ->
        Parallel_scheduler.with_concurrent ~max_workers:1 (fun concurrent ->
          let concurrent = Concurrent.with_context concurrent (fun _ _ ~f -> f ()) in
          f concurrent [@nontail])
        [@nontail])
  }
;;

let state_machine_test scheduler ~custom () =
  Hegel.run_hegel_test ~settings (fun tc ->
    scheduler.run ~f:(fun concurrent ->
      let concurrency =
        if custom
        then
          J.create_local ~spawn_join_n:(fun ~n ~f ->
            Concurrent.spawn_join_n
              concurrent
              ()
              ~n
              ~f:(fun _scope context concurrent index ->
                f { J.context; concurrent } index [@nontail])
            |> Base.Iarray.to_list)
        else J.of_concurrent concurrent
      in
      S.run_concurrent
        ~concurrency
        tc
        (module Counter)
        ~step_count:10
        ~init:(Atomic.make 0)
        ~min_concurrency:4
        ~max_concurrency:4 [@nontail]))
;;

let parallel_scheduler_test () =
  let scheduler = Parallel_scheduler.scheduler () in
  Hegel.run_hegel_test ~settings (fun tc ->
    Parallel_scheduler.parallel scheduler (fun _kernel concurrent ->
      S.run_concurrent
        ~concurrency:(J.of_concurrent concurrent)
        tc
        (module On_parallel)
        ~step_count:10
        ~init:(Atomic.make 0)
        ~min_concurrency:4
        ~max_concurrency:4 [@nontail]))
;;

module%hegel_concurrent_state_machine Ppx_counter = struct
  type ctx = unit J.ctx

  let increment _tc (_ : ctx) (n : int Atomic.t) = Atomic.incr n [@@rule]
end

let ppx_machine_test scheduler () =
  Hegel.run_hegel_test ~settings (fun tc ->
    scheduler.run ~f:(fun concurrent ->
      Ppx_counter.run
        tc
        ~concurrency:(J.of_concurrent concurrent)
        ~init:(Atomic.make 0)
        ~step_count:10
        ~min_concurrency:4
        ~max_concurrency:4 [@nontail]))
;;

exception Boom of int

let worker_exception_test scheduler () =
  let active = Atomic.make 0 in
  let module M = struct
    type ctx = unit J.ctx
    type state = unit

    let rules =
      [ S.Concurrent_rule.create
          ~name:"boom"
          ~step:(fun tc (_ : ctx) () -> raise (Boom (Hegel.draw tc (Hegel.integers ()))))
          ()
      ]
    ;;

    let invariants = []
  end
  in
  match
    Hegel.run_hegel_test
      ~settings:{ settings with verbosity = Hegel.Settings.Quiet }
      (fun tc ->
         scheduler.run ~f:(fun concurrent ->
           let concurrent =
             Concurrent.with_context concurrent (fun () _ ~f ->
               Atomic.incr active;
               match f () with
               | () -> Atomic.decr active
               | exception exn ->
                 let bt = Printexc.get_raw_backtrace () in
                 Atomic.decr active;
                 Printexc.raise_with_backtrace exn bt)
           in
           S.run_concurrent
             ~concurrency:(J.of_concurrent concurrent)
             tc
             (module M)
             ~step_count:5
             ~init:()
             ~min_concurrency:2
             ~max_concurrency:2 [@nontail]))
  with
  | () -> Alcotest.fail "expected Boom"
  | exception Boom _ ->
    Alcotest.(check int) "workers exited after rule failure" 0 (Atomic.get active)
;;

let worker_lifetime_test scheduler ~fail () =
  let started = Atomic.make 0 in
  let active = Atomic.make 0 in
  let checks = ref 0 in
  Hegel.run_hegel_test ~settings:{ settings with test_cases = 1 } (fun tc ->
    scheduler.run ~f:(fun concurrent ->
      let concurrent =
        Concurrent.with_context concurrent (fun () _ ~f ->
          let id = Atomic.fetch_and_add started 1 in
          Atomic.incr active;
          match f id with
          | () -> Atomic.decr active
          | exception exn ->
            let bt = Printexc.get_raw_backtrace () in
            Atomic.decr active;
            Printexc.raise_with_backtrace exn bt)
      in
      let seen = Atomic.make 0 in
      let rule =
        S.Concurrent_rule.create
          ~name:"step"
          ~step:(fun _tc (ctx : int J.ctx) () ->
            let id = ctx.context in
            let rec record () =
              let previous = Atomic.get seen in
              if not (Atomic.compare_and_set seen previous (previous lor (1 lsl id)))
              then record ()
            in
            record ())
          ()
      in
      let invariant =
        S.Invariant.create
          ~name:"workers"
          ~always_check:true
          ~inv:(fun _tc () ->
            incr checks;
            if Atomic.get seen <> 0
            then (
              Alcotest.(check int) "fixed workers across rounds" 4 (Atomic.get started);
              if fail then failwith "invariant"))
          ()
      in
      let result =
        try
          S.run_concurrent_internal
            tc
            ~concurrency:(J.of_concurrent concurrent)
            ~init:()
            ~rules:[ rule ]
            ~invariants:[ invariant ]
            ~min_concurrency:4
            ~max_concurrency:4
            ~step_count:20;
          None
        with
        | Failure msg -> Some msg
      in
      Alcotest.(check (option string))
        "run result"
        (if fail then Some "invariant" else None)
        result;
      Alcotest.(check int) "workers exited before returning" 0 (Atomic.get active);
      if not fail
      then (
        Alcotest.(check bool) "multiple rounds" true (!checks > 3);
        Alcotest.(check int) "every worker ran" 15 (Atomic.get seen))))
;;

let tests_for scheduler =
  [ Alcotest.test_case
      "runs state machines and child tasks"
      `Quick
      (state_machine_test scheduler ~custom:false)
  ; Alcotest.test_case
      "accepts a custom local backend"
      `Quick
      (state_machine_test scheduler ~custom:true)
  ; Alcotest.test_case "runs a ppx state machine" `Quick (ppx_machine_test scheduler)
  ; Alcotest.test_case
      "worker exceptions reach the caller"
      `Quick
      (worker_exception_test scheduler)
  ; Alcotest.test_case
      "reuses workers and joins them after success"
      `Quick
      (worker_lifetime_test scheduler ~fail:false)
  ; Alcotest.test_case
      "joins workers after invariant failure"
      `Quick
      (worker_lifetime_test scheduler ~fail:true)
  ]
;;

let () =
  Alcotest.run
    "hegel-jane-concurrent"
    [ "threads", tests_for in_thread
    ; "parallel", tests_for on_parallel
    ; ( "parallel kernel"
      , [ Alcotest.test_case
            "rules use the parallel context"
            `Quick
            parallel_scheduler_test
        ] )
    ]
;;
