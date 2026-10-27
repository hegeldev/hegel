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

let rec push list value =
  let old = Atomic.get list in
  if not (Atomic.compare_and_set list old (value :: old)) then push list value
;;

module Bank = struct
  type t =
    { next_id : int Atomic.t
    ; accounts : (int * int Atomic.t) list Atomic.t
    }

  let create () = { next_id = Atomic.make 0; accounts = Atomic.make [] }
  let find bank id = List.assoc id (Atomic.get bank.accounts)

  let open_account bank ~deposit =
    let id = Atomic.fetch_and_add bank.next_id 1 in
    push bank.accounts (id, Atomic.make deposit);
    id
  ;;

  let balance bank id = Atomic.get (find bank id)
  let deposit bank id ~amount = ignore (Atomic.fetch_and_add (find bank id) amount : int)

  let transfer bank ~source ~dest ~amount =
    let source = find bank source in
    let dest = find bank dest in
    let source_balance = Atomic.get source in
    if source_balance >= amount
    then (
      let dest_balance = Atomic.get dest in
      Unix.sleepf 0.0001;
      Atomic.set source (source_balance - amount);
      Atomic.set dest (dest_balance + amount))
  ;;
end

module%hegel_concurrent_state_machine Bank_test = struct
  type ctx = Parallel_kernel.t Hegel_jane_concurrent.ctx

  type t =
    { bank : Bank.t
    ; accounts : int Stateful.Concurrent_pool.t
    ; all : int list Atomic.t
    ; total : int Atomic.t
    }

  let sexp_of_state state =
    Sexplib0.Sexp.List
      (List.rev_map
         (fun id ->
            Sexplib0.Sexp.Atom (Printf.sprintf "#%d=%d" id (Bank.balance state.bank id)))
         (Atomic.get state.all))
  ;;

  let accounts_gen = Stateful.Concurrent_pool.values_reusable
  let max_accounts = 10

  let open_account tc (_ : ctx) (state : t) =
    assume tc (List.length (Atomic.get state.all) < max_accounts);
    let deposit = draw tc (integers ~min_value:0 ~max_value:1_000 ()) in
    let id = Bank.open_account state.bank ~deposit in
    push state.all id;
    ignore (Atomic.fetch_and_add state.total deposit : int);
    Stateful.Concurrent_pool.add state.accounts tc id;
    note tc (Printf.sprintf "opened #%d" id)
  [@@rule { group = "accounts" }]
  ;;

  let deposit tc (_ : ctx) (state : t) =
    let id = draw_silent tc (accounts_gen state.accounts) in
    let amount = draw tc (integers ~min_value:1 ~max_value:1_000 ()) in
    note tc (Printf.sprintf "into #%d" id);
    Bank.deposit state.bank id ~amount;
    ignore (Atomic.fetch_and_add state.total amount : int)
  [@@rule { group = "accounts" }]
  ;;

  let transfer tc (_ : ctx) (state : t) =
    let source, balance =
      draw_silent
        tc
        (filter
           (fun (_, balance) -> balance > 0)
           (map (fun id -> id, Bank.balance state.bank id) (accounts_gen state.accounts)))
    in
    let dest =
      draw_silent tc (filter (fun id -> id <> source) (accounts_gen state.accounts))
    in
    let amount = draw tc (integers ~min_value:1 ~max_value:balance ()) in
    note tc (Printf.sprintf "from #%d to #%d" source dest);
    Bank.transfer state.bank ~source ~dest ~amount
  [@@rule { group = "transfers"; weight = 6 }]
  ;;

  let check_bal (state : t) =
    assert (
      List.for_all (fun id -> Bank.balance state.bank id >= 0) (Atomic.get state.all))
  ;;

  let no_negative_balance _tc (_ : ctx) state = check_bal state
  [@@rule { group = "transfers" }]
  ;;

  let no_money_lost _tc (state : t) =
    let sum =
      List.fold_left
        (fun sum id -> sum + Bank.balance state.bank id)
        0
        (Atomic.get state.all)
    in
    assert (sum = Atomic.get state.total)
  [@@invariant { always_check = true }]
  ;;

  let no_negative_balance_inv _tc state = check_bal state
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
   ;;
   ]} *)

let () = workload ()
