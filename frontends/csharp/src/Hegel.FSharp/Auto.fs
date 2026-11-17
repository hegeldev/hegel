namespace Hegel.FSharp

open System
open System.Collections.Concurrent
open System.Collections.Generic
open System.Reflection
open Microsoft.FSharp.Reflection
open Hegel

exception internal NoBaseCase of Type

/// The engine recursion a derived recursive value is being drawn under, and the depth of the
/// value now being drawn.
[<AllowNullLiteral>]
type internal Frame(recursion: nativeint) =
    member _.Recursion = recursion
    member val Depth = 0UL with get, set

[<AbstractClass; Sealed>]
type internal Frames =
    [<ThreadStatic; DefaultValue>]
    static val mutable private current: Frame

    static member Current
        with get () = Frames.current
        and set (frame: Frame) = Frames.current <- frame

type internal INode =
    abstract Tie: leaf: IGenerator * branch: IGenerator -> unit

/// The generator of one recursive type: drawn on its own it starts an engine recursion, and
/// drawn inside another node's value it is one more position of that recursion, where the engine
/// decides between the type's base case and its full form. One node per type serves every
/// reference to it, so mutually recursive types share one recursion and one leaf budget.
type internal Node<'T>() =
    inherit PrintableGenerator<'T>()

    let label = Label.Combine(Label.Recursive, Label.Of typeof<'T>.FullName)
    let mutable leaf: Generator<'T> = null
    let mutable branch: Generator<'T> = null

    interface INode with
        member _.Tie(leafCase, branchCase) =
            leaf <- leafCase :?> Generator<'T>
            branch <- branchCase :?> Generator<'T>

    member private _.Position(tc: TestCase, printer: Printer, frame: Frame) : 'T =
        let depth = frame.Depth

        if tc.RecursionBranch(frame.Recursion, depth) then
            frame.Depth <- depth + 1UL

            try
                Components.Draw(tc, branch, printer)
            finally
                frame.Depth <- depth
        elif tc.RecursionLeaf frame.Recursion then
            Components.Draw(tc, leaf, printer)
        else
            raise (RecursionOverBudget())

    member internal this.Draw(tc: TestCase, printer: Printer) : 'T =
        match Frames.Current with
        | null ->
            Recursion.Draw(
                tc,
                printer,
                RecursiveGenerator<'T>.DefaultMaxDepth,
                RecursiveGenerator<'T>.DefaultMaxLeaves,
                fun recursion ->
                    let frame = Frame recursion
                    Frames.Current <- frame

                    try
                        this.Position(tc, printer, frame)
                    finally
                        Frames.Current <- null
            )
        | frame -> this.Position(tc, printer, frame)

    override _.Label = label

    override this.DoDraw tc = this.Draw(tc, null)

    override this.DoDrawAndPrint(tc, printer) = this.Draw(tc, printer)

type internal Shapes =
    static member Constant<'T>(value: obj) : IGenerator = Generators.Just<'T>(unbox value)

    static member ListOf<'T>(inner: IGenerator) : IGenerator =
        ListGen<'T>(inner :?> Generator<'T>, 0, Native.Abi.Unbounded, None)

    static member ArrayOf<'T>(inner: IGenerator) : IGenerator =
        ArrayGen<'T>(inner :?> Generator<'T>, 0, Native.Abi.Unbounded, None)

    static member SetOf<'T when 'T: comparison>(inner: IGenerator) : IGenerator =
        SetGen<'T>(inner :?> Generator<'T>, 0, Native.Abi.Unbounded)

    static member MapOf<'Key, 'Value when 'Key: comparison>(keys: IGenerator, values: IGenerator) : IGenerator =
        MapGen<'Key, 'Value>(keys :?> Generator<'Key>, values :?> Generator<'Value>, 0, Native.Abi.Unbounded)

    static member OptionOf<'T>(inner: IGenerator) : IGenerator = OptionGen<'T>(inner :?> Generator<'T>)

    static member ValueOptionOf<'T>(inner: IGenerator) : IGenerator =
        ValueOptionGen<'T>(inner :?> Generator<'T>)

    static member TupleOf<'T>(elements: IGenerator[]) : IGenerator =
        let construct = FSharpValue.PreComputeTupleConstructor typeof<'T>
        let openText = if typeof<'T>.IsValueType then "struct (" else "("
        TupleGen<'T>(elements, (fun values -> construct values :?> 'T), openText)

    static member RecordOf<'T>(fields: (string * IGenerator)[]) : IGenerator =
        RecordGen<'T>(
            fields
            |> Array.map (fun (name, generator) -> name, Lazy<IGenerator>.CreateFromValue generator)
            |> Map.ofArray
        )

    static member UnionOf<'T>(cases: (UnionCaseInfo * IGenerator[])[]) : IGenerator = UnionGen<'T>(cases)

    static member NodeOf<'T>() : IGenerator = Node<'T>()

/// Derives generators from types: the F# shapes here, everything else through
/// <c>Generators.ForType</c>.
module internal Auto =
    let private flags = BindingFlags.Public ||| BindingFlags.NonPublic

    let private shape (name: string) (typeArgs: Type[]) (args: obj[]) : IGenerator =
        let method = typeof<Shapes>.GetMethod(name, BindingFlags.Static ||| flags)
        method.MakeGenericMethod(typeArgs).Invoke(null, args) :?> IGenerator

    let private definition (t: Type) =
        if t.IsGenericType then t.GetGenericTypeDefinition() else t

    /// The types whose values hold none of their element type when empty.
    let private emptiable (t: Type) =
        (t.IsArray && t.GetArrayRank() = 1)
        || List.contains
            (definition t)
            [
                typedefof<list<_>>
                typedefof<option<_>>
                typedefof<voption<_>>
                typedefof<Set<_>>
                typedefof<Map<_, _>>
            ]

    let private empty (t: Type) : obj =
        if t.IsArray then
            Array.CreateInstance(t.GetElementType(), 0)
        elif definition t = typedefof<option<_>> then
            null
        elif definition t = typedefof<voption<_>> then
            Activator.CreateInstance t
        else
            t.GetProperty("Empty", BindingFlags.Static ||| flags).GetValue null

    let private fieldTypes (case: UnionCaseInfo) =
        case.GetFields() |> Array.map (fun field -> field.PropertyType)

    let private children (t: Type) : Type seq =
        if t.IsArray then
            Seq.singleton (t.GetElementType())
        elif emptiable t then
            t.GetGenericArguments() :> seq<_>
        elif FSharpType.IsTuple t then
            FSharpType.GetTupleElements t :> seq<_>
        elif FSharpType.IsRecord(t, flags) then
            FSharpType.GetRecordFields(t, flags)
            |> Seq.map (fun field -> field.PropertyType)
        elif FSharpType.IsUnion(t, flags) then
            FSharpType.GetUnionCases(t, flags) |> Seq.collect fieldTypes
        elif t.IsGenericType then
            t.GetGenericArguments() :> seq<_>
        else
            Seq.empty

    let private reaches (target: Type) (from: Type) : bool =
        let visited = HashSet<Type>()

        let rec go (t: Type) =
            t = target || (visited.Add t && children t |> Seq.exists go)

        go from

    let private noBaseCase (t: Type) =
        HegelUsageException(
            "Cannot derive a generator for "
            + t.FullName
            + ": it refers to itself with no base case (a union case, or a field under an option, list, array, set or map, that does not contain another "
            + Repr.typeName t
            + "), so every value would be infinite. Add one, or build the generator with Gen.recursive"
        )

    /// The generator of <paramref name="entry"/>. The records and unions that can contain
    /// themselves each get one <c>Node</c>, tied to a base case and a full form: the full form
    /// refers to the nodes, and the base case to no node at all, a collection or option that
    /// could hold one being left empty and a field of such a type taking that type's base case.
    let private derive (entry: Type) : IGenerator =
        let reachable = HashSet<Type>()

        let rec visit (t: Type) =
            if reachable.Add t then
                children t |> Seq.iter visit

        visit entry

        let recursive =
            reachable
            |> Seq.filter (fun t ->
                not (emptiable t)
                && (FSharpType.IsRecord(t, flags) || FSharpType.IsUnion(t, flags))
                && children t |> Seq.exists (reaches t))
            |> HashSet

        let touching = Dictionary<Type, bool>()

        let touches (t: Type) =
            match touching.TryGetValue t with
            | true, known -> known
            | false, _ ->
                let found = recursive |> Seq.exists (fun target -> reaches target t)
                touching[t] <- found
                found

        let bases = Dictionary<Type, IGenerator>()
        let nodes = Dictionary<Type, IGenerator>()
        let plain = Dictionary<Type, IGenerator>()

        let rec build (leaf: bool) (t: Type) : IGenerator =
            if not (touches t) then
                match plain.TryGetValue t with
                | true, known -> known
                | false, _ ->
                    let built = body leaf t
                    plain[t] <- built
                    built
            elif recursive.Contains t then
                match (if leaf then bases else nodes).TryGetValue t with
                | true, known -> known
                | false, _ when leaf || not (bases.ContainsKey t) -> raise (NoBaseCase t)
                | false, _ ->
                    let node = shape "NodeOf" [| t |] [||]
                    nodes[t] <- node
                    (node :?> INode).Tie(bases[t], body false t)
                    node
            elif emptiable t then
                if leaf then
                    shape "Constant" [| t |] [| empty t |]
                else
                    try
                        body false t
                    with NoBaseCase _ ->
                        shape "Constant" [| t |] [| empty t |]
            else
                body leaf t

        and body (leaf: bool) (t: Type) : IGenerator =
            let args = if t.IsGenericType then t.GetGenericArguments() else [||]

            let inner (name: string) =
                shape name args (args |> Array.map (build leaf >> box))

            if t = typeof<unit> then
                Generators.Just(())
            elif t.IsArray && t.GetArrayRank() = 1 then
                let element = t.GetElementType()
                shape "ArrayOf" [| element |] [| build leaf element |]
            elif definition t = typedefof<list<_>> then
                inner "ListOf"
            elif definition t = typedefof<option<_>> then
                inner "OptionOf"
            elif definition t = typedefof<voption<_>> then
                inner "ValueOptionOf"
            elif definition t = typedefof<Set<_>> then
                inner "SetOf"
            elif definition t = typedefof<Map<_, _>> then
                inner "MapOf"
            elif FSharpType.IsTuple t then
                shape "TupleOf" [| t |] [| FSharpType.GetTupleElements t |> Array.map (build leaf) |]
            elif FSharpType.IsRecord(t, flags) then
                let fields =
                    FSharpType.GetRecordFields(t, flags)
                    |> Array.map (fun field -> field.Name, build leaf field.PropertyType)

                shape "RecordOf" [| t |] [| fields |]
            elif FSharpType.IsUnion(t, flags) then
                let buildable =
                    FSharpType.GetUnionCases(t, flags)
                    |> Array.choose (fun case ->
                        try
                            Some(case, fieldTypes case |> Array.map (build leaf))
                        with NoBaseCase _ ->
                            None)

                if buildable.Length = 0 then
                    raise (NoBaseCase t)

                shape "UnionOf" [| t |] [| buildable |]
            else
                Derive.FromType t

        let mutable settled = false

        while not settled do
            let found =
                recursive
                |> Seq.filter (fun t -> not (bases.ContainsKey t))
                |> Seq.choose (fun t ->
                    try
                        Some(t, body true t)
                    with NoBaseCase _ ->
                        None)
                |> List.ofSeq

            for t, baseCase in found do
                bases[t] <- baseCase

            settled <- found.IsEmpty

        build false entry

    let private cache = ConcurrentDictionary<Type, IGenerator>()

    /// The generator derived for <paramref name="t"/>, built once per type.
    let forType (t: Type) : IGenerator =
        cache.GetOrAdd(
            t,
            fun t ->
                try
                    derive t
                with NoBaseCase cycle ->
                    raise (noBaseCase cycle)
        )
