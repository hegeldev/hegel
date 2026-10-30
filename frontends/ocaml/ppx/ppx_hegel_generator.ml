(** PPX deriver for [@@deriving hegel_generator].

    Reads OCaml type declarations annotated with [@@deriving hegel_generator]
    and synthesizes a generator value. The type [t] derives [hegel_generator]
    and any other type [foo] derives [hegel_generator_foo].

    Generated code resolves every type constructor by name with the same
    convention. [int] becomes [hegel_generator_int], [M.t] becomes
    [M.hegel_generator], and a parameterized type applies the arguments
    ([int list] becomes [hegel_generator_list hegel_generator_int]).

    Supported type declarations:
    - Records: generates all fields, then constructs the record
    - Variants: picks a constructor via [sampled_from] then generates args
    - Type aliases: delegates to the generator for the aliased type
    - Tuples inside any of the above
    - Recursive types. A variant whose constructors refer to the type itself
      is generated with [Hegel.Generators.recursive]: the constructors that do
      not refer to it are the leaves, the others the branches. Any other
      recursive (or mutually recursive) group is generated through
      [Hegel.Generators.deferred] forward references.

    The deriver also emits [sexp_of_<t>] through ppx_sexp_conv's expander,
    and every derived generator is printable, with [sexp_of_<t>] as its
    printer. A derived generator prints what it draws as OCaml syntax, as it
    draws it: it opens the record, constructor or tuple on the printer, and
    each part prints itself inside it ([Hegel.Generators.draw_and_print]).
    Every part's generator must therefore be printable: a [[@hegel.generator]]
    override or a type alias whose generator cannot print is a type error. *)

open Ppxlib

let generator_name = Expansion_helpers.(mangle (Prefix "hegel_generator"))

(* [Expansion_helpers.mangle_lid] raises [Invalid_argument] on [Lapply]. *)
let mangle_lid ~loc = function
  | Lident name -> Lident (generator_name name)
  | Ldot (path, name) -> Ldot (path, generator_name name)
  | Lapply _ ->
    Location.raise_errorf
      ~loc
      "ppx_hegel_generator: functor application types not supported"
;;

let generator_override_attribute =
  Attribute.declare
    "hegel.generator"
    Attribute.Context.core_type
    Ast_pattern.(single_expr_payload __)
    Fun.id
;;

let do_not_generate_attribute =
  Attribute.declare
    "hegel.do_not_generate"
    Attribute.Context.constructor_declaration
    Ast_pattern.(pstr nil)
    ()
;;

(** [with_mode ~portable e] is [(e [@mode portable])] when [portable], else
    [e]. ppx_template then selects the portable instance of [e]. *)
let with_mode ~portable (e : expression) : expression =
  if portable
  then (
    let loc = e.pexp_loc in
    let attr =
      Ast_builder.Default.attribute
        ~loc
        ~name:(Ast_builder.Default.Located.mk ~loc "mode")
        ~payload:(PStr [ Ast_builder.Default.pstr_eval ~loc [%expr portable] [] ])
    in
    { e with pexp_attributes = e.pexp_attributes @ [ attr ] })
  else e
;;

let internal ~loc name =
  Ast_builder.Default.evar ~loc ("Hegel.Generators.Ppx_internal." ^ name)
;;

let out ~loc name = Ast_builder.Default.evar ~loc ("Hegel.Printer.Out." ^ name)

(** [composite ~portable ~loc ~label ~sexp_of make_body] is a derived
    generator drawing with [fun tc printer -> make_body tc printer], whose
    body draws the value and prints it to [printer], inside a span tagged
    [label], and printable with [sexp_of]. *)
let composite ~portable ~loc ~label ~sexp_of make_body =
  let tc = gen_symbol ~prefix:"_hegel_tc" () in
  let printer = gen_symbol ~prefix:"_hegel_printer" () in
  [%expr
    [%e with_mode ~portable (internal ~loc "composite_with_label")]
      ~label:[%e label]
      ~sexp_of:[%e sexp_of]
      (fun
          [%p Ast_builder.Default.pvar ~loc tc]
           [%p Ast_builder.Default.pvar ~loc printer]
         -> [%e make_body tc printer])]
;;

(** [in_group ~loc ~printer ?indent open_ close body] is [body], run with
    [printer] rebound to the inside of the group [open_ … close]. *)
let in_group ~loc ~printer ?indent open_ close body =
  let indent =
    Option.map (fun n -> Labelled "indent", Ast_builder.Default.eint ~loc n) indent
  in
  Ast_builder.Default.pexp_apply
    ~loc
    (out ~loc "group")
    ((Nolabel, Ast_builder.Default.evar ~loc printer)
     :: (Option.to_list indent
         @ [ Nolabel, Ast_builder.Default.estring ~loc open_
           ; Nolabel, Ast_builder.Default.estring ~loc close
           ; Nolabel, [%expr fun [%p Ast_builder.Default.pvar ~loc printer] -> [%e body]]
           ]))
;;

(** [print_text ~loc ~printer s then_] prints the text [s], then evaluates
    [then_]. *)
let print_text ~loc ~printer s then_ =
  [%expr
    [%e out ~loc "text"]
      [%e Ast_builder.Default.evar ~loc printer]
      [%e Ast_builder.Default.estring ~loc s];
    [%e then_]]
;;

(** [sexp_of_type_name ~loc name] is the [sexp_of] converter the deriver
    emits for the type [name]. *)
let sexp_of_type_name ~loc name = Ast_builder.Default.evar ~loc ("sexp_of_" ^ name)

let rec generator_expr_of_core_type ~portable (ct : core_type) : expression =
  let loc = ct.ptyp_loc in
  match Attribute.get generator_override_attribute ct with
  | Some override -> override
  | None ->
    (match ct.ptyp_desc with
     | Ptyp_constr ({ txt = lid; _ }, args) ->
       let gen_ident =
         Ast_builder.Default.(pexp_ident ~loc (Located.mk ~loc (mangle_lid ~loc lid)))
       in
       (match args with
        | [] -> gen_ident
        | args ->
          Ast_builder.Default.pexp_apply
            ~loc
            (with_mode ~portable gen_ident)
            (List.map
               (fun arg -> Nolabel, generator_expr_of_core_type ~portable arg)
               args))
     | _ ->
       (match Ppx_compat.extract_tuple_types ct with
        | Some components ->
          composite
            ~portable
            ~loc
            ~label:[%expr Hegel.Generators.Ppx_internal.Labels.fixed_dict]
            ~sexp_of:(Ppx_compat.sexp_of_core_type ct)
            (fun tc printer ->
               drawn_tuple
                 ~portable
                 ~loc
                 ~tc
                 ~printer
                 components
                 (Ast_builder.Default.pexp_tuple ~loc))
        | None ->
          Location.raise_errorf
            ~loc
            "ppx_hegel_generator: unsupported type in [@@deriving hegel_generator]"))

(** [draw_all ~portable ~loc ~tc ~printer ~before types k] draws one value
    of each type in order, printing each to [printer] after what [before i]
    prints ([i] counting from 0), and continues with [k values]. *)
and draw_all ~portable ~loc ~tc ~printer ~before (types : core_type list) k =
  let named = List.map (fun ct -> gen_symbol ~prefix:"_hegel_value" (), ct) types in
  let values = List.map (fun (v, _) -> Ast_builder.Default.evar ~loc v) named in
  List.fold_right
    (fun (i, (v, ct)) acc ->
       before
         i
         [%expr
           let [%p Ast_builder.Default.pvar ~loc v] =
             Hegel.Generators.draw_and_print
               [%e Ast_builder.Default.evar ~loc tc]
               [%e Ast_builder.Default.evar ~loc printer]
               [%e generator_expr_of_core_type ~portable ct]
           in
           [%e acc]])
    (List.mapi (fun i x -> i, x) named)
    (k values)

(** [separated ~loc ~printer sep i then_] prints the separator [sep] before
    every part but the first, then evaluates [then_]. *)
and separated ~loc ~printer sep i then_ =
  if i = 0
  then then_
  else
    [%expr
      [%e out ~loc "separator"]
        [%e Ast_builder.Default.evar ~loc printer]
        [%e Ast_builder.Default.estring ~loc sep];
      [%e then_]]

(** [drawn_tuple ~portable ~loc ~tc ~printer types k] draws a tuple of the
    [types], printing it as [(a, b, …)], and continues with [k values]. *)
and drawn_tuple ~portable ~loc ~tc ~printer types k =
  in_group
    ~loc
    ~printer
    "("
    ")"
    (draw_all ~portable ~loc ~tc ~printer ~before:(separated ~loc ~printer ",") types k)
;;

(** [drawn_record ~portable ~loc ~tc ~printer ~wrap labels] draws the fields
    in declaration order, printing the record [{ x = …; y = … }], and is
    [wrap] of the record literal. [wrap] lets an inline-record constructor
    enclose the literal. *)
let drawn_record ~portable ~loc ~tc ~printer ~wrap (labels : label_declaration list)
  : expression
  =
  if List.is_empty labels
  then Location.raise_errorf ~loc "ppx_hegel_generator: empty record types not supported";
  let names = List.map (fun (ld : label_declaration) -> ld.pld_name.txt) labels in
  in_group
    ~loc
    ~printer
    ~indent:2
    "{"
    " }"
    [%expr
      [%e out ~loc "breakable"] [%e Ast_builder.Default.evar ~loc printer] " ";
      [%e
        draw_all
          ~portable
          ~loc
          ~tc
          ~printer
          ~before:(fun i then_ ->
            separated
              ~loc
              ~printer
              ";"
              i
              (print_text ~loc ~printer (List.nth names i ^ " = ") then_))
          (List.map (fun (ld : label_declaration) -> ld.pld_type) labels)
          (fun values ->
             wrap
               (Ast_builder.Default.pexp_record
                  ~loc
                  (List.map2
                     (fun field value ->
                        Ast_builder.Default.Located.lident ~loc field, value)
                     names
                     values)
                  None))]]
;;

(** [type_label ~loc ~type_name kind] is the label expression for the derived
    generator of [type_name]: the structural [kind] label combined with one from
    the type's name, so two derived types of the same shape stay distinct. *)
let type_label ~loc ~type_name kind =
  let kind =
    Ast_builder.Default.evar ~loc ("Hegel.Generators.Ppx_internal.Labels." ^ kind)
  in
  [%expr
    Hegel.Generators.Ppx_internal.Labels.combine
      [ [%e kind]
      ; Hegel.Generators.Ppx_internal.Labels.from_name
          [%e Ast_builder.Default.estring ~loc type_name]
      ]]
;;

(** [drawn_constructor ~loc ~printer name body] prints the constructor
    application [name arg], parenthesized in argument position, where [body]
    draws and prints the argument with [printer] rebound to argument
    position. *)
let drawn_constructor ~loc ~printer name body =
  let p = Ast_builder.Default.evar ~loc printer in
  [%expr
    [%e out ~loc "parens_if_argument"]
      [%e p]
      (fun [%p Ast_builder.Default.pvar ~loc printer] ->
         [%e
           in_group
             ~loc
             ~printer
             ~indent:2
             ""
             ""
             (print_text
                ~loc
                ~printer
                name
                [%expr
                  [%e out ~loc "breakable"] [%e p] " ";
                  let [%p Ast_builder.Default.pvar ~loc printer] =
                    [%e out ~loc "argument"] [%e p]
                  in
                  [%e body]])])]
;;

let generator_of_data_variant
      ~portable
      ~loc
      ~type_name
      (constrs : constructor_declaration list)
  : expression
  =
  composite
    ~portable
    ~loc
    ~label:(type_label ~loc ~type_name "enum_variant")
    ~sexp_of:(sexp_of_type_name ~loc type_name)
    (fun tc printer ->
       let variant_idx = gen_symbol ~prefix:"_variant_idx" () in
       let n = List.length constrs in
       let index_options = List.init n (fun i -> Ast_builder.Default.eint ~loc i) in
       let match_arms =
         List.mapi
           (fun i (cd : constructor_declaration) ->
              let name = cd.pcd_name.txt in
              let constr_lid = Ast_builder.Default.Located.lident ~loc name in
              let construct arg =
                Ast_builder.Default.pexp_construct ~loc constr_lid arg
              in
              let rhs =
                match Ppx_compat.extract_constr_args cd.pcd_args with
                | Tuple [] -> print_text ~loc ~printer name (construct None)
                | Tuple [ ct ] ->
                  drawn_constructor
                    ~loc
                    ~printer
                    name
                    (draw_all
                       ~portable
                       ~loc
                       ~tc
                       ~printer
                       ~before:(fun _ then_ -> then_)
                       [ ct ]
                       (fun values -> construct (Some (List.hd values))))
                | Tuple cts ->
                  drawn_constructor
                    ~loc
                    ~printer
                    name
                    (drawn_tuple ~portable ~loc ~tc ~printer cts (fun values ->
                       construct (Some (Ast_builder.Default.pexp_tuple ~loc values))))
                | Record labels ->
                  drawn_constructor
                    ~loc
                    ~printer
                    name
                    (drawn_record
                       ~portable
                       ~loc
                       ~tc
                       ~printer
                       ~wrap:(fun record -> construct (Some record))
                       labels)
              in
              Ast_builder.Default.case
                ~lhs:(Ast_builder.Default.pint ~loc i)
                ~guard:None
                ~rhs)
           constrs
       in
       let catch_all =
         Ast_builder.Default.case
           ~lhs:[%pat? _]
           ~guard:None
           ~rhs:[%expr failwith "ppx_hegel_generator: unreachable variant index"]
       in
       [%expr
         let [%p Ast_builder.Default.pvar ~loc variant_idx] =
           Hegel.draw_silent
             [%e Ast_builder.Default.evar ~loc tc]
             ([%e with_mode ~portable [%expr Hegel.Generators.sampled_from]]
                [%e Ast_builder.Default.elist ~loc index_options])
         in
         [%e
           Ast_builder.Default.pexp_match
             ~loc
             (Ast_builder.Default.evar ~loc variant_idx)
             (match_arms @ [ catch_all ])]])
;;

(** [generated_constructors ~loc constrs] is [constrs] without the ones marked
    [[@hegel.do_not_generate]]. *)
let generated_constructors ~loc (constrs : constructor_declaration list) =
  if List.is_empty constrs
  then Location.raise_errorf ~loc "ppx_hegel_generator: empty variant types not supported";
  let generated =
    List.filter
      (fun cd -> Option.is_none (Attribute.get do_not_generate_attribute cd))
      constrs
  in
  if List.is_empty generated
  then
    Location.raise_errorf
      ~loc
      "ppx_hegel_generator: all constructors are marked [@hegel.do_not_generate]";
  generated
;;

(** [variant_of_constructors ~portable ~loc ~type_name ~all generated] is the
    generator of the constructors [generated], a subset of the type's
    constructors [all]. *)
let variant_of_constructors
      ~portable
      ~loc
      ~type_name
      ~(all : constructor_declaration list)
      (generated : constructor_declaration list)
  : expression
  =
  let is_nullary (cd : constructor_declaration) =
    match Ppx_compat.extract_constr_args cd.pcd_args with
    | Tuple [] -> true
    | _ -> false
  in
  if List.for_all is_nullary generated
  then (
    let construct (cd : constructor_declaration) =
      Ast_builder.Default.pexp_construct
        ~loc
        (Ast_builder.Default.Located.lident ~loc cd.pcd_name.txt)
        None
    in
    let print_case (cd : constructor_declaration) =
      Ast_builder.Default.case
        ~lhs:
          (Ast_builder.Default.ppat_construct
             ~loc
             (Ast_builder.Default.Located.lident ~loc cd.pcd_name.txt)
             None)
        ~guard:None
        ~rhs:
          [%expr Hegel.Printer.text [%e Ast_builder.Default.estring ~loc cd.pcd_name.txt]]
    in
    let excluded_case =
      if List.length generated = List.length all
      then []
      else
        [ Ast_builder.Default.case
            ~lhs:[%pat? _]
            ~guard:None
            ~rhs:[%expr Hegel.Printer.opaque]
        ]
    in
    [%expr
      [%e with_mode ~portable (internal ~loc "with_printers")]
        ~sexp_of:[%e sexp_of_type_name ~loc type_name]
        ~print:
          [%e
            Ppx_compat.function_of_cases
              ~loc
              (List.map print_case generated @ excluded_case)]
        ([%e with_mode ~portable [%expr Hegel.Generators.sampled_from]]
           [%e Ast_builder.Default.elist ~loc (List.map construct generated)])])
  else generator_of_data_variant ~portable ~loc ~type_name generated
;;

let generator_of_variant ~portable ~loc ~type_name constrs =
  variant_of_constructors
    ~portable
    ~loc
    ~type_name
    ~all:constrs
    (generated_constructors ~loc constrs)
;;

(** [mentions names ct] is [true] when the core type [ct] refers to one of the
    types [names] declared in the same group, other than through a
    [[@hegel.generator]] override. *)
let rec mentions names (ct : core_type) =
  match Attribute.get generator_override_attribute ct with
  | Some _ -> false
  | None ->
    (match ct.ptyp_desc with
     | Ptyp_constr ({ txt = Lident name; _ }, args) ->
       List.mem name names || List.exists (mentions names) args
     | Ptyp_constr (_, args) -> List.exists (mentions names) args
     | _ ->
       (match Ppx_compat.extract_tuple_types ct with
        | Some components -> List.exists (mentions names) components
        | None -> false))
;;

let constructor_mentions names (cd : constructor_declaration) =
  match Ppx_compat.extract_constr_args cd.pcd_args with
  | Tuple cts -> List.exists (mentions names) cts
  | Record labels ->
    List.exists (fun (ld : label_declaration) -> mentions names ld.pld_type) labels
;;

(** [declaration_mentions names td] is [true] when [td] refers to one of the
    types [names]. *)
let declaration_mentions names (td : type_declaration) =
  match td.ptype_kind, td.ptype_manifest with
  | Ptype_record labels, _ ->
    List.exists (fun (ld : label_declaration) -> mentions names ld.pld_type) labels
  | Ptype_variant constrs, _ -> List.exists (constructor_mentions names) constrs
  | _, Some ct -> mentions names ct
  | _, None -> false
;;

(** [excluded_constructor_uses ~loc td] returns one [let _ = ...] item per
    excluded constructor, each constructing it once. Without these the compiler
    reports a constructor never used to build values warning. *)
let excluded_constructor_uses ~loc (td : type_declaration) : structure =
  match td.ptype_kind with
  | Ptype_variant constrs ->
    List.filter_map
      (fun (cd : constructor_declaration) ->
         match Attribute.get do_not_generate_attribute cd with
         | None -> None
         | Some () ->
           let constr_lid = Ast_builder.Default.Located.lident ~loc cd.pcd_name.txt in
           let construct arg = Ast_builder.Default.pexp_construct ~loc constr_lid arg in
           let evar name = Ast_builder.Default.evar ~loc name in
           (* turn an excluded constructor C(a,b,...) into fun a -> fun b ->
              ... -> C(a, b, ...) *)
           let to_curried_lam names body =
             List.fold_right
               (fun name acc ->
                  [%expr fun [%p Ast_builder.Default.pvar ~loc name] -> [%e acc]])
               names
               body
           in
           let build =
             match Ppx_compat.extract_constr_args cd.pcd_args with
             | Tuple [] -> construct None
             | Tuple cts ->
               let names = List.map (fun _ -> gen_symbol ~prefix:"_arg" ()) cts in
               let arg =
                 match names with
                 | [ name ] -> evar name
                 | _ -> Ast_builder.Default.pexp_tuple ~loc (List.map evar names)
               in
               to_curried_lam names (construct (Some arg))
             | Record labels ->
               let named =
                 List.map
                   (fun (ld : label_declaration) ->
                      ld.pld_name.txt, gen_symbol ~prefix:"_arg" ())
                   labels
               in
               let record =
                 Ast_builder.Default.pexp_record
                   ~loc
                   (List.map
                      (fun (field, vname) ->
                         Ast_builder.Default.Located.lident ~loc field, evar vname)
                      named)
                   None
               in
               to_curried_lam (List.map snd named) (construct (Some record))
           in
           Some [%stri let _ = [%e build]])
      constrs
  | _ -> []
;;

(** [opaque_excluded_args td] adds [[@sexp.opaque]] to the argument types of
    [[@hegel.do_not_generate]] constructors. *)
let opaque_excluded_args (td : type_declaration) : type_declaration =
  match td.ptype_kind with
  | Ptype_variant constrs ->
    let opaque ct =
      let loc = ct.ptyp_loc in
      let attr =
        Ast_builder.Default.attribute
          ~loc
          ~name:(Ast_builder.Default.Located.mk ~loc "sexp.opaque")
          ~payload:(PStr [])
      in
      { ct with ptyp_attributes = ct.ptyp_attributes @ [ attr ] }
    in
    let constrs =
      List.map
        (fun (cd : constructor_declaration) ->
           match Attribute.get do_not_generate_attribute cd with
           | Some () ->
             { cd with pcd_args = Ppx_compat.map_constr_arg_types opaque cd.pcd_args }
           | None -> cd)
        constrs
    in
    { td with ptype_kind = Ptype_variant constrs }
  | _ -> td
;;

(** The main deriver. Emits the [sexp_of_<t>] definitions first (via
    ppx_sexp_conv's expander), then one printable generator per type, which
    prints what it draws as OCaml syntax. *)
let generate_impl
      ~ctxt
      ((rec_flag, type_decls) : rec_flag * type_declaration list)
      portable
  : structure
  =
  let loc = Expansion_context.Deriver.derived_item_loc ctxt in
  let path = Code_path.to_string_path (Expansion_context.Deriver.code_path ctxt) in
  let sexp_items =
    Ppx_compat.sexp_of_str_type_decl
      ~loc
      ~path
      (rec_flag, List.map opaque_excluded_args type_decls)
  in
  let names = List.map (fun (td : type_declaration) -> td.ptype_name.txt) type_decls in
  let generator_expr (td : type_declaration) =
    let type_name = td.ptype_name.txt in
    match td.ptype_kind, td.ptype_manifest with
    | Ptype_record labels, _ ->
      composite
        ~portable
        ~loc
        ~label:(type_label ~loc ~type_name "fixed_dict")
        ~sexp_of:(sexp_of_type_name ~loc type_name)
        (fun tc printer -> drawn_record ~portable ~loc ~tc ~printer ~wrap:Fun.id labels)
    | Ptype_variant constrs, _ -> generator_of_variant ~portable ~loc ~type_name constrs
    | Ptype_abstract, Some ct ->
      [%expr
        ([%e generator_expr_of_core_type ~portable ct]
         : (_, Hegel.Generators.printable) Hegel.Generators.generator)]
    | Ptype_abstract, None ->
      Location.raise_errorf
        ~loc
        "ppx_hegel_generator: abstract types without manifest not supported"
    | _, _ ->
      Location.raise_errorf
        ~loc
        "ppx_hegel_generator: unsupported type kind in [@@deriving hegel_generator]"
  in
  let pvar name = Ast_builder.Default.pvar ~loc name in
  let evar name = Ast_builder.Default.evar ~loc name in
  let generator_items =
    match type_decls with
    | _ when not (List.exists (declaration_mentions names) type_decls) ->
      List.map
        (fun (td : type_declaration) ->
           [%stri
             let [%p pvar (generator_name td.ptype_name.txt)] = [%e generator_expr td]])
        type_decls
    | [ ({ ptype_kind = Ptype_variant constrs; _ } as td) ]
      when let generated = generated_constructors ~loc constrs in
           List.exists (constructor_mentions names) generated
           && not (List.for_all (constructor_mentions names) generated) ->
      let type_name = td.ptype_name.txt in
      let gen_name = generator_name type_name in
      let generated = generated_constructors ~loc constrs in
      let leaves, branches =
        List.partition (fun cd -> not (constructor_mentions names cd)) generated
      in
      let variant = variant_of_constructors ~portable ~loc ~type_name ~all:constrs in
      [ [%stri
          let [%p pvar gen_name] =
            [%e with_mode ~portable [%expr Hegel.Generators.recursive]]
              [%e variant leaves]
              (fun [%p pvar gen_name] -> [%e variant branches])
          ;;]
      ]
    | _ ->
      if portable
      then
        Location.raise_errorf
          ~loc
          "ppx_hegel_generator: ~portable supports a recursive type only when it is a \
           variant with constructors that do not refer to it";
      let define name = "_hegel_define_" ^ name in
      let tuple_pattern pats =
        match pats with
        | [ pat ] -> pat
        | pats -> Ast_builder.Default.ppat_tuple ~loc pats
      in
      let tuple_expr exprs =
        match exprs with
        | [ e ] -> e
        | exprs -> Ast_builder.Default.pexp_tuple ~loc exprs
      in
      let gen_names = List.map generator_name names in
      let definitions =
        List.fold_right
          (fun (td : type_declaration) acc ->
             [%expr
               [%e evar (define td.ptype_name.txt)] [%e generator_expr td];
               [%e acc]])
          type_decls
          (tuple_expr (List.map evar gen_names))
      in
      let body =
        List.fold_right
          (fun name acc ->
             [%expr
               let [%p pvar (generator_name name)], [%p pvar (define name)] =
                 Hegel.Generators.deferred ()
               in
               [%e acc]])
          names
          definitions
      in
      [ [%stri let [%p tuple_pattern (List.map pvar gen_names)] = [%e body]] ]
  in
  sexp_items
  @ List.concat_map (excluded_constructor_uses ~loc) type_decls
  @ generator_items
;;

let generate_intf
      ~ctxt:_
      ((_rec_flag, type_decls) : rec_flag * type_declaration list)
      portable
  : signature_item list
  =
  List.map
    (fun (td : type_declaration) ->
       let loc = td.ptype_loc in
       (match td.ptype_params with
        | [] -> ()
        | _ :: _ ->
          Location.raise_errorf
            ~loc
            "ppx_hegel_generator: parameterized types not supported");
       let gen_name = generator_name td.ptype_name.txt in
       let type_constr =
         Ast_builder.Default.(ptyp_constr ~loc (Located.lident ~loc td.ptype_name.txt) [])
       in
       let gen_type =
         [%type:
           ([%t type_constr], Hegel.Generators.printable) Hegel.Generators.generator]
       in
       let vd =
         Ast_builder.Default.value_description
           ~loc
           ~name:(Ast_builder.Default.Located.mk ~loc gen_name)
           ~type_:gen_type
           ~prim:[]
       in
       Ast_builder.Default.psig_value
         ~loc
         (if portable then Ppx_compat.portable_value_description vd else vd))
    type_decls
;;

let args () = Deriving.Args.(empty +> flag "portable")

let _deriver =
  Deriving.add
    "hegel_generator"
    ~str_type_decl:(Deriving.Generator.V2.make (args ()) generate_impl)
    ~sig_type_decl:(Deriving.Generator.V2.make (args ()) generate_intf)
;;
