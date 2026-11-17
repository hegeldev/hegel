// from_lark: strings of a Lark grammar, explicit terminals, imports, and
// parameter validation.

#include <gtest/gtest.h>

#include <cstddef>
#include <filesystem>
#include <fstream>
#include <regex>
#include <stdexcept>
#include <string>
#include <system_error>

#include <hegel/hegel.h>

namespace gs = hegel::generators;

namespace {
    hegel::Settings quick(uint64_t test_cases = 50) {
        return hegel::Settings{.test_cases = test_cases,
                               .database = hegel::Database::disabled()};
    }

    size_t char_count(const std::string& s) {
        size_t chars = 0;
        for (char c : s) {
            chars += (static_cast<unsigned char>(c) & 0xC0) != 0x80;
        }
        return chars;
    }

    const char* expressions = R"lark(
        ?start: sum
        ?sum: product | sum "+" product
        ?product: atom | product "*" atom
        ?atom: NUMBER | "(" sum ")"
        %import common.NUMBER
        %import common.WS
        %ignore WS
    )lark";

    const char* assignments = R"lark(
        start: pair ("," pair)*
        pair: KEY "=" VALUE
        KEY: /[a-z]+/
        %declare VALUE
    )lark";

    gs::Generator<std::string> digits() {
        return gs::integers<int>({.min_value = 0, .max_value = 999})
            .map([](int x) { return std::to_string(x); });
    }

    // A fresh, empty directory under the system temporary directory.
    std::filesystem::path temp_dir(const std::string& name) {
        std::filesystem::path dir =
            std::filesystem::temp_directory_path() / ("hegel-cpp-" + name);
        std::filesystem::remove_all(dir);
        std::filesystem::create_directories(dir);
        return dir;
    }

    void write_file(const std::filesystem::path& path,
                    const std::string& text) {
        std::ofstream out(path);
        out << text;
    }
} // namespace

TEST(Grammar, DrawsStringsOfTheGrammar) {
    const std::regex oracle(R"(^[0-9.eE+\-*() \t\f\r\n]+$)");
    hegel::test(
        [&](hegel::TestCase& tc) {
            std::string s = tc.draw(gs::from_lark(expressions));
            EXPECT_TRUE(std::regex_match(s, oracle)) << s;
        },
        quick());
}

TEST(Grammar, ExplicitTerminalsTakeTheirGeneratorsValues) {
    const std::regex oracle(R"(^[a-z]+=[0-9]+(,[a-z]+=[0-9]+)*$)");
    hegel::test(
        [&](hegel::TestCase& tc) {
            std::string s = tc.draw(gs::from_lark(
                assignments, {.explicit_terminals = {{"VALUE", digits()}}}));
            EXPECT_TRUE(std::regex_match(s, oracle)) << s;
        },
        quick());
}

TEST(Grammar, ExplicitTerminalCanOverrideAPattern) {
    hegel::test(
        [](hegel::TestCase& tc) {
            std::string s = tc.draw(
                gs::from_lark("start: WORD\nWORD: /[a-z]+/",
                              {.explicit_terminals = {
                                   {"WORD", gs::just(std::string("X"))}}}));
            EXPECT_EQ(s, "X");
        },
        quick(10));
}

TEST(Grammar, StartSelectsTheRule) {
    hegel::test(
        [](hegel::TestCase& tc) {
            std::string s = tc.draw(gs::from_lark("start: \"a\"\nother: \"b\"",
                                                  {.start = "other"}));
            EXPECT_EQ(s, "b");
        },
        quick(10));
}

TEST(Grammar, AlphabetConstrainsPatternCharacters) {
    hegel::test(
        [](hegel::TestCase& tc) {
            std::string s = tc.draw(
                gs::from_lark("start: /[a-z]+/",
                              {.alphabet = gs::CharactersParams{
                                   .categories = std::vector<std::string>{},
                                   .include_characters = "ab"}}));
            EXPECT_FALSE(s.empty());
            EXPECT_EQ(s.find_first_not_of("ab"), std::string::npos) << s;
        },
        quick());
}

TEST(Grammar, MaxSizeCountsExplicitValues) {
    hegel::test(
        [](hegel::TestCase& tc) {
            std::string s = tc.draw(gs::from_lark(
                assignments, {.explicit_terminals = {{"VALUE", digits()}},
                              .max_size = 20,
                              .max_depth = 50}));
            EXPECT_LE(char_count(s), 20u) << s;
        },
        quick());
}

TEST(Grammar, ValuesThatNeverFitRejectEveryCase) {
    EXPECT_THROW(
        hegel::test(
            [](hegel::TestCase& tc) {
                (void)tc.draw(gs::from_lark(
                    "start: VALUE\n%declare VALUE",
                    {.explicit_terminals = {{"VALUE", gs::just(std::string(
                                                          "far too long"))}},
                     .max_size = 5}));
            },
            quick()),
        hegel::RunError);
}

TEST(Grammar, ExplicitValueMustBeUtf8) {
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         (void)tc.draw(gs::from_lark(
                             "start: VALUE\n%declare VALUE",
                             {.explicit_terminals = {
                                  {"VALUE", gs::just(std::string("\xff"))}}}));
                     },
                     quick(5)),
                 std::invalid_argument);
}

TEST(Grammar, ImportsReadFromImportPath) {
    std::filesystem::path dir = temp_dir("grammar-imports");
    write_file(dir / "words.lark", "GREETING: \"hello\"\n");
    hegel::test(
        [&](hegel::TestCase& tc) {
            std::string s = tc.draw(
                gs::from_lark("start: GREETING (WS GREETING)*\n"
                              "%import words.GREETING\n%import common.WS",
                              {.import_path = dir.string()}));
            EXPECT_EQ(s.substr(0, 5), "hello") << s;
        },
        quick());
    std::filesystem::remove_all(dir);
}

TEST(Grammar, UnreadableImportThrows) {
    std::filesystem::path dir = temp_dir("grammar-unreadable");
    std::filesystem::create_directories(dir / "words.lark");
    EXPECT_THROW(gs::from_lark("start: GREETING\n%import words.GREETING",
                               {.import_path = dir.string()}),
                 std::invalid_argument);
    std::filesystem::remove_all(dir);
}

TEST(Grammar, ImportWithoutPathIsMissing) {
    EXPECT_THROW(gs::from_lark("start: GREETING\n%import words.GREETING"),
                 std::invalid_argument);
}

TEST(Grammar, InvalidGrammarsThrow) {
    EXPECT_THROW(gs::from_lark("start: ("), std::invalid_argument);
    EXPECT_THROW(gs::from_lark("start: \"a\"", {.start = "missing"}),
                 std::invalid_argument);
    EXPECT_THROW(gs::from_lark("start: VALUE\n%declare VALUE"),
                 std::invalid_argument);
    EXPECT_THROW(gs::from_lark("start: \"a\"", {.max_size = 0}),
                 std::invalid_argument);
    EXPECT_THROW(gs::from_lark("start: \"a\"", {.max_depth = 0}),
                 std::invalid_argument);
    EXPECT_THROW(gs::from_lark("start: \"abc\"", {.max_size = 2}),
                 std::invalid_argument);
    EXPECT_THROW(gs::from_lark("start: VALUE\n%declare VALUE",
                               {.explicit_terminals = {{"VALUE", digits()},
                                                       {"VALUE", digits()}}}),
                 std::invalid_argument);
}

TEST(Grammar, LabelDependsOnGrammarStartAndTerminals) {
    std::string grammar = "start: \"a\"\nother: \"b\"";
    EXPECT_EQ(gs::from_lark(grammar).label(), gs::from_lark(grammar).label());
    EXPECT_NE(gs::from_lark(grammar).label(),
              gs::from_lark("start: \"c\"").label());
    EXPECT_NE(gs::from_lark(grammar).label(),
              gs::from_lark(grammar, {.start = "other"}).label());
    EXPECT_NE(gs::from_lark(assignments,
                            {.explicit_terminals = {{"VALUE", digits()}}})
                  .label(),
              gs::from_lark(assignments,
                            {.explicit_terminals = {{"VALUE", gs::text()}}})
                  .label());
}
