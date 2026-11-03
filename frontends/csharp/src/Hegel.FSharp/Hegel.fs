namespace Hegel.FSharp

open System
open System.Runtime.CompilerServices
open System.Runtime.InteropServices
open Hegel

[<assembly: HegelInfrastructure>]
do ()

/// Entry points: <c>Hegel.test</c> runs a property and throws its failure, as a test function wants;
/// <c>Hegel.run</c> returns the <see cref="RunReport"/> instead. Both print the failing test case as
/// F# (<c>let xs_1 = [0; 0]</c>) and identify the test to the database from the calling function.
/// <code>
/// [&lt;Fact&gt;]
/// let ``addition commutes`` () =
///     Hegel.test (fun tc ->
///         let a = tc.Draw Gen.int
///         let b = tc.Draw Gen.int
///         Assert.Equal(a + b, b + a))
/// </code>
[<AbstractClass; Sealed>]
type Hegel =
    static member private Go
        (
            body: TestCase -> unit,
            settings: Settings,
            reporter: Reporter,
            file: string,
            line: int,
            name: string,
            throwFailure: bool
        ) =
        use _dialect = Dialect.Use Repr.dialect
        let body = Action<TestCase> body

        if throwFailure then
            Property.Test(body, settings, reporter, file, line, name)
        else
            Property.Run(body, settings, reporter, file, line, name)

    /// Run <paramref name="body"/> as a property-based test with the default settings and throw the
    /// failure it finds: the minimal failing test case's own exception, with its stack. The remaining
    /// parameters are filled in by the compiler and identify the test to the database.
    static member test
        (
            body: TestCase -> unit,
            [<CallerFilePath; Optional; DefaultParameterValue("")>] file: string,
            [<CallerLineNumber; Optional; DefaultParameterValue(0)>] line: int,
            [<CallerMemberName; Optional; DefaultParameterValue("")>] name: string
        ) : RunReport =
        Hegel.Go(body, null, null, file, line, name, true)

    /// Run <paramref name="body"/> as a property-based test with <paramref name="settings"/> and throw
    /// the failure it finds.
    static member test
        (
            settings: Settings,
            body: TestCase -> unit,
            [<CallerFilePath; Optional; DefaultParameterValue("")>] file: string,
            [<CallerLineNumber; Optional; DefaultParameterValue(0)>] line: int,
            [<CallerMemberName; Optional; DefaultParameterValue("")>] name: string
        ) : RunReport =
        Hegel.Go(body, settings, null, file, line, name, true)

    /// Run <paramref name="body"/> as a property-based test with <paramref name="settings"/>, reporting
    /// to <paramref name="reporter"/>, and throw the failure it finds.
    static member test
        (
            settings: Settings,
            reporter: Reporter,
            body: TestCase -> unit,
            [<CallerFilePath; Optional; DefaultParameterValue("")>] file: string,
            [<CallerLineNumber; Optional; DefaultParameterValue(0)>] line: int,
            [<CallerMemberName; Optional; DefaultParameterValue("")>] name: string
        ) : RunReport =
        Hegel.Go(body, settings, reporter, file, line, name, true)

    /// Run <paramref name="body"/> as <c>test</c> does, but return the report instead of throwing when
    /// it fails. A run that cannot complete (a health check, a stale <c>ReproduceFailure</c> blob)
    /// still throws.
    static member run
        (
            body: TestCase -> unit,
            [<CallerFilePath; Optional; DefaultParameterValue("")>] file: string,
            [<CallerLineNumber; Optional; DefaultParameterValue(0)>] line: int,
            [<CallerMemberName; Optional; DefaultParameterValue("")>] name: string
        ) : RunReport =
        Hegel.Go(body, null, null, file, line, name, false)

    /// Run <paramref name="body"/> with <paramref name="settings"/> and return the report.
    static member run
        (
            settings: Settings,
            body: TestCase -> unit,
            [<CallerFilePath; Optional; DefaultParameterValue("")>] file: string,
            [<CallerLineNumber; Optional; DefaultParameterValue(0)>] line: int,
            [<CallerMemberName; Optional; DefaultParameterValue("")>] name: string
        ) : RunReport =
        Hegel.Go(body, settings, null, file, line, name, false)

    /// Run <paramref name="body"/> with <paramref name="settings"/>, reporting to <paramref name="reporter"/>, and return the report.
    static member run
        (
            settings: Settings,
            reporter: Reporter,
            body: TestCase -> unit,
            [<CallerFilePath; Optional; DefaultParameterValue("")>] file: string,
            [<CallerLineNumber; Optional; DefaultParameterValue(0)>] line: int,
            [<CallerMemberName; Optional; DefaultParameterValue("")>] name: string
        ) : RunReport =
        Hegel.Go(body, settings, reporter, file, line, name, false)

    /// The version of the engine (libhegel) this frontend is running on.
    static member engineVersion: string = Property.EngineVersion
