RELEASE_TYPE: patch

`Generator<T>` also has `Select`, `Where` and `SelectMany`, so generators can be built with C# query expressions. A later `from` clause can use any earlier value, which makes dependent generators read top to bottom instead of as nested `FlatMap` calls:

```csharp
var withIndex =
    from xs in Generators.Lists(Generators.Integers()).MinSize(1)
    from i in Generators.Integers().Min(0).Max(xs.Count - 1)
    select (xs, i);
```

`select` is `Map`, `where` is `Filter` (with the same rejection cost), and `let` clauses work too. A failing test case prints the query's final value.
