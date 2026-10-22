(* Locates the native libhegel shared library at runtime

   Search order: 1. [$HEGEL_LIBHEGEL_PATH] — an explicit path to the library
   file (or a directory containing [libhegel.<ext>]). 2. A prebuilt libhegel
   bundled into the installed package via the [libhegel] dune-site (release
   tarballs ship the matching-platform binary there). 3. The in-tree build of
   the hegeldev/hegel checkout this frontend is part of: the [libhegel_c.<ext>]
   that [cargo build -p hegeltest-c] leaves under [target/release/] (then
   [target/debug/]) of [$CARGO_TARGET_DIR], or of the nearest ancestor of the
   working directory that has a [target/] with one — so the test executables
   find the checkout they run in whether dune runs them from [_build] or from
   a sandbox.

   This module lives in the (uninstrumented) [hegel_ffi] library, so its
   filesystem branches are not subject to the coverage gate. *)

let getenv_nonempty name =
  match Sys.getenv_opt name with
  | Some s when String.length s > 0 -> Some s
  | _ -> None
;;

let read_line_cmd cmd =
  try
    let ic = Unix.open_process_in cmd in
    let line =
      try input_line ic with
      | End_of_file -> ""
    in
    ignore (Unix.close_process_in ic);
    String.trim line
  with
  | _ -> ""
;;

(* OS identifier used in the libhegel release-artifact name. The values
   ("darwin" / "linux" / "windows") follow that artifact naming convention. *)
let os_id () =
  match Sys.os_type with
  | "Win32" | "Cygwin" -> "windows"
  | _ ->
    (match read_line_cmd "uname -s" with
     | "Darwin" -> "darwin"
     | "Linux" -> "linux"
     | other -> failwith (Printf.sprintf "hegel: unsupported operating system %S" other))
;;

let ext_of_os = function
  | "darwin" -> "dylib"
  | "windows" -> "dll"
  | _ -> "so"
;;

(* Name of the library file as published on the libhegel GitHub release. *)
let local_basename ext = "libhegel." ^ ext

(* Name of the cdylib [cargo build -p hegeltest-c] leaves in the target
   directory: the crate's library is [hegel_c], so cargo names it
   [libhegel_c.<ext>] (no [lib] prefix on Windows). *)
let cargo_basename ext = (if String.equal ext "dll" then "" else "lib") ^ "hegel_c." ^ ext

let is_dir p =
  try Sys.is_directory p with
  | Sys_error _ -> false
;;

let is_file p =
  try Sys.file_exists p && not (Sys.is_directory p) with
  | Sys_error _ -> false
;;

(* 1. Explicit [$HEGEL_LIBHEGEL_PATH] override. *)
let from_env ext =
  Option.map
    (fun p -> if is_dir p then Filename.concat p (local_basename ext) else p)
    (getenv_nonempty "HEGEL_LIBHEGEL_PATH")
;;

(* 2. A libhegel bundled into the installed package. Release tarballs install
   the matching-platform binary into the [libhegel] dune-site under the fixed
   name [libhegel_bundled] (the loader opens it by path, so the missing
   extension is irrelevant to [dlopen]). *)
let from_site () =
  List.find_opt
    is_file
    (List.map
       (fun dir -> Filename.concat dir "libhegel_bundled")
       Hegel_sites.Sites.libhegel)
;;

(* The library under one cargo target directory, the release build first. *)
let in_target_dir dir ext =
  List.find_opt
    is_file
    (List.map
       (fun profile -> List.fold_left Filename.concat dir [ profile; cargo_basename ext ])
       [ "release"; "debug" ])
;;

(* 3. The in-tree build: the [target/] directory of [start] or of its nearest
   ancestor that holds a built library. Exposed so the test suite can exercise
   the walk on a directory tree of its own. *)
let from_in_tree ~start ext =
  let rec walk dir =
    match in_target_dir (Filename.concat dir "target") ext with
    | Some p -> Some p
    | None ->
      let parent = Filename.dirname dir in
      if String.equal parent dir then None else walk parent
  in
  walk start
;;

let from_cargo_target_dir ext =
  Option.bind (getenv_nonempty "CARGO_TARGET_DIR") (fun dir -> in_target_dir dir ext)
;;

(** [locate ()] returns the path to a usable libhegel shared library. Raises
    [Failure] with a descriptive message if no library can be found. *)
let locate () =
  let os_id = os_id () in
  let ext = ext_of_os os_id in
  let cwd = Sys.getcwd () in
  match from_env ext with
  | Some p -> p
  | None ->
    (match from_site () with
     | Some p -> p
     | None ->
       (match from_cargo_target_dir ext with
        | Some p -> p
        | None ->
          (match from_in_tree ~start:cwd ext with
           | Some p -> p
           | None ->
             failwith
               (Printf.sprintf
                  "hegel: libhegel not found. Looked at $HEGEL_LIBHEGEL_PATH, the \
                   installed package's libhegel site, and target/{release,debug}/%s \
                   under $CARGO_TARGET_DIR and under every directory above %s. Build it \
                   with `cargo build --release -p hegeltest-c` in a hegeldev/hegel \
                   checkout, or set HEGEL_LIBHEGEL_PATH to a libhegel shared library."
                  (cargo_basename ext)
                  cwd))))
;;
