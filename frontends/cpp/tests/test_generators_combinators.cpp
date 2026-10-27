// Combinator and composition generators: compose, map / flat_map / filter,
// one_of / variant / optional, just / sampled_from, deferred, and the
// function-backed (CompositeGenerator) fallback path.

#include <gtest/gtest.h>

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <stdexcept>
#include <string>
#include <tuple>
#include <type_traits>
#include <vector>

#include <hegel/hegel.h>

#include "common/utils.h"

namespace gs = hegel::generators;

// ---------------------------------------------------------------------------
// compose
// ---------------------------------------------------------------------------

namespace {
    struct Point {
        int x;
        int y;
    };
} // namespace

TEST(Compose, DeducesTypeFromLambdaReturn) {
    auto gen = gs::compose([](const hegel::TestCase& tc) {
        int x = tc.draw(gs::integers<int>());
        int y = tc.draw(gs::integers<int>());
        return Point{x, y};
    });
    static_assert(std::is_same_v<decltype(gen), gs::Generator<Point>>);
    static_assert(std::is_same_v<decltype(gen)::value_type, Point>);

    hegel::test([&gen](hegel::TestCase& tc) { tc.draw(gen); });
}

TEST(Compose, TrailingReturnTypeForcesGeneratorType) {
    auto gen = gs::compose([](const hegel::TestCase& tc) -> long {
        return tc.draw(gs::integers<int>());
    });
    static_assert(std::is_same_v<decltype(gen), gs::Generator<long>>);

    hegel::test([&gen](hegel::TestCase& tc) { tc.draw(gen); });
}

// ---------------------------------------------------------------------------
// deferred
// ---------------------------------------------------------------------------

namespace {
    // A recursive tree: either a leaf holding an int, or a branch holding two
    // subtrees. Recursion is expressed through deferred().
    struct Tree {
        int leaf = 0;
        std::vector<Tree> children;
    };
} // namespace

// A self-referential generator built with deferred() produces values. Depth is
// bounded by the engine's draw budget, so the run completes without failing.
TEST(Deferred, RecursiveTreeGenerates) {
    auto tree = gs::deferred<Tree>();

    auto leaf = gs::integers<int>().map([](int v) { return Tree{v, {}}; });
    auto branch = gs::compose([tree](const hegel::TestCase& tc) -> Tree {
        Tree t;
        t.children.push_back(tc.draw(tree.generator()));
        t.children.push_back(tc.draw(tree.generator()));
        return t;
    });
    tree.set(gs::one_of<Tree>({leaf, branch}));

    EXPECT_NO_THROW(hegel::test(
        [&](hegel::TestCase& tc) { (void)tc.draw(tree.generator()); },
        hegel::Settings{
            .test_cases = 50,
            .derandomize = true,
            .database = hegel::Database::disabled(),
            .suppress_health_check = {hegel::HealthCheck::FilterTooMuch}}));
}

// Drawing from a handle before set() has supplied an implementation is an
// error.
TEST(Deferred, DrawBeforeSetThrows) {
    auto def = gs::deferred<int>();
    auto handle = def.generator();
    EXPECT_THROW(
        hegel::test([&](hegel::TestCase& tc) { (void)tc.draw(handle); },
                    hegel::Settings{.test_cases = 1,
                                    .derandomize = true,
                                    .database = hegel::Database::disabled()}),
        std::runtime_error);
}

// set() installs the implementation exactly once; a second call is an error.
TEST(Deferred, SetTwiceThrows) {
    auto def = gs::deferred<int>();
    def.set(gs::just(1));
    EXPECT_THROW(def.set(gs::just(2)), std::runtime_error);
}

// ---------------------------------------------------------------------------
// just / sampled_from with opaque (non-serializable) types
// ---------------------------------------------------------------------------

TEST(JustOverload, StringLiteralDefaultsToString) {
    auto g = gs::just("hello");
    static_assert(
        std::is_same_v<decltype(g), gs::Generator<std::string>>,
        "just(\"hello\") should deduce Generator<std::string> via the "
        "const char* overload");
}

TEST(JustOverload, ExplicitCharPtrKeepsCharPtr) {
    auto g = gs::just<const char*>("hello");
    static_assert(std::is_same_v<decltype(g), gs::Generator<const char*>>,
                  "just<const char*>(\"hello\") should select the template and "
                  "yield Generator<const char*>");
}

namespace {
    // A type with no public fields for reflect-cpp and no default
    // constructor - unambiguously non-serializable. just() and
    // sampled_from() never hand T to the engine (they only draw an index,
    // if anything), so this must still work.
    class OpaqueHandle {
      public:
        explicit OpaqueHandle(int id) : id_(id) {}
        int id() const { return id_; }
        bool operator==(const OpaqueHandle& other) const {
            return id_ == other.id_;
        }

      private:
        int id_;
    };
} // namespace

TEST(NonSerializable, JustWorksWithOpaqueType) {
    auto gen = gs::just(OpaqueHandle{42});
    hegel::test([&gen](hegel::TestCase& tc) {
        OpaqueHandle drawn = tc.draw_silent(gen);
        EXPECT_EQ(drawn.id(), 42);
    });
}

TEST(NonSerializable, SampledFromWorksWithOpaqueType) {
    std::vector<OpaqueHandle> options;
    options.emplace_back(1);
    options.emplace_back(2);
    options.emplace_back(3);
    auto gen = gs::sampled_from(options);
    hegel::test([&gen](hegel::TestCase& tc) {
        OpaqueHandle drawn = tc.draw_silent(gen);
        EXPECT_TRUE(drawn == OpaqueHandle{1} || drawn == OpaqueHandle{2} ||
                    drawn == OpaqueHandle{3});
    });
}

TEST(Validation, OneOfEmptyVector) {
    EXPECT_THROW(gs::one_of(std::vector<gs::Generator<int>>{}),
                 std::invalid_argument);
}

// ---------------------------------------------------------------------------
// Function-backed (CompositeGenerator) fallback path
// ---------------------------------------------------------------------------

namespace {
    gs::Generator<int> always_even() {
        return gs::integers<int>({.min_value = 0, .max_value = 100})
            .filter([](const int& x) { return x % 2 == 0; });
    }

    hegel::Settings fast() {
        // The even-only filter rejects ~half its draws; across a composite of
        // several elements that adds up, so suppress the health checks.
        return hegel::Settings{.test_cases = 50,
                               .database = hegel::Database::disabled(),
                               .suppress_health_check =
                                   hegel::all_health_checks()};
    }
} // namespace

TEST(CompositeFallback, SetsWithFunctionBackedElement) {
    hegel::test(
        [](hegel::TestCase& tc) {
            auto s = tc.draw(
                gs::sets(always_even(), {.min_size = 1, .max_size = 5}));
            EXPECT_GE(s.size(), 1u);
            EXPECT_LE(s.size(), 5u);
            for (int x : s) {
                EXPECT_EQ(x % 2, 0) << "set element should be even";
            }
        },
        fast());
}

TEST(CompositeFallback, MapsWithFunctionBackedKey) {
    hegel::test(
        [](hegel::TestCase& tc) {
            auto m = tc.draw(gs::maps(always_even(), gs::integers<int>(),
                                      {.min_size = 1, .max_size = 5}));
            EXPECT_GE(m.size(), 1u);
            EXPECT_LE(m.size(), 5u);
            for (const auto& [k, v] : m) {
                EXPECT_EQ(k % 2, 0) << "map key should be even";
            }
        },
        fast());
}

TEST(CompositeFallback, TuplesWithFunctionBackedElement) {
    hegel::test(
        [](hegel::TestCase& tc) {
            auto t = tc.draw(gs::tuples(always_even(), gs::booleans()));
            EXPECT_GE(std::get<0>(t), 0);
            EXPECT_LE(std::get<0>(t), 100);
            EXPECT_EQ(std::get<0>(t) % 2, 0) << "tuple element should be even";
        },
        fast());
}

TEST(CompositeFallback, VariantWithFunctionBackedBranch) {
    hegel::test(
        [](hegel::TestCase& tc) {
            auto v = tc.draw(gs::variant(always_even(), gs::booleans()));
            EXPECT_LT(v.index(), 2u);
            if (v.index() == 0) {
                EXPECT_EQ(std::get<0>(v) % 2, 0)
                    << "variant int should be even";
            }
        },
        fast());
}

TEST(CompositeFallback, SampledFromEmptyThrows) {
    EXPECT_THROW(gs::sampled_from(std::vector<int>{}), std::invalid_argument);
}

TEST(CompositeFallback, UniqueVectorsRequireEqualityComparable) {
    struct NoEq {
        int value;
    };
    auto elements = gs::compose(
        [](const hegel::TestCase& tc) { return NoEq{tc.draw(always_even())}; });
    EXPECT_THROW(gs::vectors(elements, {.unique = true}),
                 std::invalid_argument);
}

TEST(CompositeFallback, SampledFromStringLiterals) {
    hegel::test(
        [](hegel::TestCase& tc) {
            // The initializer_list<const char*> overload yields std::string.
            std::string s = tc.draw(gs::sampled_from({"red", "green", "blue"}));
            EXPECT_TRUE(s == "red" || s == "green" || s == "blue");
        },
        fast());
}

TEST(CompositeFallback, UnsatisfiableUniqueMapIsRejected) {
    try {
        hegel::test(
            [](hegel::TestCase& tc) {
                // Only two possible keys (0, 1) but at least five required.
                (void)tc.draw(gs::maps(
                    gs::integers<int>({.min_value = 0, .max_value = 1}),
                    gs::integers<int>(), {.min_size = 5}));
            },
            hegel::Settings{.test_cases = 10,
                            .database = hegel::Database::disabled(),
                            .suppress_health_check =
                                hegel::all_health_checks()});
        FAIL() << "expected an Unsatisfiable error";
    } catch (const std::runtime_error& error) {
        std::string message = error.what();
        EXPECT_NE(message.find("Unsatisfiable"), std::string::npos) << message;
    }
}

// ---------------------------------------------------------------------------
// recursive
// ---------------------------------------------------------------------------

namespace {
    struct Node {
        int value;
        std::vector<Node> children;
    };

    size_t depth_of(const Node& node) {
        size_t deepest = 0;
        for (const Node& child : node.children) {
            deepest = std::max(deepest, depth_of(child) + 1);
        }
        return deepest;
    }

    size_t leaves_of(const Node& node) {
        if (node.children.empty()) {
            return 1;
        }
        size_t total = 0;
        for (const Node& child : node.children) {
            total += leaves_of(child);
        }
        return total;
    }

    gs::Generator<Node> nodes(gs::RecursiveParams params = {},
                              size_t min_children = 1) {
        return gs::recursive(
            gs::integers<int>({.min_value = 0, .max_value = 9}).map([](int v) {
                return Node{v, {}};
            }),
            [min_children](gs::Generator<Node> subtrees) {
                return gs::vectors(subtrees,
                                   {.min_size = min_children, .max_size = 3})
                    .map([](std::vector<Node> children) {
                        return Node{-1, std::move(children)};
                    });
            },
            params);
    }

    hegel::Settings recursive_settings(uint64_t test_cases = 200) {
        return hegel::Settings{
            .test_cases = test_cases,
            .derandomize = true,
            .database = hegel::Database::disabled(),
            .suppress_health_check = {hegel::HealthCheck::FilterTooMuch}};
    }
} // namespace

TEST(Recursive, RespectsDepthAndLeafLimits) {
    hegel::test(
        [](hegel::TestCase& tc) {
            Node node = tc.draw(nodes({.max_depth = 3, .max_leaves = 10}));
            EXPECT_LE(depth_of(node), 3u);
            EXPECT_LE(leaves_of(node), 10u);
        },
        recursive_settings());
}

TEST(Recursive, GeneratesNestedValues) {
    auto found = hegel::tests::common::find_any<Node>(
        nodes(), [](const Node& node) { return depth_of(node) >= 2; });
    EXPECT_GE(depth_of(found), 2u);
}

TEST(Recursive, ZeroDepthGeneratesOnlyLeaves) {
    hegel::tests::common::assert_all_examples<Node>(
        nodes({.max_depth = 0}),
        [](const Node& node) { return node.children.empty(); });
}

TEST(Recursive, ShrinksToALeaf) {
    Node found = hegel::tests::common::minimal<Node>(
        nodes(), [](const Node&) { return true; });
    EXPECT_TRUE(found.children.empty());
    EXPECT_EQ(found.value, 0);
}

// Branches that need more leaves than the budget allows are discarded and
// tried again. When the engine gives up, it rejects the test case.
TEST(Recursive, OverBudgetAttemptsAreRetried) {
    int drawn = 0;
    hegel::test(
        [&drawn](hegel::TestCase& tc) {
            Node node = tc.draw(nodes({.max_leaves = 4}, 3));
            EXPECT_LE(leaves_of(node), 4u);
            drawn++;
        },
        recursive_settings());
    EXPECT_GT(drawn, 0);
}

TEST(Recursive, SubtreesShareTheLabelOfTheWholeValue) {
    namespace hi = hegel::internal;
    auto leaf = gs::integers<int>({.min_value = 0, .max_value = 100});
    uint64_t expected =
        hi::combine_labels({hi::label(hi::LabelName::Recursive), leaf.label()});
    uint64_t seen = 0;
    auto gen = gs::recursive(leaf, [&seen](gs::Generator<int> subtrees) {
        seen = subtrees.label();
        return subtrees.map([](int x) { return x + 1; });
    });
    EXPECT_EQ(gen.label(), expected);
    hegel::test([&gen](hegel::TestCase& tc) { (void)tc.draw(gen); },
                recursive_settings(50));
    EXPECT_EQ(seen, expected);
}
