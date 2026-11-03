using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class DialectTests
{
    private sealed class LetDialect : Dialect
    {
        public override void Repr(Printer printer, object? value, Type staticType) => printer.Text($"{staticType.Name}:{value ?? "null"}");

        public override void BeginDraw(Printer printer, string name) => printer.Text($"let {name} = ");

        public override void EndDraw(Printer printer)
        {
        }
    }

    [Fact]
    public void TheCSharpDialectIsCurrentUnlessAnotherIsInstalled()
    {
        Assert.Same(Dialect.CSharp, Dialect.Current);
        var outer = new LetDialect();
        var inner = new LetDialect();
        using (Dialect.Use(outer))
        {
            Assert.Same(outer, Dialect.Current);
            using (Dialect.Use(inner))
            {
                Assert.Same(inner, Dialect.Current);
            }
            Assert.Same(outer, Dialect.Current);
        }
        Assert.Same(Dialect.CSharp, Dialect.Current);
    }

    [Fact]
    public void UseRejectsNull() => Assert.Throws<ArgumentNullException>(() => Dialect.Use(null!));

    [Fact]
    public void ReprRequiresAStaticType() => Assert.Throws<ArgumentNullException>(() => Runs.Print(p => p.Repr(1, null!)));

    [Fact]
    public void TheCSharpDialectPrintsValuesAsCSharp()
    {
        Assert.Equal("\"a\"", Runs.Print(p => Dialect.CSharp.Repr(p, "a", typeof(string))));
        Assert.Equal("var x = 1;", Runs.Print(p =>
        {
            Dialect.CSharp.BeginDraw(p, "x");
            p.Repr(1);
            Dialect.CSharp.EndDraw(p);
        }));
    }

    [Fact]
    public void DrawsArePrintedInTheInstalledDialect()
    {
        using var scope = Dialect.Use(new LetDialect());
        var failure = Runs.Failing(tc =>
        {
            tc.Draw(Generators.Just(1), "x");
            tc.Draw(Generators.Just<string?>(null), "s");
            var pool = new Pool<int>(tc);
            pool.Add(2);
            tc.Draw(pool.Reusable(), "p");
        });
        Assert.Equal(["let x_1 = Int32:1", "let s_1 = String:null", "let p_1 = Int32:2"], failure.Output);
    }

    [Fact]
    public void ExplicitDrawsArePrintedInTheInstalledDialect()
    {
        using var scope = Dialect.Use(new LetDialect());
        using var draws = new ExplicitDraws(Engines.Real, Example.Of(3));
        Assert.Equal(3, draws.Take("n_1"));
        Assert.Equal(["let n_1 = Object:3"], draws.Output());
    }
}
