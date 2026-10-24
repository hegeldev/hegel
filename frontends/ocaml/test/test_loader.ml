(** Tests for the in-tree libhegel lookup ([Hegel_ffi.Loader.from_in_tree]):
    the walk from a working directory up to the nearest [target/] holding a
    [cargo build -p hegeltest-c] output, exercised on a directory tree of the
    test's own rather than the real checkout. *)

open! Core
module Loader = Hegel_ffi.Loader

let rec remove_tree path =
  if Stdlib.Sys.is_directory path
  then (
    Stdlib.Sys.readdir path
    |> Array.iter ~f:(fun name -> remove_tree (Filename.concat path name));
    Core_unix.rmdir path)
  else Stdlib.Sys.remove path
;;

(* A tree under a fresh tempdir, removed (recursively, unlike
   [Test_helpers.with_tempdir]'s flat cleanup) on exit. *)
let with_tree ~f =
  Test_helpers.with_tempdir ~prefix:"hegel-loader-test-" ~f:(fun dir ->
    let root = Filename.concat dir "checkout" in
    Core_unix.mkdir_p root;
    Exn.protect ~finally:(fun () -> remove_tree root) ~f:(fun () -> f root))
;;

let touch root rel =
  let path = Filename.concat root rel in
  Core_unix.mkdir_p (Filename.dirname path);
  Out_channel.write_all path ~data:"";
  path
;;

let test_finds_the_nearest_ancestor_build () =
  with_tree ~f:(fun root ->
    let built = touch root "target/release/libhegel_c.so" in
    let start = Filename.concat root "frontends/ocaml/_build/default/test" in
    Core_unix.mkdir_p start;
    Alcotest.(check (option string))
      "walks up to the checkout's target/"
      (Some built)
      (Loader.from_in_tree ~start "so"))
;;

let test_prefers_release_over_debug () =
  with_tree ~f:(fun root ->
    let debug = touch root "target/debug/libhegel_c.so" in
    Alcotest.(check (option string))
      "a debug build is found when it is the only one"
      (Some debug)
      (Loader.from_in_tree ~start:root "so");
    let release = touch root "target/release/libhegel_c.so" in
    Alcotest.(check (option string))
      "the release build wins once it exists"
      (Some release)
      (Loader.from_in_tree ~start:root "so"))
;;

let test_ignores_other_extensions () =
  with_tree ~f:(fun root ->
    ignore (touch root "target/release/libhegel_c.so" : string);
    Alcotest.(check (option string))
      "a library for another platform does not count"
      None
      (Loader.from_in_tree ~start:root "dylib"))
;;

let tests =
  [ Alcotest.test_case
      "finds the nearest ancestor build"
      `Quick
      test_finds_the_nearest_ancestor_build
  ; Alcotest.test_case "prefers release over debug" `Quick test_prefers_release_over_debug
  ; Alcotest.test_case "ignores other extensions" `Quick test_ignores_other_extensions
  ]
;;
