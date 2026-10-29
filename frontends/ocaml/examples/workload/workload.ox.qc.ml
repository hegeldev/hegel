(** The bank workload of {!Workload}, written with Base_quickcheck instead of
    Hegel.

    Base_quickcheck has no stateful testing, so the state machine is spelled
    out by hand: an [action] type with one constructor per Hegel rule, an
    [apply] function that runs an action against the bank, and a generated
    list of actions, with balances checked after each one. The actions
    run one after another, not on concurrent workers, so the race in
    [Bank.transfer] is never hit and the test passes.

    Actions are generated before the test runs, so they can't name an account
    that exists or a balance it has. They carry plain ints instead, and
    [apply] turns them into an account and an amount from the state at the
    time it runs. Try:

    {v
      dune exec ./workload_qc.exe
    v} *)

open Base_quickcheck
open Sexplib0.Sexp_conv

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

(* [account], [source], [dest] and [amount] in [Deposit] and [Transfer] are
   resolved by [apply]: an index into the accounts that qualify, and a
   fraction of the source's balance. *)
type action =
  | Open_account of
      { deposit : (int [@quickcheck.generator Generator.int_inclusive 0 1_000]) }
  | Deposit of
      { account : (int [@quickcheck.generator Generator.small_positive_or_zero_int])
      ; amount : (int [@quickcheck.generator Generator.int_inclusive 1 1_000])
      }
  | Transfer of
      { source : (int [@quickcheck.generator Generator.small_positive_or_zero_int])
      ; dest : (int [@quickcheck.generator Generator.small_positive_or_zero_int])
      ; amount : (int [@quickcheck.generator Generator.small_positive_or_zero_int])
      } [@quickcheck.weight 6.]
[@@deriving quickcheck, sexp_of]

type state =
  { bank : Bank.t
  ; mutable accounts : int list
  }

let pick l i = List.nth l (i mod List.length l)

let apply ({ bank; accounts } as state) = function
  | Open_account { deposit } ->
    if List.length accounts < 10
    then (
      let id = Bank.open_account bank ~deposit in
      state.accounts <- id :: accounts)
  | Deposit { account; amount } ->
    let id = pick accounts account in
    Bank.deposit bank id ~amount
  | Transfer { source; dest; amount } ->
    let source = pick accounts source in
    (match List.filter (fun id -> id <> source) accounts with
    | [] -> ()
    | others ->
      let dest = pick others dest in
      Bank.transfer bank ~source ~dest ~amount)
;;

let check { bank; accounts } =
  assert (List.for_all (fun id -> Bank.balance bank id >= 0) accounts)
;;

let () =
  Test.run_exn
    (module struct
      type t = action list [@@deriving quickcheck, sexp_of]
    end)
    ~f:(fun actions ->
      let state = { bank = Bank.create (); accounts = [] } in
      apply state (Open_account { deposit = 0 });
      apply state (Open_account { deposit = 0 });
      List.iter
        (fun action -> apply state action; check state)
        actions)
;;
