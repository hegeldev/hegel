RELEASE_TYPE: patch

This patch adds the `Hegel.NUnit` package: a `[HegelTest]` attribute for NUnit 4, as `Hegel.Xunit` has for xUnit. A method that takes one `TestCase` and returns `void` becomes a property-based test in NUnit's test tree:

```csharp
using Hegel;
using Hegel.NUnit;
using NUnit.Framework;

public class SortTests
{
    [HegelTest(TestCases = 1000)]
    public void SortPreservesLength(TestCase tc)
    {
        var xs = tc.Draw(Generators.Lists(Generators.Integers()), "xs");
        Assert.That(MySort(xs), Has.Count.EqualTo(xs.Count));
    }
}
```

The attribute takes a `Profile` name and a `TestCases` count. The minimal failing test case is printed to the test's output and the test fails with the assertion's own message. `[SetUp]` and `[TearDown]` run once around the whole property, and `[Ignore]`, `[Category]` and `[Description]` work as on a `[Test]`.
