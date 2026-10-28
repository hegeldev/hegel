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

TEST(Subsequences, KeepTheOriginalOrder) {
    std::vector<int> values{1, 2, 3, 4, 5};
    hegel::test(
        [&](hegel::TestCase& tc) {
            std::vector<int> sub = tc.draw(gs::subsequences(values));
            EXPECT_LE(sub.size(), values.size());
            EXPECT_TRUE(std::is_sorted(sub.begin(), sub.end())) << sub.size();
            EXPECT_TRUE(std::includes(values.begin(), values.end(), sub.begin(),
                                      sub.end()));
            EXPECT_GE(tc.draw(gs::subsequences(values, {.min_size = 2})).size(),
                      2u);
            EXPECT_LE(tc.draw(gs::subsequences(values, {.max_size = 1})).size(),
                      1u);
            EXPECT_EQ(tc.draw(gs::subsequences(
                          values, {.min_size = 5, .max_size = 10})),
                      values);
            EXPECT_TRUE(tc.draw(gs::subsequences(std::vector<int>{})).empty());
            std::vector<int> dup = tc.draw(gs::subsequences({1, 1, 2}));
            EXPECT_LE(std::count(dup.begin(), dup.end(), 1), 2);
        },
        hegel::Settings{.test_cases = 100,
                        .database = hegel::Database::disabled()});
}

TEST(Subsequences, ShrinkTowardTheEarliestElements) {
    auto sub = hegel::tests::common::minimal<std::vector<int>>(
        gs::subsequences({1, 2, 3, 4, 5}),
        [](const std::vector<int>& v) { return v.size() >= 2; });
    EXPECT_EQ(sub, (std::vector<int>{1, 2}));
}

TEST(Validation, SubsequenceSizes) {
    EXPECT_THROW(gs::subsequences({1, 2}, {.min_size = 3}),
                 std::invalid_argument);
    EXPECT_THROW(gs::subsequences({1, 2}, {.min_size = 2, .max_size = 1}),
                 std::invalid_argument);
}

TEST(Samples, WithReplacementRepeatElements) {
    bool repeated = false;
    hegel::test(
        [&](hegel::TestCase& tc) {
            std::vector<int> sample =
                tc.draw(gs::samples({1, 2, 3}, {.min_size = 4, .max_size = 6}));
            EXPECT_GE(sample.size(), 4u);
            EXPECT_LE(sample.size(), 6u);
            for (int x : sample) {
                EXPECT_TRUE(x >= 1 && x <= 3) << x;
            }
            std::set<int> distinct(sample.begin(), sample.end());
            repeated = repeated || distinct.size() < sample.size();
            EXPECT_TRUE(tc.draw(gs::samples(std::vector<int>{})).empty());
        },
        hegel::Settings{.test_cases = 100,
                        .database = hegel::Database::disabled()});
    EXPECT_TRUE(repeated);
}

TEST(Samples, WithoutReplacementAreReorderedSubsets) {
    std::vector<int> values{1, 2, 3, 4};
    hegel::test(
        [&](hegel::TestCase& tc) {
            std::vector<int> sample =
                tc.draw(gs::samples(values, {.replacement = false}));
            EXPECT_LE(sample.size(), values.size());
            std::set<int> distinct(sample.begin(), sample.end());
            EXPECT_EQ(distinct.size(), sample.size());
            for (int x : sample) {
                EXPECT_TRUE(x >= 1 && x <= 4) << x;
            }
            EXPECT_EQ(tc.draw(gs::samples(values, {.min_size = 4,
                                                   .max_size = 10,
                                                   .replacement = false}))
                          .size(),
                      4u);
            EXPECT_LE(tc.draw(gs::samples(values, {.max_size = 2,
                                                   .replacement = false}))
                          .size(),
                      2u);
        },
        hegel::Settings{.test_cases = 100,
                        .database = hegel::Database::disabled()});
}

TEST(Samples, ShrinkTowardTheEarliestElements) {
    auto with = hegel::tests::common::minimal<std::vector<int>>(
        gs::samples({5, 6, 7}),
        [](const std::vector<int>& v) { return v.size() >= 2; });
    EXPECT_EQ(with, (std::vector<int>{5, 5}));
    auto without = hegel::tests::common::minimal<std::vector<int>>(
        gs::samples({5, 6, 7}, {.replacement = false}),
        [](const std::vector<int>& v) { return v.size() >= 2; });
    EXPECT_EQ(without, (std::vector<int>{5, 6}));
}

TEST(Validation, SampleSizes) {
    EXPECT_THROW(gs::samples(std::vector<int>{}, {.min_size = 1}),
                 std::invalid_argument);
    EXPECT_THROW(gs::samples({1, 2}, {.min_size = 3, .replacement = false}),
                 std::invalid_argument);
    EXPECT_THROW(gs::samples({1, 2}, {.min_size = 2, .max_size = 1}),
                 std::invalid_argument);
    EXPECT_NO_THROW(gs::samples({1, 2}, {.min_size = 3}));
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
