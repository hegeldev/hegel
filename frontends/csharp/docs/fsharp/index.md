# Hegel for F#

The API reference of the [`Hegel.FSharp`](https://www.nuget.org/packages/Hegel.FSharp) NuGet
package: property-based testing for F#, driven by the [Hegel](https://hegel.dev) engine.

```bash
dotnet add package Hegel.FSharp
```

- [`Hegel`]({{root}}reference/hegel-fsharp-hegel.html) runs a property: `Hegel.test` and `Hegel.run` take
  the test body as an F# function over a `TestCase`.
- [`Gen`]({{root}}reference/hegel-fsharp-gen.html) holds the generators and their combinators, the
  `gen { }` builder, and `Gen.auto<'T>`, which derives a generator for records, unions, tuples
  and collections.
- [Everything else]({{root}}reference/index.html) in the `Hegel.FSharp` namespace.

The [README](https://github.com/hegeldev/hegel/tree/main/frontends/csharp#f) is the guide. The C#
packages, `Hegel` and `Hegel.Xunit`, have [their own reference](https://hegel.dev/csharp).
