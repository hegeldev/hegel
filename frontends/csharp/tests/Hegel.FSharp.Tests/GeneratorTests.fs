module Hegel.FSharp.Tests.GeneratorTests

open System
open System.Reflection
open System.Runtime.ExceptionServices
open Microsoft.FSharp.Reflection
open Xunit
open Hegel
open Hegel.FSharp
open Hegel.FSharp.Tests.Support

type Point = { X: int; Y: int }

type Colour =
    | Red
    | Green

type Shape =
    | Circle of radius: float
    | Rect of width: int * height: int

let private holds (generator: Generator<'T>) (check: 'T -> unit) =
    Hegel.test (quiet, fun tc -> check (tc.Draw generator)) |> ignore

let private minimal (generator: Generator<'T>) (bad: 'T -> bool) : string =
    let report =
        Hegel.run (
            quiet,
            fun tc ->
                if bad (tc.Draw(generator, "x")) then
                    failwith "boom"
        )

    String.Join("\n", (Seq.exactlyOne report.Failures).Output)

let private small = Gen.int.Min(0).Max(3)

let private outOfRange (what: string) (build: unit -> obj) =
    let e = Assert.Throws<ArgumentOutOfRangeException>(fun () -> build () |> ignore)
    Assert.Contains(what, e.Message)

[<Fact>]
let ``lists honour their sizes`` () =
    holds ((Gen.list Gen.int).MinSize(2).MaxSize(4)) (fun xs -> Assert.InRange(xs.Length, 2, 4))
    holds ((Gen.list Gen.int).MaxSize(0)) (fun xs -> Assert.Empty xs)

[<Fact>]
let ``lists validate their sizes`` () =
    outOfRange "Gen.list: MinSize must be >= 0" (fun () -> box ((Gen.list Gen.int).MinSize((-1))))
    outOfRange "Gen.list: MaxSize must be >= 0" (fun () -> box ((Gen.list Gen.int).MaxSize((-1))))

    outOfRange "Gen.list: MaxSize (1) must be >= MinSize (2)" (fun () -> box ((Gen.list Gen.int).MaxSize(1).MinSize(2)))

[<Fact>]
let ``lists can be unique`` () =
    holds ((Gen.list small).Unique().MaxSize(4)) (fun xs -> Assert.Equal(xs.Length, (List.distinct xs).Length))

    holds (((Gen.list (Gen.tuple2 small Gen.int)).UniqueBy fst).MaxSize(4)) (fun xs ->
        Assert.Equal(xs.Length, (List.distinctBy fst xs).Length))

    Assert.Equal("let x_1 = [0; 1]", printed ((Gen.list small).Unique().MinSize(2)))

    Assert.Equal(
        "let x_1 = [(0, 0); (1, 0)]",
        printed (((Gen.list (Gen.tuple2 small Gen.int)).UniqueBy fst).MinSize(2))
    )

[<Fact>]
let ``lists print and shrink as F# lists`` () =
    Assert.Equal("let x_1 = []", printed (Gen.list Gen.int))
    Assert.Equal("let x_1 = [0; 0]", printed ((Gen.list Gen.int).MinSize(2)))
    Assert.Equal("let x_1 = [[]; []]", printed ((Gen.list (Gen.list Gen.int)).MinSize(2)))
    Assert.Equal("let x_1 = [1]", minimal (Gen.list Gen.int) (List.exists (fun x -> x > 0)))

[<Fact>]
let ``arrays honour their sizes`` () =
    holds ((Gen.array Gen.int).MinSize(2).MaxSize(4)) (fun xs -> Assert.InRange(xs.Length, 2, 4))

[<Fact>]
let ``arrays validate their sizes`` () =
    outOfRange "Gen.array: MinSize must be >= 0" (fun () -> box ((Gen.array Gen.int).MinSize((-1))))
    outOfRange "Gen.array: MaxSize must be >= 0" (fun () -> box ((Gen.array Gen.int).MaxSize((-1))))

    outOfRange "Gen.array: MaxSize (1) must be >= MinSize (2)" (fun () ->
        box ((Gen.array Gen.int).MaxSize(1).MinSize(2)))

[<Fact>]
let ``arrays can be unique`` () =
    holds ((Gen.array small).Unique().MaxSize(4)) (fun xs -> Assert.Equal(xs.Length, (Array.distinct xs).Length))

    holds (((Gen.array (Gen.tuple2 small Gen.int)).UniqueBy fst).MaxSize(4)) (fun xs ->
        Assert.Equal(xs.Length, (Array.distinctBy fst xs).Length))

    Assert.Equal("let x_1 = [|0; 1|]", printed ((Gen.array small).Unique().MinSize(2)))

[<Fact>]
let ``arrays print and shrink as F# arrays`` () =
    Assert.Equal("let x_1 = [||]", printed (Gen.array Gen.int))
    Assert.Equal("let x_1 = [|0; 0|]", printed ((Gen.array Gen.int).MinSize(2)))
    Assert.Equal("let x_1 = [|1|]", minimal (Gen.array Gen.int) (Array.exists (fun x -> x > 0)))

[<Fact>]
let ``sets honour their sizes and are unique`` () =
    holds ((Gen.set small).MinSize(2).MaxSize(3)) (fun xs -> Assert.InRange(xs.Count, 2, 3))
    Assert.Equal("let x_1 = set []", printed (Gen.set Gen.int))
    Assert.Equal("let x_1 = set [0; 1]", printed ((Gen.set small).MinSize(2)))
    Assert.Equal("let x_1 = set [1]", minimal (Gen.set Gen.int) (Set.exists (fun x -> x > 0)))

[<Fact>]
let ``sets validate their sizes`` () =
    outOfRange "Gen.set: MinSize must be >= 0" (fun () -> box ((Gen.set Gen.int).MinSize((-1))))
    outOfRange "Gen.set: MaxSize must be >= 0" (fun () -> box ((Gen.set Gen.int).MaxSize((-1))))

    outOfRange "Gen.set: MaxSize (1) must be >= MinSize (2)" (fun () -> box ((Gen.set Gen.int).MaxSize(1).MinSize(2)))

[<Fact>]
let ``maps honour their sizes and have unique keys`` () =
    holds ((Gen.mapOf small Gen.int).MinSize(2).MaxSize(3)) (fun m -> Assert.InRange(m.Count, 2, 3))
    Assert.Equal("let x_1 = Map []", printed (Gen.mapOf Gen.int Gen.int))
    Assert.Equal("let x_1 = Map [(0, 0)]", printed ((Gen.mapOf Gen.int Gen.int).MinSize(1)))
    Assert.Equal("let x_1 = Map [(0, 0); (1, 0)]", printed ((Gen.mapOf small Gen.int).MinSize(2)))
    Assert.Equal("let x_1 = Map [(0, 1)]", minimal (Gen.mapOf Gen.int Gen.int) (Map.exists (fun _ v -> v > 0)))

[<Fact>]
let ``maps validate their sizes`` () =
    outOfRange "Gen.mapOf: MinSize must be >= 0" (fun () -> box ((Gen.mapOf Gen.int Gen.int).MinSize((-1))))
    outOfRange "Gen.mapOf: MaxSize must be >= 0" (fun () -> box ((Gen.mapOf Gen.int Gen.int).MaxSize((-1))))

    outOfRange "Gen.mapOf: MaxSize (1) must be >= MinSize (2)" (fun () ->
        box ((Gen.mapOf Gen.int Gen.int).MaxSize(1).MinSize(2)))

[<Fact>]
let ``collection labels combine the element labels`` () =
    Assert.Equal(Label.Combine(Label.List, Gen.int.Label), (Gen.list Gen.int).Label)
    Assert.Equal(Label.Combine(Label.Array, Gen.int.Label), (Gen.array Gen.int).Label)
    Assert.Equal(Label.Combine(Label.Set, Gen.int.Label), (Gen.set Gen.int).Label)
    Assert.Equal(Label.Combine(Label.Map, Gen.int.Label, Gen.string.Label), (Gen.mapOf Gen.int Gen.string).Label)
    Assert.Equal(Label.Combine(Label.Optional, Gen.int.Label), (Gen.option Gen.int).Label)
    Assert.Equal(Label.Combine(Label.Optional, Gen.int.Label), (Gen.voption Gen.int).Label)
    Assert.Equal(Label.Combine(Label.Tuple, Gen.int.Label, Gen.string.Label), (Gen.tuple2 Gen.int Gen.string).Label)
    Assert.Equal(Label.Combine(Label.FixedDict, Label.Of typeof<Point>.FullName), Gen.record<Point>.Label)
    Assert.Equal(Label.Combine(Label.OneOf, Label.Of typeof<Colour>.FullName), Gen.auto<Colour>.Label)
    Assert.Equal(Gen.auto<Colour>.Label, Gen.auto<Colour>.Label)

[<Fact>]
let ``options draw both branches and shrink to None`` () =
    Assert.Equal("let x_1 = None", printed (Gen.option Gen.int))
    Assert.Equal("let x_1 = Some(0)", minimal (Gen.option Gen.int) Option.isSome)
    Assert.Equal("let x_1 = Some(1)", minimal (Gen.option Gen.int) (fun o -> o |> Option.exists (fun x -> x > 0)))
    holds (Gen.option Gen.int) (fun o -> Assert.True(o.IsNone || o.IsSome))

[<Fact>]
let ``value options draw both branches and shrink to ValueNone`` () =
    Assert.Equal("let x_1 = ValueNone", printed (Gen.voption Gen.int))
    Assert.Equal("let x_1 = ValueSome(0)", minimal (Gen.voption Gen.int) ValueOption.isSome)

    Assert.Equal(
        "let x_1 = ValueSome(1)",
        minimal (Gen.voption Gen.int) (fun o -> o |> ValueOption.exists (fun x -> x > 0))
    )

    holds (Gen.voption Gen.int) (fun o -> Assert.True(o.IsNone || o.IsSome))

[<Fact>]
let ``tuples print in call style`` () =
    Assert.Equal("let x_1 = (0, 0)", printed (Gen.tuple2 Gen.int Gen.int))
    Assert.Equal("let x_1 = (0, \"\", 0)", printed (Gen.tuple3 Gen.int Gen.string Gen.int))
    Assert.Equal("let x_1 = (1, 0)", minimal (Gen.tuple2 Gen.int Gen.int) (fun (a, _) -> a > 0))
    holds (Gen.tuple2 (Gen.int.Min(1)) (Gen.int.Max(0))) (fun (a, b) -> Assert.True(a > b))

[<Fact>]
let ``records derive their fields and take overrides`` () =
    Assert.Equal("let x_1 = { X = 0; Y = 0 }", printed Gen.record<Point>)
    Assert.Equal("let x_1 = { X = 7; Y = 0 }", printed (Gen.record<Point>.With("X", Gen.constant 7)))

    Assert.Equal(
        "let x_1 = { X = 7; Y = 9 }",
        printed (Gen.record<Point>.With("X", Gen.constant 7).With("Y", Gen.constant 9))
    )

    Assert.Equal("let x_1 = { X = 0; Y = 1 }", minimal Gen.record<Point> (fun p -> p.Y > 0))
    holds (Gen.record<Point>.With("X", Gen.int.Min(1))) (fun p -> Assert.True(p.X >= 1))

[<Fact>]
let ``records reject unknown and mistyped fields`` () =
    let unknown =
        Assert.Throws<ArgumentException>(fun () -> Gen.record<Point>.With("Z", Gen.int) |> ignore)

    Assert.Contains("Point has no field Z; its fields are X, Y", unknown.Message)

    let mistyped =
        Assert.Throws<ArgumentException>(fun () -> Gen.record<Point>.With("X", Gen.string) |> ignore)

    Assert.Contains("field X of ", mistyped.Message)
    Assert.Contains("Point is a int, not a string", mistyped.Message)

[<Fact>]
let ``records must be F# records`` () =
    let e = Assert.Throws<ArgumentException>(fun () -> Gen.record<int> |> ignore)
    Assert.Equal("System.Int32 is not an F# record", e.Message)

let private igenerator =
    typeof<Generator<int>>.Assembly.GetType("Hegel.IGenerator", true)

let private caseType =
    typedefof<Tuple<_, _>>.MakeGenericType(typeof<UnionCaseInfo>, igenerator.MakeArrayType())

let private unionFrom<'T> (cases: UnionCaseInfo[]) (generators: UnionCaseInfo -> obj list) : UnionGen<'T> =
    let entries = Array.CreateInstance(caseType, cases.Length)

    cases
    |> Array.iteri (fun i case ->
        let gens = generators case
        let fields = Array.CreateInstance(igenerator, gens.Length)
        gens |> List.iteri (fun j g -> fields.SetValue(g, j))
        entries.SetValue(FSharpValue.MakeTuple([| box case; box fields |], caseType), i))

    try
        Activator.CreateInstance(
            typeof<UnionGen<'T>>,
            BindingFlags.Instance ||| BindingFlags.NonPublic ||| BindingFlags.Public,
            null,
            [| box entries |],
            null
        )
        :?> UnionGen<'T>
    with :? TargetInvocationException as e ->
        ExceptionDispatchInfo.Capture(e.InnerException).Throw()
        Unchecked.defaultof<UnionGen<'T>>

let private unionOf<'T> (generators: UnionCaseInfo -> obj list) : UnionGen<'T> =
    unionFrom<'T> (FSharpType.GetUnionCases typeof<'T>) generators

[<Fact>]
let ``unions draw a case and its fields`` () =
    let colours = unionOf<Colour>(fun _ -> [])
    Assert.Equal("let x_1 = Red", printed colours)
    Assert.Equal("let x_1 = Green", minimal colours (fun c -> c = Green))

    let shapes =
        unionOf<Shape>(fun case ->
            match case.Name with
            | "Circle" -> [ box Gen.float ]
            | _ -> [ box Gen.int; box Gen.int ])

    Assert.Equal("let x_1 = Circle(0.0)", printed shapes)

    Assert.Equal(
        "let x_1 = Rect(0, 1)",
        minimal shapes (function
            | Rect(_, h) -> h > 0
            | Circle _ -> false)
    )

    Assert.Equal(Label.Combine(Label.OneOf, Label.Of typeof<Shape>.FullName), shapes.Label)

[<Fact>]
let ``unions need at least one case`` () =
    let e =
        Assert.Throws<ArgumentException>(fun () -> unionFrom<Colour> [||] (fun _ -> []) |> ignore)

    Assert.Contains("Colour has no cases to draw", e.Message)

[<Fact>]
let ``every generator draws silently`` () =
    Hegel.test (
        quiet,
        fun tc ->
            Assert.InRange(tc.DrawSilent((Gen.list Gen.int).MaxSize(3)).Length, 0, 3)
            Assert.InRange(tc.DrawSilent((Gen.array Gen.int).MaxSize(3)).Length, 0, 3)
            Assert.InRange(tc.DrawSilent((Gen.set small).MaxSize(3)).Count, 0, 3)
            Assert.InRange(tc.DrawSilent((Gen.mapOf small Gen.int).MaxSize(3)).Count, 0, 3)
            tc.DrawSilent(Gen.option Gen.int) |> ignore
            tc.DrawSilent(Gen.voption Gen.int) |> ignore
            tc.DrawSilent(Gen.tuple2 Gen.int Gen.int) |> ignore
            tc.DrawSilent(Gen.record<Point>.With("X", Gen.constant 1)).X |> ignore
            tc.DrawSilent(unionOf<Colour>(fun _ -> [])) |> ignore
    )
    |> ignore
