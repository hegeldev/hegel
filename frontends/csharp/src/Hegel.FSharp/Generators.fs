namespace Hegel.FSharp

open System
open System.Collections.Generic
open System.Runtime.CompilerServices
open Hegel
open Hegel.Native

module internal Collections =
    let draw
        (tc: TestCase)
        (printer: Printer)
        (openText: string)
        (closeText: string)
        (minSize: uint64)
        (maxSize: uint64)
        (element: TestCase -> Printer -> 'T)
        (uniqueKey: ('T -> obj) option)
        : ResizeArray<'T> =
        let p = Components.Out printer
        let id = tc.NewCollection(minSize, maxSize)

        try
            let output = ResizeArray<'T>()
            let keys = HashSet<ContentKey>()
            p.BeginGroup openText

            while tc.CollectionMore id do
                use speculation = p.Speculate()

                if output.Count > 0 then
                    Repr.separator p

                let value = element tc printer

                match uniqueKey with
                | Some key when not (keys.Add(ContentKey(key value))) ->
                    speculation.Abort()
                    tc.CollectionReject(id, "duplicate element")
                | _ ->
                    speculation.Commit()
                    output.Add value

            p.EndGroup closeText
            output
        finally
            tc.CollectionFree id

    [<MethodImpl(MethodImplOptions.NoInlining)>]
    let one (generator: Generator<'T>) (tc: TestCase) (printer: Printer) = Components.Draw(tc, generator, printer)

/// Draws F# lists from an element generator, printed as <c>[a; b]</c>.
type ListGen<'T> internal (element: Generator<'T>, minSize: int, maxSize: uint64, uniqueKey: ('T -> obj) option) =
    inherit PrintableGenerator<'T list>()
    let minSize = Sizes.Validate(minSize, maxSize, "Gen.list")
    let label = CachedLabel()

    member internal _.Draw(tc: TestCase, printer: Printer) : 'T list =
        Collections.draw tc printer "[" "]" minSize maxSize (Collections.one element) uniqueKey
        |> List.ofSeq

    /// At least <paramref name="size"/> elements.
    member _.MinSize(size: int) =
        ListGen<'T>(element, size, maxSize, uniqueKey)

    /// At most <paramref name="size"/> elements.
    member _.MaxSize(size: int) =
        ListGen<'T>(element, int minSize, Sizes.Max(size, "Gen.list"), uniqueKey)

    /// No two equal elements.
    member _.Unique() =
        ListGen<'T>(element, int minSize, maxSize, Some box)

    /// No two elements with equal keys.
    member _.UniqueBy(key: 'T -> 'Key) =
        ListGen<'T>(element, int minSize, maxSize, Some(fun value -> box (key value)))

    override _.Label = label.Get(fun () -> Label.Combine(Label.List, element.Label))

    override this.DoDraw tc = this.Draw(tc, null)

    override this.DoDrawAndPrint(tc, printer) = this.Draw(tc, printer)

/// Draws arrays from an element generator, printed as <c>[|a; b|]</c>.
type ArrayGen<'T> internal (element: Generator<'T>, minSize: int, maxSize: uint64, uniqueKey: ('T -> obj) option) =
    inherit PrintableGenerator<'T[]>()
    let minSize = Sizes.Validate(minSize, maxSize, "Gen.array")
    let label = CachedLabel()

    member internal _.Draw(tc: TestCase, printer: Printer) : 'T[] =
        (Collections.draw tc printer "[|" "|]" minSize maxSize (Collections.one element) uniqueKey).ToArray()

    /// At least <paramref name="size"/> elements.
    member _.MinSize(size: int) =
        ArrayGen<'T>(element, size, maxSize, uniqueKey)

    /// At most <paramref name="size"/> elements.
    member _.MaxSize(size: int) =
        ArrayGen<'T>(element, int minSize, Sizes.Max(size, "Gen.array"), uniqueKey)

    /// No two equal elements.
    member _.Unique() =
        ArrayGen<'T>(element, int minSize, maxSize, Some box)

    /// No two elements with equal keys.
    member _.UniqueBy(key: 'T -> 'Key) =
        ArrayGen<'T>(element, int minSize, maxSize, Some(fun value -> box (key value)))

    override _.Label = label.Get(fun () -> Label.Combine(Label.Array, element.Label))

    override this.DoDraw tc = this.Draw(tc, null)

    override this.DoDrawAndPrint(tc, printer) = this.Draw(tc, printer)

/// Draws F# sets from an element generator, printed as <c>set [a; b]</c>.
type SetGen<'T when 'T: comparison> internal (element: Generator<'T>, minSize: int, maxSize: uint64) =
    inherit PrintableGenerator<Set<'T>>()
    let minSize = Sizes.Validate(minSize, maxSize, "Gen.set")
    let label = CachedLabel()

    member internal _.Draw(tc: TestCase, printer: Printer) : Set<'T> =
        Collections.draw tc printer "set [" "]" minSize maxSize (Collections.one element) (Some box)
        |> Set.ofSeq

    /// At least <paramref name="size"/> elements.
    member _.MinSize(size: int) = SetGen<'T>(element, size, maxSize)

    /// At most <paramref name="size"/> elements.
    member _.MaxSize(size: int) =
        SetGen<'T>(element, int minSize, Sizes.Max(size, "Gen.set"))

    override _.Label = label.Get(fun () -> Label.Combine(Label.Set, element.Label))

    override this.DoDraw tc = this.Draw(tc, null)

    override this.DoDrawAndPrint(tc, printer) = this.Draw(tc, printer)

/// Draws F# maps from a key and a value generator, printed as <c>Map [(k, v)]</c>.
type MapGen<'Key, 'Value when 'Key: comparison>
    internal (keys: Generator<'Key>, values: Generator<'Value>, minSize: int, maxSize: uint64) =
    inherit PrintableGenerator<Map<'Key, 'Value>>()
    let minSize = Sizes.Validate(minSize, maxSize, "Gen.mapOf")
    let label = CachedLabel()

    let pair (tc: TestCase) (printer: Printer) =
        let p = Components.Out printer
        p.BeginGroup "("
        let key = Components.Draw(tc, keys, printer)
        p.Separator()
        let value = Components.Draw(tc, values, printer)
        p.EndGroup ")"
        key, value

    member internal _.Draw(tc: TestCase, printer: Printer) : Map<'Key, 'Value> =
        Collections.draw tc printer "Map [" "]" minSize maxSize pair (Some(fun (key, _) -> box key))
        |> Map.ofSeq

    /// At least <paramref name="size"/> entries.
    member _.MinSize(size: int) =
        MapGen<'Key, 'Value>(keys, values, size, maxSize)

    /// At most <paramref name="size"/> entries.
    member _.MaxSize(size: int) =
        MapGen<'Key, 'Value>(keys, values, int minSize, Sizes.Max(size, "Gen.mapOf"))

    override _.Label =
        label.Get(fun () -> Label.Combine(Label.Map, keys.Label, values.Label))

    override this.DoDraw tc = this.Draw(tc, null)

    override this.DoDrawAndPrint(tc, printer) = this.Draw(tc, printer)

/// Draws options: <c>None</c> or <c>Some(value)</c> from the inner generator, with equal probability.
type OptionGen<'T> internal (inner: Generator<'T>) =
    inherit PrintableGenerator<'T option>()
    let label = CachedLabel()

    member internal _.Draw(tc: TestCase, printer: Printer) : 'T option =
        let p = Components.Out printer

        if tc.GenerateBoolean 0.5 then
            p.BeginGroup "Some("
            let value = Components.Draw(tc, inner, printer)
            p.EndGroup ")"
            Some value
        else
            p.Text "None"
            None

    override _.Label = label.Get(fun () -> Label.Combine(Label.Optional, inner.Label))

    override this.DoDraw tc = this.Draw(tc, null)

    override this.DoDrawAndPrint(tc, printer) = this.Draw(tc, printer)

/// Draws value options: <c>ValueNone</c> or <c>ValueSome(value)</c>, with equal probability.
type ValueOptionGen<'T> internal (inner: Generator<'T>) =
    inherit PrintableGenerator<'T voption>()
    let label = CachedLabel()

    member internal _.Draw(tc: TestCase, printer: Printer) : 'T voption =
        let p = Components.Out printer

        if tc.GenerateBoolean 0.5 then
            p.BeginGroup "ValueSome("
            let value = Components.Draw(tc, inner, printer)
            p.EndGroup ")"
            ValueSome value
        else
            p.Text "ValueNone"
            ValueNone

    override _.Label = label.Get(fun () -> Label.Combine(Label.Optional, inner.Label))

    override this.DoDraw tc = this.Draw(tc, null)

    override this.DoDrawAndPrint(tc, printer) = this.Draw(tc, printer)

/// Draws a tuple, one element from each generator, printed as <c>(a, b)</c>.
type TupleGen<'T> internal (elements: IGenerator[], make: obj[] -> 'T, openText: string) =
    inherit PrintableGenerator<'T>()
    let label = CachedLabel()

    member internal _.Draw(tc: TestCase, printer: Printer) : 'T =
        let p = Components.Out printer
        p.BeginGroup openText

        let values =
            elements
            |> Array.mapi (fun i element ->
                if i > 0 then
                    p.Separator()

                element.Draw(tc, printer))

        p.EndGroup ")"
        make values

    override _.Label = label.Get(fun () -> CachedLabel.Combine(Label.Tuple, elements))

    override this.DoDraw tc = this.Draw(tc, null)

    override this.DoDrawAndPrint(tc, printer) = this.Draw(tc, printer)

module internal Records =
    let private flags =
        Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic

    let fields (t: Type) =
        if not (Microsoft.FSharp.Reflection.FSharpType.IsRecord(t, flags)) then
            raise (ArgumentException(t.FullName + " is not an F# record"))

        Microsoft.FSharp.Reflection.FSharpType.GetRecordFields(t, flags)

/// Draws F# records by drawing each field from the generator <paramref name="generators"/> holds
/// for it, printed as <c>{ A = a; B = b }</c>. <c>Gen.record</c> starts from the generators derived
/// from the field types, which <c>.With</c> replaces one at a time.
type RecordGen<'T> internal (generators: Map<string, Lazy<IGenerator>>) =
    inherit PrintableGenerator<'T>()

    let fields = Records.fields typeof<'T>

    let construct =
        Microsoft.FSharp.Reflection.FSharpValue.PreComputeRecordConstructor(
            typeof<'T>,
            Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic
        )

    let label = CachedLabel()

    let components =
        lazy (fields |> Array.map (fun field -> generators[field.Name].Value))

    /// A generator of <typeparamref name="T"/> with every field derived from its type by
    /// <paramref name="derive"/>, when first drawn.
    static member internal Derived(derive: Type -> IGenerator) : RecordGen<'T> =
        RecordGen<'T>(
            Records.fields typeof<'T>
            |> Array.map (fun field -> field.Name, lazy (derive field.PropertyType))
            |> Map.ofArray
        )

    /// This generator, drawing the field <paramref name="field"/> from <paramref name="generator"/>.
    member _.With(field: string, generator: Generator<'Field>) : RecordGen<'T> =
        match fields |> Array.tryFind (fun f -> f.Name = field) with
        | None ->
            raise (
                ArgumentException(
                    typeof<'T>.FullName
                    + " has no field "
                    + field
                    + "; its fields are "
                    + String.Join(", ", fields |> Array.map (fun f -> f.Name))
                )
            )
        | Some f when not (f.PropertyType.IsAssignableFrom typeof<'Field>) ->
            raise (
                ArgumentException(
                    "field "
                    + field
                    + " of "
                    + typeof<'T>.FullName
                    + " is a "
                    + Repr.typeName f.PropertyType
                    + ", not a "
                    + Repr.typeName typeof<'Field>
                )
            )
        | Some _ -> RecordGen<'T>(generators.Add(field, lazy (generator :> IGenerator)))

    member internal _.Draw(tc: TestCase, printer: Printer) : 'T =
        let p = Components.Out printer
        p.BeginGroup "{ "

        let values =
            Array.map2
                (fun (field: Reflection.PropertyInfo) (generator: IGenerator) ->
                    if not (obj.ReferenceEquals(field, fields[0])) then
                        Repr.separator p

                    p.Text(field.Name + " = ")
                    generator.Draw(tc, printer))
                fields
                components.Value

        p.EndGroup " }"
        construct values :?> 'T

    override _.Label =
        label.Get(fun () -> Label.Combine(Label.FixedDict, Label.Of typeof<'T>.FullName))

    override this.DoDraw tc = this.Draw(tc, null)

    override this.DoDrawAndPrint(tc, printer) = this.Draw(tc, printer)

/// Draws F# union values: a case chosen uniformly, its fields drawn from their generators, printed
/// as <c>Case</c> or <c>Case(a, b)</c>.
type UnionGen<'T> internal (cases: (Microsoft.FSharp.Reflection.UnionCaseInfo * IGenerator[])[]) =
    inherit PrintableGenerator<'T>()
    let label = CachedLabel()

    let constructors =
        cases
        |> Array.map (fun (case, _) ->
            Microsoft.FSharp.Reflection.FSharpValue.PreComputeUnionConstructor(
                case,
                Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic
            ))

    do
        if cases.Length = 0 then
            raise (ArgumentException(typeof<'T>.FullName + " has no cases to draw"))

    member internal _.Draw(tc: TestCase, printer: Printer) : 'T =
        let p = Components.Out printer
        let index = int (tc.GenerateInteger(0L, int64 (cases.Length - 1)))
        let case, fields = cases[index]

        if fields.Length = 0 then
            p.Text case.Name
            constructors[index] [||] :?> 'T
        else
            p.BeginGroup(case.Name + "(")

            let values =
                fields
                |> Array.mapi (fun i field ->
                    if i > 0 then
                        p.Separator()

                    field.Draw(tc, printer))

            p.EndGroup ")"
            constructors[index] values :?> 'T

    override _.Label =
        label.Get(fun () -> Label.Combine(Label.OneOf, Label.Of typeof<'T>.FullName))

    override this.DoDraw tc = this.Draw(tc, null)

    override this.DoDrawAndPrint(tc, printer) = this.Draw(tc, printer)
