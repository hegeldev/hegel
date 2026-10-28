(** The current version of Hegel for OCaml. *)
let version = Hegel_version.version

let engine_version () =
  let ctx = Hegel_ffi.Ffi.context_new () in
  Fun.protect
    ~finally:(fun () -> Hegel_ffi.Ffi.context_free ctx)
    (fun () -> Hegel_ffi.Ffi.version ctx)
;;

(** Generators for composable test data generation. *)
module Generators = Generators

(** Stateful property-based testing on top of {!Generators}. *)
module Stateful = Stateful

(** Concurrency capabilities for concurrent stateful tests. *)
module Concurrency = Concurrency

module Settings = Settings
module Printer = Printer

(** Auxiliary submodule for [@@deriving hegel_generator]. Included
    below so [open Hegel] alone makes derived code resolve. *)
module Derive = Derive

include Derive

(** Test runner and run-loop internals; re-exported (doc-hidden) for white-box
    tests, not for direct use. *)
module Internal = Internal

(** Command-line parsing for {!main}; re-exported (doc-hidden) for white-box
    tests. *)
module Cli = Cli

module type Io = Io.S

module Make = Io.Make

type ('a, 'p) generator = ('a, 'p) Generators.generator
type printable = Generators.printable
type unprintable = Generators.unprintable

include Generators.Public

(* Test-case and test-location types re-exported so the whole
   public API lives directly under Hegel. The module re-exports above are
   doc-hidden in the mli: white-box surfaces for the test suite. *)

type test_case = Internal.test_case

type test_location = Internal.test_location =
  { function_name : string
  ; file : string
  ; begin_line : int
  }

exception Assume_rejected = Internal.Assume_rejected
exception Usage_error = Internal.Usage_error
exception Failed_health_check = Internal.Failed_health_check
exception Flaky = Internal.Flaky
exception Unsatisfiable = Internal.Unsatisfiable
exception Run_error = Internal.Run_error
exception Multiple_failures = Internal.Multiple_failures
exception Did_not_reproduce = Internal.Did_not_reproduce

(** {2 Convenience re-exports} *)

(** [run_hegel_test ?settings ?test_location ?database_key ?failure_blobs test_fn]
    runs a property test against the native engine, defaulting to
    [Settings.default ()]. The [let%hegel_test] PPX runs tests through the
    equivalent {!run_hegel_test_ppx}. *)
let run_hegel_test ?settings ?test_location ?database_key ?failure_blobs test_fn =
  Internal.run_hegel_test ?settings ?test_location ?database_key ?failure_blobs test_fn
;;

(** [run_hegel_test_ppx] is {!run_hegel_test} with the PPX replay hint enabled;
    the [let%hegel_test] PPX targets it. Not for direct use. *)
let run_hegel_test_ppx
      ?settings
      ?test_location
      ?database_key
      ?failure_blobs
      ?examples
      test_fn
  =
  Internal.run_hegel_test
    ?settings
    ?test_location
    ~rerun_hint:Attribute
    ?database_key
    ?failure_blobs
    ?examples
    test_fn
;;

(** [main ?settings ?test_location ?database_key test_fn] runs [test_fn] as
    one test case configured from the command line, then exits. *)
let main ?settings ?test_location ?database_key test_fn =
  Cli.main ?settings ?test_location ?database_key test_fn
;;

(** [assume tc condition] rejects the current test case if [condition] is
    [false]. *)
let assume = Internal.assume

(** [reject tc] rejects the current test case unconditionally. *)
let reject _tc = raise Internal.Assume_rejected

(** [note tc message] prints [message] to stderr subject to the run's verbosity:
    never under [Quiet], only on the final (failing) replay under [Normal], and
    on every test case under [Verbose] or [Debug]. *)
let note = Internal.note

(** [require tc ?msg condition] fails the current test case when [condition] is
    [false]. See {!Internal.require}. *)
let require = Internal.require

(** [target tc ~label ~value] sends a target command to guide the search engine
    toward higher values. *)
let target = Internal.target

(** [event tc ~label] records [label] as observed on this test case for the
    end-of-run statistics report. *)
let event = Internal.event

(** [event_value tc ~label ~value] records the finite observation [value] under
    [label] for the end-of-run statistics report. *)
let event_value = Internal.event_value

(** [draw ?label tc gen] produces a typed value from the printable generator
    [gen]. On the final replay of a failing test, an outermost draw prints its
    value. See {!Generators.draw}. *)
let draw = Generators.draw

(** [draw_named ~label ~repeatable tc gen] is the naming-aware draw the
    [let%hegel_test] PPX rewrites bindings to; not intended for direct use
    (prefer {!draw}). See {!Generators.draw_named}. *)
let draw_named = Generators.draw_named

(** [draw_silent tc gen] is {!draw} without printing the value on the final
    replay, and accepts a generator with no printer. *)
let draw_silent = Generators.draw_silent

(** [draw_silent_named ~name tc gen] is the naming-aware {!draw_silent} the
    [let%hegel_test] PPX rewrites bindings to; not intended for direct use
    (prefer {!draw_silent}). See {!Generators.draw_silent_named}. *)
let draw_silent_named = Generators.draw_silent_named

(** [clone tc] forks an independent clone of [tc] for driving generation
    concurrently. Its native resources are owned by the test case and freed
    once the case completes. See {!Internal.clone}. *)
let clone = Internal.clone

(** [set_worker tc worker_index] attributes [tc]'s output to a concurrent
    worker. See {!Internal.set_worker_index}. *)
let set_worker = Internal.set_worker_index

(** [run_iteration tc ~repeat body] runs one iteration of {!repeat}: [body]
    on a block of [tc]. A rejected iteration is skipped; any other failure is
    reported to the engine before it propagates. *)
let run_iteration tc ~repeat body =
  match Internal.with_block ~share_names:true tc ~indent:2 body with
  | () | (exception Assume_rejected) -> ()
  | exception
      ((Internal.Stop_test | Usage_error _ | Internal.Internal_error _) as control) ->
    raise control
  | exception exn ->
    let backtrace = Printexc.get_raw_backtrace () in
    Internal.repeat_failed tc ~repeat;
    Printexc.raise_with_backtrace exn backtrace
;;

(** [repeat_generated tc body] runs [body] on a block of [tc] until the
    engine ends the loop, then ends the test case. *)
let repeat_generated tc body =
  let repeat = Internal.new_repeat tc in
  Fun.protect
    ~finally:(fun () -> Internal.repeat_free tc ~repeat)
    (fun () ->
       let rec loop iteration =
         if Internal.repeat_more tc ~repeat
         then (
           note tc (Printf.sprintf "(* Repetition #%d *)" iteration);
           run_iteration tc ~repeat body;
           loop (iteration + 1))
       in
       loop 1);
  raise Internal.Repeat_done
;;

(** [repeat_explicit tc body] is {!repeat} in an explicit example: [body]
    runs once, as an explicit value can be drawn only once. *)
let repeat_explicit tc body =
  note tc "(* Repetition #1 *)";
  body tc;
  raise Internal.Repeat_done
;;

(** [repeat tc body] is {!repeat_explicit} in an explicit example and
    {!repeat_generated} otherwise. *)
let repeat tc body =
  if Internal.is_explicit tc then repeat_explicit tc body else repeat_generated tc body
;;

(** [with_printer sexp_of gen] attaches [sexp_of] so [gen] can be drawn with
    {!draw}. See {!Generators.with_printer}. *)
let%template with_printer = Generators.with_printer [@mode m]
[@@mode m = (nonportable, portable)]
;;

(** [with_ocaml_printer print gen] attaches [print] so [gen] can be drawn with
    {!draw}, its values printing as [print]'s documents. See
    {!Generators.with_ocaml_printer}. *)
let%template with_ocaml_printer = Generators.with_ocaml_printer [@mode m]
[@@mode m = (nonportable, portable)]
;;
