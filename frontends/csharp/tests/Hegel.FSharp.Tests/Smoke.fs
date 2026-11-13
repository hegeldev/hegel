module Hegel.FSharp.Tests.Smoke

open Xunit
open Hegel
open Hegel.FSharp
open Hegel.FSharp.Tests.Support

type Colour =
    | Red
    | Green
    | Blue

type Primary =
    | Cyan = 1
    | Magenta = 2

type Point = { X: int; Y: int }

type Tree =
    | Leaf
    | Node of Tree * int * Tree

type Shape =
    | Circle of radius: float
    | Rect of width: int * height: int

[<Fact>]
let ``smoke: a sort test fails with an F# transcript`` () =
    let report =
        Hegel.run (
            quiet,
            fun tc ->
                let xs = tc.Draw(Gen.list Gen.int, "xs")
                let sorted = xs |> Set.ofList |> Set.toList
                Assert.Equal(xs.Length, sorted.Length)
        )

    let failure = Seq.exactlyOne report.Failures
    Assert.Equal("let xs_1 = [0; 0]", String.concat "\n" failure.Output)
    Assert.StartsWith("EqualException at Smoke.fs:", failure.Origin)

[<Fact>]
let ``smoke: printing`` () =
    Assert.Equal("[1; 2]", show [ 1; 2 ])
    Assert.Equal("[|1uy|]", show [| 1uy |])
    Assert.Equal("Some(-1)", show (Some -1))
    Assert.Equal("None", show (None: int option))
    Assert.Equal("()", show ())
    Assert.Equal("{ X = 1; Y = 2 }", show { X = 1; Y = 2 })
    Assert.Equal("Node(Leaf, 1, Leaf)", show (Node(Leaf, 1, Leaf)))
    Assert.Equal("Red", show Red)
    Assert.Equal("Primary.Cyan", show Primary.Cyan)
    Assert.Equal("\"a\\u0000b\"", show "a\000b")
    Assert.Equal("(1, \"a\")", show (1, "a"))
    Assert.Equal("Map [(1, \"a\")]", show (Map [ 1, "a" ]))
    Assert.Equal("set [1; 2]", show (set [ 1; 2 ]))
    Assert.Equal("nan", show nan)
    Assert.Equal("1.5f", show 1.5f)
    Assert.Equal("{| A = 1 |}", show {| A = 1 |})
    Assert.Equal("Circle(1.5)", show (Circle 1.5))
    Assert.Equal("Ok(1)", show (Ok 1: Result<int, string>))

[<Fact>]
let ``smoke: auto derives trees and records`` () =
    let tree = printed Gen.auto<Tree>
    Assert.StartsWith("let x_1 = ", tree)
    let point = printed Gen.auto<Point>
    Assert.StartsWith("let x_1 = { X = ", point)
    let shape = printed Gen.auto<Shape>
    Assert.StartsWith("let x_1 = ", shape)
    let colour = printed Gen.auto<Colour>
    Assert.StartsWith("let x_1 = ", colour)
    let opt = printed Gen.auto<int option list>
    Assert.StartsWith("let x_1 = [", opt)

[<Fact>]
let ``smoke: gen builder`` () =
    let ranges =
        gen {
            let! lo = Gen.int
            let! hi = Gen.int.Min lo
            return (lo, hi)
        }

    Hegel.test (
        quiet,
        fun tc ->
            let (lo, hi) = tc.Draw ranges
            Assert.True(lo <= hi)
    )
    |> ignore

    Assert.Equal("let x_1 = (0, 0)", printed ranges)

[<Fact>]
let ``smoke: size and bound helpers`` () =
    let xs = (Gen.list Gen.int).MinSize(1).MaxSize(3)
    let n = Gen.int.Min(5).Max(7)

    Hegel.test (
        quiet,
        fun tc ->
            let value = tc.Draw(Gen.tuple2 xs n)
            Assert.InRange((fst value).Length, 1, 3)
            Assert.InRange(snd value, 5, 7)
    )
    |> ignore
