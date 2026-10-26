(* Differential testing of OCaml compilers on generated, well-typed, total,
   deterministic programs. Every installed opam switch contributes ocamlopt
   (plus -O3 when it has flambda), the current one ocamlc too; a program whose
   output differs between backends, or that only some backends compile, fails.

   Without --ox, OxCaml switches are skipped and each switch also runs
   ocamlopt -linscan, plus -unsafe -inline 1000 without flambda or -Oclassic,
   -O3 -linscan and an aggressive -O3 with it; a crash fails the run even when
   every backend crashes.

   With --ox, the programs also use OxCaml features (unboxed float#, mixed
   records, local and stack allocation, inlining attributes) and are compiled
   only by flambda2 switches, at -Oclassic, the default level, and -O3.

   dune exec fuzz/ocaml_diff.exe -- [--sample] [--ox] [TEST_CASES] *)

open Hegel

let p = Printf.sprintf

type backend =
  { name : string
  ; argv : string list (* compiler and flags; the source and output are appended *)
  }

let read_process cmd =
  let ic = Unix.open_process_args_in cmd.(0) cmd in
  let out = In_channel.input_all ic in
  match Unix.close_process_in ic with
  | WEXITED 0 -> Some (String.trim out)
  | _ -> None
;;

let switch_bin switch =
  read_process [| "opam"; "var"; "--switch"; switch; "bin" |]
  |> Option.map (fun bin -> switch, bin)
;;

(* With [~ox], only flambda2 switches, at each optimisation level. *)
let discover ~ox =
  let switches =
    match read_process [| "opam"; "switch"; "list"; "-s" |] with
    | Some out -> String.split_on_char '\n' out |> List.filter (( <> ) "")
    | None -> []
  in
  (* OCAML_DIFF_SWITCHES="a,b" restricts discovery to those switches. *)
  let switches =
    match Sys.getenv_opt "OCAML_DIFF_SWITCHES" with
    | None -> switches
    | Some only ->
      let only = String.split_on_char ',' only in
      List.filter (fun s -> List.mem s only) switches
  in
  List.filter_map switch_bin switches
  |> List.concat_map (fun (switch, bin) ->
    let ocamlopt = Filename.concat bin "ocamlopt" in
    let ocamlc = Filename.concat bin "ocamlc" in
    if not (Sys.file_exists ocamlopt)
    then []
    else (
      let config var = read_process [| ocamlopt; "-config-var"; var |] in
      let flambda2 = config "flambda2" = Some "true" in
      let flambda = config "flambda" = Some "true" || flambda2 in
      if ox
      then
        if not flambda2
        then []
        else
          { name = p "%s/ocamlc" switch; argv = [ ocamlc ] }
          :: List.map
               (fun level ->
                  { name = p "%s/ocamlopt%s" switch level
                  ; argv = ocamlopt :: (if level = "" then [] else [ level ])
                  })
               [ "-Oclassic"; ""; "-O3" ]
      else if flambda2
      then []
      else
        [ { name = p "%s/ocamlc" switch; argv = [ ocamlc ] }
        ; { name = p "%s/ocamlopt" switch; argv = [ ocamlopt ] }
        ; { name = p "%s/ocamlopt-linscan" switch; argv = [ ocamlopt; "-linscan" ] }
        ]
        @
        if flambda
        then
          [ { name = p "%s/ocamlopt-O3" switch; argv = [ ocamlopt; "-O3" ] }
          ; { name = p "%s/ocamlopt-Oclassic" switch; argv = [ ocamlopt; "-Oclassic" ] }
          ; { name = p "%s/ocamlopt-O3-linscan" switch; argv = [ ocamlopt; "-O3"; "-linscan" ] }
          ; { name = p "%s/ocamlopt-O3-aggressive" switch
            ; argv =
                [ ocamlopt; "-O3"; "-unbox-closures"; "-inline"; "1000"; "-rounds"; "4"
                ; "-inline-max-depth"; "3"; "-remove-unused-arguments"; "-unsafe"
                ]
            }
          ]
        else
          (* programs never rely on bounds checks, so -unsafe keeps their meaning *)
          [ { name = p "%s/ocamlopt-unsafe-inline" switch
            ; argv = [ ocamlopt; "-unsafe"; "-inline"; "1000" ]
            }
          ]))
;;

(* OCAML_DIFF_EXTRA="name=compiler flags..;name2=..." adds backends. *)
let extra () =
  match Sys.getenv_opt "OCAML_DIFF_EXTRA" with
  | None -> []
  | Some spec ->
    String.split_on_char ';' spec
    |> List.filter (( <> ) "")
    |> List.map (fun entry ->
      match String.index_opt entry '=' with
      | None -> failwith ("OCAML_DIFF_EXTRA: expected name=command, got " ^ entry)
      | Some i ->
        { name = String.sub entry 0 i
        ; argv =
            String.sub entry (i + 1) (String.length entry - i - 1)
            |> String.split_on_char ' '
            |> List.filter (( <> ) "")
        })
;;

let clean_env =
  Unix.environment ()
  |> Array.to_list
  |> List.filter (fun kv ->
    not
      (List.exists
         (fun prefix -> String.starts_with ~prefix kv)
         [ "OCAML"; "CAML_LD_LIBRARY_PATH"; "OPAM_SWITCH_PREFIX" ]))
  |> Array.of_list
;;

type exit =
  | Exited of int
  | Signaled of int
  | Timeout

(* Runs [argv] in [dir], returning its exit and its stdout and stderr. *)
let spawn ~dir argv =
  let out = Filename.concat dir (p "out%d" (Random.bits ())) in
  let err = out ^ ".err" in
  let fd path = Unix.openfile path [ O_WRONLY; O_CREAT; O_TRUNC ] 0o644 in
  let fo = fd out
  and fe = fd err in
  let pid =
    Unix.create_process_env (List.hd argv) (Array.of_list argv) clean_env Unix.stdin fo fe
  in
  Unix.close fo;
  Unix.close fe;
  ( pid
  , fun () ->
      ( In_channel.with_open_bin out In_channel.input_all
      , In_channel.with_open_bin err In_channel.input_all ) )
;;

let wait_all ~timeout pids =
  let deadline = Unix.gettimeofday () +. timeout in
  let rec wait pid =
    match Unix.waitpid [ WNOHANG ] pid with
    | 0, _ when Unix.gettimeofday () > deadline ->
      Unix.kill pid Sys.sigkill;
      ignore (Unix.waitpid [] pid);
      Timeout
    | 0, _ ->
      Unix.sleepf 0.002;
      wait pid
    | _, WEXITED c -> Exited c
    | _, (WSIGNALED s | WSTOPPED s) -> Signaled s
  in
  List.map wait pids
;;

let show_exit = function
  | Exited c -> p "exit %d" c
  | Signaled s -> p "signal %d" s
  | Timeout -> "timeout"
;;

let strip ~prefix s =
  let n = String.length prefix in
  let b = Buffer.create (String.length s) in
  let i = ref 0 in
  while !i < String.length s do
    if !i + n <= String.length s && String.sub s !i n = prefix
    then i := !i + n
    else (
      Buffer.add_char b s.[!i];
      incr i)
  done;
  Buffer.contents b
;;

type result =
  | Compile_error of string
  | Ran of exit * string * string

let run_all ?(source_for = Fun.const Fun.id) backends source =
  let root = Filename.temp_dir "ocaml_diff" "" in
  Fun.protect
    ~finally:(fun () ->
      ignore (Sys.command (Filename.quote_command "rm" [ "-rf"; root ])))
    (fun () ->
       let dirs =
         List.mapi
           (fun i b ->
              let dir = Filename.concat root (string_of_int i) in
              Sys.mkdir dir 0o755;
              Out_channel.with_open_bin (Filename.concat dir "prog.ml") (fun oc ->
                output_string oc (source_for b source));
              dir)
           backends
       in
       let exe dir = Filename.concat dir "prog.exe" in
       let compiles =
         List.map2
           (fun b dir ->
              spawn
                ~dir
                (b.argv @ [ "-w"; "-a"; Filename.concat dir "prog.ml"; "-o"; exe dir ]))
           backends
           dirs
       in
       let compiled = wait_all ~timeout:120. (List.map fst compiles) in
       let runs =
         List.map2
           (fun dir status ->
              match status with
              | Exited 0 -> Some (spawn ~dir [ exe dir ])
              | _ -> None)
           dirs
           compiled
       in
       let ran = wait_all ~timeout:10. (List.filter_map (Option.map fst) runs) in
       let rec combine compiles compiled runs ran =
         match compiles, compiled, runs, ran with
         | [], [], [], _ -> []
         | (_, cout) :: compiles, status :: compiled, None :: runs, ran ->
           let o, e = cout () in
           Compile_error (p "%s\n%s%s" (show_exit status) o e)
           :: combine compiles compiled runs ran
         | _ :: compiles, _ :: compiled, Some (_, rout) :: runs, status :: ran ->
           let o, e = rout () in
           Ran (status, o, e) :: combine compiles compiled runs ran
         | _ -> assert false
       in
       combine compiles compiled runs ran
       |> List.map2
            (fun dir -> function
               | Compile_error msg -> Compile_error (strip ~prefix:(dir ^ "/") msg)
               | r -> r)
            dirs)
;;

let observable = function
  | Compile_error _ -> `Compile_error
  | Ran (status, out, _) -> `Ran (status, out)
;;

(* Backends with identical results are listed together. *)
let report backends results =
  let body = function
    | Compile_error msg -> p "compile error\n%s" msg
    | Ran (status, out, err) -> p "%s\n%s%s" (show_exit status) out err
  in
  List.fold_left2
    (fun groups b r ->
       let text = body r in
       if List.mem_assoc text groups
       then
         List.map
           (fun (t, names) -> t, if t = text then names @ [ b.name ] else names)
           groups
       else groups @ [ text, [ b.name ] ])
    []
    backends
    results
  |> List.map (fun (text, names) -> p "--- %s: %s" (String.concat ", " names) text)
  |> String.concat "\n"
;;

let cases = ref 0
let timeouts = ref 0
let known = ref 0

let contains s sub =
  let n = String.length sub in
  let rec go i = i + n <= String.length s && (String.sub s i n = sub || go (i + 1)) in
  go 0
;;

(* Reported bugs, recognised by which backends disagree with the most common
   result, so they don't stop a run looking for new ones. *)
let atomic_shim =
  {|module Atomic = struct
  include Atomic
  let[@inline never] exchange a v = let old = get a in set a v; old
  let[@inline never] compare_and_set a o n = if get a == o then (set a n; true) else false
  let[@inline never] fetch_and_add a n = let old = get a in set a (old + n); old
  let[@inline never] incr a = set a (get a + 1)
  let[@inline never] decr a = set a (get a - 1)
end
|}
;;

let known_bug backends results source =
  let obs = List.map observable results in
  let count o = List.length (List.filter (( = ) o) obs) in
  let majority =
    List.fold_left
      (fun best o -> if count o > count best then o else best)
      (List.hd obs)
      obs
  in
  let odd =
    List.filter_map
      (fun (b, o) -> if o = majority then None else Some b.name)
      (List.combine backends obs)
  in
  (* ocaml/ocaml#15154: ocamlopt without flambda cannot size an allocation
     field that is a mutable let, such as an unused ref. *)
  let size_expr =
    odd <> []
    && List.for_all
         (fun r ->
            observable r = majority
            ||
            match r with
            | Compile_error msg -> contains msg "Selection.size_expr"
            | Ran _ -> false)
         results
  in
  size_expr
  ||
  (* Unreported: after a nested perform, native code leaves a shallow fiber's
     effect handler pointing into the minor heap, so the GC later crashes on
     it. It is that bug only if every crashing backend, rebuilt with the debug
     runtime, fails the runtime's check for it. *)
  (odd <> []
   && List.for_all
        (fun (b, r) ->
           (not (List.mem b.name odd))
           ||
           match r with
           | Ran (Signaled s, _, _) -> s = Sys.sigbus || s = Sys.sigsegv
           | _ -> false)
        (List.combine backends results)
   && List.for_all
        (function
          | Ran (_, _, err) -> contains err "Assertion failed: !Is_young(v)"
          | Compile_error _ -> false)
        (run_all
           (List.filter_map
              (fun b ->
                 if List.mem b.name odd
                 then Some { b with argv = b.argv @ [ "-runtime-variant"; "d" ] }
                 else None)
              backends)
           source))
  ||
  (* ocaml/ocaml#15189: trunk -linscan ignores the interference that keeps
     the native Atomic exchange, compare-and-set and fetch-and-add results out
     of their arguments' registers. It is that bug only if the odd backends
     agree once those are ordinary calls. *)
  odd <> []
  && contains source "Atomic."
  && List.for_all
       (fun n -> contains n "trunk" && String.ends_with ~suffix:"-linscan" n)
       odd
  &&
  let odd_backends = List.filter (fun b -> List.mem b.name odd) backends in
  List.for_all
    (fun r -> observable r = majority)
    (run_all odd_backends (atomic_shim ^ source))
;;

(* Every backend's compiler is OCaml 5.3 or later, so it has effect syntax. *)
let effect_syntax ~ox backends =
  (not ox)
  && List.for_all
       (fun b ->
          match read_process [| List.hd b.argv; "-version" |] with
          | Some v ->
            (match String.split_on_char '.' (String.trim v) with
             | major :: minor :: _ ->
               (int_of_string major, int_of_string minor) >= (5, 3)
             | _ -> false)
          | None -> false)
       backends
;;

(* After the first failure, shrinking only runs the reference and one backend
   that differs from it, since each attempt otherwise compiles on all of them. *)
let focus = ref None

let fail backends results msg =
  let text = msg ^ report backends results in
  if !focus = None
  then (
    prerr_endline text;
    let ref_obs = observable (List.hd results) in
    let odd =
      List.filteri (fun i _ -> i > 0 && observable (List.nth results i) <> ref_obs) backends
    in
    focus := Some (List.hd backends :: List.filteri (fun i _ -> i = 0) odd));
  failwith text
;;

let check ~ox ~effect_syntax backends tc =
  let backends = Option.value !focus ~default:backends in
  incr cases;
  let source = Ocaml_diff_gen.program ~ox ~effect_syntax tc in
  note tc source;
  (* -linscan miscompiles native atomics on trunk (see [known_bug]), so it
     gets them as ordinary calls. *)
  let source_for b source =
    if String.ends_with ~suffix:"linscan" b.name && contains source "Atomic."
    then atomic_shim ^ source
    else source
  in
  let results = run_all ~source_for backends source in
  if
    List.for_all
      (function
        | Compile_error _ -> true
        | Ran _ -> false)
      results
  then (
    let crashed =
      List.exists
        (function
          | Compile_error msg -> contains msg "Fatal error"
          | Ran _ -> false)
        results
    in
    failwith
      ((if crashed
        then "compiler crash: no backend compiled the program\n"
        else "generator bug: no backend compiled the program\n")
       ^ report backends results));
  if
    List.exists
      (function
        | Ran (Timeout, _, _) -> true
        | _ -> false)
      results
  then incr timeouts;
  assume
    tc
    (not
       (List.exists
          (function
            | Ran (Timeout, _, _) -> true
            | _ -> false)
          results));
  if known_bug backends results source
  then (
    incr known;
    (* OCAML_DIFF_SAVE=dir keeps these programs, to audit the filter. *)
    Option.iter
      (fun dir ->
         Out_channel.with_open_bin
           (Filename.concat dir (p "known%d-%d.ml" (Unix.getpid ()) !known))
           (fun oc -> output_string oc source))
      (Sys.getenv_opt "OCAML_DIFF_SAVE");
    assume tc false);
  (* The programs never fail except by an escaping Exit, so a crash or any
     other uncaught exception is a bug even where every backend agrees. *)
  if
    List.exists
      (function
        | Ran (Signaled _, _, _) -> true
        | Ran (Exited 2, _, err) ->
          not (String.starts_with ~prefix:"Fatal error: exception Stdlib.Exit" err)
        | _ -> false)
      results
  then fail backends results "backend crashed\n";
  match List.map observable results with
  | first :: rest when List.for_all (( = ) first) rest -> ()
  | _ -> fail backends results "backends disagree\n"
;;

let () =
  let args = List.tl (Array.to_list Sys.argv) in
  let sample = List.mem "--sample" args in
  let ox = List.mem "--ox" args in
  let test_cases =
    match List.filter (fun a -> a <> "--sample" && a <> "--ox") args with
    | [ n ] -> int_of_string n
    | _ -> if sample then 5 else 200
  in
  let settings =
    { (Settings.create ~test_cases ()) with
      suppress_health_check = [ Too_slow; Test_cases_too_large; Large_initial_test_case ]
    ; database = Disabled
    }
  in
  if sample
  then
    run_hegel_test ~settings (fun tc ->
      print_endline (Ocaml_diff_gen.program ~ox ~effect_syntax:(not ox) tc);
      print_endline "(* ---- *)")
  else (
    let backends = discover ~ox @ extra () in
    List.iter
      (fun b -> Printf.printf "backend %s: %s\n%!" b.name (String.concat " " b.argv))
      backends;
    if List.length backends < 2 then failwith "need at least two backends";
    at_exit (fun () ->
      Printf.printf
        "ran %d programs, %d timed out, %d hit known bugs\n%!"
        !cases
        !timeouts
        !known);
    let effect_syntax = effect_syntax ~ox backends in
    Printf.printf "effect syntax: %b\n%!" effect_syntax;
    run_hegel_test ~settings (check ~ox ~effect_syntax backends))
;;
