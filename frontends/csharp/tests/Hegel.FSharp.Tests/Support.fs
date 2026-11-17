module Hegel.FSharp.Tests.Support

open System
open System.IO
open Hegel
open Hegel.FSharp

/// Settings that leave nothing behind and print nothing: no database, quiet, no blob.
let quiet =
    Settings(Database = Database.Disabled, Verbosity = Verbosity.Quiet, PrintBlob = false)

/// The text a value prints as under the F# dialect.
let render (t: Type) (value: obj) : string =
    use doc = new Document()
    Repr.render doc.Printer t value
    doc.Finish()

/// The text a typed value prints as.
let show (value: 'T) : string = render typeof<'T> (box value)

/// The transcript of the minimal failing test case of a property that always fails.
let failing (body: TestCase -> unit) : Failure =
    let report =
        Hegel.run (
            quiet,
            fun tc ->
                body tc
                failwith "boom"
        )

    Seq.exactlyOne report.Failures

/// What one draw of `generator` prints as in a failing test case.
let printed (generator: Generator<'T>) : string =
    let failure = failing (fun tc -> tc.Draw(generator, "x") |> ignore)
    String.Join("\n", failure.Output)
