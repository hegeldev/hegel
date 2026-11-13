module Hegel.FSharp.Tests.GenTests

open System
open System.Numerics
open Xunit
open Hegel.FSharp.Tests.Support
open Hegel
open Hegel.FSharp

type Point = { X: int; Y: int }

let private draw (generator: Generator<'T>) : 'T =
    let mutable value = Unchecked.defaultof<'T>

    Hegel.test (
        Settings(Database = Database.Disabled, Verbosity = Verbosity.Quiet, PrintBlob = false, TestCases = 1L),
        fun tc -> value <- tc.Draw generator
    )
    |> ignore

    value

let private holds (generator: Generator<'T>) (property: 'T -> bool) =
    Hegel.test (
        Settings(Database = Database.Disabled, Verbosity = Verbosity.Quiet, PrintBlob = false, TestCases = 30L),
        fun tc -> Assert.True(property (tc.Draw generator))
    )
    |> ignore

[<Fact>]
let ``constant prints its value, including typed None and unit`` () =
    Assert.Equal("let x_1 = 5", printed (Gen.constant 5))
    Assert.Equal("let x_1 = None", printed (Gen.constant (None: int option)))
    Assert.Equal("let x_1 = ()", printed (Gen.constant ()))

[<Fact>]
let ``item chooses among the values and rejects none`` () =
    holds (Gen.item [ "a"; "b" ]) (fun s -> s = "a" || s = "b")
    Assert.Equal("let x_1 = \"a\"", printed (Gen.item [ "a"; "b" ]))

    Assert.Throws<ArgumentException>(fun () -> Gen.item ([]: int list) |> ignore)
    |> ignore

[<Fact>]
let ``oneOf draws from one of the generators and rejects none`` () =
    holds (Gen.oneOf [ Gen.constant 1; Gen.constant 2 ]) (fun n -> n = 1 || n = 2)
    Assert.Equal("let x_1 = 1", printed (Gen.oneOf [ Gen.constant 1; Gen.constant 2 ]))

    Assert.Throws<ArgumentException>(fun () -> Gen.oneOf ([]: Generator<int> list) |> ignore)
    |> ignore

[<Fact>]
let ``map prints the result as F#`` () =
    Assert.Equal("let x_1 = [0; 0]", printed (Gen.int |> Gen.map (fun n -> [ n; n ])))
    holds (Gen.int |> Gen.map (fun n -> n * 2)) (fun n -> n % 2 = 0)

[<Fact>]
let ``filter keeps the values the predicate accepts`` () =
    holds (Gen.int |> Gen.filter (fun n -> n % 2 = 0)) (fun n -> n % 2 = 0)
    Assert.Equal("let x_1 = 1", printed (Gen.int |> Gen.filter (fun n -> n % 2 = 1)))

[<Fact>]
let ``bind draws from the generator chosen for the first value`` () =
    let pairs =
        Gen.int.Min(1).Max(3) |> Gen.bind (fun n -> Gen.constant [ for _ in 1..n -> n ])

    holds pairs (fun xs -> xs.Length = xs.Head)
    Assert.Equal("let x_1 = [1]", printed pairs)

[<Fact>]
let ``option and voption draw both cases`` () =
    holds (Gen.option Gen.int) (fun _ -> true)
    holds (Gen.voption Gen.int) (fun _ -> true)
    Assert.Equal("let x_1 = None", printed (Gen.option Gen.int))
    Assert.Equal("let x_1 = ValueNone", printed (Gen.voption Gen.int))
    Assert.Equal("let x_1 = Some(0)", printed (Gen.option Gen.int |> Gen.filter Option.isSome))
    Assert.Equal("let x_1 = ValueSome(0)", printed (Gen.voption Gen.int |> Gen.filter ValueOption.isSome))

[<Fact>]
let ``collections honour the size helpers`` () =
    holds ((Gen.list Gen.int).MinSize(1).MaxSize(3)) (fun xs -> xs.Length >= 1 && xs.Length <= 3)
    holds ((Gen.array Gen.int).MinSize(2).MaxSize(2)) (fun xs -> xs.Length = 2)
    holds ((Gen.set Gen.int).MinSize(1).MaxSize(4)) (fun s -> s.Count >= 1 && s.Count <= 4)
    holds ((Gen.mapOf Gen.int Gen.string).MinSize(1).MaxSize(2)) (fun m -> m.Count >= 1 && m.Count <= 2)
    Assert.Equal("let x_1 = [0]", printed ((Gen.list Gen.int).MinSize(1)))
    Assert.Equal("let x_1 = [|0; 0|]", printed ((Gen.array Gen.int).MinSize(2)))
    Assert.Equal("let x_1 = set [0]", printed ((Gen.set Gen.int).MinSize(1)))
    Assert.Equal("let x_1 = Map [(0, \"\")]", printed ((Gen.mapOf Gen.int Gen.string).MinSize(1)))

[<Fact>]
let ``tuples draw one element from each generator`` () =
    Assert.Equal("let x_1 = (0, false)", printed (Gen.tuple2 Gen.int Gen.bool))
    Assert.Equal("let x_1 = (0, false, \"\")", printed (Gen.tuple3 Gen.int Gen.bool Gen.string))
    Assert.Equal("let x_1 = (0, false, \"\", 0L)", printed (Gen.tuple4 Gen.int Gen.bool Gen.string Gen.int64))

    Assert.Equal(
        "let x_1 = (0, false, \"\", 0L, ())",
        printed (Gen.tuple5 Gen.int Gen.bool Gen.string Gen.int64 (Gen.constant ()))
    )

    Assert.Equal("let x_1 = (0, 0)", printed (Gen.zip Gen.int Gen.int))
    holds (Gen.tuple5 Gen.int Gen.bool Gen.string Gen.int64 Gen.float) (fun (_, _, s, _, _) -> not (isNull s))

[<Fact>]
let ``map2 and map3 combine draws`` () =
    holds (Gen.map2 (fun a b -> a + b) (Gen.constant 1) (Gen.constant 2)) (fun n -> n = 3)
    holds (Gen.map3 (fun a b c -> a + b + c) (Gen.constant 1) (Gen.constant 2) (Gen.constant 3)) (fun n -> n = 6)
    Assert.Equal("let x_1 = 3", printed (Gen.map2 (fun a b -> a + b) (Gen.constant 1) (Gen.constant 2)))

[<Fact>]
let ``composite draws inside and prints its result`` () =
    let composite = Gen.composite (fun tc -> tc.Draw Gen.int + tc.Draw Gen.int + 1)
    holds composite (fun _ -> true)
    Assert.Equal("let x_1 = 1", printed composite)

[<Fact>]
let ``recursive builds bounded structures`` () =
    let sums =
        Gen.recursive (Gen.constant 1) (fun self -> Gen.map2 (fun a b -> a + b) self self)

    holds (sums.MaxDepth(3).MaxLeaves(8)) (fun n -> n >= 1 && n <= 8)
    Assert.Equal("let x_1 = 1", printed (sums.MaxDepth 2))

[<Fact>]
let ``deferred draws from the generator set later`` () =
    let later: Deferred<int> = Gen.deferred ()
    later.Set(Gen.constant 7)
    holds later (fun n -> n = 7)

[<Fact>]
let ``auto and record derive generators`` () =
    holds Gen.auto<Point> (fun _ -> true)
    holds (Gen.record<Point>.With("X", Gen.constant 1)) (fun p -> p.X = 1)
    Assert.Equal("let x_1 = { X = 1; Y = 0 }", printed (Gen.record<Point>.With("X", Gen.constant 1)))

[<Fact>]
let ``permutations subsequences and samples are F# lists of the input`` () =
    holds (Gen.permutations [ 3; 1; 2 ]) (fun xs -> List.sort xs = [ 1; 2; 3 ])
    holds (Gen.subsequences [ 1; 2; 3 ]) (fun xs -> xs = List.filter (fun x -> List.contains x xs) [ 1; 2; 3 ])
    holds (Gen.samples [ 1; 2; 3 ]) (fun xs -> List.forall (fun x -> List.contains x [ 1; 2; 3 ]) xs)
    Assert.Equal("let x_1 = [3; 1; 2]", printed (Gen.permutations [ 3; 1; 2 ]))
    Assert.Equal("let x_1 = []", printed (Gen.subsequences [ 1; 2; 3 ]))
    Assert.Equal("let x_1 = []", printed (Gen.samples [ 1; 2; 3 ]))

[<Fact>]
let ``bound helpers work on the C# builders`` () =
    holds (Gen.string.MaxSize(3)) (fun s -> Seq.length (s.EnumerateRunes()) <= 3)
    holds (Gen.string.MinSize(2)) (fun s -> s.Length >= 2)
    holds (Gen.float.Min(0.0)) (fun d -> d >= 0.0)
    holds (Gen.float.Max(0.0)) (fun d -> d <= 0.0)
    holds (Gen.int.Min(5).Max(7)) (fun n -> n >= 5 && n <= 7)
    holds (Gen.int64.Min(5L).Max(7L)) (fun n -> n >= 5L && n <= 7L)

[<Fact>]
let ``numeric scalars print with F# suffixes`` () =
    Assert.Equal("let x_1 = 0", printed Gen.int)
    Assert.Equal("let x_1 = 0L", printed Gen.int64)
    Assert.Equal("let x_1 = 0I", printed Gen.bigint)
    Assert.Equal("let x_1 = 0.0", printed Gen.float)
    Assert.Equal("let x_1 = 0.0f", printed Gen.float32)
    Assert.Equal("let x_1 = false", printed Gen.bool)
    Assert.Equal("let x_1 = false", printed (Gen.weightedBool 0.3))
    holds (Gen.float.AllowNan true) (fun _ -> true)
    Assert.Equal("let x_1 = nan", printed (Gen.float |> Gen.map (fun _ -> nan)))
    Assert.Equal("let x_1 = infinity", printed (Gen.float |> Gen.map (fun _ -> infinity)))
    let big: BigInteger = draw Gen.bigint
    Assert.Equal(big, big)
    holds (Gen.weightedBool 1.0) id
    holds (Gen.weightedBool 0.0) not

[<Fact>]
let ``text scalars draw the right types`` () =
    Assert.Equal("let x_1 = \"\"", printed Gen.string)
    Assert.Equal("let x_1 = '\\u0000'", printed Gen.char)
    holds Gen.char (fun c -> not (Char.IsSurrogate c))
    Assert.Equal("let x_1 = [||]", printed Gen.bytes)
    holds (Gen.regex "[a-c]+") (fun s -> s.Length > 0 && Seq.forall (fun c -> c >= 'a' && c <= 'c') s)
    Assert.Equal("let x_1 = \"a\"", printed (Gen.regex "[a-c]+"))
    holds (Gen.lark "start: \"x\" | \"y\"") (fun s -> s = "x" || s = "y")
    holds Gen.email (fun s -> s.Contains "@")
    holds Gen.url (fun s -> s.Contains "://")
    holds Gen.domain (fun s -> s.Contains ".")
    holds (Gen.ipAddress.V4()) (fun s -> s.Split('.').Length = 4)
    holds (Gen.ipAddress.V6()) (fun s -> s.Contains ":")
    let address: string = draw Gen.ipAddress
    Assert.True(address.Contains "." || address.Contains ":")
    Assert.StartsWith("let x_1 = \"", printed Gen.email)

[<Fact>]
let ``guid and temporal scalars draw the right types`` () =
    let id: Guid = draw Gen.guid
    Assert.NotEqual(Guid.Empty, id)
    Assert.StartsWith("let x_1 = Guid.Parse(\"", printed Gen.guid)
    Assert.Equal("let x_1 = DateOnly.Parse(\"2000-01-01\")", printed Gen.dateOnly)
    Assert.Equal("let x_1 = TimeOnly.Parse(\"00:00:00\")", printed Gen.timeOnly)
    Assert.Equal("let x_1 = DateTime.Parse(\"2000-01-01T00:00:00\")", printed Gen.dateTime)
    Assert.Equal("let x_1 = DateTimeOffset.Parse(\"2000-01-01T00:00:00+00:00\")", printed Gen.dateTimeOffset)
    Assert.Equal("let x_1 = TimeSpan.Parse(\"00:00:00\")", printed Gen.timeSpan)
    Assert.Equal("let x_1 = TimeSpan.Parse(\"00:00:00\")", printed Gen.utcOffset)
    Assert.StartsWith("let x_1 = TimeZoneInfo.FindSystemTimeZoneById(\"", printed Gen.timeZone)
    let zone: TimeZoneInfo = draw Gen.timeZone
    Assert.NotNull zone
    let offset: DateTimeOffset = draw Gen.dateTimeOffset
    Assert.True(abs offset.Offset.TotalHours <= 24.0)

[<Fact>]
let ``gen blocks bind return and print their value`` () =
    let pairs =
        gen {
            let! lo = Gen.int
            let! hi = Gen.int.Min(lo)
            return (lo, hi)
        }

    holds pairs (fun (lo, hi) -> lo <= hi)
    Assert.Equal("let x_1 = (0, 0)", printed pairs)

    let mapped =
        gen {
            let! n = Gen.int
            return n + 1
        }

    Assert.Equal("let x_1 = 1", printed mapped)
    let forwarded = gen { return! Gen.constant "z" }
    Assert.Equal("let x_1 = \"z\"", printed forwarded)

    let nested =
        gen {
            let! inner = forwarded
            let! more = mapped
            return! Gen.constant (inner, more)
        }

    Assert.Equal("let x_1 = (\"z\", 1)", printed nested)
    Assert.Equal("let x_1 = 3", printed (gen.Return 3))
    Assert.Equal("let x_1 = 4", printed (gen.Delay(fun () -> gen.Return 4)))
    Assert.Equal("let x_1 = 6", printed (gen.BindReturn(Gen.constant 5, fun n -> n + 1)))
    Assert.Equal("let x_1 = 7", printed (gen.Bind(Gen.constant 6, fun n -> gen.Return(n + 1))))
    Assert.Equal("let x_1 = 8", printed (gen.ReturnFrom(Gen.constant 8)))
