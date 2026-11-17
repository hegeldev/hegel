#include <gtest/gtest.h>

#include <stdexcept>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include <hegel/hegel.h>

namespace {
    std::string finish(hegel::Document& document) {
        return std::move(document).finish();
    }

    struct CountsPrints {
        int* calls;
        void print_to(hegel::Printer& printer) const {
            ++*calls;
            printer.text("counted");
        }
    };
} // namespace

TEST(Printer, TextHonoursNewlinesAsHardBreaks) {
    hegel::Document document;
    document.printer().begin_group(2, "[");
    document.printer().text("a\nb\n\nc\n");
    document.printer().end_group("]");
    EXPECT_EQ(finish(document), "[a\n  b\n  \n  c\n  ]");
}

TEST(Printer, ReprWritesTheCppRepresentation) {
    hegel::Document wide;
    wide.printer().repr(std::vector<int>{1, 2});
    EXPECT_EQ(finish(wide), "std::vector<int>{1, 2}");

    hegel::Document narrow(12);
    narrow.printer().repr(std::vector<int>{1000, 2000});
    EXPECT_EQ(finish(narrow), "std::vector<int>{\n    1000,\n    2000\n}");
}

TEST(Printer, ReflowRecoversBracketStructure) {
    hegel::Document document(10);
    document.printer().reflow("f(aaaa, bbbb)");
    EXPECT_EQ(finish(document), "f(aaaa,\n  bbbb)");
}

TEST(Printer, BreakableIsASpaceUnlessTheGroupBreaks) {
    auto render = [](std::size_t width) {
        hegel::Document document(width);
        hegel::Printer& p = document.printer();
        p.group(1, "(", ")", [](hegel::Printer& p) {
            p.text("aaaa");
            p.breakable();
            p.text("bbbb");
        });
        return finish(document);
    };
    EXPECT_EQ(render(79), "(aaaa bbbb)");
    EXPECT_EQ(render(8), "(aaaa\n bbbb)");
}

namespace {
    std::string print_list(std::size_t width) {
        hegel::Document document(width);
        hegel::Printer& printer = document.printer();
        printer.begin_group(1, "{");
        printer.text("1000");
        printer.separator();
        printer.text("2000");
        printer.if_break(",");
        printer.end_group("}");
        return finish(document);
    }
} // namespace

TEST(Printer, IfBreakTextShowsOnlyInABrokenGroup) {
    EXPECT_EQ(print_list(79), "{1000, 2000}");
    EXPECT_EQ(print_list(8), "{1000,\n 2000,}");
}

TEST(Printer, HardBreakAlwaysBreaks) {
    hegel::Document document;
    document.printer().text("a");
    document.printer().hard_break();
    document.printer().text("b");
    EXPECT_EQ(finish(document), "a\nb");
}

TEST(Printer, SeqFitsOnOneLineOrPutsEachItemOnItsOwn) {
    auto render = [](std::size_t width) {
        hegel::Document document(width);
        document.printer().seq("make{", "}", std::vector<int>{1000, 2000},
                               [](hegel::Printer& p, int n) { p.repr(n); });
        return finish(document);
    };
    EXPECT_EQ(render(79), "make{1000, 2000}");
    EXPECT_EQ(render(12), "make{\n    1000,\n    2000\n}");
}

TEST(Printer, ShiftIndentMovesLaterBreakPoints) {
    hegel::Document document(6);
    hegel::Printer& p = document.printer();
    p.begin_group(0, "");
    p.text("aaaa");
    p.shift_indent(3);
    p.breakable();
    p.text("bbbb");
    p.shift_indent(-3);
    p.end_group("");
    EXPECT_EQ(finish(document), "aaaa\n   bbbb");
}

TEST(Printer, ACommentEndsItsLineAndBreaksTheGroup) {
    hegel::Document document;
    hegel::Printer& printer = document.printer();
    printer.begin_group(1, "{");
    printer.text("1,");
    printer.comment("the first");
    printer.breakable(" ");
    printer.text("2");
    printer.end_group("}");
    EXPECT_EQ(finish(document), "{1,  // the first\n 2\n}");
}

TEST(Printer, AnUnbalancedEndGroupIsALogicError) {
    hegel::Document document;
    EXPECT_THROW(document.printer().end_group(")"), std::logic_error);
}

TEST(Printer, ZeroWidthIsAnError) {
    EXPECT_THROW(hegel::Document(0), std::runtime_error);
}

TEST(Printer, WritesAfterTheDocumentIsReadAreIgnored) {
    hegel::Document document;
    hegel::Printer& printer = document.printer();
    printer.text("kept");
    EXPECT_EQ(finish(document), "kept");
    EXPECT_FALSE(printer.should_print());
    printer.text("dropped");
    EXPECT_EQ(finish(document), "kept");
}

TEST(Printer, ASpeculationKeepsOnlyWhatItCommits) {
    hegel::Document document;
    hegel::Printer& printer = document.printer();
    {
        hegel::Speculation kept = printer.speculate();
        printer.text("kept");
        kept.commit();
        kept.abort();
    }
    {
        hegel::Speculation dropped = printer.speculate();
        printer.text("dropped");
        dropped.abort();
    }
    {
        hegel::Speculation unwound = printer.speculate();
        printer.text("unwound");
    }
    EXPECT_EQ(finish(document), "kept");
}

TEST(Printer, AMovedSpeculationClosesOnce) {
    hegel::Document document;
    hegel::Printer& printer = document.printer();
    {
        hegel::Speculation first = printer.speculate();
        printer.text("kept");
        hegel::Speculation second(std::move(first));
        second.commit();
    }
    EXPECT_EQ(finish(document), "kept");
}

TEST(Printer, ANestedSpeculationDiesWithAnAbortedOuterOne) {
    hegel::Document document;
    hegel::Printer& printer = document.printer();
    hegel::Speculation outer = printer.speculate();
    printer.text("outer");
    hegel::Speculation inner = printer.speculate();
    printer.text("inner");
    inner.commit();
    outer.abort();
    printer.text("after");
    EXPECT_EQ(finish(document), "after");
}

TEST(Printer, ADeferredPrinterFillsItsHoleLater) {
    hegel::Document document;
    hegel::Printer& printer = document.printer();
    printer.text("[");
    hegel::Printer hole = printer.deferred();
    printer.text("]");
    EXPECT_TRUE(hole.should_print());
    hole.text("filled");
    EXPECT_EQ(finish(document), "[filled]");
    EXPECT_FALSE(hole.should_print());
    hole.text("dropped");
    hegel::Printer dead = hole.deferred();
    EXPECT_FALSE(dead.should_print());
    dead.text("dropped");
    EXPECT_EQ(finish(document), "[filled]");
}

TEST(Printer, ADeferredPrinterCanBeFilledFromAnotherThread) {
    hegel::Document document;
    hegel::Printer& printer = document.printer();
    printer.text("[");
    hegel::Printer hole = printer.deferred();
    printer.text("]");
    std::thread([hole = std::move(hole)]() mutable {
        hole.text("from a thread");
    }).join();
    EXPECT_EQ(finish(document), "[from a thread]");
}

TEST(Printer, ADeferredPrinterInAnAbortedSpeculationIsDead) {
    hegel::Document document;
    hegel::Printer& printer = document.printer();
    hegel::Printer hole = hegel::Printer::noop();
    {
        hegel::Speculation speculation = printer.speculate();
        hole = printer.deferred();
        EXPECT_TRUE(hole.should_print());
    }
    EXPECT_FALSE(hole.should_print());
    hole.text("dropped");
    printer.text("kept");
    EXPECT_EQ(finish(document), "kept");
}

TEST(Printer, TheNoopPrinterDiscardsEverything) {
    int calls = 0;
    hegel::Printer printer = hegel::Printer::noop();
    EXPECT_FALSE(printer.should_print());
    printer.text("text");
    printer.repr(CountsPrints{&calls});
    printer.reflow("reflow(1)");
    printer.breakable();
    printer.if_break(",");
    printer.hard_break();
    printer.begin_group(1, "(");
    printer.end_group(")");
    printer.end_group(")");
    printer.separator();
    printer.shift_indent(4);
    printer.comment("comment");
    printer.seq("{", "}", std::vector<int>{1, 2},
                [](hegel::Printer& p, int n) { p.repr(n); });
    hegel::Speculation speculation = printer.speculate();
    speculation.commit();
    hegel::Speculation unclosed = printer.speculate();
    hegel::Printer child = printer.deferred();
    EXPECT_FALSE(child.should_print());
    EXPECT_EQ(calls, 0);
}

TEST(Printer, ReprFormatsOnlyWhenPrinting) {
    int calls = 0;
    hegel::Document document;
    document.printer().repr(CountsPrints{&calls});
    EXPECT_EQ(calls, 1);
    EXPECT_EQ(finish(document), "counted");
    document.printer().repr(CountsPrints{&calls});
    EXPECT_EQ(calls, 1);
}
