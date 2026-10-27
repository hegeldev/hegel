(** A standalone workload binary built with [let%hegel_main].

    Each invocation runs exactly one test case: a long sequence of rounds in
    which several workers open accounts and transfer money between them. A
    transfer reads both balances and writes them back in separate steps, so two
    concurrent transfers that touch the same account can overwrite each other
    and money disappears or appears. The workers run on Jane Street's
    [Parallel] scheduler, one domain each. Try:

    {v
      dune exec examples/workload/workload.exe -- --help
      dune exec examples/workload/workload.exe
      dune exec examples/workload/workload.exe -- --seed 1 --verbosity verbose
      dune exec examples/workload/workload.exe -- --failure-blob <blob it printed>
    v} *)

open Hegel

module Bank = struct
  type t = { next_id : int Atomic.t }

  type account =
    { id : int
    ; balance : int Atomic.t
    }

  let create () = { next_id = Atomic.make 0 }

  let open_account bank ~deposit =
    { id = Atomic.fetch_and_add bank.next_id 1; balance = Atomic.make deposit }
  ;;

  let balance account = Atomic.get account.balance

  let transfer ~source ~dest ~amount =
    let source_balance = Atomic.get source.balance in
    if source_balance >= amount
    then (
      let dest_balance = Atomic.get dest.balance in
      Unix.sleepf 0.0001;
      Atomic.set source.balance (source_balance - amount);
      Atomic.set dest.balance (dest_balance + amount))
  ;;
end

module%hegel_concurrent_state_machine Bank_test = struct
  type ctx = Parallel_kernel.t Hegel_jane_concurrent.ctx

  type t =
    { bank : Bank.t
    ; accounts : Bank.account Stateful.Concurrent_pool.t
    ; all : Bank.account list Atomic.t
    ; total : int Atomic.t
    }

  let sexp_of_state state =
    Sexplib0.Sexp.List
      (List.rev_map
         (fun (account : Bank.account) ->
            Sexplib0.Sexp.Atom (Printf.sprintf "#%d=%d" account.id (Bank.balance account)))
         (Atomic.get state.all))
  ;;

  let rec register all account =
    let old = Atomic.get all in
    if not (Atomic.compare_and_set all old (account :: old)) then register all account
  ;;

  let accounts_gen = Stateful.Concurrent_pool.values_reusable

  let open_account tc (_ : ctx) (state : t) =
    let deposit = draw tc (integers ~min_value:0 ~max_value:1_000 ()) in
    let account = Bank.open_account state.bank ~deposit in
    register state.all account;
    ignore (Atomic.fetch_and_add state.total deposit : int);
    Stateful.Concurrent_pool.add state.accounts tc account;
    note tc (Printf.sprintf "opened #%d" account.id)
  [@@rule { group = "bank" }]
  ;;

  let transfer tc (_ : ctx) (state : t) =
    let source =
      draw_silent tc (accounts_gen state.accounts)
    in
    let dest =
      draw_silent
        tc
        (filter
           (fun (account : Bank.account) -> account.id <> source.id)
           (accounts_gen state.accounts))
    in
    let amount = draw tc (integers ~min_value:1 ~max_value:500 ()) in
    note tc (Printf.sprintf "from #%d to #%d" source.id dest.id);
    Bank.transfer ~source ~dest ~amount
  [@@rule { group = "bank"; weight = 6 }]
  ;;

  let check_balance tc (_ : ctx) (state : t) =
    let account =
      draw_silent tc (accounts_gen state.accounts)
    in
    note tc (Printf.sprintf "#%d" account.id);
    assert (Bank.balance account >= 0)
  [@@rule { group = "bank" }]
  ;;

  let no_money_lost _tc (state : t) =
    let sum =
      List.fold_left
        (fun sum account -> sum + Bank.balance account)
        0
        (Atomic.get state.all)
    in
    assert (sum = Atomic.get state.total)
  [@@invariant { always_check = true }]
  ;;

  let no_negative_balance _tc (state : t) =
    assert (List.for_all (fun account -> Bank.balance account >= 0) (Atomic.get state.all))
  [@@invariant { always_check = true }]
  ;;
end

let scheduler = Parallel_scheduler.scheduler ()

let%hegel_main workload tc =
  let state =
    { Bank_test.bank = Bank.create ()
    ; accounts = Stateful.Concurrent_pool.create tc
    ; all = Atomic.make []
    ; total = Atomic.make 0
    }
  in
  Parallel_scheduler.parallel scheduler (fun context concurrent ->
    let ctx = { Hegel_jane_concurrent.context; concurrent } in
    Bank_test.open_account tc ctx state;
    Bank_test.open_account tc ctx state;
    Bank_test.run
      ~concurrency:(Hegel_jane_concurrent.of_concurrent concurrent)
      tc
      ~init:state
      ~step_count:1_000
      ~min_concurrency:2
      ~max_concurrency:4 [@nontail])
[@@settings
  let s = Settings.default () in
  { s with phases = List.filter (fun phase -> phase <> Settings.Shrink) s.phases }]
;;

(* The same workload on systhreads instead of a Parallel scheduler. To use it,
   replace the main above with this one and change [Bank_test]'s context to
   [type ctx = unit].

   {[
   let%hegel_main workload tc =
     let state =
       { Bank_test.bank = Bank.create ()
       ; accounts = Stateful.Concurrent_pool.create tc
       ; all = Atomic.make []
       ; total = Atomic.make 0
       }
     in
     Bank_test.open_account tc () state;
     Bank_test.open_account tc () state;
     Bank_test.run
       ~concurrency:Concurrency.threads
       tc
       ~init:state
       ~step_count:1_000
       ~min_concurrency:2
       ~max_concurrency:4
   [@@settings
     let s = Settings.default () in
     { s with phases = List.filter (fun phase -> phase <> Settings.Shrink) s.phases }]
   ;;
   ]} *)

let () = workload ()
