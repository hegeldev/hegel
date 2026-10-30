using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class GeneratorTests
{
    private sealed class Silent : Generator<int>
    {
        public override int DoDraw(TestCase tc) => (int)tc.GenerateInteger(3, 3);
    }

    private sealed class Recording : PrintableGenerator<int>
    {
        internal Printer? Printer { get; private set; }

        public override int DoDrawAndPrint(TestCase tc, Printer printer)
        {
            Printer = printer;
            printer.Text("recorded");
            return 9;
        }
    }

    private sealed class Labelled : Generator<int>
    {
        public override ulong Label => 12345;

        public override int DoDraw(TestCase tc) => 1;
    }

    [Fact]
    public void DefaultLabelComesFromTheTypeName()
    {
        Assert.Equal(Label.Of(typeof(Silent).FullName!), new Silent().Label);
        Assert.Equal(Label.Of(typeof(Recording).FullName!), new Recording().Label);
        Assert.Equal(12345UL, new Labelled().Label);
        Assert.Equal(12345UL, ((IGenerator)new Labelled()).Label);
    }

    [Fact]
    public void PrintableDoDrawPrintsToNoop()
    {
        var generator = new Recording();
        using var run = new RawRun();
        Assert.Equal(9, generator.DoDraw(run.Case()));
        Assert.Same(Printer.Noop, generator.Printer);
    }

    [Fact]
    public void InterfaceDrawChoosesSilentOrPrinted()
    {
        var generator = new Recording();
        var failure = Runs.Failing(tc =>
        {
            Assert.Equal(9, ((IGenerator)generator).Draw(tc, null));
            Assert.Same(Printer.Noop, generator.Printer);
            using var doc = new Document();
            Assert.Equal(9, ((IGenerator)generator).Draw(tc, doc.Printer));
            Assert.Same(doc.Printer, generator.Printer);
            Assert.Equal("recorded", doc.Finish());
        });
        Assert.Empty(failure.Output);
    }

    [Fact]
    public void MapPrintsTheMappedValue()
    {
        var failure = Runs.Failing(tc => Assert.Equal(6, tc.Draw(new Silent().Map(x => x * 2), "doubled")));
        Assert.Equal(["var doubled_1 = 6;"], failure.Output);
        Assert.Equal(6, failure.Draws["doubled_1"]);
    }

    [Fact]
    public void FilterKeepsDrawingUntilAccepted()
    {
        Runs.Passing(tc =>
        {
            var even = tc.Draw(Generators.Integers().Min(0).Max(100).Filter(x => x % 2 == 0));
            Assert.Equal(0, even % 2);
        }, Engines.Quiet with { TestCases = 20 });
    }

    [Fact]
    public void FlatMapDrawsFromTheDerivedGenerator()
    {
        var failure = Runs.Failing(tc =>
        {
            var xs = tc.Draw(Generators.Integers().Min(1).Max(3).FlatMap(n => Generators.Lists(Generators.Just(n)).MinSize(n).MaxSize(n)), "xs");
            Assert.All(xs, x => Assert.Equal(xs.Count, x));
        });
        Assert.Equal(["var xs_1 = new List<int> {1};"], failure.Output);
    }

    [Fact]
    public void PrintWithUsesTheGivenPrinter()
    {
        var failure = Runs.Failing(tc => Assert.Equal(3, tc.Draw(new Silent().PrintWith((value, printer) => printer.Text($"three={value}")))));
        Assert.Equal(["var draw_1 = three=3;"], failure.Output);
    }

    [Fact]
    public void PrintAsReprMakesAGeneratorPrintable()
    {
        var failure = Runs.Failing(tc => Assert.Equal(3, tc.Draw(new Silent().PrintAsRepr(), "n")));
        Assert.Equal(["var n_1 = 3;"], failure.Output);
    }
}
