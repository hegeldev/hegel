(* Differential testing of OCaml compilers on generated, well-typed, total,
   deterministic programs. Every installed opam switch contributes ocamlopt
   (plus -O3 when it has flambda), the current one ocamlc too; a program whose
   output differs between backends, or that only some backends compile, fails.

   Without --ox, OxCaml switches are skipped and each switch also runs
   ocamlopt -linscan, plus -unsafe -inline 1000 without flambda or -Oclassic,
   -O3 -linscan and an aggressive -O3 with it; a crash fails the run even when
   every backend crashes.

   With --ox, the programs also use OxCaml features (unboxed numbers, tuples
   and records, or_null, arrays, let mutable, local and stack allocation) and
   are compiled only by flambda2 switches, at -Oclassic, the default level,
   and -O3, plus a toggles backend.

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
      (* the compiler's own consistency checks, so a broken invariant fails
         the compile instead of hiding in the output *)
      (* OxCaml programs use the unboxed number modules of these libraries *)
      let libs ext =
        if ox
        then
          [ "-I"; "+stdlib_stable"; "-I"; "+stdlib_upstream_compatible"; "stdlib_stable" ^ ext
          ; "stdlib_upstream_compatible" ^ ext
          ]
        else []
      in
      let nat flags =
        (ocamlopt :: "-dcmm-invariants" :: (if flambda then [ "-dflambda-invariants" ] else []))
        @ libs ".cmxa"
        @ flags
      in
      (* the reference: bytecode on the debug runtime *)
      let byte =
        { name = p "%s/ocamlc" switch; argv = (ocamlc :: libs ".cma") @ [ "-runtime-variant"; "d" ] }
      in
      if ox
      then
        if not flambda2
        then []
        else
          byte
          :: List.map
               (fun level ->
                  { name = p "%s/ocamlopt%s" switch level
                  ; argv = nat (if level = "" then [] else [ level ])
                  })
               [ "-Oclassic"; ""; "-O3" ]
          @ [ { name = p "%s/ocamlopt-toggles" switch; argv = nat [] } ]
      else if flambda2
      then []
      else
        [ byte
        ; { name = p "%s/ocamlopt" switch; argv = nat [] }
        ; { name = p "%s/ocamlopt-linscan" switch; argv = nat [ "-linscan" ] }
        ]
        @
        if flambda
        then
          [ { name = p "%s/ocamlopt-O3" switch; argv = nat [ "-O3" ] }
          ; { name = p "%s/ocamlopt-Oclassic" switch; argv = nat [ "-Oclassic" ] }
          ; { name = p "%s/ocamlopt-O3-linscan" switch; argv = nat [ "-O3"; "-linscan" ] }
          ; { name = p "%s/ocamlopt-O3-aggressive" switch
            ; argv =
                nat
                  [ "-O3"; "-unbox-closures"; "-inline"; "1000"; "-rounds"; "4"
                  ; "-inline-max-depth"; "3"; "-remove-unused-arguments"; "-unsafe"
                  ; "-runtime-variant"; "d"
                  ]
            }
          ]
        else
          (* programs never rely on bounds checks, so -unsafe keeps their meaning *)
          [ { name = p "%s/ocamlopt-unsafe-inline" switch
            ; argv = nat [ "-unsafe"; "-inline"; "1000"; "-runtime-variant"; "d" ]
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

let find s sub =
  let n = String.length sub in
  let rec go i =
    if i + n > String.length s then None else if String.sub s i n = sub then Some i else go (i + 1)
  in
  go 0
;;

let contains s sub = find s sub <> None

(* A program with the generator's boundary line becomes two modules, the
   second opening the first, so code is inlined across modules. *)
let modules source =
  let boundary = Ocaml_diff_gen.module_boundary in
  match find source boundary with
  | None -> [ "prog.ml", source ]
  | Some i ->
    let j = i + String.length boundary in
    [ "m1.ml", String.sub source 0 i
    ; "prog.ml", "open M1\n" ^ String.sub source j (String.length source - j)
    ]
;;

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
              let files = modules (source_for b source) in
              List.iter
                (fun (f, text) ->
                   Out_channel.with_open_bin (Filename.concat dir f) (fun oc ->
                     output_string oc text))
                files;
              dir, List.map (fun (f, _) -> Filename.concat dir f) files)
           backends
       in
       let dirs, files = List.split dirs in
       let exe dir = Filename.concat dir "prog.exe" in
       (* macOS scans each new executable on its first launch, which takes up
          to 2s; bytecode skips that by starting the already-seen runtime. *)
       let run b dir =
         let c = List.hd b.argv in
         if String.starts_with ~prefix:"ocamlc" (Filename.basename c)
         then (
           let d = List.mem "d" b.argv in
           [ Filename.concat (Filename.dirname c) (if d then "ocamlrund" else "ocamlrun"); exe dir ])
         else [ exe dir ]
       in
       let compiles =
         List.map2
           (fun b (dir, files) ->
              spawn ~dir (b.argv @ [ "-w"; "-a"; "-I"; dir ] @ files @ [ "-o"; exe dir ]))
           backends
           (List.combine dirs files)
       in
       let compiled = wait_all ~timeout:120. (List.map fst compiles) in
       let runs =
         List.map2
           (fun (b, dir) status ->
              match status with
              | Exited 0 -> Some (spawn ~dir (run b dir))
              | _ -> None)
           (List.combine backends dirs)
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

(* Reported bugs, recognised by which backends disagree with the reference
   (the first backend), so they don't stop a run looking for new ones. *)
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

(* ocaml/ocaml#15154: ocamlopt without flambda cannot size an allocation
   field that is a mutable let, such as an unused ref.
   ocaml/ocaml#15171: releases before trunk crash on a redundant or-pattern
   with a guard after a catch-all; 5.2 (and so OxCaml) reports it as
   Matching.comp_exit.
   Unreported: OxCaml -Oclassic keeps a dead trap handler around a call that
   cannot raise, and linscan reads a ref there from a register nothing set.
   oxcaml/oxcaml#7668, #7705: -Oclassic, a raise inside a try (Array.iter2 in
   Cfg_selectgen). #7704: -flambda2-expert-inline-effects-in-cmm.
   #7683: -Oclassic, a float# read in an unreachable match arm.
   #7701, #7702: -use-ssa's validator rejects the SSA pipeline's CFG.
   Unreported: OxCaml on arm64 emits fmov with a float32# constant it cannot
   encode, (1 + m/16) * 2^k for k in 61..68, and the assembler rejects it. *)
let known_compile_crash msg =
  contains msg "Selection.size_expr"
  || contains msg "Matching: impossible empty matrix in a Total match"
  || contains msg "Matching.comp_exit"
  || contains msg "While verifying locations destroyed at raise"
  || contains msg "Array.iter2: arrays must have the same length"
  || contains msg "Duplicate order for bindings when flushing"
  || contains msg "Illegal move between registers of differing types"
  || contains msg "CFG comparison MISMATCH"
  || (contains msg "expected compatible register or floating-point constant" && contains msg "fmov");;

let known_bug backends results source =
  let obs = List.map observable results in
  let reference = List.hd obs in
  let odd =
    List.filter_map
      (fun (b, o) -> if o = reference then None else Some b.name)
      (List.combine backends obs)
  in
  let compile_crash =
    odd <> []
    && List.for_all
         (fun r ->
            observable r = reference
            ||
            match r with
            | Compile_error msg -> known_compile_crash msg
            (* oxcaml/oxcaml#7684: -flambda2-reaper with phantom lets aborts at run time. *)
            | Ran (_, _, err) -> contains err "Dead variable in bound pattern")
         results
  in
  compile_crash
  ||
  (* ocaml/ocaml#15196: after a nested perform, native code leaves a shallow fiber's
     effect handler pointing into the minor heap, so the GC later crashes on
     it. It is that bug only if every crashing backend, rebuilt with the debug
     runtime, fails the runtime's check for it. *)
  (odd <> []
   && List.for_all
        (fun (b, r) ->
           (not (List.mem b.name odd))
           ||
           match r with
           | Ran (Signaled s, _, _) -> List.mem s Sys.[ sigbus; sigsegv; sigabrt; sigtrap ]
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
  (odd <> []
   && contains source "Atomic."
   && List.for_all
        (fun n -> contains n "trunk" && String.ends_with ~suffix:"-linscan" n)
        odd
   &&
   let odd_backends = List.filter (fun b -> List.mem b.name odd) backends in
   List.for_all
     (fun r -> observable r = reference)
     (run_all odd_backends (atomic_shim ^ source)))
  ||
  (* ocaml/ocaml#15202: flambda's -Oclassic reads a constant field of a lifted
     block as the block itself. It is that bug only if the odd backends agree
     when compiled by OCAML_DIFF_FIXED_OCLASSIC, an ocamlopt with the fix. *)
  match Sys.getenv_opt "OCAML_DIFF_FIXED_OCLASSIC" with
  | None -> false
  | Some fixed ->
    odd <> []
    && List.for_all (fun n -> contains n "flambda/" && String.ends_with ~suffix:"-Oclassic" n) odd
    &&
    let odd_backends =
      List.filter_map
        (fun b -> if List.mem b.name odd then Some { b with argv = fixed :: List.tl b.argv } else None)
        backends
    in
    List.for_all (fun r -> observable r = reference) (run_all odd_backends source)
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
      List.find_opt (fun i -> observable (List.nth results i) <> ref_obs)
        (List.init (List.length results - 1) succ)
    in
    focus := Some (0 :: Option.to_list odd));
  failwith text
;;

(* Switches that must not change a program's meaning, most of them off by
   default; the toggles backend gets a few of them per program. *)
let ox_toggles =
  [ "-cfg-merge-blocks"; "-reorder-blocks-random 7"; "-vectorize"; "-g"
  ; "-g -gdwarf-may-alter-codegen"; "-dcfg-invariants"; "-debug-long-frames-threshold 0"
  ; "-function-layout source"; "-no-cfg-prologue-shrink-wrap"
  ; "-cfg-prologue-shrink-wrap-threshold 1"; "-no-cfg-value-propagation"
  ; "-no-cfg-peephole-optimize"; "-no-cfg-eliminate-dead-trap-handlers"
  ; "-cfg-stack-checks-threshold 0"; "-caml-apply-inline-fast-path"
  ; "-flambda2-advanced-meet"; "-no-flambda2-join-points"
  ; "-no-flambda2-unbox-along-intra-function-control-flow"
  ; "-flambda2-expert-max-unboxing-depth 0"; "-flambda2-expert-cmm-safe-subst"
  ; "-flambda2-expert-phantom-lets"; "-flambda2-backend-cse-at-toplevel"
  ; "-flambda2-inline-max-depth 0"
  ; "-flambda2-inline-threshold 1000 -flambda2-inline-max-depth 6"
  ; "-flambda2-cse-depth 0"; "-flambda2-join-depth 0"; "-regalloc irc"; "-regalloc ls"
  ; "-regalloc gi"; "-regalloc-linscan-threshold 0"; "-runtime-variant d"
  ; "-flambda2-reaper"; "-flambda2-expert-inline-effects-in-cmm"; "-use-ssa"
    (* oxfuzzer's aggressive configuration *)
  ; "-flambda2-result-types-all-functions"; "-flambda2-expert-can-inline-recursive-functions"
  ; "-flambda2-match-in-match"; "-flambda2-join-algorithm n-way"; "-experimental-optimizations"
  ]
;;

(* Newer switches add flags, so each compiler only gets the ones its -help lists. *)
let help = Hashtbl.create 4

let supports compiler toggle =
  let text =
    match Hashtbl.find_opt help compiler with
    | Some t -> t
    | None ->
      let t = Option.value ~default:"" (read_process [| compiler; "-help" |]) in
      Hashtbl.add help compiler t;
      t
  in
  contains text ("  " ^ List.hd (String.split_on_char ' ' toggle) ^ " ")
;;

let with_toggles tc backends =
  let level =
    List.nth [ "-O3"; ""; "-O2"; "-Oclassic"; "-O4" ] (draw_silent tc (integers ~min_value:0 ~max_value:4 ()))
  in
  let picked = List.filter (fun _ -> draw_silent tc (booleans ~p:0.08 ())) ox_toggles in
  List.map
    (fun b ->
       if String.ends_with ~suffix:"-toggles" b.name
       then (
         let picked = List.filter (supports (List.hd b.argv)) picked in
         let flags =
           List.concat_map (String.split_on_char ' ') (level :: picked) |> List.filter (( <> ) "")
         in
         let reaper = List.mem "-flambda2-reaper" flags || List.mem "-O4" flags in
         (* The reaper with phantom lets leaves dangling symbols (#7684 and
            unreported link failures), so the two are never combined. *)
         let flags = if reaper then List.filter (( <> ) "-flambda2-expert-phantom-lets") flags else flags in
         (* The reaper (also on at -O4) trips -dcmm-invariants with
            oxcaml/oxcaml#7737, so with it the output is what gets checked. *)
         let argv =
           if reaper
           then List.filter (( <> ) "-dcmm-invariants") b.argv
           else b.argv
         in
         { name = p "%s %s" b.name (String.concat " " flags); argv = argv @ flags })
       else b)
    backends
;;

let check ~ox ~effect_syntax backends tc =
  let backends =
    match !focus with
    | None -> backends
    | Some keep -> List.filteri (fun i _ -> List.mem i keep) backends
  in
  let backends = if ox then with_toggles tc backends else backends in
  incr cases;
  let source = Ocaml_diff_gen.program ~ox ~effect_syntax tc in
  note tc source;
  (* -dcmm-invariants counts a call returning an unboxed tuple or record as
     one value, so it rejects every handler that takes the results apart. *)
  let backends =
    if ox && (contains source "#(" || contains source "#{")
    then List.map (fun b -> { b with argv = List.filter (( <> ) "-dcmm-invariants") b.argv }) backends
    else backends
  in
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
    if List.for_all
         (function
           | Compile_error msg -> known_compile_crash msg
           | Ran _ -> false)
         results
    then (
      incr known;
      assume tc false);
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
          (* contains: the debug runtime prints a banner first *)
          not (contains err "Fatal error: exception Stdlib.Exit")
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
