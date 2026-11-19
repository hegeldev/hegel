type 'ctx ctx =
  { context : 'ctx
  ; concurrent : 'ctx Concurrent.t
  }

let create_local ~(spawn_join_n @ local) = exclave_
  Hegel.Concurrency_internal.of_dispatch spawn_join_n
;;

module Mvar = Await.Await.Mvar

type 'ctx job =
  { body : 'ctx ctx @ local -> int -> Hegel.Concurrency.outcome @@ aliased many portable }

type finished = { outcome : Hegel.Concurrency.outcome @@ aliased many }

type 'ctx worker =
  { jobs : 'ctx job option Mvar.t
  ; outcomes : finished Mvar.t
  }

let rec run_worker (ctx @ local) worker index =
  match Mvar.take (Concurrent.await ctx.concurrent) worker.jobs with
  | None -> ()
  | Some job ->
    let outcome =
      try job.body ctx index with
      | exn -> Some (exn, Printexc.get_raw_backtrace ())
    in
    Mvar.put_exn worker.outcomes { outcome };
    run_worker ctx worker index
;;

let rec start_workers (scope @ local) index = function
  | [] -> ()
  | worker :: rest ->
    Concurrent.spawn scope ~f:(fun _scope context concurrent ->
      run_worker { context; concurrent } worker index [@nontail]);
    start_workers scope (index + 1) rest
;;

let rec collect_outcomes (await @ local) = function
  | [] -> []
  | worker :: rest ->
    let result = Mvar.take await worker.outcomes in
    result.outcome :: collect_outcomes await rest
;;

let of_concurrent (c @ local) = exclave_
  Hegel.Concurrency_internal.of_pool_runner
    { run =
        (fun ~f ->
          Concurrent.with_scope c () ~f:(fun scope ->
            let pool = ref None in
            let spawn_join_n ~n ~f =
              let workers =
                match !pool with
                | Some workers -> workers
                | None ->
                  let workers =
                    List.init n (fun _ ->
                      { jobs = Mvar.create (); outcomes = Mvar.create () })
                  in
                  start_workers scope 0 workers;
                  pool := Some workers;
                  workers
              in
              List.iter
                (fun worker -> Mvar.put_exn worker.jobs (Some { body = f }))
                workers;
              collect_outcomes (Concurrent.Spawn.await scope) workers [@nontail]
            in
            let result = f spawn_join_n in
            Option.iter (List.iter (fun worker -> Mvar.put_exn worker.jobs None)) !pool;
            result)
          [@nontail])
    }
;;
