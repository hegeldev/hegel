namespace Hegel.FSharp

open System
open System.Collections.Concurrent
open System.Collections.Generic
open System.Reflection
open Microsoft.FSharp.Reflection
open Hegel

/// The generators standing for the recursive types being built, and what the plans applied to
/// them have built so far, so a plan reached along several paths builds one generator.
type internal Selves(known: (Type * IGenerator) list) =
    member val Built = Dictionary<int, IGenerator>()

    member _.Find(t: Type) : IGenerator =
        known |> List.find (fun (self, _) -> self = t) |> snd

    member _.With(t: Type, generator: IGenerator) = Selves((t, generator) :: known)

type internal Plan = Selves -> IGenerator

type internal Scope =
    {
        Selves: Type list
        Empties: Type list
    }

exception internal NoBaseCase of Type

/// What one derivation has already worked out: the plan, or the missing base case, of each type
/// in each scope.
type internal Memo() =
    member val Plans = Dictionary<nativeint list * nativeint list * Type, Result<Plan, Type>>(HashIdentity.Structural)

    member this.Share(plan: Plan) : Plan =
        let id = this.Plans.Count

        fun selves ->
            match selves.Built.TryGetValue id with
            | true, built -> built
            | false, _ ->
                let built = plan selves
                selves.Built[id] <- built
                built

type internal Plans =
    static member Fixed(generator: IGenerator) : Plan = fun _ -> generator

    static member Self(t: Type) : Plan = fun selves -> selves.Find t

    static member Constant<'T>(value: obj) : Plan =
        Plans.Fixed(Generators.Just<'T>(unbox value))

    static member ListPlan<'T>(inner: Plan) : Plan =
        fun selves -> ListGen<'T>(inner selves :?> Generator<'T>, 0, Native.Abi.Unbounded, None)

    static member ArrayPlan<'T>(inner: Plan) : Plan =
        fun selves -> ArrayGen<'T>(inner selves :?> Generator<'T>, 0, Native.Abi.Unbounded, None)

    static member SetPlan<'T when 'T: comparison>(inner: Plan) : Plan =
        fun selves -> SetGen<'T>(inner selves :?> Generator<'T>, 0, Native.Abi.Unbounded)

    static member MapPlan<'Key, 'Value when 'Key: comparison>(keys: Plan, values: Plan) : Plan =
        fun selves ->
            MapGen<'Key, 'Value>(
                keys selves :?> Generator<'Key>,
                values selves :?> Generator<'Value>,
                0,
                Native.Abi.Unbounded
            )

    static member OptionPlan<'T>(inner: Plan) : Plan =
        fun selves -> OptionGen<'T>(inner selves :?> Generator<'T>)

    static member ValueOptionPlan<'T>(inner: Plan) : Plan =
        fun selves -> ValueOptionGen<'T>(inner selves :?> Generator<'T>)

    static member TuplePlan<'T>(elements: Plan[]) : Plan =
        let construct = FSharpValue.PreComputeTupleConstructor typeof<'T>
        let openText = if typeof<'T>.IsValueType then "struct (" else "("

        fun selves ->
            TupleGen<'T>(
                elements |> Array.map (fun element -> element selves),
                (fun values -> construct values :?> 'T),
                openText
            )

    static member RecordPlan<'T>(fields: (string * Plan)[]) : Plan =
        fun selves ->
            RecordGen<'T>(
                fields
                |> Array.map (fun (name, plan) -> name, Lazy<IGenerator>.CreateFromValue(plan selves))
                |> Map.ofArray
            )

    static member UnionPlan<'T>(cases: (UnionCaseInfo * Plan[])[]) : Plan =
        fun selves ->
            UnionGen<'T>(
                cases
                |> Array.map (fun (case, fields) -> case, fields |> Array.map (fun field -> field selves))
            )

    static member RecursivePlan<'T>(t: Type, baseCase: Plan, full: Plan) : Plan =
        fun selves ->
            Generators.Recursive(
                baseCase selves :?> Generator<'T>,
                fun self -> full (selves.With(t, self)) :?> Generator<'T>
            )

/// Derives generators from types: the F# shapes here, everything else through
/// <c>Generators.ForType</c>.
module internal Auto =
    let private invoke (name: string) (typeArgs: Type[]) (args: obj[]) : Plan =
        let method =
            typeof<Plans>.GetMethod(name, BindingFlags.Static ||| BindingFlags.Public ||| BindingFlags.NonPublic)

        method.MakeGenericMethod(typeArgs).Invoke(null, args) :?> Plan

    let private definition (t: Type) =
        if t.IsGenericType then t.GetGenericTypeDefinition() else t

    let private children (t: Type) : Type seq =
        if t.IsArray then
            Seq.singleton (t.GetElementType())
        elif FSharpType.IsTuple t then
            FSharpType.GetTupleElements t :> seq<_>
        elif FSharpType.IsRecord(t, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic) then
            FSharpType.GetRecordFields(t, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic)
            |> Seq.map (fun field -> field.PropertyType)
        elif FSharpType.IsUnion(t, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic) then
            FSharpType.GetUnionCases(t, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic)
            |> Seq.collect (fun case -> case.GetFields() |> Seq.map (fun field -> field.PropertyType))
        elif t.IsGenericType then
            t.GetGenericArguments() :> seq<_>
        else
            Seq.empty

    /// Whether a value of type <paramref name="t"/> can contain one of the <paramref name="targets"/>,
    /// looking through the types the scope already has a generator for.
    let private mentions (scope: Scope) (targets: Type list) (t: Type) : bool =
        let visited = HashSet<Type>()

        let rec go (t: Type) =
            if List.contains t targets then
                true
            elif List.contains t scope.Selves || not (visited.Add t) then
                false
            else
                children t |> Seq.exists go

        go t

    let private noBaseCase (t: Type) =
        HegelUsageException(
            "Cannot derive a generator for "
            + t.FullName
            + ": it refers to itself with no base case (a union case, or a field under an option, list, array, set or map, that does not contain another "
            + Repr.typeName t
            + "), so every value would be infinite. Add one, or build the generator with Gen.recursive"
        )

    let private empty (t: Type) =
        t.GetProperty("Empty", BindingFlags.Static ||| BindingFlags.Public ||| BindingFlags.NonPublic).GetValue null

    let private handles (types: Type list) =
        types |> List.map (fun t -> t.TypeHandle.Value) |> List.sort

    let rec private derive (memo: Memo) (scope: Scope) (t: Type) : Plan =
        let key = handles scope.Selves, handles scope.Empties, t

        let result =
            match memo.Plans.TryGetValue key with
            | true, known -> known
            | false, _ ->
                let derived =
                    try
                        Ok(memo.Share(deriveNew memo scope t))
                    with NoBaseCase cycle ->
                        Error cycle

                memo.Plans[key] <- derived
                derived

        match result with
        | Ok plan -> plan
        | Error cycle -> raise (NoBaseCase cycle)

    and private deriveNew (memo: Memo) (scope: Scope) (t: Type) : Plan =
        let emptiable (name: string) (typeArgs: Type[]) (inners: Type[]) (empty: unit -> obj) =
            if inners |> Array.exists (mentions scope scope.Empties) then
                invoke "Constant" [| t |] [| empty () |]
            else
                invoke name typeArgs (inners |> Array.map (fun inner -> box (derive memo scope inner)))

        let args = if t.IsGenericType then t.GetGenericArguments() else [||]

        if List.contains t scope.Selves then
            Plans.Self t
        elif List.contains t scope.Empties then
            raise (NoBaseCase t)
        elif t = typeof<unit> then
            Plans.Fixed(Generators.Just(()))
        elif t.IsArray && t.GetArrayRank() = 1 then
            let element = t.GetElementType()
            emptiable "ArrayPlan" [| element |] [| element |] (fun () -> Array.CreateInstance(element, 0))
        elif definition t = typedefof<list<_>> then
            emptiable "ListPlan" args args (fun () -> t.GetProperty("Empty").GetValue null)
        elif definition t = typedefof<option<_>> then
            emptiable "OptionPlan" args args (fun () -> null)
        elif definition t = typedefof<voption<_>> then
            emptiable "ValueOptionPlan" args args (fun () -> Activator.CreateInstance t)
        elif definition t = typedefof<Set<_>> then
            emptiable "SetPlan" args args (fun () -> empty t)
        elif definition t = typedefof<Map<_, _>> then
            emptiable "MapPlan" args args (fun () -> empty t)
        elif FSharpType.IsTuple t then
            invoke "TuplePlan" [| t |] [| box (FSharpType.GetTupleElements t |> Array.map (derive memo scope)) |]
        elif FSharpType.IsRecord(t, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic) then
            let fields =
                FSharpType.GetRecordFields(t, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic)

            let plan (scope: Scope) =
                invoke
                    "RecordPlan"
                    [| t |]
                    [|
                        box (
                            fields
                            |> Array.map (fun field -> field.Name, derive memo scope field.PropertyType)
                        )
                    |]

            if fields |> Array.exists (fun field -> mentions scope [ t ] field.PropertyType) then
                recursive
                    t
                    (plan
                        { scope with
                            Empties = t :: scope.Empties
                        })
                    (plan
                        { scope with
                            Selves = t :: scope.Selves
                        })
            else
                plan scope
        elif FSharpType.IsUnion(t, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic) then
            let cases =
                FSharpType.GetUnionCases(t, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic)

            let fieldTypes (case: UnionCaseInfo) =
                case.GetFields() |> Array.map (fun field -> field.PropertyType)

            let plan (scope: Scope) =
                let buildable =
                    cases
                    |> Array.choose (fun case ->
                        try
                            Some(case, fieldTypes case |> Array.map (derive memo scope))
                        with NoBaseCase _ ->
                            None)

                if buildable.Length = 0 then
                    raise (NoBaseCase t)

                invoke "UnionPlan" [| t |] [| box buildable |]

            if
                cases
                |> Array.exists (fun case -> fieldTypes case |> Array.exists (mentions scope [ t ]))
            then
                recursive
                    t
                    (plan
                        { scope with
                            Empties = t :: scope.Empties
                        })
                    (plan
                        { scope with
                            Selves = t :: scope.Selves
                        })
            else
                plan scope
        else
            Plans.Fixed(Derive.FromType t)

    and private recursive (t: Type) (baseCase: Plan) (full: Plan) : Plan =
        invoke "RecursivePlan" [| t |] [| box t; box baseCase; box full |]

    let private cache = ConcurrentDictionary<Type, IGenerator>()

    /// The generator derived for <paramref name="t"/>, built once per type.
    let forType (t: Type) : IGenerator =
        cache.GetOrAdd(
            t,
            fun t ->
                try
                    derive (Memo()) { Selves = []; Empties = [] } t (Selves [])
                with NoBaseCase cycle ->
                    raise (noBaseCase cycle)
        )
