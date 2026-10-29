(** Two properties, each written twice: with Hegel, and with Base_quickcheck
    through {{:https://github.com/janestreet/ppx_quick_test} ppx_quick_test}.

    The first: removing an element that is in a list makes the list one
    shorter. The element must come {e from} the list, so the second value
    depends on the first. The second: flipping any one byte of an encoding is
    detected. The byte's index is bounded by the encoding's length, so test
    code ([Codec.encode]) runs between two draws. That is where the two
    libraries differ most:

    - {b Mixing test code and generation.} Hegel draws values inside the test
      body, so test code can run between draws and decide what to draw next,
      and the shrinker reruns that code on every attempt. A Base_quickcheck
      generator can also run test code ([let%bind] can call [Codec.encode]),
      but its shrinker only sees the finished value. So the test code has to be
      written again in the shrinker, or shrinking reports inputs the test could
      never have drawn. Without the [Shrinker.filter] below, a decoder bug was
      reported as [(() 29)]: index 29 into a 4-byte encoding, which flips
      nothing. (An assertion moved into a generator is worse: its failure is an
      uncaught exception, with no input printed and no shrinking.)
    - {b Shrinking.} Hegel shrinks the choices it recorded while the test ran
      and replays the test body, so a shrunk input is always one the test could
      have drawn ([x] stays in [l]). Base_quickcheck shrinks the finished value
      with a separate [Shrinker] that knows nothing about how it was generated.
      Each parameter defaults to its type's shrinker
      ([[%quickcheck.shrinker: t]]), and the one for [int list * int] would
      shrink [l] without regard to [x] and report failures where [x] is not
      in [l]. So the example wraps it in [Shrinker.filter] to drop those
      candidates. That keeps the invariant, but shrinking can stop at a local
      minimum like [([5], 5)]. The built-in [int] shrinker is
      [Shrinker.atomic], so ints are never shrunk at all, and a failing
      [4611686018427387903] is reported as is. Hegel shrinks ints toward [0].
    - {b Distribution.} Base_quickcheck lets you shape what gets generated:
      [Generator.weighted_union], [int_log_uniform_inclusive], [int_geometric],
      and a [size] parameter ([Generator.size], [with_size]). Hegel's engine
      picks distributions itself; [booleans ?p] is the only weight, and
      [one_of] and [sampled_from] are unweighted. Bias by hand with inline
      draws (e.g. draw [integers ~max_value:9 ()] and branch on it).
    - {b What has to be derived.} ppx_quick_test needs a generator, a shrinker
      and [sexp_of] for every parameter type, usually from
      [[@@deriving quickcheck, sexp_of]]. Hegel needs only a generator.
    - {b Printing.} Base_quickcheck generators carry no printer: the printer
      comes from the input's type ([Test.S] requires [sexp_of_t]), so
      [Generator.map], [bind] or a custom [[@generator]] never lose it, but
      every input must be sexpable and prints as one sexp. Hegel puts the
      printer on the generator and tracks it in the type
      ([('a, printable) generator]). [map], [flat_map], [composite] and
      [sampled_from] return unprintable generators, so you either restore the
      printer with [with_printer [%sexp_of: t] gen] (as below), or
      [draw_silent] and [note] the value by hand. In return, each [draw] prints on its
      own, named after its [let] binding, and inputs with no [sexp_of] at all
      (functions, handles) can still be drawn.
    - {b Remembering failures.} Hegel saves failing cases in its database
      ([Settings.database]) and replays them first on the next run.
      ppx_quick_test with [[@remember_failures]] writes the failing input's
      sexp into that attribute in the source, which you accept by promoting
      the [.corrected] file.
    - {b Harness.} [let%quick_test] is an inline test (v0.17 syntax below; newer
      releases put it inside a [let%expect_test]) and reports failures as CRs
      in a [.corrected] file. [let%hegel_test] defines a
      plain [unit -> unit] function that raises on failure, so it runs under
      any test framework (or none, like {!Workload}).
    - {b Health checks.} Hegel fails a test that rejects most of its inputs,
      takes too long per input, or generates inputs too large to test well,
      rather than passing after testing almost nothing (see the health check
      tests at the end). Base_quickcheck has no such checks.
      [Generator.filter] retries until a value passes, so the [Filter_too_much]
      test below passes under Base_quickcheck after about 10,000 retries per
      input, and the [Too_slow] one would run 10,000 four-second tests.
    - {b Beyond single properties.} Hegel also runs stateful and concurrent
      state machines ([module%hegel_state_machine],
      [module%hegel_concurrent_state_machine], see {!Workload}). Base_quickcheck
      has nothing equivalent. *)

open Hegel
open Base_quickcheck
open Generator.Let_syntax

let rec remove_first x = function
  | [] -> []
  | y :: rest -> if x = y then rest else y :: remove_first x rest
;;

(* Hegel: the second draw uses the first, and shrinking keeps that true. *)
let%hegel_test remove_first_shortens tc =
  let l = draw tc (lists (integers ()) ~min_size:1 ()) in
  let x = draw tc (with_printer [%sexp_of: int] (sampled_from l)) in
  assert (List.length (remove_first x l) = List.length l - 1)
;;

let%quick_test "remove_first shortens" =
  fun
    (l, x :
      (int list * int
      [@generator
        let%bind l = Generator.(list_non_empty int) in
        let%map x = Generator.of_list l in
        l, x]
      [@shrinker
        Shrinker.filter
          [%quickcheck.shrinker: int list * int]
          ~f:(fun (l, x) -> List.mem x l)]))
  -> assert (List.length (remove_first x l) = List.length l - 1)
;;

let%expect_test "remove_first shortens (Hegel)" = remove_first_shortens ()

(* Second example: test code runs between two draws. The range for [i] depends
   on what [Codec.encode] returned. [Codec] is a stand-in system under test:
   it writes the payload twice, so any one flipped byte is detectable. *)

module Codec = struct
  let payload v = "[" ^ String.concat "," (List.map string_of_int v) ^ "]"
  let encode v = payload v ^ payload v

  let decode s =
    let n = String.length s / 2 in
    if String.length s mod 2 <> 0 || String.sub s 0 n <> String.sub s n n
    then Error "corrupt"
    else (
      let p = String.sub s 0 n in
      if n < 2 || p.[0] <> '[' || p.[n - 1] <> ']'
      then Error "corrupt"
      else (
        match String.sub p 1 (n - 2) with
        | "" -> Ok []
        | body ->
          (match List.map int_of_string_opt (String.split_on_char ',' body) with
           | ints when List.for_all Option.is_some ints -> Ok (List.map Option.get ints)
           | _ -> Error "corrupt")))
  ;;
end

let flip_byte s i =
  String.mapi (fun j c -> if j = i then Char.chr (Char.code c lxor 1) else c) s
;;

let%hegel_test corruption_is_detected tc =
  let v = draw tc (lists (integers ()) ()) in
  let bytes = Codec.encode v in
  let i = draw tc (integers ~min_value:0 ~max_value:(String.length bytes - 1) ()) in
  assert (Codec.decode (flip_byte bytes i) <> Ok v)
;;

(* ppx_quick_test: [encode] has to run in the generator to bound [i], and again
   in the shrinker, which otherwise shrinks [v] and leaves [i] past the end. *)
let%quick_test "corruption is detected" =
  fun
    (v, i :
      (int list * int
      [@generator
        let%bind v = Generator.(list int) in
        let%map i = Generator.int_inclusive 0 (String.length (Codec.encode v) - 1) in
        v, i]
      [@shrinker
        Shrinker.filter
          [%quickcheck.shrinker: int list * int]
          ~f:(fun (v, i) -> i < String.length (Codec.encode v))]))
  -> assert (Codec.decode (flip_byte (Codec.encode v) i) <> Ok v)
;;

let%expect_test "corruption is detected (Hegel)" = corruption_is_detected ()

(* Health checks: each test below is written so that Hegel refuses to run it.
   The comment after each shows the failure it raises. *)

(* Filter_too_much: 1 in 10,001 inputs passes the [assume]. *)
let%hegel_test rare_assume tc =
  let x = draw tc (integers ~min_value:0 ~max_value:10_000 ()) in
  assume tc (x = 42)
;;

(* FailedHealthCheck: FilterTooMuch — it looks like this test is filtering out
   too many inputs. 50 inputs were rejected by assume() or a filter while only 0
   valid inputs were generated. If this is expected, suppress the FilterTooMuch
   health check with the suppress_health_check setting. *)

(* Filter_too_much through [filter]: after 3 failed attempts it rejects the
   input like [assume false]. Few 5-element lists are sorted. *)
let%hegel_test sorted_by_filter tc =
  let l =
    draw
      tc
      (filter (fun l -> List.sort compare l = l) (lists (integers ()) ~min_size:5 ()))
  in
  assert (List.sort compare l = l)
;;

(* FailedHealthCheck: FilterTooMuch — it looks like this test is filtering out
   too many inputs. 50 inputs were rejected by assume() or a filter while only 2
   valid inputs were generated. If this is expected, suppress the FilterTooMuch
   health check with the suppress_health_check setting. *)

(* Too_slow: 30 seconds of generation with fewer than 10 valid inputs. *)
let%hegel_test slow tc =
  let (_ : int) = draw tc (integers ()) in
  Unix.sleepf 4.0
;;

(* FailedHealthCheck: TooSlow — input generation is slow: only 8 valid inputs
   after 32.029687s (threshold 30s). Slow generation makes property testing much
   less effective. If this is expected, suppress the TooSlow health check with
   the suppress_health_check setting. *)

(* Test_cases_too_large: the smallest input (100,000 empty lists) fits, but
   random inner lists push most inputs past the 2^20 choice limit. *)
let%hegel_test nested_lists tc =
  let m = draw_silent tc (lists (lists (integers ()) ()) ~min_size:100_000 ()) in
  assert (List.length m >= 100_000)
;;

(* FailedHealthCheck: TestCasesTooLarge — generated inputs routinely exceeded
   the maximum size: 1 inputs were generated successfully, while 20 inputs
   overran the choice limit during generation. Testing with inputs this large
   is slow and shrinks poorly. Try reducing the amount of data generated, e.g. a
   smaller minimum size on collections. If this is expected, suppress the
   TestCasesTooLarge health check with the suppress_health_check setting, which
   also removes the limit on the size of a test case. *)

(* Large_initial_test_case: even the smallest input uses more than half the
   choice limit. *)
let%hegel_test huge_minimum tc =
  let l = draw_silent tc (lists (integers ()) ~min_size:600_000 ()) in
  assert (List.length l >= 600_000)
;;

(* FailedHealthCheck: LargeInitialTestCase — the smallest natural input for this
   test is very large, which makes it hard to generate and shrink good inputs.
   Consider reducing the amount of data generated, or introducing small
   alternatives (e.g. a one_of with an empty option). If this is expected,
   suppress the LargeInitialTestCase health check with the suppress_health_check
   setting. *)
