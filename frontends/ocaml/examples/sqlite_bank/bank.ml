(** Bank workload for testing an ACID database with Hegel's concurrent stateful
    testing.

    [main.ml] is a [let%hegel_main] binary whose every invocation is one run of
    the workload.

    The state is a handle to a file-backed SQLite database in WAL mode with one
    [accounts(account_id, balance, opCount)] table and the client-side
    bookkeeping of the workload (attempted, committed, total) and a pool of
    idle connections that rules use for each transaction.

    WAL mode lets the verification rules read while transfers are in flight
    without blocking them or failing with SQLITE_BUSY, and each read sees a
    consistent snapshot. Writers are still serialized, so transfers take the
    write lock up front with [BEGIN IMMEDIATE]. *)

open Hegel

(** How long a connection waits in milliseconds on a locked database before failing
    with [SQLITE_BUSY]. Such a failure counts as "attempted but not known to have
    committed". *)
let busy_timeout_ms = 5_000

(** Set [BANK_INJECT_BUG] to skip the balance check in the transfer
    transaction, so the run fails on the "no negative balance" check. *)
let inject_bug = Option.is_some (Sys.getenv_opt "BANK_INJECT_BUG")

type config =
  { accounts : int (** [N], the number of accounts. *)
  ; initial_balance : int (** [B], the initial balance of every account. *)
  ; max_amount : int
    (** The largest amount a transfer may move. Larger than [B] so that the
        insufficient-funds path is exercised. *)
  }

type workload =
  { bank : config
  ; steps : int
  ; min_concurrency : int
  ; max_concurrency : int
  }

let env_int name ~min =
  match Option.map String.trim (Sys.getenv_opt name) with
  | None | Some "" -> None
  | Some raw ->
    (match int_of_string_opt raw with
     | None -> Printf.ksprintf failwith "%s=%S is not an integer" name raw
     | Some value when value < min ->
       Printf.ksprintf failwith "%s=%d must be at least %d" name value min
     | v -> v)
;;

let env_int_or name ~min ~default =
  match env_int name ~min with
  | Some value -> value
  | None -> default ()
;;

(** Resolve the workload from the environment and draw values for unset variables.

    {v
    BANK_ACCOUNTS         N, number of accounts (>= 1)          drawn, 1..16
    BANK_INITIAL_BALANCE  B, starting balance per account (>= 0) drawn, 0..1000
    BANK_MAX_AMOUNT       largest transfer amount (>= 0)        drawn, 0..2B+10
    BANK_STEPS            rounds per test case (>= 1)           200
    BANK_MIN_CONCURRENCY  fewest worker threads (>= 1)          2
    BANK_MAX_CONCURRENCY  most worker threads (>= min)          8
    v} *)
let workload_from_env_or_draw tc =
  let accounts =
    env_int_or "BANK_ACCOUNTS" ~min:1 ~default:(fun () ->
      draw tc (integers ~min_value:1 ~max_value:16 ()))
  in
  let initial_balance =
    env_int_or "BANK_INITIAL_BALANCE" ~min:0 ~default:(fun () ->
      draw tc (integers ~min_value:0 ~max_value:1_000 ()))
  in
  let max_amount =
    env_int_or "BANK_MAX_AMOUNT" ~min:0 ~default:(fun () ->
      draw tc (integers ~min_value:0 ~max_value:((2 * initial_balance) + 10) ()))
  in
  let min_concurrency = Option.value (env_int "BANK_MIN_CONCURRENCY" ~min:1) ~default:2 in
  let max_concurrency =
    Option.value
      (env_int "BANK_MAX_CONCURRENCY" ~min:min_concurrency)
      ~default:(max 8 min_concurrency)
  in
  { bank = { accounts; initial_balance; max_amount }
  ; steps = Option.value (env_int "BANK_STEPS" ~min:1) ~default:200
  ; min_concurrency
  ; max_concurrency
  }
;;

(** What the client observed for a transfer attempt. *)
type transfer_outcome =
  | Committed (** [COMMIT] returned successfully. *)
  | Insufficient_funds (** Rolled back because the source balance was insufficient. *)
  | Unknown
  (** SQLite returned an error, such as [SQLITE_BUSY]. Whether the transaction
      committed is unknown. *)

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
    (** Idle connections. Rules run concurrently on worker threads, so each
        rule checks one out for the duration of a transaction. *)
  ; config : config
  ; attempted : int Atomic.t
  ; committed : int Atomic.t
  ; total : int
  }

(** Runs [sql] with [params] to its first row, returning that row's first
    column, or [None] if there is no row. *)
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

(** Create the [accounts] table with [N] rows of balance [B] and [opCount] 0. *)
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

(** The transfer transaction. Returns what the client observed. *)
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
      if (not inject_bug) && balance < amount
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

(** No money is created or destroyed. *)
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

(** No committed transfer is lost. [committed] is read before the snapshot, so
    every transfer it counts must be visible in it. [attempted] is read after,
    so every transfer visible in the snapshot is counted by it. *)
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

(** No account balance is below zero. *)
let verify_no_negative t =
  let negative =
    with_conn t (fun db ->
      select_int db "SELECT COUNT(*) FROM accounts WHERE balance < 0" ())
  in
  if negative <> 0
  then Printf.ksprintf failwith "%d account(s) have a negative balance" negative
;;

(** All rules share the ["bank"] group, so any of them may overlap with any
    other. Each verify_* is both a rule and an invariant on purpose. As rules
    they run while transfers are in flight, testing what a reader sees
    mid-write. As invariants they are checked on the final state and sampled
    between rounds, so a lost commit is always caught by the end of the run.
    [no_committed_transfer_lost] is checked after every round. *)
module%hegel_concurrent_state_machine Machine = struct
  (* Before the rules, which shadow the functions these call. *)
  let total_balance_preserved _tc bank = verify_total_balance bank [@@invariant]

  let no_committed_transfer_lost _tc bank = verify_ops_not_lost bank
  [@@invariant { always_check = true }]
  ;;

  let no_negative_balance _tc bank = verify_no_negative bank [@@invariant]

  let transfer tc () bank =
    let n = bank.config.accounts in
    let source = draw tc (integers ~min_value:0 ~max_value:(n - 1) ()) in
    let dest = draw tc (integers ~min_value:0 ~max_value:(n - 1) ()) in
    let amount = draw tc (integers ~min_value:0 ~max_value:bank.config.max_amount ()) in
    let outcome = string_of_outcome (transfer bank ~source ~dest ~amount) in
    note tc (Printf.sprintf "transfer %d from %d to %d: %s" amount source dest outcome)
  [@@rule { groups = [ "bank" ]; weight = 6 }]
  ;;

  let verify_total_balance _tc () bank = verify_total_balance bank
  [@@rule { groups = [ "bank" ] }]
  ;;

  let verify_ops_not_lost _tc () bank = verify_ops_not_lost bank
  [@@rule { groups = [ "bank" ] }]
  ;;

  let verify_no_negative _tc () bank = verify_no_negative bank
  [@@rule { groups = [ "bank" ] }]
  ;;
end

let run tc workload =
  note
    tc
    (Printf.sprintf
       "workload: accounts = %d, initial_balance = %d, max_amount = %d, steps = %d, \
        concurrency = %d..%d"
       workload.bank.accounts
       workload.bank.initial_balance
       workload.bank.max_amount
       workload.steps
       workload.min_concurrency
       workload.max_concurrency);
  with_bank workload.bank (fun bank ->
    Machine.run
      tc
      ~init:bank
      ~step_count:workload.steps
      ~min_concurrency:workload.min_concurrency
      ~max_concurrency:workload.max_concurrency)
;;
