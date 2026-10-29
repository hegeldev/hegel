using System.Collections;
using System.Numerics;
using Hegel.Native;
using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class PrinterTests
{
    private sealed class NullToString
    {
        public override string? ToString() => null;
    }

    private sealed class Named
    {
        public override string ToString() => "Named()";
    }

    [Flags]
    private enum Bits
    {
        A = 1,
        B = 2,
    }

    private enum Colour
    {
        Red,
    }

    [Fact]
    public void RendersScalars()
    {
        Assert.Equal("null", Runs.Render(null));
        Assert.Equal("true", Runs.Render(true));
        Assert.Equal("false", Runs.Render(false));
        Assert.Equal("42", Runs.Render(42));
        Assert.Equal("-7", Runs.Render(-7L));
        Assert.Equal("1.5m", Runs.Render(1.5m));
        Assert.Equal("'x'", Runs.Render('x'));
        Assert.Equal("'\\''", Runs.Render('\''));
        Assert.Equal("\"it's\"", Runs.Render("it's"));
        Assert.Equal("Named()", Runs.Render(new Named()));
        Assert.Equal("", Runs.Render(new NullToString()));
        Assert.Equal("\"h\U0001F600\"", Runs.Render("h\U0001F600"));
        Assert.Equal("Colour.Red", Runs.Render(Colour.Red));
        Assert.Equal("(Colour)42", Runs.Render((Colour)42));
        Assert.Equal("Bits.A | Bits.B", Runs.Render(Bits.A | Bits.B));
        Assert.Equal("TimeZoneInfo.FindSystemTimeZoneById(\"UTC\")", Runs.Render(TimeZoneInfo.Utc));
        Assert.Equal("340282366920938463463374607431768211456", Runs.Render(BigInteger.Pow(2, 128)));
    }

    [Fact]
    public void RendersFloats()
    {
        Assert.Equal("1.5f", Runs.Render(1.5f));
        Assert.Equal("1.0f", Runs.Render(1f));
        Assert.Equal("1E+30f", Runs.Render(1e30f));
        Assert.Equal("float.NaN", Runs.Render(float.NaN));
        Assert.Equal("float.PositiveInfinity", Runs.Render(float.PositiveInfinity));
        Assert.Equal("float.NegativeInfinity", Runs.Render(float.NegativeInfinity));
        Assert.Equal("1.0", Runs.Render(1.0));
        Assert.Equal("0.1", Runs.Render(0.1));
        Assert.Equal("1E+300", Runs.Render(1e300));
        Assert.Equal("double.NaN", Runs.Render(double.NaN));
        Assert.Equal("double.PositiveInfinity", Runs.Render(double.PositiveInfinity));
        Assert.Equal("double.NegativeInfinity", Runs.Render(double.NegativeInfinity));
    }

    [Fact]
    public void RendersStringsWithEscapes()
    {
        Assert.Equal("\"a\\\"b\\\\\"", Runs.Render("a\"b\\"));
        Assert.Equal("\"\\n\\r\\t\\0\"", Runs.Render("\n\r\t\0"));
        Assert.Equal("\"\\u0001\\u007f\\u00a0\\u2028\\u2029\"", Runs.Render("\u0001\u007f\u00a0\u2028\u2029"));
        Assert.Equal("\"\\ud800\"", Runs.Render("\ud800"));
        Assert.Equal("\"héllo\"", Runs.Render("héllo"));
        Assert.Equal("'\"'", Runs.Render('"'));
    }

    [Fact]
    public void RendersTemporalValuesAndGuids()
    {
        Assert.Equal("Guid.Parse(\"12345678-9abc-def0-1234-56789abcdef0\")", Runs.Render(Guid.Parse("12345678-9abc-def0-1234-56789abcdef0")));
        Assert.Equal("DateOnly.Parse(\"2024-02-29\")", Runs.Render(new DateOnly(2024, 2, 29)));
        Assert.Equal("TimeOnly.Parse(\"13:04:05\")", Runs.Render(new TimeOnly(13, 4, 5)));
        Assert.Equal("TimeOnly.Parse(\"13:04:05.1234567\")", Runs.Render(new TimeOnly(13, 4, 5).Add(TimeSpan.FromTicks(1234567))));
        Assert.Equal("DateTime.Parse(\"2024-02-29T13:04:05\")", Runs.Render(new DateTime(2024, 2, 29, 13, 4, 5)));
        Assert.Equal("DateTimeOffset.Parse(\"2024-02-29T13:04:05+02:00\")", Runs.Render(new DateTimeOffset(2024, 2, 29, 13, 4, 5, TimeSpan.FromHours(2))));
        Assert.Equal("TimeSpan.Parse(\"1.02:03:04\")", Runs.Render(new TimeSpan(1, 2, 3, 4)));
    }

    [Fact]
    public void RendersCollections()
    {
        Assert.Equal("new int[] {1, 2}", Runs.Render(new[] { 1, 2 }));
        Assert.Equal("new int?[] {null, 1}", Runs.Render(new int?[] { null, 1 }));
        Assert.Equal("new int[][] {new int[] {1}}", Runs.Render(new[] { new[] { 1 } }));
        Assert.Equal("new List<int>[] {new List<int> {1}}", Runs.Render(new[] { new List<int> { 1 } }));
        Assert.Equal("new string[] {}", Runs.Render(Array.Empty<string>()));
        Assert.Equal("new List<int> {1, 2}", Runs.Render(new List<int> { 1, 2 }));
        Assert.Equal("[1, 2]", Runs.Render(new Queue<int>([1, 2])));
        Assert.Equal("new HashSet<string> {\"a\"}", Runs.Render(new HashSet<string> { "a" }));
        Assert.Equal("(1, \"a\")", Runs.Render((1, "a")));
        Assert.Equal("new Dictionary<string, int> {[\"a\"] = 1, [\"b\"] = 2}", Runs.Render(new Dictionary<string, int> { ["a"] = 1, ["b"] = 2 }));
        Assert.Equal("new Dictionary<object, object> {[1] = true}", Runs.Render(new Hashtable { [1] = true }));
        Assert.Equal("new Dictionary<object, object> {}", Runs.Render(new Hashtable()));
    }

    [Fact]
    public void BreaksLongCollectionsAtTheWidth()
    {
        var text = Runs.Render(Enumerable.Range(100, 8).ToList(), 20);
        var pad = new string(' ', "new List<int> {".Length);
        Assert.Equal(["new List<int> {100,", pad + "101,", pad + "102,", pad + "103,", pad + "104,", pad + "105,", pad + "106,", pad + "107}"], text.Split('\n'));
    }

    [Fact]
    public void TypeNamesUseKeywordsAndGenerics()
    {
        Assert.Equal("int", Printer.TypeName(typeof(int)));
        Assert.Equal("string[]", Printer.TypeName(typeof(string[])));
        Assert.Equal("int?", Printer.TypeName(typeof(int?)));
        Assert.Equal("Dictionary<string, List<bool>>", Printer.TypeName(typeof(Dictionary<string, List<bool>>)));
        Assert.Equal("Guid", Printer.TypeName(typeof(Guid)));
        Assert.Equal("object", Printer.TypeName(typeof(object)));
    }

    [Fact]
    public void InvariantFormatting()
    {
        Assert.Equal("null", Printer.Invariant(null));
        Assert.Equal("2.5f", Printer.Invariant(2.5f));
        Assert.Equal("2.0", Printer.Invariant(2.0));
        Assert.Equal("true", Printer.Invariant(true));
        Assert.Equal("12", Printer.Invariant(new BigInteger(12)));
        Assert.Equal("1.5", Printer.Invariant(1.5m));
        Assert.Equal("Named()", Printer.Invariant(new Named()));
        Assert.Equal("", Printer.Invariant(new NullToString()));
    }

    [Fact]
    public void TextSplitsOnNewlines()
    {
        Assert.Equal("a\nb", Runs.Print(p => p.Text("a\nb")));
        Assert.Equal("a\n", Runs.Print(p => p.Text("a\n")));
        Assert.Equal("", Runs.Print(p => p.Text("")));
        Assert.Equal("\n", Runs.Print(p => p.Text("\n")));
    }

    [Fact]
    public void ReflowBreaksAtBrackets()
    {
        Assert.Equal("foo(1, 2)", Runs.Print(p => p.Reflow("foo(1, 2)")));
        var text = Runs.Print(p => p.Reflow("foo(100000, 200000, 300000)"), 20);
        Assert.Contains("\n", text);
        Assert.StartsWith("foo(", text);
        Assert.EndsWith(")", text);
    }

    [Fact]
    public void GroupsBreakableAndIfBreak()
    {
        var flat = Runs.Print(p =>
        {
            p.BeginGroup(2, "f(");
            p.Text("1");
            p.Breakable(" ");
            p.IfBreak("// broken");
            p.Text("2");
            p.EndGroup(")");
        });
        Assert.Equal("f(1 2)", flat);
        var broken = Runs.Print(p =>
        {
            p.BeginGroup(2, "f(");
            p.Text("1111111111");
            p.Breakable(" ");
            p.Text("2222222222");
            p.IfBreak(",");
            p.EndGroup(")");
        }, 12);
        Assert.Equal("f(1111111111\n  2222222222,)", broken);
    }

    [Fact]
    public void GroupHelpersAndSeq()
    {
        var text = Runs.Print(p =>
        {
            p.Group(1, "(", ")", () => p.Text("a"));
            p.Text(" ");
            var result = p.Group(1, "<", ">", () =>
            {
                p.Text("b");
                return 7;
            });
            p.Text($" {result} ");
            p.Seq("[", "]", new[] { 1, 2, 3 }, item => p.Text(item.ToString()));
            p.Text(" ");
            p.Seq("{", "}", Array.Empty<int>(), item => p.Text("never"));
        });
        Assert.Equal("(a) <b> 7 [1, 2, 3] {}", text);
    }

    [Fact]
    public void HardBreakShiftIndentAndComment()
    {
        var text = Runs.Print(p =>
        {
            p.Text("a");
            p.Comment("note");
            p.HardBreak();
            p.ShiftIndent(4);
            p.Text("b");
            p.HardBreak();
            p.ShiftIndent(-4);
            p.Text("c");
        });
        Assert.Equal("a  // note\nb\n    c", text);
    }

    [Fact]
    public void BeginGroupWithoutIndentUsesTheOpenersWidth()
    {
        var text = Runs.Print(p =>
        {
            p.BeginGroup("é(");
            p.Text("1111111111");
            p.Breakable(" ");
            p.Text("2222222222");
            p.EndGroup(")");
        }, 12);
        Assert.Equal("é(1111111111\n  2222222222)", text);
    }

    [Fact]
    public void SpeculationCommitsAbortsAndDisposes()
    {
        var text = Runs.Print(p =>
        {
            using (var s = p.Speculate())
            {
                p.Text("kept ");
                s.Commit();
            }
            using (var s = p.Speculate())
            {
                p.Text("aborted ");
                s.Abort();
            }
            using (p.Speculate())
            {
                p.Text("disposed ");
            }
            p.Text("end");
        });
        Assert.Equal("kept end", text);
    }

    [Fact]
    public void DeferredRegionsAreSplicedInWhenResolved()
    {
        var text = Runs.Print(p =>
        {
            p.Text("a");
            var deferred = p.Deferred();
            p.Text("c");
            Assert.True(deferred.ShouldPrint);
            deferred.Text("b");
            var nested = deferred.Deferred();
            nested.Text("!");
        });
        Assert.Equal("ab!c", text);
    }

    [Fact]
    public void ShouldPrintIsFalseForDeadDeferredRegions()
    {
        var lib = Engines.Real;
        var options = lib.PrinterOptionsNew();
        var handle = lib.PrinterNew(options);
        lib.PrinterOptionsFree(options);
        var regions = new Printer.Regions();
        var root = new Printer(lib, handle, regions);
        var deferred = root.Deferred();
        Assert.True(root.ShouldPrint);
        Assert.True(deferred.ShouldPrint);
        Assert.Equal("", root.Read());
        Assert.False(deferred.ShouldPrint);
        deferred.Text("ignored");
        regions.FreeAll();
        root.Free();
        Assert.False(root.ShouldPrint);
    }

    [Fact]
    public void NoopPrinterDoesNothing()
    {
        var p = Printer.Noop;
        Assert.False(p.ShouldPrint);
        p.Text("x\ny");
        p.Repr(1);
        p.Reflow("x");
        p.Breakable(" ");
        p.IfBreak(",");
        p.HardBreak();
        p.BeginGroup("[");
        p.EndGroup("]");
        p.ShiftIndent(1);
        p.Comment("c");
        var ran = false;
        p.Group(1, "(", ")", () => ran = true);
        Assert.True(ran);
        Assert.Equal(3, p.Group(1, "(", ")", () => 3));
        p.Seq("[", "]", new[] { 1 }, _ => { });
        using (var s = p.Speculate())
        {
            s.Commit();
        }
        Assert.Same(Printer.Noop, p.Deferred());
    }

    [Fact]
    public void LinesSplitsTheDocument()
    {
        Assert.Empty(Printer.Lines(""));
        Assert.Equal(["a", "b"], Printer.Lines("a\nb\n"));
        Assert.Equal(["a", "b"], Printer.Lines("a\nb"));
        Assert.Equal(["a", ""], Printer.Lines("a\n\n"));
    }

    [Fact]
    public void QuoteEscapesTheDelimiterOnly()
    {
        Assert.Equal("'\"'", Printer.Quote("\"", '\''));
        Assert.Equal("\"'\"", Printer.Quote("'", '"'));
        Assert.Equal("`a\\`b`", Printer.Quote("a`b", '`'));
    }

    private sealed class RcLib(int text, int deferred, int resolve, int value) : DelegatingLibhegel(Engines.Real)
    {
        public override string LastErrorMessage() => "fake error";

        public override int PrinterText(nint printer, string text1) => text == Abi.Ok ? base.PrinterText(printer, text1) : text;

        public override int PrinterDeferred(nint printer, out nint child)
        {
            if (deferred == Abi.Ok)
            {
                return base.PrinterDeferred(printer, out child);
            }
            child = 0;
            return deferred;
        }

        public override int PrinterResolve(nint printer) => resolve == Abi.Ok ? base.PrinterResolve(printer) : resolve;

        public override int PrinterValue(nint printer, out string value1)
        {
            if (value == Abi.Ok)
            {
                return base.PrinterValue(printer, out value1);
            }
            value1 = "";
            return value;
        }
    }

    private static Printer Faked(RcLib lib, out Action free)
    {
        var options = lib.PrinterOptionsNew();
        var handle = lib.PrinterNew(options);
        lib.PrinterOptionsFree(options);
        var regions = new Printer.Regions();
        var printer = new Printer(lib, handle, regions);
        free = () =>
        {
            regions.FreeAll();
            printer.Free();
        };
        return printer;
    }

    [Fact]
    public void CheckMapsErrorCodes()
    {
        var printer = Faked(new RcLib(Abi.EInvalidArg, Abi.Ok, Abi.Ok, Abi.Ok), out var free);
        var usage = Assert.Throws<HegelUsageException>(() => printer.Text("x"));
        Assert.Equal("hegel_printer_text failed (rc=-5): fake error", usage.Message);
        free();
        printer = Faked(new RcLib(Abi.EInternal, Abi.Ok, Abi.Ok, Abi.Ok), out free);
        var internalError = Assert.Throws<HegelException>(() => printer.Text("x"));
        Assert.Equal("hegel_printer_text failed (rc=-8): fake error", internalError.Message);
        free();
        printer = Faked(new RcLib(Abi.EInvalidHandle, Abi.Ok, Abi.Ok, Abi.Ok), out free);
        printer.Text("ignored");
        Assert.Equal("", printer.Read());
        free();
    }

    [Fact]
    public void DeferredFallsBackToNoopOnDeadHandles()
    {
        var printer = Faked(new RcLib(Abi.Ok, Abi.EInvalidHandle, Abi.Ok, Abi.Ok), out var free);
        Assert.Same(Printer.Noop, printer.Deferred());
        free();
        printer = Faked(new RcLib(Abi.Ok, Abi.EInternal, Abi.Ok, Abi.Ok), out free);
        Assert.Throws<HegelException>(() => printer.Deferred());
        free();
    }

    [Fact]
    public void ReadReportsResolveAndValueFailures()
    {
        var printer = Faked(new RcLib(Abi.Ok, Abi.Ok, Abi.EInternal, Abi.Ok), out var free);
        var resolve = Assert.Throws<HegelException>(() => printer.Read());
        Assert.Equal("hegel_printer_resolve failed (rc=-8): fake error", resolve.Message);
        free();
        printer = Faked(new RcLib(Abi.Ok, Abi.Ok, Abi.EInvalidArg, Abi.Ok), out free);
        printer.Text("tolerated");
        Assert.Equal("tolerated", printer.Read());
        free();
        printer = Faked(new RcLib(Abi.Ok, Abi.Ok, Abi.Ok, Abi.EInternal), out free);
        var value = Assert.Throws<HegelException>(() => printer.Read());
        Assert.Equal("hegel_printer_value failed (rc=-8): fake error", value.Message);
        free();
    }
}
