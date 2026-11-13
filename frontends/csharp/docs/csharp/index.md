# Hegel for C#

The API reference of the [`Hegel`](https://www.nuget.org/packages/Hegel) and
[`Hegel.Xunit`](https://www.nuget.org/packages/Hegel.Xunit) NuGet packages: property-based testing
for C#, driven by the [Hegel](https://hegel.dev) engine.

```bash
dotnet add package Hegel
dotnet add package Hegel.Xunit
```

- [`Hegel`](api/Hegel.yml) is the library: `Property.Test` runs a property written against a
  `TestCase`, `Generators` holds the generators, `Settings` configures a run and `Stateful` drives
  a state machine.
- [`Hegel.Xunit`](api/Hegel.Xunit.yml) adds `[HegelTest]`, which makes a method taking a `TestCase`
  an xUnit v2 test.

The [README](https://github.com/hegeldev/hegel/tree/main/frontends/csharp#readme) is the guide. F#
has its own package, `Hegel.FSharp`, and [its own reference](https://hegel.dev/fsharp).
