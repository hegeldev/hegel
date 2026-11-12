---
title: Using Hegel for OCaml for Antithesis workloads
sidebar:
  label: OCaml
---

```ocaml
open Hegel

let busy_timeout_ms = 5_000

type config =
  { accounts : int
  ; initial_balance : int
  ; max_amount : int
  }

type transfer_outcome =
  | Committed
  | Insufficient_funds
  | Unknown

let string_of_outcome = function
  | Committed -> "Committed"
  | Insufficient_funds -> "Insufficient_funds"
  | Unknown -> "Unknown"
;;

type t =
  { dir : string
  ; path : string
  ; mutex : Mutex.t
  ; mutable idle : Sqlite3.db list
  ; config : config
  ; attempted : int Atomic.t
  ; committed : int Atomic.t
  ; total : int
  }
```

```ocaml
let step db sql ?(params = []) () =
  let stmt = Sqlite3.prepare db sql in
  Fun.protect
    ~finally:(fun () -> ignore (Sqlite3.finalize stmt))
    (fun () ->
       Sqlite3.Rc.check (Sqlite3.bind_values stmt params);
       match Sqlite3.step stmt with
       | Sqlite3.Rc.ROW -> Some (Sqlite3.column_int stmt 0)
       | rc ->
         Sqlite3.Rc.check rc;
         None)
;;

let int n = Sqlite3.Data.INT (Int64.of_int n)
let select_int db sql ?params () = Option.get (step db sql ?params ())
let execute db sql ?params () = ignore (step db sql ?params ())

let open_db path =
  let db = Sqlite3.db_open ~mutex:`NO path in
  Sqlite3.busy_timeout db busy_timeout_ms;
  db
;;

let with_db path f =
  let db = open_db path in
  Fun.protect ~finally:(fun () -> ignore (Sqlite3.db_close db)) (fun () -> f db)
;;
```

```ocaml
let init config =
  let dir = Filename.temp_dir "sqlite_bank" "" in
  let path = Filename.concat dir "bank.sqlite3" in
  with_db path (fun db ->
    execute db "PRAGMA journal_mode = WAL" ();
    execute db "PRAGMA synchronous = FULL" ();
    execute
      db
      "CREATE TABLE accounts (account_id INTEGER PRIMARY KEY, balance INTEGER NOT NULL, \
       opCount INTEGER NOT NULL)"
      ();
    execute db "BEGIN" ();
    for id = 0 to config.accounts - 1 do
      execute
        db
        "INSERT INTO accounts (account_id, balance, opCount) VALUES (?, ?, 0)"
        ~params:[ int id; int config.initial_balance ]
        ()
    done;
    execute db "COMMIT" ());
  { dir
  ; path
  ; mutex = Mutex.create ()
  ; idle = []
  ; config
  ; attempted = Atomic.make 0
  ; committed = Atomic.make 0
  ; total = config.accounts * config.initial_balance
  }
;;

let close t =
  List.iter (fun db -> ignore (Sqlite3.db_close db)) t.idle;
  t.idle <- [];
  Array.iter (fun file -> Sys.remove (Filename.concat t.dir file)) (Sys.readdir t.dir);
  Sys.rmdir t.dir
;;

let with_bank config f =
  let bank = init config in
  Fun.protect ~finally:(fun () -> close bank) (fun () -> f bank)
;;

let with_conn t f =
  let pooled =
    Mutex.protect t.mutex (fun () ->
      match t.idle with
      | [] -> None
      | db :: rest ->
        t.idle <- rest;
        Some db)
  in
  let db =
    match pooled with
    | Some db -> db
    | None -> open_db t.path
  in
  Fun.protect
    ~finally:(fun () -> Mutex.protect t.mutex (fun () -> t.idle <- db :: t.idle))
    (fun () -> f db)
;;
```

```ocaml
let transfer t ~source ~dest ~amount =
  Atomic.incr t.attempted;
  let transaction db =
    execute db "BEGIN IMMEDIATE" ();
    try
      let balance =
        select_int
          db
          "SELECT balance FROM accounts WHERE account_id = ?"
          ~params:[ int source ]
          ()
      in
      if balance < amount
      then (
        execute db "ROLLBACK" ();
        Insufficient_funds)
      else (
        execute
          db
          "UPDATE accounts SET balance = balance - ?, opCount = opCount + 1 WHERE \
           account_id = ?"
          ~params:[ int amount; int source ]
          ();
        execute
          db
          "UPDATE accounts SET balance = balance + ?, opCount = opCount + 1 WHERE \
           account_id = ?"
          ~params:[ int amount; int dest ]
          ();
        execute db "COMMIT" ();
        Committed)
    with
    | exn ->
      let backtrace = Printexc.get_raw_backtrace () in
      ignore (Sqlite3.exec db "ROLLBACK");
      Printexc.raise_with_backtrace exn backtrace
  in
  match with_conn t transaction with
  | outcome ->
    if outcome = Committed then Atomic.incr t.committed;
    outcome
  | exception (Sqlite3.Error _ | Sqlite3.SqliteError _) -> Unknown
;;
```

```ocaml
let verify_total_balance t =
  let sum =
    with_conn t (fun db -> select_int db "SELECT SUM(balance) FROM accounts" ())
  in
  if sum <> t.total
  then
    Printf.ksprintf
      failwith
      "money was created or destroyed: SUM(balance) = %d, expected %d"
      sum
      t.total
;;

let verify_ops_not_lost t =
  let committed = Atomic.get t.committed in
  let sum =
    with_conn t (fun db -> select_int db "SELECT SUM(opCount) FROM accounts" ())
  in
  let attempted = Atomic.get t.attempted in
  if sum mod 2 <> 0
  then
    Printf.ksprintf failwith "SUM(opCount) = %d is odd: a transfer was half-applied" sum;
  if sum < 2 * committed
  then
    Printf.ksprintf
      failwith
      "committed transfers were lost: SUM(opCount) = %d < 2 * %d committed"
      sum
      committed;
  if sum > 2 * attempted
  then
    Printf.ksprintf
      failwith
      "phantom transfers: SUM(opCount) = %d > 2 * %d attempted"
      sum
      attempted
;;

let verify_no_negative t =
  let negative =
    with_conn t (fun db ->
      select_int db "SELECT COUNT(*) FROM accounts WHERE balance < 0" ())
  in
  if negative <> 0
  then Printf.ksprintf failwith "%d account(s) have a negative balance" negative
;;
```

```ocaml
module%hegel_concurrent_state_machine Machine = struct
  let total_balance_preserved _tc bank = verify_total_balance bank [@@invariant]
  let no_committed_transfer_lost _tc bank = verify_ops_not_lost bank [@@invariant]
  let no_negative_balance _tc bank = verify_no_negative bank [@@invariant]

  let transfer tc () bank =
    let n = bank.config.accounts in
    let source = draw tc (integers ~min_value:0 ~max_value:(n - 1) ()) in
    let dest = draw tc (integers ~min_value:0 ~max_value:(n - 1) ()) in
    let amount = draw tc (integers ~min_value:0 ~max_value:bank.config.max_amount ()) in
    let outcome = string_of_outcome (transfer bank ~source ~dest ~amount) in
    note tc (Printf.sprintf "transfer %d from %d to %d: %s" amount source dest outcome)
  [@@rule { group = "bank"; weight = 6 }]
  ;;

  let verify_total_balance _tc () bank = verify_total_balance bank
  [@@rule { group = "bank" }]
  ;;

  let verify_ops_not_lost _tc () bank = verify_ops_not_lost bank
  [@@rule { group = "bank" }]
  ;;

  let verify_no_negative _tc () bank = verify_no_negative bank [@@rule { group = "bank" }]
end
```



```ocaml
let%hegel_main workload tc =
  with_bank workload.bank (fun bank ->
    Machine.run
      tc
      ~init:{ accounts = 16; initial_balance = 1_000; max_amount = 10_000 }
      ~step_count:Int.max_int
      ~min_concurrency:2
      ~max_concurrency:8)
;;

let () = workload ()
```

We can now use `let%hegel_main` to turn the workload into an standalone program, which allows us to pass test settings at command line options,
such as `dune exec workload.exe -- --seed 1 --verbosity verbose`.

`init` represents the initial state of the stateful test, but since all of the state is stored in the database, we use it to store the bank
configuration instead. `step_count` sets how many rounds execute within a test. Inside of Antithesis, this number should be very high to allow
the fuzzer ample time to explore a branch. Finally, `min_concurrency` and `max_concurrency` set the bounds for the number of concurrent workers,
which can be threads, domains, or any other unit of concurrency. Hegel spawns some number of workers within those bounds.

For the full runnable workload, see [the GitHub repository](https://github.com/hegeldev/hegel/tree/main/frontends/ocaml/examples/sqlite_bank).

For more information on concurrent stateful testing, see the [documentation](/ocaml/hegel/Hegel/Stateful/index.html#concurrent-stateful-testing).
