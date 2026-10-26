(* Generates well-typed OCaml programs that terminate and whose output does
   not depend on evaluation order or on any unspecified behaviour. *)

open Hegel

type ty =
  | Int
  | Bool
  | Float
  | Char
  | Int64
  | Str
  | List of ty
  | Option of ty
  | Pair of ty * ty
  | Arrow of ty * ty
  | Named of int
  | Float_u (* OxCaml's unboxed float#: never inside a list, option or tuple *)

(* Declared types refer only to earlier ones and hold only first-order
   values. *)
type decl =
  | Record of (string * ty * bool) list (* name, type, mutable *)
  | Variant of (string * ty list) list

let rec show_ty = function
  | Int -> "int"
  | Bool -> "bool"
  | Float -> "float"
  | Char -> "char"
  | Int64 -> "int64"
  | Str -> "string"
  | List t -> Printf.sprintf "(%s list)" (show_ty t)
  | Option t -> Printf.sprintf "(%s option)" (show_ty t)
  | Pair (a, b) -> Printf.sprintf "(%s * %s)" (show_ty a) (show_ty b)
  | Arrow (a, b) -> Printf.sprintf "(%s -> %s)" (show_ty a) (show_ty b)
  | Named i -> Printf.sprintf "t%d" i
  | Float_u -> "float#"
;;

(* Whether OxCaml features are generated. *)
let ox_mode = ref false
let use_effect_syntax = ref false

(* (child, parent): polymorphic variant t<child> extends t<parent>. *)
let poly_parent = ref []

(* Swarm testing: each program uses only some features, so the ones it does
   use are not crowded out, and a shrunk failure names the ones it needs. *)
let all_features =
  [ "atomics"; "deep"; "effect_syntax"; "effects"; "exits"; "exns"; "gadt"; "gc"; "lazy"
  ; "locals"; "loops"; "manyargs"; "modules"; "opaque"; "partial"; "poly"; "records"
  ; "recvals"; "shallow"; "strings"; "tmc"
  ]

let features = ref all_features
let on feature = List.mem feature !features

(* A switched-on feature's productions are one choice among many; repeating
   them makes the feature show up in the programs that switch it on. *)
let boost l = List.concat (List.init 5 (fun _ -> l))

(* The declared types that hold a float#, whose values polymorphic comparison
   rejects. *)
let mixed = ref [||]

let rec first_order = function
  | Int | Bool | Float | Char | Int64 | Str -> true
  | Named i -> not !mixed.(i)
  | Float_u -> false
  | List t | Option t -> first_order t
  | Pair (a, b) -> first_order a && first_order b
  | Arrow _ -> false
;;

let rec printer = function
  | Int -> "string_of_int"
  | Bool -> "string_of_bool"
  | Float -> "show_float"
  | Char -> "Char.escaped"
  | Int64 -> "Int64.to_string"
  | Str -> {|(Printf.sprintf "%S")|}
  | List t ->
    Printf.sprintf
      {|(fun l -> "[" ^ String.concat "; " (List.map %s l) ^ "]")|}
      (printer t)
  | Option t ->
    Printf.sprintf
      {|(function None -> "None" | Some v -> "Some (" ^ %s v ^ ")")|}
      (printer t)
  | Pair (a, b) ->
    Printf.sprintf
      {|(fun (a, b) -> "(" ^ %s a ^ ", " ^ %s b ^ ")")|}
      (printer a)
      (printer b)
  | Arrow _ -> {|(fun _ -> "<fun>")|}
  | Named i -> Printf.sprintf "show_t%d" i
  | Float_u -> "(fun (x : float#) -> show_float (box_float x))"
;;

let rec has_float_u = function
  | Float_u -> true
  | Named i -> !mixed.(i)
  | List t | Option t -> has_float_u t
  | Pair (a, b) -> has_float_u a || has_float_u b
  | Int | Bool | Float | Char | Int64 | Str | Arrow _ -> false
;;

let declaration ?parent i decl =
  let p = Printf.sprintf in
  match decl with
  | Record fields ->
    let field (f, t, m) = p "%s%s : %s" (if m then "mutable " else "") f (show_ty t) in
    let show (f, t, _) = p {|"%s = " ^ %s v.%s|} f (printer t) f in
    (* OxCaml requires a record of only boxed and unboxed floats to say how it
       is laid out. *)
    let has t = List.exists (fun (_, t', _) -> t' = t) fields in
    let flatten =
      if
        List.for_all (fun (_, t, _) -> t = Float || t = Float_u) fields
        && has Float
        && has Float_u
      then " [@@flatten_floats]"
      else ""
    in
    p
      "type t%d = { %s }%s\nlet show_t%d (v : t%d) = \"{\" ^ %s ^ \"}\"\n"
      i
      (String.concat "; " (List.map field fields))
      flatten
      i
      i
      (String.concat {| ^ "; " ^ |} (List.map show fields))
  | Variant constrs ->
    let constr (c, args) =
      match args with
      | [] -> c
      | _ -> p "%s of %s" c (String.concat " * " (List.map show_ty args))
    in
    let show (c, args) =
      match args with
      | [] -> p {|%s -> "%s"|} c c
      | _ ->
        let xs = List.mapi (fun j _ -> p "a%d" j) args in
        p
          {|%s (%s) -> "%s (" ^ %s ^ ")"|}
          c
          (String.concat ", " xs)
          c
          (String.concat
             {| ^ ", " ^ |}
             (List.map2 (fun x t -> p "%s %s" (printer t) x) xs args))
    in
    let alts cs = String.concat " | " (List.map constr cs) in
    let body =
      match parent with
      | Some (j, inherited) ->
        p "[ t%d | %s ]" j (alts (List.filter (fun c -> not (List.mem c inherited)) constrs))
      | None when (fst (List.hd constrs)).[0] = '`' -> p "[ %s ]" (alts constrs)
      | None -> alts constrs
    in
    p
      "type t%d = %s\nlet show_t%d (v : t%d) = match v with %s\n"
      i
      body
      i
      i
      (String.concat " | " (List.map show constrs))
;;

(* NaN's sign is not preserved consistently by constant folding, so all NaNs
   print alike; compare promises only a sign, which flambda folds to the
   difference of chars, so its result goes through sgn. *)
let prelude () =
  {|let show_float f = if Float.is_nan f then "nan" else Printf.sprintf "%h" f
let sdiv a b = if b = 0 then a else a / b
let smod a b = if b = 0 then a else a mod b
let sgn c = if c < 0 then -1 else if c > 0 then 1 else 0
let i64div a b = if b = 0L then a else Int64.div a b
let i64rem a b = if b = 0L then a else Int64.rem a b
let ssub s i n = let l = String.length s in let i = (i land max_int) mod (l + 1) in String.sub s i ((n land max_int) mod (l - i + 1))
let rec take n l = if n <= 0 then [] else match l with [] -> [] | x :: r -> x :: take (n - 1) r
type _ gx =
  | GInt : int -> int gx
  | GBool : bool -> bool gx
  | GAdd : int gx * int gx -> int gx
  | GLe : int gx * int gx -> bool gx
  | GIf : bool gx * 'a gx * 'a gx -> 'a gx
  | GPair : 'a gx * 'b gx -> ('a * 'b) gx
  | GFst : ('a * 'b) gx -> 'a gx
  | GSnd : ('a * 'b) gx -> 'b gx
  | GLit : 'a -> 'a gx
let rec geval : type a. a gx -> a = function
  | GInt n -> n
  | GBool b -> b
  | GAdd (a, b) -> let x = geval a in x + geval b
  | GLe (a, b) -> let x = geval a in x <= geval b
  | GIf (c, a, b) -> if geval c then geval a else geval b
  | GPair (a, b) -> let x = geval a in (x, geval b)
  | GFst q -> fst (geval q)
  | GSnd q -> snd (geval q)
  | GLit v -> v
let gbump : type a. a gx -> a -> a = fun g v ->
  match g with GInt _ -> v + 1 | GBool _ -> not v | GAdd _ -> v - 1 | GLe _ -> v | _ -> v
|}
  ^
  if !ox_mode
  then
    {|external box_float : float# -> (float[@local_opt]) = "%box_float"
external unbox_float : (float[@local_opt]) -> float# = "%unbox_float"
let[@inline always] fu_add x y = unbox_float (box_float x +. box_float y)
let[@inline always] fu_sub x y = unbox_float (box_float x -. box_float y)
let[@inline always] fu_mul x y = unbox_float (Sys.opaque_identity (box_float x *. box_float y))
let[@inline never] fu_div x y = unbox_float (box_float x /. box_float y)
let fu_sqrt x = unbox_float (sqrt (box_float x))
|}
  else ""
;;

type ctx =
  { tc : test_case
  ; mutable next : int
  ; mutable decls : decl array
  ; mutable effects : (string * ty * ty) list (* constructor, argument, result *)
  ; used : (string, unit) Hashtbl.t (* variables the program refers to so far *)
  }

let label = Generators.Ppx_internal.Labels.from_name "ocaml_diff.expr"

let fresh ctx prefix =
  ctx.next <- ctx.next + 1;
  Printf.sprintf "%s%d" prefix ctx.next
;;

let int_in ctx lo hi = draw_silent ctx.tc (integers ~min_value:lo ~max_value:hi ())
let choose ctx options = (List.nth options (int_in ctx 0 (List.length options - 1))) ()
let p = Printf.sprintf

let span ctx f =
  draw_silent ctx.tc (Generators.Ppx_internal.composite_with_label ~label (fun _ -> f ()))
;;

(* With [~unboxed], the type may be float#, which only a binding, a function
   argument or result, or a record field can have. *)
let rec gen_ty ?(unboxed = false) ctx ~first_order depth =
  let sub () = gen_ty ctx ~first_order (depth - 1) in
  let named =
    List.filter
      (fun i -> (not first_order) || not !mixed.(i))
      (List.init (Array.length ctx.decls) Fun.id)
  in
  choose
    ctx
    ([ (fun () -> Int)
     ; (fun () -> Bool)
     ; (fun () -> Float)
     ; (fun () -> Float)
     ; (fun () -> Float)
     ; (fun () -> Char)
     ; (fun () -> Int64)
     ]
     @ (if on "strings" then [ (fun () -> Str) ] else [])
     @ (if named <> []
        then [ (fun () -> Named (List.nth named (int_in ctx 0 (List.length named - 1)))) ]
        else [])
     @ (if unboxed && !ox_mode then [ (fun () -> Float_u) ] else [])
     @ (if depth = 0
        then []
        else
          [ (fun () -> List (sub ()))
          ; (fun () -> Option (sub ()))
          ; (fun () ->
              let a = sub () in
              Pair (a, sub ()))
          ])
     @
     if depth = 0 || first_order
     then []
     else
       [ (fun () ->
           let a = gen_ty ~unboxed:true ctx ~first_order (depth - 1) in
           Arrow (a, gen_ty ~unboxed:true ctx ~first_order (depth - 1)))
       ])
;;

let any_ty ctx = gen_ty ctx ~first_order:false 2
let top_ty ctx = gen_ty ~unboxed:true ctx ~first_order:false 2
let fo_ty ctx = gen_ty ctx ~first_order:true 2
let paren s = if String.length s > 0 && s.[0] = '-' then p "(%s)" s else s

let int_literal ctx =
  choose
    ctx
    [ (fun () -> "0")
    ; (fun () -> paren (string_of_int (int_in ctx (-2) 5)))
    ; (fun () -> "1")
    ; (fun () -> "(-1)")
    ; (fun () -> paren (string_of_int (int_in ctx (-100) 100)))
    ; (fun () -> "max_int")
    ; (fun () -> "min_int")
    ; (fun () ->
        let k = int_in ctx 0 61 in
        paren (string_of_int ((1 lsl k) + int_in ctx (-1) 1)))
    ; (fun () -> paren (string_of_int (draw_silent ctx.tc (integers ()))))
    ]
;;

let float_literal ctx =
  choose
    ctx
    [ (fun () -> "0.")
    ; (fun () -> "1.")
    ; (fun () -> "(-0.)")
    ; (fun () -> "nan")
    ; (fun () -> "infinity")
    ; (fun () -> "neg_infinity")
    ; (fun () -> "max_float")
    ; (fun () -> "min_float")
    ; (fun () -> "epsilon_float")
    ; (fun () -> "0x0.0000000000001p-1022")
    ; (fun () ->
        paren
          (p "%h" (draw_silent ctx.tc (floats ~allow_nan:false ~allow_infinity:false ()))))
    ]
;;

let char_literal ctx =
  choose
    ctx
    [ (fun () -> "'a'")
    ; (fun () -> p "%C" (Char.chr (int_in ctx (Char.code 'a') (Char.code 'h'))))
    ; (fun () -> p "%C" (Char.chr (int_in ctx 0 255)))
    ]
;;

let int64_lit n = if Int64.compare n 0L < 0 then p "(%LdL)" n else p "%LdL" n

let int64_literal ctx =
  choose
    ctx
    [ (fun () -> "0L")
    ; (fun () -> int64_lit (Int64.of_int (int_in ctx (-2) 5)))
    ; (fun () -> "Int64.max_int")
    ; (fun () -> "Int64.min_int")
    ; (fun () -> "(-1L)")
    ; (fun () -> int64_lit (Int64.shift_left 1L (int_in ctx 0 63)))
    ; (fun () ->
        int64_lit
          (Int64.logor
             (Int64.shift_left (Int64.of_int (draw_silent ctx.tc (integers ()))) 1)
             (Int64.of_int (int_in ctx 0 1))))
    ]
;;

let string_literal ctx =
  let n = int_in ctx 0 8 in
  p
    "%S"
    (String.init n (fun _ ->
       if draw_silent ctx.tc (booleans ~p:0.8 ())
       then Char.chr (int_in ctx (Char.code 'a') (Char.code 'e'))
       else Char.chr (int_in ctx 0 255)))
;;

let rec literal ctx ty =
  match ty with
  | Int -> int_literal ctx
  | Bool -> if draw_silent ctx.tc (booleans ()) then "true" else "false"
  | Float -> float_literal ctx
  | Char -> char_literal ctx
  | Int64 -> int64_literal ctx
  | Str -> string_literal ctx
  | List t ->
    choose
      ctx
      [ (fun () -> "[]")
      ; (fun () -> p "[%s]" (literal ctx t))
      ; (fun () -> p "[%s; %s; %s]" (literal ctx t) (literal ctx t) (literal ctx t))
      ]
  | Option t ->
    if draw_silent ctx.tc (booleans ()) then p "(Some %s)" (literal ctx t) else "None"
  | Pair (a, b) -> p "(%s, %s)" (literal ctx a) (literal ctx b)
  | Arrow (a, b) -> p "(fun (_ : %s) -> %s)" (show_ty a) (literal ctx b)
  | Float_u ->
    choose
      ctx
      [ (fun () -> "#0.")
      ; (fun () -> "#1.")
      ; (fun () -> "#0x1.8p+1")
      ; (fun () -> p "(unbox_float %s)" (float_literal ctx))
      ]
  | Named i ->
    (match ctx.decls.(i) with
     | Record fields ->
       p
         "{ %s }"
         (String.concat
            "; "
            (List.map (fun (f, t, _) -> p "%s = %s" f (literal ctx t)) fields))
     | Variant constrs ->
       let c, args = List.nth constrs (int_in ctx 0 (List.length constrs - 1)) in
       construct c (List.map (literal ctx) args))

and construct c = function
  | [] -> c
  | args -> p "(%s (%s))" c (String.concat ", " args)
;;

let gen_decl ctx =
  if on "records" && draw_silent ctx.tc (booleans ())
  then (
    let all_float = draw_silent ctx.tc (booleans ~p:0.3 ()) in
    let float = if !ox_mode && draw_silent ctx.tc (booleans ()) then Float_u else Float in
    Record
      (List.init (int_in ctx 1 4) (fun _ ->
         let f = fresh ctx "f" in
         let t =
           if all_float
           then float
           else if !ox_mode && draw_silent ctx.tc (booleans ~p:0.25 ())
           then Float_u
           else fo_ty ctx
         in
         f, t, draw_silent ctx.tc (booleans ~p:0.3 ()))))
  else (
    let poly = (not !ox_mode) && on "poly" && draw_silent ctx.tc (booleans ~p:0.35 ()) in
    let own () =
      List.init (int_in ctx 1 4) (fun _ ->
        let c = fresh ctx (if poly then "`C" else "C") in
        c, List.init (int_in ctx 0 3) (fun _ -> fo_ty ctx))
    in
    let polys =
      List.filter_map
        (fun (j, d) ->
           match d with
           | Variant ((c, _) :: _ as cs) when c.[0] = '`' -> Some (j, cs)
           | _ -> None)
        (List.mapi (fun j d -> j, d) (Array.to_list ctx.decls))
    in
    if poly && polys <> [] && draw_silent ctx.tc (booleans ())
    then (
      let j, inherited = List.nth polys (int_in ctx 0 (List.length polys - 1)) in
      poly_parent := (Array.length ctx.decls, j) :: !poly_parent;
      Variant (inherited @ own ()))
    else Variant (own ()))
;;

let small_int ctx = paren (string_of_int (int_in ctx (-2) 5))

let pattern_int ctx =
  choose
    ctx
    [ (fun () -> "0")
    ; (fun () -> small_int ctx)
    ; (fun () -> small_int ctx)
    ; (fun () -> "1")
    ; (fun () -> "(-1)")
    ; (fun () -> paren (string_of_int (int_in ctx (-100) 100)))
    ; (fun () -> paren (string_of_int (1 lsl int_in ctx 0 61)))
    ; (fun () -> "4611686018427387903")
    ]
;;

(* A pattern of type [ty] and the variables it binds; with [~binds:false] it
   binds none, as both sides of an or-pattern must agree. *)
let rec pattern ?(top = false) ctx ty depth ~binds =
  let none s = s, [] in
  let var () =
    if binds
    then (
      let x = fresh ctx "q" in
      x, [ x, ty ])
    else none "_"
  in
  let sub t = pattern ctx t (depth - 1) ~binds in
  let structural =
    match ty with
    | Int -> [ (fun () -> none (pattern_int ctx)) ]
    | Bool -> [ (fun () -> none "true"); (fun () -> none "false") ]
    | Char ->
      [ (fun () -> none (char_literal ctx))
      ; (fun () ->
          let a = int_in ctx 0 255 in
          let b = int_in ctx a (min 255 (a + int_in ctx 0 30)) in
          none (p "(%C .. %C)" (Char.chr a) (Char.chr b)))
      ; (fun () ->
          let a = int_in ctx (Char.code 'a') (Char.code 'h') in
          let b = int_in ctx a (Char.code 'h') in
          none (p "(%C .. %C)" (Char.chr a) (Char.chr b)))
      ]
    | Int64 ->
      [ (fun () ->
          none
            (choose
               ctx
               [ (fun () -> int64_lit (Int64.of_int (int_in ctx (-2) 5)))
               ; (fun () -> "0L")
               ; (fun () -> "9223372036854775807L")
               ; (fun () -> int64_lit (Int64.shift_left 1L (int_in ctx 0 62)))
               ]))
      ]
    | Str -> [ (fun () -> none (string_literal ctx)) ]
    | Float | Float_u | Arrow _ -> []
    | List t ->
      [ (fun () -> none "[]")
      ; (fun () ->
          let ph, bh = sub t in
          let pt, bt = sub ty in
          p "(%s :: %s)" ph pt, bh @ bt)
      ; (fun () ->
          let a, ba = sub t in
          let b, bb = sub t in
          p "[%s; %s]" a b, ba @ bb)
      ]
    | Option t ->
      [ (fun () -> none "None")
      ; (fun () ->
          let a, ba = sub t in
          p "(Some %s)" a, ba)
      ]
    | Pair (a, b) ->
      [ (fun () ->
          let pa, ba = sub a in
          let pb, bb = sub b in
          p "(%s, %s)" pa pb, ba @ bb)
      ]
    | Named i ->
      (match ctx.decls.(i) with
       | Record fields ->
         [ (fun () ->
             let picked =
               match List.filter (fun _ -> draw_silent ctx.tc (booleans ())) fields with
               | [] -> [ List.hd fields ]
               | picked -> picked
             in
             let parts = List.map (fun (f, t, _) -> f, sub t) picked in
             ( p
                 "{ %s_ }"
                 (String.concat
                    ""
                    (List.map (fun (f, (q, _)) -> p "%s = %s; " f q) parts))
             , List.concat_map (fun (_, (_, b)) -> b) parts ))
         ]
       | Variant constrs ->
         List.map
           (fun (c, args) () ->
              let parts = List.map sub args in
              construct c (List.map fst parts), List.concat_map snd parts)
           constrs)
  in
  let combined =
    if structural = []
    then []
    else
      [ (fun () ->
          let a, _ = pattern ctx ty (depth - 1) ~binds:false in
          let b, _ = pattern ctx ty (depth - 1) ~binds:false in
          none (p "(%s | %s)" a b))
      ]
      @
      if binds
      then
        [ (fun () ->
            let q, bq = sub ty in
            let x = fresh ctx "q" in
            p "(%s as %s)" q x, (x, ty) :: bq)
        ]
      else []
  in
  if depth <= 0
  then choose ctx [ (fun () -> none "_"); var ]
  else if structural <> [] && (top || draw_silent ctx.tc (booleans ~p:0.75 ()))
  then choose ctx (structural @ combined)
  else choose ctx [ (fun () -> none "_"); var ]
;;

(* [locals] are OxCaml stack-allocated values. They are only ever matched on
   or have an immediate field read, never returned, stored or captured. *)
type env =
  { vars : (string * ty) list
  ; locals : (string * ty) list
  ; in_try : bool
  ; effs : (string * ty * ty) list (* the effects a handler around here handles *)
  }

let bind env x t = { env with vars = (x, t) :: env.vars }
let bind_local env x t = { env with locals = (x, t) :: env.locals }
let closure env = { env with locals = [] }

(* A function that may be called after the handlers around it are gone. *)
let escape env = { (closure env) with effs = [] }

let immediate = function
  | Int | Bool -> true
  | _ -> false
;;

(* A type that [extract] finds in a variable in scope, usually, so that
   eliminators consume values the program computed rather than literals. *)
let from_env ctx env extract fallback =
  let all = env.vars @ env.locals in
  let unused = List.filter (fun (x, _) -> not (Hashtbl.mem ctx.used x)) all in
  (* Unused variables first, so that parameters get used (Frank et al.,
     "Generating Well-Typed Terms That Are Not Useless", POPL 2024). *)
  let pick vars = List.filter_map (fun (_, t) -> extract t) vars in
  match pick unused, pick all with
  | [], [] -> fallback ()
  | found, _ :: _ when found <> [] && draw_silent ctx.tc (booleans ~p:0.7 ()) ->
    List.nth found (int_in ctx 0 (List.length found - 1))
  | _, found when found <> [] && draw_silent ctx.tc (booleans ~p:0.7 ()) ->
    List.nth found (int_in ctx 0 (List.length found - 1))
  | _ -> fallback ()
;;

let env_ty ctx env extract = from_env ctx env extract (fun () -> any_ty ctx)

let elem = function
  | List s -> Some s
  | _ -> None
;;

let some_ty t = Some t

let fo_env_ty ctx env =
  from_env ctx env (fun t -> if first_order t then Some t else None) (fun () -> fo_ty ctx)
;;

let rec expr ctx env ty depth =
  span ctx (fun () ->
    let vars = List.filter (fun (_, t) -> t = ty) env.vars in
    let unused = List.filter (fun (x, _) -> not (Hashtbl.mem ctx.used x)) vars in
    let use (x, _) =
      Hashtbl.replace ctx.used x ();
      x
    in
    let leaf () =
      let i = int_in ctx 0 (List.length vars) in
      if unused <> [] && draw_silent ctx.tc (booleans ~p:0.7 ())
      then use (List.nth unused (int_in ctx 0 (List.length unused - 1)))
      else if i < List.length vars
      then use (List.nth vars i)
      else (
        let l = literal ctx ty in
        (* Sys.opaque_identity keeps the compiler from folding the value away. *)
        if ty <> Float_u && on "opaque" && draw_silent ctx.tc (booleans ~p:0.15 ())
        then p "(Sys.opaque_identity %s)" l
        else l)
    in
    let performs = List.filter (fun (_, _, r) -> r = ty) env.effs in
    if env.in_try && draw_silent ctx.tc (booleans ~p:0.15 ())
    then if ty = Float_u then "(unbox_float (raise Exit))" else "(raise Exit)"
    else if
      depth <= 0
      || (unused <> [] && draw_silent ctx.tc (booleans ~p:0.5 ()))
      || (vars <> [] && draw_silent ctx.tc (booleans ~p:0.25 ()))
    then leaf ()
    else if performs <> [] && draw_silent ctx.tc (booleans ~p:0.2 ())
    then (
      let c, a, _ = List.nth performs (int_in ctx 0 (List.length performs - 1)) in
      p "(Effect.perform (%s %s))" c (expr ctx env a (depth - 1)))
    else if env.effs <> [] && draw_silent ctx.tc (booleans ~p:0.35 ())
    then (
      let c, a, r = List.nth env.effs (int_in ctx 0 (List.length env.effs - 1)) in
      let x = fresh ctx "y" in
      p
        "(let %s : %s = Effect.perform (%s %s) in %s)"
        x
        (show_ty r)
        c
        (expr ctx env a (depth - 1))
        (expr ctx (bind env x r) ty (depth - 1)))
    else (
      let e env t = expr ctx env t (depth - 1) in
      let other =
        List.filter
          (fun (x, t) -> t <> ty && first_order t && not (Hashtbl.mem ctx.used x))
          env.vars
      in
      (* Favour the operations on [ty] over the structural forms, so values
         are computed with rather than only passed around. *)
      let p_specific =
        match ty with
        | Int | Bool | Float | Char | Int64 | Str | List _ -> 0.7
        | Option _ | Pair _ | Arrow _ | Named _ | Float_u -> 0.4
      in
      match other with
      | (x, t) :: _ when draw_silent ctx.tc (booleans ~p:0.3 ()) ->
        (* the newest unused variable of another type, so it is not left dead *)
        p "(if %s = %s then %s else %s)" (use (x, t)) (literal ctx t) (e env ty) (e env ty)
      | _ ->
      if !ox_mode && immediate ty && on "locals" && draw_silent ctx.tc (booleans ~p:0.15 ())
      then local_binding ctx env ty e
      else if draw_silent ctx.tc (booleans ~p:p_specific ())
      then choose ctx (specific ctx env ty e)
      else choose ctx (leaf :: generic ctx env ty e)))

and generic ctx env ty e =
  let fo = first_order ty in
  let value = ty <> Float_u in
  (* A scrutinee of type [s]: sometimes a local, whose components are then
     local too. *)
  let scrutinee env s =
    match List.filter (fun (_, t) -> t = s) env.locals with
    | locals when locals <> [] && draw_silent ctx.tc (booleans ~p:0.6 ()) ->
      fst (List.nth locals (int_in ctx 0 (List.length locals - 1))), true
    | _ -> e env s, false
  in
  let bind_part local env x t =
    if local && not (immediate t) then bind_local env x t else bind env x t
  in
  let rec_attr () =
    choose
      ctx
      (List.map
         (fun a () -> a)
         ([ ""; "[@inline never]"; "[@inline always]" ]
          @ if !ox_mode then [ "[@unrolled 2]"; "[@loop always]" ] else []))
  in
  let call_attr go =
    choose
      ctx
      [ (fun () -> go)
      ; (fun () -> p "(%s [@inlined never])" go)
      ; (fun () -> p "(%s [@inlined])" go)
      ]
  in
  [ (fun () -> p "(if %s then %s else %s)" (e env Bool) (e env ty) (e env ty))
  ; (fun () ->
      let s = from_env ctx env some_ty (fun () -> top_ty ctx) in
      let x = fresh ctx "x" in
      p "(let %s : %s = %s in %s)" x (show_ty s) (e env s) (e (bind env x s) ty))
  ; (fun () ->
      let s =
        from_env
          ctx
          env
          (function
            | Arrow (s, r) when r = ty -> Some s
            | _ -> None)
          (fun () -> top_ty ctx)
      in
      p "(%s %s)" (e env (Arrow (s, ty))) (e env s))
  ; (fun () ->
      let s = top_ty ctx in
      let x = fresh ctx "x" in
      p
        "((fun (%s : %s) -> %s) %s)"
        x
        (show_ty s)
        (e (bind (closure env) x s) ty)
        (e env s))
  ; (fun () ->
      let s = env_ty ctx env elem in
      let h = fresh ctx "h" in
      let t = fresh ctx "t" in
      let l, local = scrutinee env (List s) in
      p
        "(match %s with [] -> %s | %s :: %s -> %s)"
        l
        (e env ty)
        h
        t
        (e (bind_part local (bind_part local env h s) t (List s)) ty))
  ; (fun () ->
      let s =
        env_ty ctx env (function
          | Option s -> Some s
          | _ -> None)
      in
      let x = fresh ctx "x" in
      let o, local = scrutinee env (Option s) in
      p
        "(match %s with None -> %s | Some %s -> %s)"
        o
        (e env ty)
        x
        (e (bind_part local env x s) ty))
  ; (fun () ->
      let a, b =
        from_env
          ctx
          env
          (function
            | Pair (a, b) -> Some (a, b)
            | _ -> None)
          (fun () -> any_ty ctx, any_ty ctx)
      in
      let x = fresh ctx "x" in
      let y = fresh ctx "y" in
      let v, local = scrutinee env (Pair (a, b)) in
      p
        "(let (%s, %s) = %s in %s)"
        x
        y
        v
        (e (bind_part local (bind_part local env x a) y b) ty))
  ; (fun () ->
      let go = fresh ctx "go" in
      let n = fresh ctx "n" in
      let acc = fresh ctx "acc" in
      let env = closure env in
      p
        "(let%s rec %s (%s : int) (%s : %s) : %s = if %s <= 0 then %s else %s (%s - 1) \
         %s in %s (%s land 15) %s)"
        (rec_attr ())
        go
        n
        acc
        (show_ty ty)
        (show_ty ty)
        n
        acc
        go
        n
        (e (bind (bind env n Int) acc ty) ty)
        (call_attr go)
        (e env Int)
        (e env ty))
  ; (fun () ->
      let go = fresh ctx "go" in
      let n = fresh ctx "n" in
      let r = fresh ctx "r" in
      let env = closure env in
      p
        "(let%s rec %s (%s : int) : %s = if %s <= 0 then %s else (let %s : %s = %s (%s - \
         1) in %s) in %s (%s land 15))"
        (rec_attr ())
        go
        n
        (show_ty ty)
        n
        (e (bind env n Int) ty)
        r
        (show_ty ty)
        go
        n
        (e (bind (bind env n Int) r ty) ty)
        (call_attr go)
        (e env Int))
  ; (fun () ->
      if on "exits"
      then p "(try %s with Exit -> %s)" (e { env with in_try = true } ty) (e env ty)
      else p "(if %s then %s else %s)" (e env Bool) (e env ty) (e env ty))
  ; (fun () ->
      let s =
        from_env
          ctx
          env
          (function
            | Named _ as t -> Some t
            | _ -> None)
          (fun () -> from_env ctx env some_ty (fun () -> top_ty ctx))
      in
      let v, local = scrutinee env s in
      let v =
        match s with
        | Pair (a, b) when (not local) && draw_silent ctx.tc (booleans ~p:0.4 ()) ->
          p "%s, %s" (e env a) (e env b)
        | _ -> v
      in
      let arm () =
        let q, bound = pattern ~top:true ctx s 3 ~binds:true in
        let env' = List.fold_left (fun env (x, t) -> bind_part local env x t) env bound in
        let guard =
          if draw_silent ctx.tc (booleans ~p:0.3 ())
          then p " when %s" (e env' Bool)
          else ""
        in
        p "| %s%s -> %s" q guard (e env' ty)
      in
      let arms = List.init (int_in ctx 1 4) (fun _ -> arm ()) in
      let arms =
        String.concat
          " "
          (if (not local) && on "exits" && draw_silent ctx.tc (booleans ~p:0.2 ())
           then arms @ [ p "| exception Exit -> %s" (e env ty) ]
           else arms)
      in
      if draw_silent ctx.tc (booleans ~p:0.6 ())
      then (
        let x = fresh ctx "m" in
        p "(match %s with %s | %s -> %s)" v arms x (e (bind_part local env x s) ty))
      else
        (* Without a catch-all the match may fail; the handler right around it
           keeps Match_failure from meeting another exception. *)
        p "(try (match %s with %s) with Match_failure _ -> %s)" v arms (e env ty))
  ]
  @ (if not value
     then []
     else
       [ (fun () ->
           if on "opaque" then p "(Sys.opaque_identity %s)" (e env ty) else e env ty)
       ; (fun () ->
           let s =
             env_ty ctx env (function
               | Pair (a, b) when a = ty -> Some b
               | _ -> None)
           in
           p "(fst %s)" (e env (Pair (ty, s))))
       ; (fun () ->
           let s =
             env_ty ctx env (function
               | Pair (a, b) when b = ty -> Some a
               | _ -> None)
           in
           p "(snd %s)" (e env (Pair (s, ty))))
       ; (fun () ->
           let s = env_ty ctx env elem in
           let acc = fresh ctx "acc" in
           let x = fresh ctx "x" in
           p
             "(List.fold_left (fun (%s : %s) (%s : %s) -> %s) %s %s)"
             acc
             (show_ty ty)
             x
             (show_ty s)
             (e (bind (bind (closure env) acc ty) x s) ty)
             (e env ty)
             (e env (List s)))
       ])
  @ List.concat_map
      (fun (i, decl) ->
         match decl with
         | Record fields ->
           List.filter_map
             (fun (f, t, _) ->
                if t = ty
                then
                  Some
                    (fun () ->
                      (* A local record's field is local unless it is immediate. *)
                      let r =
                        if immediate t
                        then fst (scrutinee env (Named i))
                        else e env (Named i)
                      in
                      p "(%s).%s" r f)
                else None)
             fields
         | Variant _ -> [])
      (List.mapi (fun i d -> i, d) (Array.to_list ctx.decls))
  @ (if env.in_try
     then [ (fun () -> if value then "(raise Exit)" else "(unbox_float (raise Exit))") ]
     else [])
  @ (if !ox_mode && immediate ty && on "locals"
     then [ (fun () -> local_binding ctx env ty e) ]
     else [])
  @ boost (if value && on "partial" then [ (fun () -> partial ctx env ty e) ] else [])
  @ boost (if fo && ctx.effects <> [] && on "deep" then [ (fun () -> handler ctx env ty e) ] else [])
  @ boost (if fo && ctx.effects <> [] && on "shallow" then [ (fun () -> shallow ctx env ty e) ] else [])
  @ boost (if fo && ctx.effects <> [] && !use_effect_syntax && on "effect_syntax"
     then [ (fun () -> effect_match ctx env ty e) ]
     else [])
  @ boost (if value && on "lazy" then [ (fun () -> lazy_value ctx env ty e) ] else [])
  @ boost (if value && on "recvals" then rec_value ctx env ty e else [])
  @ (if on "strings" then [ (fun () -> str_match ctx env ty e) ] else [])
  @ boost (if fo && on "gadt"
     then
       [ (fun () -> p "(geval %s)" (gadt ctx env ty e 3))
       ; (fun () ->
           match ty with
           | Int | Bool ->
             p "(gbump %s %s)" (gadt ctx env ty e 2) (e env ty)
           | _ -> p "(geval %s)" (gadt ctx env ty e 2))
       ]
     else [])
  @ boost (List.filter_map
      (fun (child, parent) ->
         if Named child = ty
         then Some (fun () -> p "(%s : t%d :> t%d)" (e env (Named parent)) parent child)
         else None)
      !poly_parent)
  @ boost (if immediate ty && on "atomics" then [ (fun () -> local_atomic ctx env ty e) ] else [])
  @ (if fo then [ (fun () -> pressure ctx env ty e) ] else [])
  @ (if value && on "manyargs" then [ (fun () -> many_args ctx env ty e) ] else [])
  @ boost (if value && on "gc" then [ (fun () -> gc ctx env ty e) ] else [])
  @ boost (if value && on "exns" then [ (fun () -> local_exn ctx env ty e) ] else [])
  @
  if (not fo) || not (on "loops")
  then []
  else
    [ (fun () ->
        let r = fresh ctx "r" in
        let i = fresh ctx "i" in
        p
          "(let %s = ref %s in for %s = 0 to (%s land 7) do %s := %s done; !%s)"
          r
          (e env ty)
          i
          (e env Int)
          r
          (e (bind (bind env (p "!%s" r) ty) i Int) ty)
          r)
    ; (fun () ->
        let r = fresh ctx "r" in
        let k = fresh ctx "k" in
        let lim = fresh ctx "lim" in
        let env' = bind env (p "!%s" r) ty in
        p
          "(let %s = ref %s in let %s = ref 0 in let %s = %s land 7 in while !%s < %s && \
           %s do %s := %s; incr %s done; !%s)"
          r
          (e env ty)
          k
          lim
          (e env Int)
          k
          lim
          (e env' Bool)
          r
          (e (bind env' (p "!%s" k) Int) ty)
          k
          r)
    ; (fun () ->
        let a = fresh ctx "a" in
        let i = fresh ctx "i" in
        let get = p "%s.(%s mod Array.length %s)" a i a in
        p
          "(let %s = Array.make (1 + (%s land 7)) %s in for %s = 0 to (%s land 7) do \
           %s.((%s land max_int) mod Array.length %s) <- %s done; Array.fold_left (fun \
           (_ : %s) (x : %s) -> x) %s.(0) %s)"
          a
          (e env Int)
          (e env ty)
          i
          (e env Int)
          a
          (e (bind env i Int) Int)
          a
          (e (bind (bind env get ty) i Int) ty)
          (show_ty ty)
          (show_ty ty)
          a
          a)
    ]

(* The effects a handler handles, the type [b] of its body, whether it
   catches Exit, and the body's environment. *)
and handler_setup ctx env =
  let handled = List.filter (fun _ -> draw_silent ctx.tc (booleans ())) ctx.effects in
  let handled = if handled = [] then [ List.hd ctx.effects ] else handled in
  let b =
    from_env
      ctx
      { env with vars = List.map (fun (c, _, r) -> c, r) handled }
      some_ty
      (fun () -> fo_env_ty ctx env)
  in
  let catch = draw_silent ctx.tc (booleans ~p:0.3 ()) in
  handled, b, catch, { env with effs = handled @ env.effs; in_try = env.in_try || catch }

(* A handler arm for an effect with argument [a] and result [r]: it resumes the
   body, resumes it and then works on its result, drops the continuation, or
   (inside a try, if [discontinue]) raises Exit at the perform. *)
and arm ctx env ty e ~resume ~discontinue (_, a, r) =
  let v = fresh ctx "v" in
  let k = fresh ctx "k" in
  let henv = bind env v a in
  let res = fresh ctx "r" in
  let resumed () = resume k (e henv r) r in
  ( v
  , k
  , choose
      ctx
      ([ resumed
       ; (fun () ->
           p "(let %s : %s = %s in %s)" res (show_ty ty) (resumed ()) (e (bind henv res ty) ty))
       ; (fun () -> p "(ignore %s; %s)" k (e henv ty))
       ]
       @
       if discontinue && env.in_try
       then [ (fun () -> p "Effect.Deep.discontinue %s Exit" k) ]
       else []) )

and deep_resume k v r = p "Effect.Deep.continue %s (%s : %s)" k v (show_ty r)

(* A deep handler for some of the program's effects around a body of type [b]. *)
and handler ctx env ty e =
  let handled, b, catch, body_env = handler_setup ctx env in
  let x = fresh ctx "x" in
  let exnc =
    if catch
    then p "(function Exit -> %s | ex -> raise ex)" (e env ty)
    else "raise"
  in
  let arm ((c, _, _) as eff) =
    let v, k, body = arm ctx env ty e ~resume:deep_resume ~discontinue:true eff in
    p
      "| %s %s -> Some (fun (%s : (a, %s) Effect.Deep.continuation) -> %s)"
      c
      v
      k
      (show_ty ty)
      body
  in
  p
    "(Effect.Deep.match_with (fun () -> %s) () { Effect.Deep.retc = (fun (%s : %s) -> \
     %s); exnc = %s; effc = (fun (type a) (eff : a Effect.t) -> match eff with %s | _ \
     -> None) })"
    (let c, a, r = List.nth handled (int_in ctx 0 (List.length handled - 1)) in
     if draw_silent ctx.tc (booleans ())
     then (
       let y = fresh ctx "y" in
       p
         "(let %s : %s = Effect.perform (%s %s) in %s)"
         y
         (show_ty r)
         c
         (e body_env a)
         (e (bind body_env y r) b))
     else e body_env b)
    x
    (show_ty b)
    (e (bind env x b) ty)
    exnc
    (String.concat " " (List.map arm handled))

(* A local function with positional, labelled and optional parameters, applied
   in one go or a few arguments at a time. *)
and partial ctx env ty e =
  let f = fresh ctx "f" in
  let n = int_in ctx 1 4 in
  let env = closure env in
  let params, body_env =
    List.fold_left
      (fun (params, env) i ->
         let x = fresh ctx "a" in
         let kind =
           if i = n - 1 then `Pos else choose ctx [ (fun () -> `Pos); (fun () -> `Lab); (fun () -> `Opt) ]
         in
         let t = if kind = `Pos then top_ty ctx else any_ty ctx in
         let decl =
           match kind with
           | `Pos -> p "(%s : %s)" x (show_ty t)
           | `Lab -> p "~(%s : %s)" x (show_ty t)
           | `Opt -> p "?(%s : %s = %s)" x (show_ty t) (e env t)
         in
         params @ [ (x, t, kind, decl) ], bind env x t)
      ([], env)
      (List.init n Fun.id)
  in
  let arg ~omittable (x, t, kind, _) =
    match kind with
    | `Pos -> Some (true, paren (e env t))
    | `Lab -> Some (false, p "~%s:(%s)" x (e env t))
    | `Opt ->
      choose
        ctx
        ([ (fun () -> Some (false, p "~%s:(%s)" x (e env t)))
         ; (fun () -> Some (false, p "?%s:(Some %s)" x (e env t)))
         ; (fun () -> Some (false, p "?%s:None" x))
         ]
         @ if omittable then [ (fun () -> None) ] else [])
  in
  (* An optional argument can only be left out before a positional one. *)
  let rec args = function
    | [] -> []
    | param :: rest ->
      let omittable = List.exists (fun (_, _, k, _) -> k = `Pos) rest in
      let a = arg ~omittable param in
      Option.to_list a @ args rest
  in
  (* Labelled arguments commute; positional ones keep their order. *)
  let shuffle args =
    let moved, kept =
      List.partition (fun (pos, _) -> (not pos) && draw_silent ctx.tc (booleans ~p:0.3 ())) args
    in
    String.concat " " (List.map snd (kept @ moved))
  in
  let rec chunks = function
    | [] -> []
    | params ->
      let k = int_in ctx 1 (List.length params) in
      let here = List.filteri (fun i _ -> i < k) params in
      let rest = List.filteri (fun i _ -> i >= k) params in
      shuffle (args here) :: chunks rest
  in
  let attr = choose ctx [ (fun () -> ""); (fun () -> "[@inline never]"); (fun () -> "[@inline]") ] in
  let head =
    choose
      ctx
      [ (fun () -> f)
      ; (fun () -> p "(%s [@inlined never])" f)
      ; (fun () -> p "(Sys.opaque_identity %s)" f)
      ]
  in
  let call =
    match chunks params with
    | [] -> assert false
    | first :: rest ->
      List.fold_left
        (fun acc chunk ->
           choose
             ctx
             [ (fun () -> p "(%s %s)" acc chunk)
             ; (fun () -> p "((Sys.opaque_identity %s) %s)" acc chunk)
             ; (fun () ->
                 let q = fresh ctx "q" in
                 p "(let %s = %s in %s %s)" q acc q chunk)
             ])
        (p "(%s %s)" head first)
        rest
  in
  p
    "(let%s %s %s : %s = %s in %s)"
    attr
    f
    (String.concat " " (List.map (fun (_, _, _, d) -> d) params))
    (show_ty ty)
    (e body_env ty)
    call

(* An atomic only this expression can see, updated in a loop and then read.
   It holds an int or a bool, so compare_and_set's physical equality is the
   same on every backend. *)
and local_atomic ctx env ty e =
  let a = fresh ctx "at" in
  let i = fresh ctx "i" in
  let body = bind (bind env (p "(Atomic.get %s)" a) ty) i Int in
  let ops = List.init (int_in ctx 1 3) (fun _ -> atomic_op ctx body a ty e) in
  p
    "(let %s = Atomic.make %s in for %s = 0 to (%s land 7) do %s done; Atomic.get %s)"
    a
    (e env ty)
    i
    (e env Int)
    (String.concat "; " ops)
    a

and atomic_op ctx env a ty e =
  (* let-bound because argument evaluation order is unspecified and differs between ocamlc and ocamlopt *)
  let cas seen v =
    let x = fresh ctx "c" and y = fresh ctx "c" in
    p "(let %s = %s in let %s = %s in Atomic.compare_and_set %s %s %s)" x seen y v a x y
  in
  choose
    ctx
    ([ (fun () -> p "Atomic.set %s %s" a (e env ty))
     ; (fun () -> p "ignore (Atomic.exchange %s %s)" a (e env ty))
     ; (fun () -> p "ignore %s" (cas (p "Atomic.get %s" a) (e env ty)))
     ; (fun () -> p "ignore %s" (cas (e env ty) (e env ty)))
     ; (fun () ->
         p
           "(if %s then %s)"
           (cas (e env ty) (e env ty))
           (match ty with
            | Int -> p "Atomic.incr %s" a
            | _ -> p "Atomic.set %s (not (Atomic.get %s))" a a))
     ]
     @
     match ty with
     | Int ->
       [ (fun () -> p "ignore (Atomic.fetch_and_add %s %s)" a (e env Int))
       ; (fun () -> p "Atomic.incr %s" a)
       ; (fun () -> p "Atomic.decr %s" a)
       ]
     | _ -> [])

(* [let local_ x = ... in body]: the value is either an ordinary one, a
   [stack_] allocation, or one a helper allocates in its caller's region with
   [exclave_]. The body's type is immediate, so no local escapes through it. *)
and local_binding ctx env ty e =
  let s =
    match any_ty ctx with
    | (Int | Bool | Float | Char | Int64 | Str | Arrow _ | Float_u) as t -> Option t
    | t -> t
  in
  let x = fresh ctx "l" in
  let allocation env =
    match s with
    | Pair (a, b) -> Some (fun () -> p "(%s, %s)" (e env a) (e env b))
    | List t -> Some (fun () -> p "(%s :: %s)" (e env t) (e env s))
    | Option t -> Some (fun () -> p "(Some %s)" (e env t))
    | Named i ->
      (match ctx.decls.(i) with
       | Record fields ->
         Some
           (fun () ->
             p
               "{ %s }"
               (String.concat
                  "; "
                  (List.map (fun (f, t, _) -> p "%s = %s" f (e env t)) fields)))
       | Variant constrs ->
         (match List.filter (fun (_, args) -> args <> []) constrs with
          | [] -> None
          | constrs ->
            Some
              (fun () ->
                let c, args = List.nth constrs (int_in ctx 0 (List.length constrs - 1)) in
                construct c (List.map (e env) args))))
    | Int | Bool | Float | Char | Int64 | Str | Arrow _ | Float_u -> None
  in
  let source =
    let mk = fresh ctx "mk" in
    let z = fresh ctx "z" in
    choose
      ctx
      ((fun () -> e env s)
       ::
       (match allocation env, allocation (bind (closure env) z Int) with
        | Some here, Some there ->
          [ (fun () -> p "stack_ %s" (here ()))
          ; (fun () ->
              p
                "(let[@inline never] %s (%s : int) = exclave_ (stack_ %s) in %s %s)"
                mk
                z
                (there ())
                mk
                (e env Int))
          ]
        | _ -> []))
  in
  p "(let local_ %s : %s = %s in %s)" x (show_ty s) source (e (bind_local env x s) ty)

and specific ctx env ty e =
  let bin op t = fun () -> p "(%s %s %s)" (e env t) op (e env t) in
  let call f args = fun () -> p "(%s %s)" f (String.concat " " (List.map (e env) args)) in
  match ty with
  | Int ->
    List.map (fun op -> bin op Int) [ "+"; "-"; "*"; "land"; "lor"; "lxor" ]
    @ List.map (fun f -> call f [ Int ]) [ "abs"; "succ"; "pred"; "lnot"; "(~-)" ]
    @ List.map (fun f -> call f [ Int; Int ]) [ "min"; "max"; "sdiv"; "smod" ]
    @ [ (fun () -> p "(%s / %s)" (e env Int) (nonzero ctx))
      ; (fun () -> p "(%s mod %s)" (e env Int) (nonzero ctx))
      ; (fun () ->
          let op = choose ctx (List.map (fun o () -> o) [ "lsl"; "lsr"; "asr" ]) in
          let amount =
            if draw_silent ctx.tc (booleans ())
            then string_of_int (int_in ctx 0 62)
            else p "(%s land 31)" (e env Int)
          in
          p "(%s %s %s)" (e env Int) op amount)
      ; (fun () -> p "(List.length %s)" (e env (List (env_ty ctx env elem))))
      ; (fun () ->
          let s = fo_env_ty ctx env in
          p "(sgn (compare %s %s))" (e env s) (e env s))
      ; call "Bool.to_int" [ Bool ]
      ; call "String.length" [ Str ]
      ; call "Hashtbl.hash" [ Str ]
      ; (fun () -> p "(sgn (String.compare %s %s))" (e env Str) (e env Str))
      ; (fun () -> str_load ctx env e Int)
      ; call "Char.code" [ Char ]
      ; call "Int64.to_int" [ Int64 ]
      ; (fun () -> p "(sgn (Int64.compare %s %s))" (e env Int64) (e env Int64))
      ]
  | Bool ->
    [ call "not" [ Bool ]
    ; bin "&&" Bool
    ; bin "||" Bool
    ; call "Float.is_integer" [ Float ]
    ; call "String.equal" [ Str; Str ]
    ; call "String.contains" [ Str; Char ]
    ; (fun () -> p "(String.starts_with ~prefix:%s %s)" (e env Str) (e env Str))
    ]
    @ List.map
        (fun op () ->
           let s = fo_env_ty ctx env in
           p "(%s %s %s)" (e env s) op (e env s))
        [ "="; "<>"; "<"; "<="; ">"; ">=" ]
    @ [ (fun () ->
          let s = fo_env_ty ctx env in
          p "(List.mem %s %s)" (e env s) (e env (List s)))
      ; (fun () ->
          let s = env_ty ctx env elem in
          let x = fresh ctx "x" in
          p
            "(List.exists (fun (%s : %s) -> %s) %s)"
            x
            (show_ty s)
            (e (bind (closure env) x s) Bool)
            (e env (List s)))
      ]
  | Float ->
    List.map (fun op -> bin op Float) [ "+."; "-."; "/." ]
    (* opaque so arm64 cannot fuse the product into an fmadd, which rounds differently from ocamlc *)
    @ [ (fun () -> p "(Sys.opaque_identity (%s *. %s))" (e env Float) (e env Float)) ]
    @ List.map
        (fun f -> call f [ Float ])
        [ "sqrt"; "abs_float"; "floor"; "ceil"; "Float.trunc"; "Float.round"; "(~-.)" ]
    @ List.map
        (fun f -> call f [ Float; Float ])
        [ "Float.min"; "Float.max"; "Float.rem" ]
    @ [ call "Float.of_int" [ Int ]; call "Float.fma" [ Float; Float; Float ] ]
    @ if !ox_mode then [ call "box_float" [ Float_u ] ] else []
  | Char ->
    [ (fun () -> p "(Char.chr (%s land 255))" (e env Int))
    ; call "Char.lowercase_ascii" [ Char ]
    ; call "Char.uppercase_ascii" [ Char ]
    ; (fun () -> p "(Char.unsafe_chr (Char.code %s))" (e env Char))
    ; (fun () -> str_load ctx env e Char)
    ]
  | Int64 ->
    List.map
      (fun f -> call f [ Int64; Int64 ])
      [ "Int64.add"
      ; "Int64.sub"
      ; "Int64.mul"
      ; "Int64.logand"
      ; "Int64.logor"
      ; "Int64.logxor"
      ; "i64div"
      ; "i64rem"
      ; "Int64.min"
      ; "Int64.max"
      ]
    @ List.map
        (fun f -> call f [ Int64 ])
        [ "Int64.neg"; "Int64.abs"; "Int64.succ"; "Int64.pred"; "Int64.lognot" ]
    @ [ call "Int64.of_int" [ Int ]
      ; (fun () -> str_load ctx env e Int64)
      ; (fun () -> p "(Int64.div %s %s)" (e env Int64) (int64_lit (nonzero64 ctx)))
      ; (fun () -> p "(Int64.rem %s %s)" (e env Int64) (int64_lit (nonzero64 ctx)))
      ; (fun () ->
          let op =
            choose
              ctx
              (List.map
                 (fun o () -> o)
                 [ "Int64.shift_left"; "Int64.shift_right"; "Int64.shift_right_logical" ])
          in
          p "(%s %s (%s land 63))" op (e env Int64) (e env Int))
      ]
  | Str ->
    [ bin "^" Str
    ; call "ssub" [ Str; Int; Int ]
    ; call "string_of_int" [ Int ]
    ; call "Int64.to_string" [ Int64 ]
    ; call "String.uppercase_ascii" [ Str ]
    ; call "String.escaped" [ Str ]
    ; call "String.trim" [ Str ]
    ; call "(String.make 1)" [ Char ]
    ; (fun () -> p "(String.make (%s land 7) %s)" (e env Int) (e env Char))
    ; (fun () -> p "(String.concat %s %s)" (e env Str) (e env (List Str)))
    ; (fun () ->
        let i = fresh ctx "i" in
        p
          "(String.init (%s land 7) (fun (%s : int) -> %s))"
          (e env Int)
          i
          (e (bind (closure env) i Int) Char))
    ; (fun () ->
        let c = fresh ctx "c" in
        p
          "(String.map (fun (%s : char) -> %s) %s)"
          c
          (e (bind (closure env) c Char) Char)
          (e env Str))
    ; (fun () ->
        let b = fresh ctx "b" in
        let i = fresh ctx "i" in
        let env' = bind env i Int in
        p
          "(let %s = Bytes.of_string %s in for %s = 0 to (%s land 7) do if Bytes.length \
           %s > 0 then Bytes.unsafe_set %s ((%s land max_int) mod Bytes.length %s) %s \
           done; Bytes.to_string %s)"
          b
          (e env Str)
          i
          (e env Int)
          b
          b
          (e env' Int)
          b
          (e env' Char)
          b)
    ; (fun () ->
        let b = fresh ctx "b" in
        let i = fresh ctx "i" in
        let env' = bind env i Int in
        p
          "(let %s = Buffer.create (%s land 15) in for %s = 0 to (%s land 7) do \
           Buffer.add_string %s %s; Buffer.add_char %s %s done; Buffer.contents %s)"
          b
          (e env Int)
          i
          (e env Int)
          b
          (e env' Str)
          b
          (e env' Char)
          b)
    ; (fun () ->
        let s = fresh ctx "s" in
        let b = fresh ctx "b" in
        let off = p "((%s land max_int) mod (String.length %s + 1))" (e env Int) s in
        let store =
          choose
            ctx
            [ (fun () -> p "Bytes.set_uint8 %s %s %s" b off (e env Int))
            ; (fun () -> p "Bytes.set_int16_le %s %s %s" b off (e env Int))
            ; (fun () -> p "Bytes.set_uint16_be %s %s %s" b off (e env Int))
            ; (fun () -> p "Bytes.set_int32_le %s %s (Int32.of_int %s)" b off (e env Int))
            ; (fun () -> p "Bytes.set_int32_be %s %s (Int32.of_int %s)" b off (e env Int))
            ; (fun () -> p "Bytes.set_int64_le %s %s %s" b off (e env Int64))
            ; (fun () -> p "Bytes.set_int64_be %s %s %s" b off (e env Int64))
            ]
        in
        p
          "(let %s = %s in let %s = Bytes.of_string (%s ^ \"\\001\\002\\003\\004\\005\\006\\007\\008\") in %s; \
           Bytes.to_string %s)"
          s
          (e env Str)
          b
          s
          store
          b)
    ]
  | Float_u ->
    List.map
      (fun f -> call f [ Float_u; Float_u ])
      [ "fu_add"; "fu_sub"; "fu_mul"; "fu_div" ]
    @ [ call "fu_sqrt" [ Float_u ]; call "unbox_float" [ Float ] ]
  | List t ->
    [ (fun () -> p "(%s :: %s)" (e env t) (e env ty))
    ; (fun () -> p "[%s; %s]" (e env t) (e env t))
    ; call "List.rev" [ ty ]
    ; bin "@" ty
    ; (fun () ->
        let s = env_ty ctx env elem in
        let x = fresh ctx "x" in
        p
          "(List.map (fun (%s : %s) -> %s) %s)"
          x
          (show_ty s)
          (e (bind (closure env) x s) t)
          (e env (List s)))
    ; (fun () ->
        let x = fresh ctx "x" in
        p
          "(List.filter (fun (%s : %s) -> %s) %s)"
          x
          (show_ty t)
          (e (bind (closure env) x t) Bool)
          (e env ty))
    ; (fun () ->
        let i = fresh ctx "i" in
        p
          "(List.init (%s land 7) (fun (%s : int) -> %s))"
          (e env Int)
          i
          (e (bind (closure env) i Int) t))
    ]
    @ (if first_order t then [ (fun () -> p "(List.sort compare %s)" (e env ty)) ] else [])
    @ boost (if on "tmc" then [ (fun () -> tmc ctx env t e) ] else [])
  | Option t ->
    [ (fun () -> p "(Some %s)" (e env t))
    ; (fun () ->
        let x = fresh ctx "x" in
        p
          "(List.find_opt (fun (%s : %s) -> %s) %s)"
          x
          (show_ty t)
          (e (bind (closure env) x t) Bool)
          (e env (List t)))
    ; (fun () ->
        let s =
          env_ty ctx env (function
            | Option s -> Some s
            | _ -> None)
        in
        let x = fresh ctx "x" in
        p
          "(Option.map (fun (%s : %s) -> %s) %s)"
          x
          (show_ty s)
          (e (bind (closure env) x s) t)
          (e env (Option s)))
    ]
  | Pair (a, b) -> [ (fun () -> p "(%s, %s)" (e env a) (e env b)) ]
  | Arrow (a, b) ->
    [ (fun () ->
        let x = fresh ctx "x" in
        p "(fun (%s : %s) -> %s)" x (show_ty a) (e (bind (escape env) x a) b))
    ]
  | Named i ->
    (match ctx.decls.(i) with
     | Variant constrs ->
       List.map (fun (c, args) () -> construct c (List.map (e env) args)) constrs
       @ List.concat_map
           (fun (child, parent) ->
              if child = i
              then (
                let up () = p "(%s : t%d :> t%d)" (e env (Named parent)) parent child in
                [ up; up; up ])
              else [])
           !poly_parent
     | Record fields ->
       let build () =
         String.concat "; " (List.map (fun (f, t, _) -> p "%s = %s" f (e env t)) fields)
       in
       [ (fun () -> p "{ %s }" (build ()))
       ; (fun () ->
           let f, t, _ = List.nth fields (int_in ctx 0 (List.length fields - 1)) in
           p "{ %s with %s = %s }" (e env ty) f (e env t))
       ]
       @
       (* Only a record the expression itself allocates is mutated, and only
          before it is returned, so no other expression can observe it. *)
       (match List.filter (fun (_, _, m) -> m) fields with
         | [] -> []
         | mutables ->
           [ (fun () ->
               let r = fresh ctx "r" in
               let i = fresh ctx "i" in
               let f, t, _ =
                 List.nth mutables (int_in ctx 0 (List.length mutables - 1))
               in
               let env' =
                 List.fold_left
                   (fun env (g, gt, _) -> bind env (p "%s.%s" r g) gt)
                   (bind env i Int)
                   fields
               in
               p
                 "(let %s = { %s } in for %s = 0 to (%s land 7) do %s.%s <- %s done; %s)"
                 r
                 (build ())
                 i
                 (e env Int)
                 r
                 f
                 (e env' t)
                 r)
           ]))

(* A read of a string at an offset its padding keeps in bounds. *)
and str_load ctx env e ty =
  let s = fresh ctx "s" in
  let off = p "((%s land max_int) mod (String.length %s + 1))" (e env Int) s in
  let padded = p "(%s ^ \"\\001\\002\\003\\004\\005\\006\\007\\008\")" s in
  let load =
    match ty with
    | Int ->
      choose
        ctx
        [ (fun () -> p "String.get_uint8 %s %s" padded off)
        ; (fun () -> p "String.get_int8 %s %s" padded off)
        ; (fun () -> p "String.get_int16_le %s %s" padded off)
        ; (fun () -> p "String.get_uint16_be %s %s" padded off)
        ; (fun () -> p "Int32.to_int (String.get_int32_le %s %s)" padded off)
        ; (fun () -> p "Int32.to_int (String.get_int32_be %s %s)" padded off)
        ; (fun () ->
            p
              "(if String.length %s = 0 then 0 else Char.code (String.unsafe_get %s ((%s \
               land max_int) mod String.length %s)))"
              s
              s
              (e env Int)
              s)
        ]
    | Int64 ->
      choose
        ctx
        [ (fun () -> p "String.get_int64_le %s %s" padded off)
        ; (fun () -> p "String.get_int64_be %s %s" padded off)
        ]
    | _ ->
      choose
        ctx
        [ (fun () -> p "%s.[%s]" padded off)
        ; (fun () -> p "String.unsafe_get %s %s" padded off)
        ]
  in
  p "(let %s = %s in %s)" s (e env Str) load

(* Many ints and floats live across an opaque call, so some must be spilled. *)
and pressure ctx env ty e =
  let n = int_in ctx 8 20 in
  let vars =
    List.init n (fun _ ->
      let t = if draw_silent ctx.tc (booleans ()) then Int else Float in
      fresh ctx (if t = Int then "i" else "f"), t)
  in
  let k = fresh ctx "k" in
  let env' = List.fold_left (fun env (x, t) -> bind env x t) (bind env k Int) vars in
  let sum t op zero =
    match List.filter (fun (_, t') -> t' = t) vars with
    | [] -> zero
    | xs -> String.concat op (List.map fst xs)
  in
  p
    "(%s let %s = (Sys.opaque_identity succ) %s in if %s = %s || %s > float_of_int %s \
     then %s else %s)"
    (String.concat "" (List.map (fun (x, t) -> p "let %s : %s = %s in " x (show_ty t) (e env t)) vars))
    k
    (e env Int)
    (sum Int " + " "0")
    k
    (sum Float " +. " "0.")
    k
    (e env' ty)
    (e env' ty)

(* More int and float parameters than arm64 passes in registers, in a
   function that calls itself in tail position. Arguments are let-bound
   because their evaluation order is unspecified. *)
and many_args ctx env ty e =
  let go = fresh ctx "go" in
  let n = fresh ctx "n" in
  let params =
    List.init (int_in ctx 9 14) (fun _ ->
      ( fresh ctx "a"
      , choose ctx [ (fun () -> Int); (fun () -> Float); (fun () -> fo_ty ctx) ] ))
  in
  let env = closure env in
  let inner = List.fold_left (fun env (x, t) -> bind env x t) (bind env n Int) params in
  (* recursive calls mostly pass parameters along, which keeps programs small *)
  let arg env t =
    let same = List.filter (fun (_, t') -> t' = t) params in
    if env != inner || same = [] || draw_silent ctx.tc (booleans ~p:0.3 ())
    then e env t
    else fst (List.nth same (int_in ctx 0 (List.length same - 1)))
  in
  let args env first =
    let bound = List.map (fun (_, t) -> fresh ctx "b", t, arg env t) params in
    ( String.concat "" (List.map (fun (y, t, v) -> p "let %s : %s = %s in " y (show_ty t) v) bound)
    , String.concat " " (first :: List.map (fun (y, _, _) -> y) bound) )
  in
  let m = fresh ctx "m" in
  let outer_lets, outer = args env m in
  let inner_lets, again = args inner (p "(%s - 1)" n) in
  p
    "(let%s rec %s (%s : int) %s : %s = if %s <= 0 then %s else (%s%s %s) in let %s = %s \
     land 7 in %s%s %s)"
    (choose ctx [ (fun () -> ""); (fun () -> "[@inline never]") ])
    go
    n
    (String.concat " " (List.map (fun (x, t) -> p "(%s : %s)" x (show_ty t)) params))
    (show_ty ty)
    n
    (e inner ty)
    inner_lets
    go
    again
    m
    (e env Int)
    outer_lets
    (choose ctx [ (fun () -> go); (fun () -> p "(%s [@inlined never])" go) ])
    outer

(* Fresh allocations live across a collection, then used: the collector must
   find each one wherever the backend keeps it. *)
and gc ctx env ty e =
  let vars = List.init (int_in ctx 2 6) (fun _ -> fresh ctx "g", fo_ty ctx) in
  p
    "(%s%s; %s)"
    (String.concat "" (List.map (fun (x, t) -> p "let %s : %s = %s in " x (show_ty t) (e env t)) vars))
    (choose
       ctx
       [ (fun () -> "Gc.minor ()")
       ; (fun () -> "Gc.minor ()")
       ; (fun () -> "Gc.full_major ()")
       ; (fun () -> "Gc.compact ()")
       ])
    (e (List.fold_left (fun env (x, t) -> bind env x t) env vars) ty)

(* A local exception carrying a value, raised from a function and caught by
   its caller. *)
and local_exn ctx env ty e =
  let x = fresh ctx "X" in
  let th = fresh ctx "th" in
  let b = fresh ctx "b" in
  let v = fresh ctx "v" in
  let cb = fresh ctx "c" in
  let cv = fresh ctx "c" in
  let s = fo_env_ty ctx env in
  let env = closure env in
  p
    "(let exception %s of %s in let%s %s (%s : bool) (%s : %s) : %s = if %s then %s (%s \
     %s) else %s in let %s = %s in let %s = %s in try %s %s %s with %s %s -> %s)"
    x
    (show_ty s)
    (choose ctx [ (fun () -> ""); (fun () -> "[@inline never]") ])
    th
    b
    v
    (show_ty s)
    (show_ty ty)
    b
    (choose ctx [ (fun () -> "raise"); (fun () -> "raise_notrace") ])
    x
    v
    (e (bind (bind env b Bool) v s) ty)
    cb
    (e env Bool)
    cv
    (e env s)
    th
    cb
    cv
    x
    v
    (e (bind env v s) ty)

(* A deferred computation, forced where its value is used. *)
and lazy_value ctx env ty e =
  let s = from_env ctx env (fun t -> if t = Float_u then None else Some t) (fun () -> any_ty ctx) in
  let l = fresh ctx "lz" in
  let x = fresh ctx "x" in
  (* An effect could leave a lazy mid-force for good, so none happens inside. *)
  let body = e { (escape env) with in_try = env.in_try } s in
  choose
    ctx
    [ (fun () ->
        p
          "(let %s = lazy %s in %s)"
          l
          body
          (e (bind env (p "(Lazy.force %s)" l) s) ty))
    ; (fun () -> p "(match lazy %s with lazy %s -> %s)" body x (e (bind env x s) ty))
    ; (fun () -> p "(Lazy.force (Lazy.from_fun (fun () -> %s)))" (e { (escape env) with in_try = env.in_try } ty))
    ]

(* Recursive value definitions: a cyclic list, and lazies, tuples and
   functions that refer to each other. *)
and rec_value ctx env ty e =
  let f = fresh ctx "f" in
  let n = fresh ctx "n" in
  let body env = e (bind (closure env) n Int) ty in
  [ (fun () ->
      let l = fresh ctx "lz" in
      (* as in [lazy_value], no effect happens inside the lazy *)
      p
        "(let rec %s : %s Lazy.t = lazy %s and %s : int -> %s = fun %s -> if %s <= 0 \
         then Lazy.force %s else %s (%s - 1) in %s (%s land 7))"
        l
        (show_ty ty)
        (e { (escape env) with in_try = env.in_try } ty)
        f
        (show_ty ty)
        n
        n
        l
        f
        n
        f
        (e env Int))
  ; (fun () ->
      let v = fresh ctx "v" in
      let pr = fresh ctx "pr" in
      p
        "(let %s : %s = %s in let rec %s = (%s, %s) and %s : int -> %s = fun %s -> if %s \
         <= 0 then snd %s else (fst %s) (%s - 1) in (fst %s) (%s land 7))"
        v
        (show_ty ty)
        (e env ty)
        pr
        f
        v
        f
        (show_ty ty)
        n
        n
        pr
        pr
        n
        pr
        (e env Int))
  ; (fun () ->
      let g = fresh ctx "g" in
      p
        "(let rec %s (%s : int) : %s = if %s <= 0 then %s else %s (%s - 1) and %s (%s : \
         int) : %s = if %s <= 0 then %s else %s (%s - 1) in %s (%s land 15))"
        f
        n
        (show_ty ty)
        n
        (body env)
        g
        n
        g
        n
        (show_ty ty)
        n
        (body env)
        f
        n
        f
        (e env Int))
  ]
  @
  match ty with
  | List t ->
    [ (fun () ->
        let a = fresh ctx "a" in
        let b = fresh ctx "b" in
        let c = fresh ctx "c" in
        p
          "(let %s : %s = %s in let %s : %s = %s in let rec %s = %s :: %s :: %s in take \
           (%s land 15) %s)"
          a
          (show_ty t)
          (e env t)
          b
          (show_ty t)
          (e env t)
          c
          a
          b
          c
          (e env Int)
          c)
    ]
  | _ -> []

(* A match on literals that share prefixes around the 8-byte word, for the
   compiler's string-match code. *)
and str_match ctx env ty e =
  let letter () = Char.chr (int_in ctx (Char.code 'a') (Char.code 'c')) in
  let base = String.init (int_in ctx 5 10) (fun _ -> letter ()) in
  let n = String.length base in
  let variant () =
    choose
      ctx
      [ (fun () -> base)
      ; (fun () -> String.sub base 0 (int_in ctx 0 n))
      ; (fun () -> base ^ String.init (int_in ctx 1 3) (fun _ -> letter ()))
      ; (fun () -> String.sub base 0 (n - 1) ^ "z")
      ]
  in
  let lits = List.sort_uniq compare (List.init (int_in ctx 1 5) (fun _ -> variant ())) in
  let s =
    choose
      ctx
      [ (fun () -> e env Str)
      ; (fun () -> p "%S" (variant ()))
      ; (fun () -> p "(%S ^ %s)" (String.sub base 0 (int_in ctx 0 n)) (e env Str))
      ]
  in
  p
    "(match %s with %s | _ -> %s)"
    s
    (String.concat " " (List.map (fun l -> p "| %S -> %s" l (e env ty)) lits))
    (e env ty)

(* A [@tail_mod_cons] list builder, its recursive call under a constructor. *)
and tmc ctx env t e =
  let go = fresh ctx "go" in
  let n = fresh ctx "n" in
  let x = fresh ctx "x" in
  let env' = bind (closure env) n Int in
  let call = p "%s (%s - 1)" go n in
  let cons () = p "(let %s : %s = %s in %s :: %s)" x (show_ty t) (e env' t) x call in
  let step =
    choose
      ctx
      [ cons
      ; (fun () -> p "(if %s then %s else %s)" (e env' Bool) (cons ()) call)
      ; (fun () ->
          let h = fresh ctx "h" in
          p "(match %s with [] -> %s | %s :: _ -> %s :: %s)" (e env' (List t)) call h h call)
      ; (fun () ->
          let y = fresh ctx "y" in
          p
            "(let %s : %s = %s in let %s : %s = %s in %s :: %s :: %s)"
            x
            (show_ty t)
            (e env' t)
            y
            (show_ty t)
            (e env' t)
            x
            y
            call)
      ]
  in
  p
    "(let[@tail_mod_cons] rec %s (%s : int) : %s = if %s <= 0 then %s else %s in %s (%s \
     land 15))"
    go
    n
    (show_ty (List t))
    n
    (e env' (List t))
    step
    go
    (e env Int)

(* A term of the prelude's GADT that evaluates to a value of [ty]. *)
and gadt ctx env ty e depth =
  let sub t = gadt ctx env t e (depth - 1) in
  let lit () = p "(GLit %s)" (e env ty) in
  if depth <= 0
  then lit ()
  else
    choose
      ctx
      ([ lit; (fun () -> p "(GIf (%s, %s, %s))" (sub Bool) (sub ty) (sub ty))
       ; (fun () -> p "(GFst %s)" (sub (Pair (ty, fo_env_ty ctx env))))
       ; (fun () -> p "(GSnd %s)" (sub (Pair (fo_env_ty ctx env, ty))))
       ]
       @
       match ty with
       | Int ->
         [ (fun () -> p "(GInt %s)" (e env Int))
         ; (fun () -> p "(GAdd (%s, %s))" (sub Int) (sub Int))
         ]
       | Bool ->
         [ (fun () -> p "(GBool %s)" (e env Bool))
         ; (fun () -> p "(GLe (%s, %s))" (sub Int) (sub Int))
         ]
       | Pair (a, b) -> [ (fun () -> p "(GPair (%s, %s))" (sub a) (sub b)) ]
       | _ -> [])

(* A shallow handler: [go] resumes the fiber under the same handler each time. *)
and shallow ctx env ty e =
  let handled, b, catch, body_env = handler_setup ctx env in
  let go = fresh ctx "go" in
  let tv = fresh ctx "c" in
  let x = fresh ctx "x" in
  let exnc =
    if catch then p "(function Exit -> %s | ex -> raise ex)" (e env ty) else "raise"
  in
  let arm ((c, _, _) as eff) =
    let resume k v _ = p "%s %s %s" go k v in
    let v, k, body = arm ctx env ty e ~resume ~discontinue:false eff in
    p
      "| %s %s -> Some (fun (%s : (a, %s) Effect.Shallow.continuation) -> %s)"
      c
      v
      k
      (show_ty b)
      body
  in
  p
    "(let rec %s : type %s. (%s, %s) Effect.Shallow.continuation -> %s -> %s = fun k v \
     -> Effect.Shallow.continue_with k v { Effect.Shallow.retc = (fun (%s : %s) -> %s); \
     exnc = %s; effc = (fun (type a) (eff : a Effect.t) -> match eff with %s | _ -> \
     None) } in %s (Effect.Shallow.fiber (fun () -> %s)) ())"
    go
    tv
    tv
    (show_ty b)
    tv
    (show_ty ty)
    x
    (show_ty b)
    (e (bind env x b) ty)
    exnc
    (String.concat " " (List.map arm handled))
    go
    (e body_env b)

(* OCaml 5.3's [match ... with effect E v, k -> ...]. *)
and effect_match ctx env ty e =
  let handled, b, catch, body_env = handler_setup ctx env in
  let x = fresh ctx "x" in
  let arm ((c, _, _) as eff) =
    let v, k, body = arm ctx env ty e ~resume:deep_resume ~discontinue:true eff in
    p "| effect %s %s, %s -> %s" c v k body
  in
  p
    "(match %s with | %s -> %s %s%s)"
    (e body_env b)
    x
    (e (bind env x b) ty)
    (String.concat " " (List.map arm handled))
    (if catch then p " | exception Exit -> %s" (e env ty) else "")

and nonzero64 ctx =
  choose
    ctx
    [ (fun () -> 1L)
    ; (fun () -> -1L)
    ; (fun () -> Int64.shift_left 1L (int_in ctx 0 62))
    ; (fun () -> Int64.of_int (int_in ctx 2 1000))
    ; (fun () -> Int64.min_int)
    ; (fun () -> Int64.of_int (-int_in ctx 2 1000))
    ]

and nonzero ctx =
  choose
    ctx
    [ (fun () -> "1")
    ; (fun () -> "(-1)")
    ; (fun () -> "2")
    ; (fun () -> paren (string_of_int (1 lsl int_in ctx 0 61)))
    ; (fun () ->
        let n = int_in ctx (-1000) 1000 in
        paren (string_of_int (if n = 0 then 7 else n)))
    ; (fun () -> "max_int")
    ; (fun () -> "min_int")
    ; (fun () ->
        let n = draw_silent ctx.tc (integers ()) in
        paren (string_of_int (if n = 0 then 3 else n)))
    ]
;;

let module_boundary = "(* module boundary *)\n"

let program ?(ox = false) ?(effect_syntax = false) tc =
  let ctx = { tc; next = 0; decls = [||]; effects = []; used = Hashtbl.create 64 } in
  ox_mode := ox;
  poly_parent := [];
  features := List.filter (fun _ -> draw_silent tc (booleans ())) all_features;
  use_effect_syntax := effect_syntax;
  mixed := [||];
  let buf = Buffer.create 1024 in
  Buffer.add_string buf (p "(* features: %s *)\n" (String.concat " " !features));
  Buffer.add_string buf (prelude ());
  for i = 0 to int_in ctx 0 4 - 1 do
    let decl = gen_decl ctx in
    ctx.decls <- Array.append ctx.decls [| decl |];
    mixed
    := Array.append
         !mixed
         [| (match decl with
             | Record fields -> List.exists (fun (_, t, _) -> has_float_u t) fields
             | Variant constrs ->
               List.exists (fun (_, args) -> List.exists has_float_u args) constrs)
         |];
    let parent =
      Option.map
        (fun j ->
           match ctx.decls.(j) with
           | Variant cs -> j, cs
           | Record _ -> assert false)
        (List.assoc_opt i !poly_parent)
    in
    Buffer.add_string buf (declaration ?parent i decl)
  done;
  let depth = int_in ctx 2 7 in
  let count = int_in ctx 1 6 in
  if (not ox) && on "effects"
  then
    ctx.effects
    <- List.init (int_in ctx 1 3) (fun _ ->
         let c = fresh ctx "E" in
         let base () = choose ctx [ (fun () -> Int); (fun () -> Bool); (fun () -> fo_ty ctx) ] in
         let a = base () in
         let r = base () in
         Buffer.add_string buf (p "type _ Effect.t += %s : %s -> %s Effect.t\n" c (show_ty a) (show_ty r));
         c, a, r);
  let env = ref { vars = []; locals = []; in_try = false; effs = [] } in
  let globals =
    List.init (if on "atomics" then int_in ctx 0 2 else 0) (fun _ ->
      let g = fresh ctx "g" in
      let ty = if draw_silent ctx.tc (booleans ()) then Bool else Int in
      Buffer.add_string
        buf
        (p "let %s : %s Atomic.t = Atomic.make %s\n" g (show_ty ty) (literal ctx ty));
      env := bind !env (p "(Atomic.get %s)" g) ty;
      g, ty)
  in
  let statement () =
    match globals with
    | [] -> ()
    | _ ->
      let g, ty = List.nth globals (int_in ctx 0 (List.length globals - 1)) in
      let e env t = expr ctx env t (depth - 1) in
      Buffer.add_string
        buf
        (p
           "let () = %s\nlet () = print_endline (%s (Atomic.get %s))\n"
           (atomic_op ctx !env g ty e)
           (printer ty)
           g)
  in
  let boundary = if on "modules" then int_in ctx 1 count else 0 in
  for i = 1 to count do
    if i = boundary then Buffer.add_string buf module_boundary;
    if draw_silent ctx.tc (booleans ~p:0.4 ()) then statement ();
    let ty =
      match !poly_parent with
      | (child, _) :: _ when draw_silent ctx.tc (booleans ()) -> Named child
      | _ -> fo_ty ctx
    in
    let v = fresh ctx "v" in
    Buffer.add_string
      buf
      (p
         "let %s : %s = %s\nlet () = print_endline (%s %s)\n"
         v
         (show_ty ty)
         (expr ctx !env ty depth)
         (printer ty)
         v);
    env := bind !env v ty
  done;
  Buffer.contents buf
;;
