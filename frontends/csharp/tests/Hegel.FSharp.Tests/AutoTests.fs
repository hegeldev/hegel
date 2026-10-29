module Hegel.FSharp.Tests.AutoTests

open System
open Xunit
open Hegel
open Hegel.FSharp
open Hegel.FSharp.Tests.Support

type Hue =
    | Cyan = 1
    | Magenta = 2

type Suit =
    | Hearts
    | Spades

type Shape =
    | Circle of float
    | Rect of int * int

type Point = { X: int; Y: int }

[<Struct>]
type SPoint = { SX: int; SY: bool }

type Secret = private { Code: int }

type Hidden = private H of int

type Nested = { Origin: Point; Label: string }

type Box<'T> = { Item: 'T }

type Tree =
    | Leaf
    | Node of Tree * int * Tree

type Rose = Rose of int * Rose list

type Chain = { Value: int; Next: Chain option }

type Spine = Spine of Spine voption

type Branches = Branches of Branches[]

type Forest = Forest of Set<Forest>

type Index = Index of Map<int, Index>

type Wrapped = Wrapped of Wrapped option

type Tagged =
    {
        Tags: Set<int>
        Pairs: (int * Tagged) list
    }

type A =
    | ALeaf
    | ANode of B

and B = BNode of A

type Item = { Name: string; Sub: Group option }

and Group =
    | Empty
    | Of of Item

type Inf = Inf of Inf

type Loop = { Self: Loop }

type Plain() =
    member _.Nothing = 0

let private minimal (generator: Generator<'T>) : 'T =
    let failure = failing (fun tc -> tc.Draw(generator, "x") |> ignore)
    failure.Draws["x_1"] :?> 'T

let private check (generator: Generator<'T>) (property: 'T -> unit) =
    Hegel.test (quiet, fun tc -> property (tc.Draw generator)) |> ignore

[<Fact>]
let ``scalars are delegated to the C# derivation`` () =
    Assert.Equal("let x_1 = 0", printed Gen.auto<int>)
    Assert.Equal("let x_1 = \"\"", printed Gen.auto<string>)
    Assert.StartsWith("let x_1 = Guid.Parse(\"", printed Gen.auto<Guid>)
    Assert.StartsWith("let x_1 = DateTime.Parse(\"", printed Gen.auto<DateTime>)
    Assert.Equal("let x_1 = Hue.Cyan", printed Gen.auto<Hue>)
    check Gen.auto<Hue> (fun hue -> Assert.True(Enum.IsDefined hue))

[<Fact>]
let ``unit draws ()`` () =
    Assert.Equal("let x_1 = ()", printed Gen.auto<unit>)

[<Fact>]
let ``byte arrays draw as F# arrays`` () =
    Assert.Equal("let x_1 = [||]", printed Gen.auto<byte[]>)
    check Gen.auto<byte[]> (fun bytes -> Assert.NotNull bytes)

[<Fact>]
let ``arrays and lists`` () =
    Assert.Equal("let x_1 = [||]", printed Gen.auto<string[]>)
    Assert.Equal("let x_1 = []", printed Gen.auto<int list>)
    check Gen.auto<int list> (fun xs -> Assert.True(List.forall (fun _ -> true) xs))

[<Fact>]
let ``options and value options`` () =
    Assert.Equal("let x_1 = []", printed Gen.auto<int option list>)
    Assert.Equal("let x_1 = None", printed Gen.auto<int option option>)
    Assert.Equal("let x_1 = ValueNone", printed Gen.auto<int voption>)

    let failure =
        failing (fun tc ->
            let value = tc.Draw(Gen.auto<int option option>, "x")
            tc.Assume(value.IsSome && value.Value.IsSome))

    Assert.Equal<string seq>([ "let x_1 = Some(Some(0))" ], failure.Output)

    let voption =
        failing (fun tc ->
            let value = tc.Draw(Gen.auto<int voption>, "x")
            tc.Assume value.IsSome)

    Assert.Equal<string seq>([ "let x_1 = ValueSome(0)" ], voption.Output)

[<Fact>]
let ``sets and maps`` () =
    Assert.Equal("let x_1 = set []", printed Gen.auto<Set<int>>)
    Assert.Equal("let x_1 = Map []", printed Gen.auto<Map<string, int list>>)
    check Gen.auto<Set<int>> (fun s -> Assert.Equal(s.Count, Set.toList s |> List.distinct |> List.length))

    let failure =
        failing (fun tc ->
            let value = tc.Draw(Gen.auto<Map<string, int list>>, "x")
            tc.Assume(not value.IsEmpty))

    Assert.Equal<string seq>([ "let x_1 = Map [(\"\", [])]" ], failure.Output)

[<Fact>]
let ``reference and struct tuples`` () =
    Assert.Equal("let x_1 = (0, false)", printed Gen.auto<int * bool>)
    Assert.Equal("let x_1 = struct (0, \"\")", printed Gen.auto<struct (int * string)>)
    check Gen.auto<int * bool * string> (fun (_, _, s) -> Assert.NotNull s)

[<Fact>]
let ``records`` () =
    Assert.Equal("let x_1 = { X = 0; Y = 0 }", printed Gen.auto<Point>)
    Assert.Equal("let x_1 = { SX = 0; SY = false }", printed Gen.auto<SPoint>)
    Assert.Equal("let x_1 = { Code = 0 }", printed Gen.auto<Secret>)
    Assert.Equal("let x_1 = { Origin = { X = 0; Y = 0 }; Label = \"\" }", printed Gen.auto<Nested>)
    Assert.Equal("let x_1 = { Item = [] }", printed Gen.auto<Box<int list>>)
    check Gen.auto<Nested> (fun nested -> Assert.NotNull nested.Label)

[<Fact>]
let ``unions`` () =
    check Gen.auto<Suit> (fun suit -> Assert.Contains(suit, [ Hearts; Spades ]))
    Assert.Equal("let x_1 = Hearts", printed Gen.auto<Suit>)
    Assert.Equal("let x_1 = Circle(0.0)", printed Gen.auto<Shape>)
    Assert.Equal("let x_1 = H(0)", printed Gen.auto<Hidden>)
    Assert.Equal("let x_1 = Ok(0)", printed Gen.auto<Result<int, string>>)
    Assert.Equal("let x_1 = Choice1Of2(0)", printed Gen.auto<Choice<int, string>>)

    let rect =
        failing (fun tc ->
            match tc.Draw(Gen.auto<Shape>, "x") with
            | Rect _ -> ()
            | Circle _ -> tc.Reject())

    Assert.Equal<string seq>([ "let x_1 = Rect(0, 0)" ], rect.Output)

[<Fact>]
let ``recursive unions bottom out at the base case`` () =
    Assert.Equal("let x_1 = Leaf", printed Gen.auto<Tree>)

    let node =
        failing (fun tc ->
            match tc.Draw(Gen.auto<Tree>, "t") with
            | Node _ -> ()
            | Leaf -> tc.Reject())

    Assert.Equal<string seq>([ "let t_1 = Node(Leaf, 0, Leaf)" ], node.Output)

    let rec depth tree =
        match tree with
        | Leaf -> 0
        | Node(l, _, r) -> 1 + max (depth l) (depth r)

    check Gen.auto<Tree> (fun tree -> Assert.InRange(depth tree, 0, RecursiveGenerator<Tree>.DefaultMaxDepth + 1))

[<Fact>]
let ``recursion through a list uses the empty list as the base`` () =
    Assert.Equal("let x_1 = Rose(0, [])", printed Gen.auto<Rose>)

    let bushy =
        failing (fun tc ->
            match tc.Draw(Gen.auto<Rose>, "r") with
            | Rose(_, []) -> tc.Reject()
            | Rose _ -> ())

    Assert.Equal<string seq>([ "let r_1 = Rose(0, [Rose(0, [])])" ], bushy.Output)

[<Fact>]
let ``recursion through an option in a record uses None as the base`` () =
    Assert.Equal("let x_1 = { Value = 0; Next = None }", printed Gen.auto<Chain>)

    let linked =
        failing (fun tc ->
            let chain = tc.Draw(Gen.auto<Chain>, "c")
            tc.Assume chain.Next.IsSome)

    Assert.Equal<string seq>([ "let c_1 = { Value = 0; Next = Some({ Value = 0; Next = None }) }" ], linked.Output)

[<Fact>]
let ``recursion through value options and arrays`` () =
    Assert.Equal("let x_1 = Spine(ValueNone)", printed Gen.auto<Spine>)
    Assert.Equal("let x_1 = Branches([||])", printed Gen.auto<Branches>)

    let deeper =
        failing (fun tc ->
            match tc.Draw(Gen.auto<Spine>, "s") with
            | Spine ValueNone -> tc.Reject()
            | Spine _ -> ())

    Assert.Equal<string seq>([ "let s_1 = Spine(ValueSome(Spine(ValueNone)))" ], deeper.Output)

[<Fact>]
let ``recursion through sets and maps`` () =
    Assert.Equal("let x_1 = Forest(set [])", printed Gen.auto<Forest>)
    Assert.Equal("let x_1 = Index(Map [])", printed Gen.auto<Index>)

    let deeper =
        failing (fun tc ->
            match tc.Draw(Gen.auto<Forest>, "f") with
            | Forest s when s.IsEmpty -> tc.Reject()
            | Forest _ -> ())

    Assert.Equal<string seq>([ "let f_1 = Forest(set [Forest(set [])])" ], deeper.Output)

[<Fact>]
let ``recursion through a tuple inside a list`` () =
    Assert.Equal("let x_1 = { Tags = set []; Pairs = [] }", printed Gen.auto<Tagged>)

    let deeper =
        failing (fun tc ->
            let tagged = tc.Draw(Gen.auto<Tagged>, "t")
            tc.Assume(not tagged.Pairs.IsEmpty))

    Assert.Equal<string seq>(
        [ "let t_1 = { Tags = set []; Pairs = [(0, { Tags = set []; Pairs = [] })] }" ],
        deeper.Output
    )

[<Fact>]
let ``a union whose only base case is through an option`` () =
    Assert.Equal("let x_1 = Wrapped(None)", printed Gen.auto<Wrapped>)

    let deeper =
        failing (fun tc ->
            match tc.Draw(Gen.auto<Wrapped>, "w") with
            | Wrapped None -> tc.Reject()
            | Wrapped _ -> ())

    Assert.Equal<string seq>([ "let w_1 = Wrapped(Some(Wrapped(None)))" ], deeper.Output)

[<Fact>]
let ``mutually recursive unions`` () =
    Assert.Equal("let x_1 = ALeaf", printed Gen.auto<A>)

    let viaB =
        failing (fun tc ->
            match tc.Draw(Gen.auto<A>, "a") with
            | ALeaf -> tc.Reject()
            | ANode _ -> ())

    Assert.Equal<string seq>([ "let a_1 = ANode(BNode(ALeaf))" ], viaB.Output)
    Assert.Equal("let x_1 = BNode(ALeaf)", printed Gen.auto<B>)

[<Fact>]
let ``a record and a union recursive through each other`` () =
    Assert.Equal("let x_1 = { Name = \"\"; Sub = None }", printed Gen.auto<Item>)
    Assert.Equal("let x_1 = Empty", printed Gen.auto<Group>)

    let nested =
        failing (fun tc ->
            let item = tc.Draw(Gen.auto<Item>, "i")
            tc.Assume item.Sub.IsSome)

    Assert.Equal<string seq>([ "let i_1 = { Name = \"\"; Sub = Some(Empty) }" ], nested.Output)

[<Fact>]
let ``a type with no base case cannot be derived`` () =
    let union = Assert.Throws<HegelUsageException>(fun () -> Gen.auto<Inf> |> ignore)
    Assert.Contains(typeof<Inf>.FullName, union.Message)
    Assert.Contains("no base case", union.Message)
    let record = Assert.Throws<HegelUsageException>(fun () -> Gen.auto<Loop> |> ignore)
    Assert.Contains(typeof<Loop>.FullName, record.Message)
    Assert.Contains("no base case", record.Message)

[<Fact>]
let ``an unsupported leaf is the C# derivation's error`` () =
    let plain = Assert.Throws<HegelUsageException>(fun () -> Gen.auto<Plain> |> ignore)
    Assert.Contains("No default generator", plain.Message)

    let nested =
        Assert.Throws<HegelUsageException>(fun () -> Gen.auto<Box<Plain>> |> ignore)

    Assert.Contains("No default generator", nested.Message)

[<Fact>]
let ``a C# collection of an F# type is not derived`` () =
    Assert.Throws<HegelUsageException>(fun () -> Gen.auto<ResizeArray<int option>> |> ignore)
    |> ignore

[<Fact>]
let ``derived generators are cached per type`` () =
    Assert.True(obj.ReferenceEquals(Gen.auto<Tree>, Gen.auto<Tree>))
    Assert.True(obj.ReferenceEquals(Gen.auto<Point>, Gen.auto<Point>))
    Assert.False(obj.ReferenceEquals(Gen.auto<Point>, Gen.auto<SPoint>))

[<Fact>]
let ``record generators derive the fields they are not given`` () =
    let generator = Gen.record<Point>.With("X", Gen.constant 7)
    Assert.Equal("let x_1 = { X = 7; Y = 0 }", printed generator)
    check generator (fun point -> Assert.Equal(7, point.X))

[<Fact>]
let ``derived generators carry the labels of their shape`` () =
    Assert.Equal(Label.Combine(Label.OneOf, Label.Of typeof<Shape>.FullName), Gen.auto<Shape>.Label)
    Assert.Equal(Label.Combine(Label.List, Gen.auto<int>.Label), Gen.auto<int list>.Label)
