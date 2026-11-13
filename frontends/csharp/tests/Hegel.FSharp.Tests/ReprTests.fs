module Hegel.FSharp.Tests.ReprTests

open System
open System.Collections.Generic
open System.Numerics
open Xunit
open Hegel.FSharp.Tests.Support
open Hegel
open Hegel.FSharp

type Primary =
    | Cyan = 1
    | Magenta = 2

[<Flags>]
type Bits =
    | A = 1
    | B = 2

type Colour =
    | Red
    | Green

type Shape =
    | Circle of radius: float
    | Rect of width: int * height: int

type Tree =
    | Leaf
    | Node of Tree * int * Tree

type Point = { X: int; Y: int }

[<Struct>]
type Vec = { Dx: int; Dy: int }

type Secret = private { Hidden: int }

type Sealed =
    private
    | Closed
    | Opened of int

type Plain() =
    override _.ToString() = "Plain()"

let private makeSecret n = { Hidden = n }
let private makeSealed n = Opened n
let private closed = Closed

[<Fact>]
let ``null, None and unit`` () =
    Assert.Equal("null", render typeof<obj> null)
    Assert.Equal("null", render typeof<string> null)
    Assert.Equal("None", show (None: int option))
    Assert.Equal("()", show ())

[<Fact>]
let ``strings are quoted with every escape`` () =
    Assert.Equal("\"a\\nb\\tc\\rd\\\\e\\\"f\"", show "a\nb\tc\rd\\e\"f")
    Assert.Equal("\"\\u0001\\u0000\\u00a0\\u2028\\u2029\"", show "\u0001\u0000\u00a0\u2028\u2029")
    Assert.Equal("\"\\ud800\"", show (string (char 0xD800)))
    Assert.Equal("\"\\udc00\"", show (string (char 0xDC00)))
    Assert.Equal("\"\\ud800x\"", show (String([| char 0xD800; 'x' |])))
    Assert.Equal("\"\U0001F600\"", show "\U0001F600")
    Assert.Equal("\"\"", show "")

[<Fact>]
let ``chars are quoted`` () =
    Assert.Equal("'a'", show 'a')
    Assert.Equal("'\\''", show '\'')
    Assert.Equal("'\"'", show '"')
    Assert.Equal("'\\n'", show '\n')

[<Fact>]
let ``quote is exposed`` () =
    Assert.Equal("'x'", Repr.quote "x" '\'')
    Assert.Equal("\"it's\"", Repr.quote "it's" '"')

[<Fact>]
let ``booleans`` () =
    Assert.Equal("true", show true)
    Assert.Equal("false", show false)

[<Fact>]
let ``integers carry their suffixes`` () =
    Assert.Equal("1", show 1)
    Assert.Equal("-1", show -1)
    Assert.Equal("1L", show 1L)
    Assert.Equal("1s", show 1s)
    Assert.Equal("1y", show 1y)
    Assert.Equal("1uy", show 1uy)
    Assert.Equal("1us", show 1us)
    Assert.Equal("1u", show 1u)
    Assert.Equal("1UL", show 1UL)
    Assert.Equal("1n", show 1n)
    Assert.Equal("1un", show 1un)
    Assert.Equal("5I", show 5I)
    Assert.Equal("-5I", show (BigInteger(-5)))

[<Fact>]
let ``floats`` () =
    Assert.Equal("1.0", show 1.0)
    Assert.Equal("1.5", show 1.5)
    Assert.Equal("-2.5", show -2.5)
    Assert.Equal("1E+100", show 1E+100)
    Assert.Equal("nan", show nan)
    Assert.Equal("infinity", show infinity)
    Assert.Equal("-infinity", show -infinity)

[<Fact>]
let ``float32s`` () =
    Assert.Equal("1.0f", show 1.0f)
    Assert.Equal("1.5f", show 1.5f)
    Assert.Equal("1E+10f", show 1E+10f)
    Assert.Equal("nanf", show nanf)
    Assert.Equal("infinityf", show infinityf)
    Assert.Equal("-infinityf", show -infinityf)

[<Fact>]
let ``decimals`` () =
    Assert.Equal("1.5M", show 1.5M)
    Assert.Equal("-3M", show -3M)

[<Fact>]
let ``enums`` () =
    Assert.Equal("Primary.Cyan", show Primary.Cyan)
    Assert.Equal("Bits.A ||| Bits.B", show (Bits.A ||| Bits.B))
    Assert.Equal("Bits.A", show Bits.A)
    Assert.Equal("enum<Primary> (42)", show (enum<Primary> 42))
    Assert.Equal("enum<Primary> (-1)", show (enum<Primary> -1))

[<Fact>]
let ``scalars with parse forms`` () =
    let guid = Guid.Parse "12345678-1234-1234-1234-123456789abc"
    Assert.Equal("Guid.Parse(\"12345678-1234-1234-1234-123456789abc\")", show guid)
    Assert.Equal("DateOnly.Parse(\"2024-02-29\")", show (DateOnly(2024, 2, 29)))
    Assert.Equal("TimeOnly.Parse(\"13:45:00\")", show (TimeOnly(13, 45)))
    Assert.Equal("DateTime.Parse(\"2024-02-29T13:45:00\")", show (DateTime(2024, 2, 29, 13, 45, 0)))

    Assert.Equal(
        "DateTimeOffset.Parse(\"2024-02-29T13:45:00+02:00\")",
        show (DateTimeOffset(2024, 2, 29, 13, 45, 0, TimeSpan.FromHours 2.0))
    )

    Assert.Equal("TimeSpan.Parse(\"01:02:03\")", show (TimeSpan(1, 2, 3)))
    Assert.Equal("TimeZoneInfo.FindSystemTimeZoneById(\"UTC\")", show TimeZoneInfo.Utc)

[<Fact>]
let ``arrays`` () =
    Assert.Equal("[|1; 2|]", show [| 1; 2 |])
    Assert.Equal("[||]", show ([||]: int[]))
    Assert.Equal("[|1uy; 2uy|]", show [| 1uy; 2uy |])
    Assert.Equal("[|\"a\"; null|]", show [| "a"; null |])

[<Fact>]
let ``lists`` () =
    Assert.Equal("[1; 2; 3]", show [ 1; 2; 3 ])
    Assert.Equal("[]", show ([]: int list))
    Assert.Equal("[[1]; []]", show [ [ 1 ]; [] ])

[<Fact>]
let ``options`` () =
    Assert.Equal("Some(1)", show (Some 1))
    Assert.Equal("Some(-1)", show (Some -1))
    Assert.Equal("Some(None)", show (Some(None: int option)))
    Assert.Equal("Some(Some(1))", show (Some(Some 1)))
    Assert.Equal("[Some(1); None]", show [ Some 1; None ])
    Assert.Equal("Some(\"a\")", show (Some "a"))

[<Fact>]
let ``value options`` () =
    Assert.Equal("ValueSome(1)", show (ValueSome 1))
    Assert.Equal("ValueNone", show (ValueNone: int voption))
    Assert.Equal("[ValueSome(1); ValueNone]", show [ ValueSome 1; ValueNone ])

[<Fact>]
let ``sets and maps`` () =
    Assert.Equal("set [1; 2]", show (set [ 2; 1 ]))
    Assert.Equal("set []", show (Set.empty: Set<int>))
    Assert.Equal("Map [(1, \"a\"); (2, \"b\")]", show (Map [ 2, "b"; 1, "a" ]))
    Assert.Equal("Map []", show (Map.empty: Map<int, string>))
    Assert.Equal("Map [(1, None)]", show (Map [ 1, (None: int option) ]))

[<Fact>]
let ``dotnet collections`` () =
    Assert.Equal("KeyValuePair(1, \"a\")", show (KeyValuePair(1, "a")))
    Assert.Equal("ResizeArray [1; 2]", show (ResizeArray [ 1; 2 ]))
    Assert.Equal("HashSet [1]", show (HashSet [ 1 ]))
    Assert.Equal("dict [(1, \"a\")]", show (Dictionary(dict [ 1, "a" ])))
    Assert.Equal("seq [1; 2]", show (seq { 1..2 }))
    Assert.Equal("seq [1; 2]", show (Seq.ofList [ 1; 2 ] |> Seq.map id))

[<Fact>]
let ``dict renders as dict`` () =
    Assert.Equal("dict [(1, \"a\")]", show (dict [ 1, "a" ]))

[<Fact>]
let ``tuples`` () =
    Assert.Equal("(1, \"a\")", show (1, "a"))
    Assert.Equal("struct (1, 2)", show (struct (1, 2)))
    Assert.Equal("(1, (2, 3), None)", show (1, (2, 3), (None: int option)))

[<Fact>]
let ``records`` () =
    Assert.Equal("{ X = 1; Y = 2 }", show { X = 1; Y = 2 })
    Assert.Equal("{ Dx = 1; Dy = 2 }", show { Dx = 1; Dy = 2 })
    Assert.Equal("{ Hidden = 3 }", show (makeSecret 3))
    Assert.Equal("{| A = 1; B = \"b\" |}", show {| A = 1; B = "b" |})
    Assert.Equal("{ X = 1; Y = 2 }", render typeof<obj> (box { X = 1; Y = 2 }))

[<Fact>]
let ``unions`` () =
    Assert.Equal("Red", show Red)
    Assert.Equal("Circle(1.5)", show (Circle 1.5))
    Assert.Equal("Rect(1, 2)", show (Rect(1, 2)))
    Assert.Equal("Node(Leaf, 1, Node(Leaf, 2, Leaf))", show (Node(Leaf, 1, Node(Leaf, 2, Leaf))))
    Assert.Equal("Ok(1)", show (Ok 1: Result<int, string>))
    Assert.Equal("Error(\"e\")", show (Error "e": Result<int, string>))
    Assert.Equal("Choice1Of2(1)", show (Choice1Of2 1: Choice<int, string>))
    Assert.Equal("Closed", show closed)
    Assert.Equal("Opened(4)", show (makeSealed 4))

[<Fact>]
let ``other objects reflow their ToString`` () =
    Assert.Equal("Plain()", show (Plain()))
    Assert.Equal("Plain()", render typeof<obj> (box (Plain())))

[<Fact>]
let ``type names`` () =
    Assert.Equal("int", Repr.typeName typeof<int>)
    Assert.Equal("string", Repr.typeName typeof<string>)
    Assert.Equal("bigint", Repr.typeName typeof<BigInteger>)
    Assert.Equal("unit", Repr.typeName typeof<unit>)
    Assert.Equal("int[]", Repr.typeName typeof<int[]>)
    Assert.Equal("int list", Repr.typeName typeof<int list>)
    Assert.Equal("int option", Repr.typeName typeof<int option>)
    Assert.Equal("int voption", Repr.typeName typeof<int voption>)
    Assert.Equal("Dictionary<int, string>", Repr.typeName typeof<Dictionary<int, string>>)
    Assert.Equal("Point", Repr.typeName typeof<Point>)
    Assert.Equal("Primary", Repr.typeName typeof<Primary>)

[<Fact>]
let ``separator`` () =
    use doc = new Document()
    doc.Printer.Text "a"
    Repr.separator doc.Printer
    doc.Printer.Text "b"
    Assert.Equal("a; b", doc.Finish())

[<Fact>]
let ``the dialect binds draws with let and prints as F#`` () =
    use doc = new Document()
    use _scope = Dialect.Use Repr.dialect
    Assert.Same(Repr.dialect, Dialect.Current)
    Dialect.Current.BeginDraw(doc.Printer, "x_1")
    doc.Printer.Repr([ 1; 2 ], typeof<int list>)
    Dialect.Current.EndDraw doc.Printer
    Assert.Equal("let x_1 = [1; 2]", doc.Finish())

[<Fact>]
let ``the dialect prints None for a typed null`` () =
    use doc = new Document()
    use _scope = Dialect.Use Repr.dialect
    doc.Printer.Repr(null, typeof<int option>)
    Assert.Equal("None", doc.Finish())

[<Fact>]
let ``long lists break at the separators`` () =
    use doc = new Document()
    doc.MaxWidth <- 12
    Repr.render doc.Printer typeof<int list> (box [ 1000; 2000; 3000 ])
    Assert.Equal("[1000;\n 2000;\n 3000]", doc.Finish())

[<Fact>]
let ``long records break at the separators`` () =
    use doc = new Document()
    doc.MaxWidth <- 12
    Repr.render doc.Printer typeof<Point> (box { X = 1000; Y = 2000 })
    Assert.Equal("{ X = 1000;\n  Y = 2000 }", doc.Finish())
