namespace Hegel.FSharp

open System
open System.Collections
open System.Collections.Generic
open System.Globalization
open System.Numerics
open System.Runtime.CompilerServices
open Microsoft.FSharp.Reflection
open Hegel

/// Prints values as the F# that builds them: what the draws of a failing test are shown as under
/// <c>Hegel.test</c>, and what a generator that prints its values itself should produce.
[<RequireQualifiedAccess>]
module Repr =
    let private keywords =
        dict
            [
                typeof<bool>, "bool"
                typeof<byte>, "byte"
                typeof<sbyte>, "sbyte"
                typeof<char>, "char"
                typeof<int16>, "int16"
                typeof<uint16>, "uint16"
                typeof<int>, "int"
                typeof<uint32>, "uint32"
                typeof<int64>, "int64"
                typeof<uint64>, "uint64"
                typeof<nativeint>, "nativeint"
                typeof<unativeint>, "unativeint"
                typeof<float32>, "float32"
                typeof<float>, "float"
                typeof<decimal>, "decimal"
                typeof<string>, "string"
                typeof<obj>, "obj"
                typeof<BigInteger>, "bigint"
                typeof<unit>, "unit"
            ]

    /// The F# name of a type, as written in a type annotation.
    let rec typeName (t: Type) : string =
        match keywords.TryGetValue t with
        | true, name -> name
        | _ when t.IsArray -> typeName (t.GetElementType()) + "[]"
        | _ when t.IsGenericType ->
            let args = t.GetGenericArguments() |> Array.map typeName
            let definition = t.GetGenericTypeDefinition()

            if definition = typedefof<list<_>> then
                args[0] + " list"
            elif definition = typedefof<option<_>> then
                args[0] + " option"
            elif definition = typedefof<voption<_>> then
                args[0] + " voption"
            else
                let name = t.Name
                let tick = name.IndexOf '`'
                let bare = if tick < 0 then name else name.Substring(0, tick)
                bare + "<" + String.Join(", ", args) + ">"
        | _ -> t.Name

    /// <paramref name="s"/> as an F# string or character literal, delimited by <paramref name="delimiter"/>.
    let quote (s: string) (delimiter: char) : string =
        let sb = Text.StringBuilder(s.Length + 2).Append(delimiter)
        let mutable i = 0

        while i < s.Length do
            let c = s[i]

            if Char.IsHighSurrogate c && i + 1 < s.Length && Char.IsLowSurrogate s[i + 1] then
                sb.Append(c).Append(s[i + 1]) |> ignore
                i <- i + 1
            else
                match c with
                | '\\' -> sb.Append "\\\\" |> ignore
                | '\n' -> sb.Append "\\n" |> ignore
                | '\r' -> sb.Append "\\r" |> ignore
                | '\t' -> sb.Append "\\t" |> ignore
                | _ when c = delimiter -> sb.Append('\\').Append(c) |> ignore
                | _ when
                    Char.IsControl c
                    || c = '\u00a0'
                    || c = '\u2028'
                    || c = '\u2029'
                    || Char.IsSurrogate c
                    ->
                    sb.Append("\\u").Append((int c).ToString("x4", CultureInfo.InvariantCulture))
                    |> ignore
                | _ -> sb.Append c |> ignore

            i <- i + 1

        sb.Append(delimiter).ToString()

    [<MethodImpl(MethodImplOptions.NoInlining)>]
    let private invariant (value: IFormattable) =
        value.ToString(null, CultureInfo.InvariantCulture)

    let private floatLiteral (d: float) =
        if Double.IsNaN d then
            "nan"
        elif Double.IsPositiveInfinity d then
            "infinity"
        elif Double.IsNegativeInfinity d then
            "-infinity"
        else
            let text = d.ToString("R", CultureInfo.InvariantCulture)

            if text.Contains '.' || text.Contains 'E' then
                text
            else
                text + ".0"

    let private float32Literal (f: float32) =
        if Single.IsNaN f then
            "nanf"
        elif Single.IsPositiveInfinity f then
            "infinityf"
        elif Single.IsNegativeInfinity f then
            "-infinityf"
        else
            let text = f.ToString("R", CultureInfo.InvariantCulture)

            (if text.Contains '.' || text.Contains 'E' then
                 text
             else
                 text + ".0")
            + "f"

    let private enumLiteral (e: Enum) =
        let name = typeName (e.GetType())
        let text = e.ToString()

        if Char.IsAsciiDigit text[0] || text[0] = '-' then
            "enum<" + name + "> (" + text + ")"
        else
            text.Split ", "
            |> Array.map (fun case -> name + "." + case)
            |> String.concat " ||| "

    let private isDefinition (definition: Type) (t: Type) =
        t.IsGenericType && t.GetGenericTypeDefinition() = definition

    let private dictionaryInterface (t: Type) =
        t.GetInterfaces() |> Array.tryFind (isDefinition typedefof<IDictionary<_, _>>)

    /// Print <c>;</c> and a breakable space: what separates the elements of a list.
    [<MethodImpl(MethodImplOptions.NoInlining)>]
    let separator (p: Printer) =
        p.Text ";"
        p.Breakable " "

    let private genericArg (index: int) (t: Type) =
        if t.IsGenericType then
            t.GetGenericArguments()[index]
        else
            typeof<obj>

    /// Print <paramref name="value"/>, drawn with the static type <paramref name="t"/>, as the F#
    /// that builds it. The static type tells <c>null</c> apart from <c>None</c> and <c>()</c>.
    let rec render (p: Printer) (t: Type) (value: obj) : unit =
        match value with
        | null ->
            if isDefinition typedefof<option<_>> t then p.Text "None"
            elif t = typeof<unit> then p.Text "()"
            else p.Text "null"
        | :? string as s -> p.Text(quote s '"')
        | :? char as c -> p.Text(quote (string c) '\'')
        | :? bool as b -> p.Text(if b then "true" else "false")
        | :? int as i -> p.Text(invariant i)
        | :? int64 as i -> p.Text(invariant i + "L")
        | :? int16 as i -> p.Text(invariant i + "s")
        | :? sbyte as i -> p.Text(invariant i + "y")
        | :? byte as i -> p.Text(invariant i + "uy")
        | :? uint16 as i -> p.Text(invariant i + "us")
        | :? uint32 as i -> p.Text(invariant i + "u")
        | :? uint64 as i -> p.Text(invariant i + "UL")
        | :? nativeint as i -> p.Text(invariant i + "n")
        | :? unativeint as i -> p.Text(invariant i + "un")
        | :? float as d -> p.Text(floatLiteral d)
        | :? float32 as f -> p.Text(float32Literal f)
        | :? decimal as m -> p.Text(invariant m + "M")
        | :? BigInteger as b -> p.Text(invariant b + "I")
        | :? Enum as e -> p.Text(enumLiteral e)
        | _ ->
            match Printer.ScalarLiteral value with
            | null -> renderStructure p value
            | literal -> p.Text literal

    and private renderStructure (p: Printer) (value: obj) =
        let rt = value.GetType()

        if rt.IsArray then
            sequence p "[|" "|]" (rt.GetElementType()) (value :?> IEnumerable)
        elif isDefinition typedefof<list<_>> rt then
            sequence p "[" "]" (genericArg 0 rt) (value :?> IEnumerable)
        elif isDefinition typedefof<option<_>> rt then
            applied p "Some(" [| genericArg 0 rt, rt.GetProperty("Value").GetValue value |]
        elif isDefinition typedefof<voption<_>> rt then
            if rt.GetProperty("IsSome").GetValue value :?> bool then
                applied p "ValueSome(" [| genericArg 0 rt, rt.GetProperty("Value").GetValue value |]
            else
                p.Text "ValueNone"
        elif isDefinition typedefof<Set<_>> rt then
            sequence p "set [" "]" (genericArg 0 rt) (value :?> IEnumerable)
        elif isDefinition typedefof<Map<_, _>> rt then
            pairs p "Map [" rt (value :?> IEnumerable)
        elif isDefinition typedefof<KeyValuePair<_, _>> rt then
            applied p "KeyValuePair(" (fields rt [| "Key"; "Value" |] value)
        elif isDefinition typedefof<ResizeArray<_>> rt then
            sequence p "ResizeArray [" "]" (genericArg 0 rt) (value :?> IEnumerable)
        elif isDefinition typedefof<HashSet<_>> rt then
            sequence p "HashSet [" "]" (genericArg 0 rt) (value :?> IEnumerable)
        elif (dictionaryInterface rt).IsSome then
            pairs p "dict [" (dictionaryInterface rt).Value (value :?> IEnumerable)
        elif FSharpType.IsTuple rt then
            let elements = FSharpType.GetTupleElements rt
            let values = FSharpValue.GetTupleFields value
            applied p (if rt.IsValueType then "struct (" else "(") (Array.zip elements values)
        elif FSharpType.IsRecord(rt, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic) then
            let anonymous = rt.Name.StartsWith("<>f__AnonymousType", StringComparison.Ordinal)
            p.BeginGroup(if anonymous then "{| " else "{ ")
            let mutable first = true

            for field in
                FSharpType.GetRecordFields(rt, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic) do
                if not first then
                    separator p

                first <- false
                p.Text(field.Name + " = ")
                render p field.PropertyType (field.GetValue value)

            p.EndGroup(if anonymous then " |}" else " }")
        elif FSharpType.IsUnion(rt, Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic) then
            let case, values =
                FSharpValue.GetUnionFields(
                    value,
                    rt,
                    Reflection.BindingFlags.Public ||| Reflection.BindingFlags.NonPublic
                )

            if values.Length = 0 then
                p.Text case.Name
            else
                let types = case.GetFields() |> Array.map (fun f -> f.PropertyType)
                applied p (case.Name + "(") (Array.zip types values)
        elif value :? IEnumerable then
            sequence p "seq [" "]" (genericArg 0 rt) (value :?> IEnumerable)
        else
            p.Reflow(Printer.Invariant value)

    and private fields (rt: Type) (names: string[]) (value: obj) =
        names
        |> Array.map (fun name ->
            let property = rt.GetProperty name
            property.PropertyType, property.GetValue value)

    and private applied (p: Printer) (openText: string) (arguments: (Type * obj)[]) =
        p.BeginGroup openText
        let mutable first = true

        for (t, argument) in arguments do
            if not first then
                p.Separator()

            first <- false
            render p t argument

        p.EndGroup ")"

    and private sequence (p: Printer) (openText: string) (closeText: string) (t: Type) (items: IEnumerable) =
        p.BeginGroup openText
        let mutable first = true

        for item in items do
            if not first then
                separator p

            first <- false
            render p t item

        p.EndGroup closeText

    and private pairs (p: Printer) (openText: string) (rt: Type) (entries: IEnumerable) =
        let keyType, valueType = genericArg 0 rt, genericArg 1 rt
        let pair = typedefof<KeyValuePair<_, _>>.MakeGenericType(keyType, valueType)
        let key, item = pair.GetProperty "Key", pair.GetProperty "Value"
        p.BeginGroup openText
        let mutable first = true

        for entry in entries do
            if not first then
                separator p

            first <- false
            applied p "(" [| keyType, key.GetValue entry; valueType, item.GetValue entry |]

        p.EndGroup "]"

    /// The F# <see cref="Dialect"/>: draws bound with <c>let</c>, values printed by <see cref="render"/>.
    type FSharpDialect() =
        inherit Dialect()

        override _.Repr(printer, value, staticType) = render printer staticType value

        override _.BeginDraw(printer, name) = printer.Text("let " + name + " = ")

        override _.EndDraw(_) = ()

    /// The F# dialect, which <c>Hegel.test</c> and <c>Hegel.run</c> install for the run.
    let dialect: Dialect = FSharpDialect()
