// Collection generators: vectors, sets, maps, and fixed-size arrays — drawing
// and parameter validation.

#include <gtest/gtest.h>

#include <algorithm>
#include <array>
#include <set>
#include <stdexcept>
#include <variant>
#include <vector>

#include <hegel/hegel.h>

#include "common/utils.h"

namespace gs = hegel::generators;

// arrays<T, N>() draws exactly N elements, each from the given generator.
TEST(Arrays, DrawsExactlyNInRange) {
    hegel::test(
        [](hegel::TestCase& tc) {
            std::array<int, 4> a = tc.draw(gs::arrays<int, 4>(
                gs::integers<int>({.min_value = 0, .max_value = 9})));
            static_assert(std::tuple_size_v<decltype(a)> == 4);
            for (int x : a) {
                EXPECT_GE(x, 0);
                EXPECT_LE(x, 9);
            }
        },
        hegel::Settings{.test_cases = 100,
                        .derandomize = true,
                        .database = hegel::Database::disabled()});
}

// A fixed-size array generator composes inside other generators.
TEST(Arrays, NestsInsideOtherGenerators) {
    hegel::test(
        [](hegel::TestCase& tc) {
            auto rows = tc.draw(gs::vectors(
                gs::arrays<int, 2>(gs::integers<int>()), {.max_size = 3}));
            EXPECT_LE(rows.size(), 3u);
            for (const std::array<int, 2>& row : rows) {
                EXPECT_EQ(row.size(), 2u);
            }
        },
        hegel::Settings{.test_cases = 100,
                        .derandomize = true,
                        .database = hegel::Database::disabled()});
}

TEST(Validation, VectorsMaxSizeLessThanMinSize) {
    EXPECT_THROW(
        gs::vectors(gs::integers<int>(), {.min_size = 10, .max_size = 5}),
        std::invalid_argument);
}

TEST(Validation, SetsMaxSizeLessThanMinSize) {
    EXPECT_THROW(gs::sets(gs::integers<int>(), {.min_size = 10, .max_size = 5}),
                 std::invalid_argument);
}

TEST(Validation, MapsMaxSizeLessThanMinSize) {
    EXPECT_THROW(gs::maps(gs::text(), gs::integers<int>(),
                          {.min_size = 10, .max_size = 5}),
                 std::invalid_argument);
}

// unique_by keeps one element per key.
TEST(UniqueBy, KeysAreDistinct) {
    hegel::test(
        [](hegel::TestCase& tc) {
            auto xs = tc.draw(gs::vectors(
                gs::integers<int>({.min_value = 0, .max_value = 100}), {},
                [](const int& x) { return x % 10; }));
            std::set<int> keys;
            for (int x : xs) {
                EXPECT_TRUE(keys.insert(x % 10).second);
            }
            EXPECT_LE(xs.size(), 10u);
        },
        hegel::Settings{.test_cases = 100,
                        .derandomize = true,
                        .database = hegel::Database::disabled()});
}

// Elements with different keys can be equal in every other way.
TEST(UniqueBy, FindsSeveralElements) {
    auto xs = hegel::tests::common::find_any<std::vector<int>>(
        gs::vectors(gs::integers<int>(), {.min_size = 3},
                    [](const int& x) { return x; }),
        [](const std::vector<int>& v) { return v.size() >= 3; });
    EXPECT_GE(xs.size(), 3u);
}

TEST(Validation, VectorsUniqueAndUniqueByTogether) {
    EXPECT_THROW(gs::vectors(gs::integers<int>(), {.unique = true},
                             [](const int& x) { return x; }),
                 std::invalid_argument);
}

// A permutation holds every value once, in some order.
TEST(Permutations, AreReorderings) {
    std::vector<int> values{1, 2, 3, 4, 5, 5};
    hegel::tests::common::assert_all_examples<std::vector<int>>(
        gs::permutations(values), [values](const std::vector<int>& p) {
            std::vector<int> sorted = p;
            std::sort(sorted.begin(), sorted.end());
            return sorted == values;
        });
}

TEST(Permutations, FindsAReordering) {
    auto p = hegel::tests::common::find_any<std::vector<int>>(
        gs::permutations({1, 2, 3}),
        [](const std::vector<int>& v) { return v.front() == 3; });
    EXPECT_EQ(p.front(), 3);
}

TEST(Permutations, ShrinkTowardTheOriginalOrder) {
    auto p = hegel::tests::common::minimal<std::vector<int>>(
        gs::permutations({1, 2, 3, 4}),
        [](const std::vector<int>& v) { return v.back() == 1; });
    EXPECT_EQ(p, (std::vector<int>{2, 3, 4, 1}));
}

TEST(Permutations, OfNothingIsEmpty) {
    hegel::tests::common::assert_all_examples<std::vector<int>>(
        gs::permutations(std::vector<int>{}),
        [](const std::vector<int>& p) { return p.empty(); }, 5);
}

TEST(None, DrawsTheUnitValue) {
    hegel::test(
        [](hegel::TestCase& tc) {
            auto value = tc.draw(gs::variant(gs::none(), gs::integers<int>()));
            if (value.index() == 0) {
                EXPECT_EQ(std::get<0>(value), std::monostate{});
            }
        },
        hegel::Settings{.test_cases = 20,
                        .database = hegel::Database::disabled()});
    EXPECT_EQ(gs::none().label(),
              hegel::internal::label(hegel::internal::LabelName::Just));
}
