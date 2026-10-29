module Hegel.FSharp.Tests.HegelTests

open System
open System.IO
open Xunit
open Hegel.FSharp.Tests.Support
open Hegel
open Hegel.FSharp

type Counter() =
    member val N = 0 with get, set

    [<Rule>]
    member this.Increment(_: TestCase) = this.N <- this.N + 1

    [<Rule>]
    member this.Decrement(tc: TestCase) =
        tc.Assume(this.N > 0)
        this.N <- this.N - 1

    [<Invariant>]
    member this.NonNegative(_: TestCase) = Assert.True(this.N >= 0)

type Captured() =
    inherit Reporter()
    member val Settings: Settings option = None with get, set
    override this.RunStarted(settings) = this.Settings <- Some settings

let private body (tc: TestCase) =
    let x = tc.Draw(Gen.list Gen.int, "x")
    Assert.Equal(0, x.Length)

[<Fact>]
let ``test and run with the default settings report a passing property`` () =
    let tested = Hegel.test (fun tc -> tc.Draw Gen.int |> ignore)
    Assert.True tested.Passed
    let report = Hegel.run (fun tc -> tc.Draw Gen.int |> ignore)
    Assert.True report.Passed
    Assert.Equal(RunStatus.Passed, report.Status)

[<Fact>]
let ``test with settings throws and run with settings reports`` () =
    Assert.Throws<Xunit.Sdk.EqualException>(fun () -> Hegel.test (quiet, body) |> ignore)
    |> ignore

    let report = Hegel.run (quiet, body)
    Assert.Equal(RunStatus.Failed, report.Status)
    Assert.False report.Passed
    let failure = Seq.exactlyOne report.Failures
    Assert.Equal<string>([ "x_1" ], failure.Draws.Keys |> List.ofSeq)
    Assert.Equal<obj>(box [ 0 ], failure.Draws["x_1"])
    Assert.Equal("let x_1 = [0]", String.concat "\n" failure.Output)
    Assert.StartsWith("EqualException at HegelTests.fs:", failure.Origin)

[<Fact>]
let ``test and run with a reporter send it the transcript`` () =
    let output = new StringWriter()

    let normal =
        Settings(Database = Database.Disabled, Verbosity = Verbosity.Normal, PrintBlob = false)

    let report = Hegel.run (normal, Reporter.Printing output, body)
    Assert.False report.Passed
    Assert.Contains("let x_1 = [0]", output.ToString())
    let again = new StringWriter()

    Assert.Throws<Xunit.Sdk.EqualException>(fun () -> Hegel.test (normal, Reporter.Printing again, body) |> ignore)
    |> ignore

    Assert.Contains("let x_1 = [0]", again.ToString())

[<Fact>]
let ``the test is identified from the calling function`` () =
    let captured = Captured()
    Hegel.test (quiet, captured, (fun tc -> tc.Draw Gen.int |> ignore)) |> ignore
    let settings = captured.Settings.Value
    Assert.EndsWith("HegelTests.fs", settings.TestLocation.File)
    Assert.Equal("the test is identified from the calling function", settings.TestLocation.Function)
    Assert.Equal("Hegel.FSharp.Tests.HegelTests", settings.TestLocation.ClassName)
    Assert.True(settings.TestLocation.BeginLine > 0)

    Assert.Equal(
        settings.TestLocation.File
        + "::the test is identified from the calling function",
        settings.Name
    )

[<Fact>]
let ``the F# dialect is installed for the run only`` () =
    Assert.Same(Dialect.CSharp, Dialect.Current)
    let mutable seen: Dialect option = None
    Hegel.run (quiet, (fun _ -> seen <- Some Dialect.Current)) |> ignore
    Assert.Same(Repr.dialect, seen.Value)
    Assert.Same(Dialect.CSharp, Dialect.Current)

[<Fact>]
let ``engine version is the core's`` () =
    Assert.Equal(Property.EngineVersion, Hegel.engineVersion)
    Assert.NotEmpty Hegel.engineVersion

[<Fact>]
let ``settings limit the test cases`` () =
    let report =
        Hegel.run (
            Settings(Database = Database.Disabled, Verbosity = Verbosity.Quiet, PrintBlob = false, TestCases = 5L),
            (fun tc -> tc.Draw Gen.int |> ignore)
        )

    Assert.True report.Passed
    Assert.Equal(5L, report.Statistics.Valid)
    Assert.Equal(5L, report.Statistics.Total)

[<Fact>]
let ``a stateful machine written in F# runs`` () =
    let mutable steps = 0

    let report =
        Hegel.run (
            quiet,
            (fun tc ->
                let machine = Counter()
                Stateful.Run(machine, tc, 5)
                steps <- max steps machine.N)
        )

    Assert.True report.Passed
    Assert.True(steps <= 5)

[<Fact>]
let ``explicit examples supply F# values`` () =
    let settings =
        Settings(
            Database = Database.Disabled,
            Verbosity = Verbosity.Normal,
            PrintBlob = false,
            Phases = Phase.Explicit,
            Examples = [ Example.Of().With("x_1", [ 1; 2 ]) ]
        )

    let report = Hegel.run (settings, Reporter.Silent, body)
    let failure = Seq.exactlyOne report.Failures
    Assert.Equal("let x_1 = [1; 2]", String.concat "\n" failure.Output)
    Assert.Equal<obj>(box [ 1; 2 ], failure.Draws["x_1"])
