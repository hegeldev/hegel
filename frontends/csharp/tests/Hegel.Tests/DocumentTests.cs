using Xunit;

namespace Hegel.Tests;

public class DocumentTests
{
    [Fact]
    public void DefaultWidthIs79()
    {
        using var doc = new Document();
        Assert.Equal(79, doc.MaxWidth);
        Assert.Equal(Document.DefaultMaxWidth, doc.MaxWidth);
    }

    [Fact]
    public void MaxWidthControlsLineBreaking()
    {
        using var doc = new Document { MaxWidth = 10 };
        doc.Printer.Repr(new List<int> { 1000, 2000, 3000 });
        Assert.Equal("new List<int> {1000,\n               2000,\n               3000}", doc.Finish());
    }

    [Fact]
    public void AnAbandonedDocumentIsFreedByItsFinalizer()
    {
        Abandon();
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
    }

    [System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
    private static void Abandon()
    {
        var doc = new Document();
        doc.Printer.Text("abandoned");
        doc.Printer.Deferred().Text("later");
    }

    [Fact]
    public void MaxWidthMustBeSetBeforePrinting()
    {
        using var doc = new Document();
        doc.Printer.Text("x");
        var error = Assert.Throws<InvalidOperationException>(() => doc.MaxWidth = 10);
        Assert.Equal("MaxWidth must be set before the document is printed to", error.Message);
    }

    [Fact]
    public void MaxWidthMustBePositive()
    {
        using var doc = new Document();
        Assert.Throws<ArgumentOutOfRangeException>(() => doc.MaxWidth = 0);
        doc.MaxWidth = 1;
        Assert.Equal(1, doc.MaxWidth);
    }

    [Fact]
    public void FinishClosesTheDocument()
    {
        var doc = new Document();
        doc.Printer.Text("done");
        Assert.Equal("done", doc.Finish());
        var error = Assert.Throws<InvalidOperationException>(() => doc.Printer);
        Assert.Equal("the document has been finished or disposed", error.Message);
        doc.Dispose();
    }

    [Fact]
    public void DisposeIsIdempotentAndFreesDeferredRegions()
    {
        var doc = new Document();
        var deferred = doc.Printer.Deferred();
        deferred.Text("x");
        doc.Dispose();
        doc.Dispose();
        Assert.False(deferred.ShouldPrint);
        Assert.Throws<InvalidOperationException>(() => doc.Printer);
    }

    [Fact]
    public void DisposingAnUnusedDocumentIsFine()
    {
        var doc = new Document();
        doc.Dispose();
    }

    [Fact]
    public void FinishingAnEmptyDocumentGivesAnEmptyString()
    {
        using var doc = new Document();
        Assert.Equal("", doc.Finish());
    }
}
