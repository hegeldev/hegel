RELEASE_TYPE: patch

This patch fixes a number of bugs in how values are printed and how generators are derived.

Printed test cases that were not valid code:

- A `BigInteger` draw now prints as `BigInteger.Parse("…")` in C#. It used to print as bare digits, which do not compile once they pass `ulong.MaxValue` and are an `int` or `long` below that.
- An undefined negative enum value now prints as `(Colour)(-1)` in C#, and in F# an undefined value of an enum that is not `int32`-backed prints as `LanguagePrimitives.EnumOfValue<byte, Small> (7uy)`.
- In F#, cases of a `[<RequireQualifiedAccess>]` union print qualified (`Shape.Circle(1.0)`), anonymous records drawn by `Gen.auto` and `Gen.record` print as `{| A = 0 |}` (`struct {| A = 0 |}` for struct ones), a union case represented as `null` prints as the case, and field and case names that are not plain identifiers print in double backticks.

Derived generators:

- `Generators.ForType<T>()` now reports a record that refers to itself through a generic record (`record A(B<int> Next)` with `record B<T>(A Back)`) as a usage error. Drawing from it used to overflow the stack and kill the test process.
- A record constructor parameter and the property it sets are now one component when their names differ only in case, as in `public Person(string name) { Name = name; }`. The property used to be drawn a second time and assigned after construction, overwriting the argument and any `With` override of it.
- The constructor a record is built with is now a public one, and the primary constructor when there are several. A longer or private secondary constructor used to be chosen instead.

Other fixes:

- `Hegel.FSharp` and `Hegel.Xunit` now depend on exactly the `Hegel` version they were built with. They used to accept any later one, which `Hegel.FSharp` cannot load against if `Hegel`'s internals have changed.
- `Database.At("")` now throws `ArgumentException`: the empty path used to silently disable the database.
- A test case count above `long.MaxValue`, from `HEGEL_TEST_CASES` or `Settings.Set`, now reads back as `long.MaxValue`. It used to throw `OverflowException`.
- The error for inverted `Dates()` and `Times()` bounds no longer formats them in the current culture's calendar.
