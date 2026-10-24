module Ffi = Hegel_ffi.Ffi

type node =
  | Text of string
  | Breakable of string
  | Group of
      { indent : int
      ; open_ : string
      ; body : node list
      ; close : string
      }
  | Seq of node list
  | Reflow of string

type doc =
  { node : node
  ; atomic : bool
  }

type 'a t = 'a -> doc

let text s = { node = Text s; atomic = true }
let breakable sep = { node = Breakable sep; atomic = true }
let concat docs = { node = Seq (List.map (fun d -> d.node) docs); atomic = false }
let reflow s = { node = Reflow s; atomic = false }
let opaque = text "<abstr>"
let compound s = { node = Text s; atomic = false }

let group ?indent open_ docs close =
  let indent = Option.value indent ~default:(String.length open_) in
  { node = Group { indent; open_; body = List.map (fun d -> d.node) docs; close }
  ; atomic = true
  }
;;

let parenthesize doc = if doc.atomic then doc else group "(" [ doc ] ")"

let separated sep docs =
  List.concat
    (List.mapi (fun i d -> if i = 0 then [ d ] else [ text sep; breakable " "; d ]) docs)
;;

let tuple = function
  | [] -> text "()"
  | [ doc ] -> doc
  | docs -> group "(" (separated "," docs) ")"
;;

let record fields =
  group
    ~indent:2
    "{"
    (breakable " "
     :: separated
          ";"
          (List.map (fun (name, d) -> concat [ text (name ^ " = "); d ]) fields))
    " }"
;;

let apply f args =
  match args with
  | [] -> f
  | args ->
    let body = List.concat_map (fun arg -> [ breakable " "; parenthesize arg ]) args in
    { (group ~indent:2 "" (f :: body) "") with atomic = false }
;;

let constructor name = function
  | [] -> text name
  | [ arg ] -> apply (text name) [ arg ]
  | args -> apply (text name) [ tuple args ]
;;

let int n = if n < 0 then compound (string_of_int n) else text (string_of_int n)

let suffixed to_string suffix is_negative n =
  let s = to_string n ^ suffix in
  if is_negative n then compound s else text s
;;

let int32 = suffixed Int32.to_string "l" (fun n -> Int32.compare n 0l < 0)
let int64 = suffixed Int64.to_string "L" (fun n -> Int64.compare n 0L < 0)
let nativeint = suffixed Nativeint.to_string "n" (fun n -> Nativeint.compare n 0n < 0)
let bool b = text (string_of_bool b)
let unit () = text "()"
let char c = text (Printf.sprintf "%C" c)

let float_literal f =
  match Float.classify_float f with
  | FP_nan -> "nan"
  | FP_infinite -> if f > 0. then "infinity" else "neg_infinity"
  | FP_normal | FP_subnormal | FP_zero ->
    let rec shortest precision =
      let s = Printf.sprintf "%.*g" precision f in
      if precision >= 17 || Float.equal (float_of_string s) f
      then s
      else shortest (precision + 1)
    in
    let s = shortest 1 in
    if String.exists (fun c -> c = '.' || c = 'e') s then s else s ^ "."
;;

let float f =
  let s = float_literal f in
  if Float.sign_bit f && Float.is_finite f then compound s else text s
;;

let utf_8_length s i =
  let d = String.get_utf_8_uchar s i in
  if Uchar.utf_decode_is_valid d then Uchar.utf_decode_length d else 0
;;

let escape ~utf_8 s =
  let b = Buffer.create (String.length s + 2) in
  Buffer.add_char b '"';
  let n = String.length s in
  let rec go i =
    if i < n
    then (
      match s.[i] with
      | '"' ->
        Buffer.add_string b "\\\"";
        go (i + 1)
      | '\\' ->
        Buffer.add_string b "\\\\";
        go (i + 1)
      | '\n' ->
        Buffer.add_string b "\\n";
        go (i + 1)
      | '\t' ->
        Buffer.add_string b "\\t";
        go (i + 1)
      | '\r' ->
        Buffer.add_string b "\\r";
        go (i + 1)
      | c when Char.code c < 0x20 || Char.code c = 0x7f ->
        Buffer.add_string b (Printf.sprintf "\\%03d" (Char.code c));
        go (i + 1)
      | c when Char.code c < 0x80 ->
        Buffer.add_char b c;
        go (i + 1)
      | c ->
        let len = if utf_8 then utf_8_length s i else 0 in
        if len > 1
        then (
          Buffer.add_string b (String.sub s i len);
          go (i + len))
        else (
          Buffer.add_string b (Printf.sprintf "\\%03d" (Char.code c));
          go (i + 1)))
  in
  go 0;
  Buffer.add_char b '"';
  Buffer.contents b
;;

let string s = text (escape ~utf_8:true s)
let bytes s = text (escape ~utf_8:false s)
let sequence open_ close print xs = group open_ (separated ";" (List.map print xs)) close
let list print xs = sequence "[" "]" print xs
let array print xs = sequence "[|" "|]" print (Array.to_list xs)

let option print = function
  | None -> text "None"
  | Some x -> constructor "Some" [ print x ]
;;

let result print_ok print_error = function
  | Ok x -> constructor "Ok" [ print_ok x ]
  | Error e -> constructor "Error" [ print_error e ]
;;

let pair p1 p2 (a, b) = tuple [ p1 a; p2 b ]
let triple p1 p2 p3 (a, b, c) = tuple [ p1 a; p2 b; p3 c ]
let of_to_string to_string x = reflow (to_string x)

let rec sexp (s : Sexplib0.Sexp.t) =
  match s with
  | Atom _ -> text (Sexplib0.Sexp.to_string s)
  | List children ->
    group
      "("
      (List.concat
         (List.mapi
            (fun i child ->
               if i = 0 then [ sexp child ] else [ breakable " "; sexp child ])
            children))
      ")"
;;

let of_sexp sexp_of x = sexp (sexp_of x)

let rec render_node ctx printer = function
  | Text s ->
    List.iteri
      (fun i line ->
         if i > 0 then Ffi.printer_hard_break ctx printer;
         if not (String.equal line "") then Ffi.printer_text ctx printer line)
      (String.split_on_char '\n' s)
  | Breakable sep -> Ffi.printer_breakable ctx printer sep
  | Group { indent; open_; body; close } ->
    Ffi.printer_begin_group ctx printer ~indent open_;
    List.iter (render_node ctx printer) body;
    Ffi.printer_end_group ctx printer close
  | Seq nodes -> List.iter (render_node ctx printer) nodes
  | Reflow s -> Ffi.printer_reflow ctx printer s
;;

let render ctx printer doc = render_node ctx printer doc.node

let to_string ?max_width doc =
  let ctx = Ffi.context_new () in
  Fun.protect
    ~finally:(fun () -> Ffi.context_free ctx)
    (fun () ->
       let options =
         Option.map
           (fun width ->
              let options = Ffi.printer_options_new ctx in
              Ffi.printer_options_set_max_width ctx options width;
              options)
           max_width
       in
       let printer = Ffi.printer_new ctx options in
       Option.iter (Ffi.printer_options_free ctx) options;
       Fun.protect
         ~finally:(fun () -> Ffi.printer_free ctx printer)
         (fun () ->
            render ctx printer doc;
            Ffi.printer_value ctx printer))
;;
