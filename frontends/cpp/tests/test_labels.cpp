// Span labels: every generator identifies itself to the engine with a label
// derived from its `hegel.<kind>` name, and a generator built from others
// combines its own label with theirs.

#include <gtest/gtest.h>

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <tuple>
#include <typeinfo>
#include <vector>

#include <hegel/hegel.h>

namespace gs = hegel::generators;
namespace hi = hegel::internal;

namespace {
    uint64_t combined(hi::LabelName name, std::vector<uint64_t> components) {
        components.insert(components.begin(), hi::label(name));
        return hi::combine_labels(components);
    }
} // namespace

TEST(Labels, LabelIsTheHashOfTheName) {
    // 64-bit FNV-1a of "hegel.integers", as every Hegel frontend derives it.
    uint64_t hash = 0xcbf29ce484222325ULL;
    for (char c : std::string("hegel.integers")) {
        hash ^= static_cast<unsigned char>(c);
        hash *= 0x100000001b3ULL;
    }
    EXPECT_EQ(hi::label(hi::LabelName::Integers), hash);
}

TEST(Labels, CombiningIsOrderSensitive) {
    uint64_t a = hi::label(hi::LabelName::Integers);
    uint64_t b = hi::label(hi::LabelName::Text);
    EXPECT_NE(hi::combine_labels({a, b}), hi::combine_labels({b, a}));
    EXPECT_NE(hi::combine_labels({a}), a);
}

namespace {
    uint64_t fnv1a(std::string_view name) {
        uint64_t hash = 0xcbf29ce484222325ULL;
        for (char c : name) {
            hash ^= static_cast<unsigned char>(c);
            hash *= 0x100000001b3ULL;
        }
        return hash;
    }

    class Custom : public gs::IGenerator<int> {
      public:
        int do_draw(const hegel::TestCase& tc) const override {
            return tc.draw_silent(gs::integers<int>());
        }
    };
} // namespace

TEST(Labels, GeneratorsOfYourOwnUseTheirTypeName) {
    uint64_t label = Custom().label();
    EXPECT_EQ(label, fnv1a(typeid(Custom).name()));
    EXPECT_EQ(Custom().label(), label);
    EXPECT_NE(gs::dates().label(), label);
}

TEST(Labels, LeafGeneratorsUseTheirName) {
    EXPECT_EQ(gs::integers<int>().label(), hi::label(hi::LabelName::Integers));
    EXPECT_EQ(gs::floats<double>().label(), hi::label(hi::LabelName::Floats));
    EXPECT_EQ(gs::booleans().label(), hi::label(hi::LabelName::Booleans));
    EXPECT_EQ(gs::just(1).label(), hi::label(hi::LabelName::Just));
    EXPECT_EQ(gs::sampled_from({1, 2}).label(),
              hi::label(hi::LabelName::SampledFrom));
    EXPECT_EQ(gs::text().label(), hi::label(hi::LabelName::Text));
    EXPECT_EQ(gs::characters().label(), hi::label(hi::LabelName::Characters));
    EXPECT_EQ(gs::from_regex("a+").label(),
              hi::label(hi::LabelName::FromRegex));
    EXPECT_EQ(gs::binary().label(), hi::label(hi::LabelName::Binary));
    EXPECT_EQ(gs::emails().label(), hi::label(hi::LabelName::Emails));
    EXPECT_EQ(gs::urls().label(), hi::label(hi::LabelName::Urls));
    EXPECT_EQ(gs::domains().label(), hi::label(hi::LabelName::Domains));
    EXPECT_EQ(gs::ip_addresses({.v = 4}).label(),
              hi::label(hi::LabelName::IpAddressesV4));
    EXPECT_EQ(gs::ip_addresses({.v = 6}).label(),
              hi::label(hi::LabelName::IpAddressesV6));
    EXPECT_EQ(gs::uuids().label(), hi::label(hi::LabelName::Uuids));
    EXPECT_EQ(gs::deferred<int>().generator().label(),
              hi::label(hi::LabelName::Deferred));
    EXPECT_EQ(gs::compose([](const hegel::TestCase&) { return 0; }).label(),
              hi::label(hi::LabelName::Composite));
}

TEST(Labels, CompoundGeneratorsCombineTheirComponents) {
    uint64_t ints = gs::integers<int>().label();
    uint64_t strings = gs::text().label();
    EXPECT_EQ(gs::vectors(gs::integers<int>()).label(),
              combined(hi::LabelName::Lists, {ints}));
    EXPECT_EQ(gs::sets(gs::integers<int>()).label(),
              combined(hi::LabelName::Sets, {ints}));
    EXPECT_EQ(gs::maps(gs::integers<int>(), gs::text()).label(),
              combined(hi::LabelName::Dicts, {ints, strings}));
    EXPECT_EQ(gs::tuples(gs::integers<int>(), gs::text()).label(),
              combined(hi::LabelName::Tuples, {ints, strings}));
    EXPECT_EQ((gs::arrays<int, 3>(gs::integers<int>()).label()),
              combined(hi::LabelName::Array, {ints}));
    EXPECT_EQ(gs::optional(gs::integers<int>()).label(),
              combined(hi::LabelName::Optional, {ints}));
    EXPECT_EQ(
        gs::one_of({gs::integers<int>(), gs::integers<int>()}).label(),
        combined(hi::LabelName::OneOf, {hi::combine_labels({ints, ints})}));
    EXPECT_EQ(
        gs::variant(gs::integers<int>(), gs::text()).label(),
        combined(hi::LabelName::OneOf, {hi::combine_labels({ints, strings})}));
    EXPECT_EQ(gs::integers<int>().map([](int x) { return x; }).label(),
              combined(hi::LabelName::Map, {ints}));
    EXPECT_EQ(gs::integers<int>().filter([](int) { return true; }).label(),
              combined(hi::LabelName::Filter, {ints}));
    EXPECT_EQ(
        gs::integers<int>().flat_map([](int) { return gs::text(); }).label(),
        combined(hi::LabelName::FlatMap, {ints}));
    EXPECT_EQ(
        gs::recursive(gs::integers<int>(),
                      [](gs::Generator<int> subtrees) { return subtrees; })
            .label(),
        combined(hi::LabelName::Recursive, {ints}));
    EXPECT_EQ(gs::permutations({1, 2}).label(),
              hi::label(hi::LabelName::Permutation));
}

TEST(Labels, SameShapeDifferentComponentsDiffer) {
    EXPECT_NE(gs::vectors(gs::integers<int>()).label(),
              gs::vectors(gs::text()).label());
    EXPECT_EQ(gs::vectors(gs::integers<int>()).label(),
              gs::vectors(gs::integers<int>()).label());
}

TEST(Labels, PoolGeneratorsNameTheirMode) {
    hegel::test(
        [](hegel::TestCase& tc) {
            hegel::stateful::Pool<int> pool(tc);
            EXPECT_EQ(hegel::stateful::values_consumed(pool).label(),
                      hi::label(hi::LabelName::PoolValuesConsumed));
            EXPECT_EQ(hegel::stateful::values_reusable(pool).label(),
                      hi::label(hi::LabelName::PoolValuesReusable));
            hegel::stateful::ConcurrentPool<int> shared(tc);
            EXPECT_EQ(hegel::stateful::values_consumed(shared).label(),
                      hi::label(hi::LabelName::PoolValuesConsumed));
            EXPECT_EQ(hegel::stateful::values_reusable(shared).label(),
                      hi::label(hi::LabelName::PoolValuesReusable));
        },
        hegel::Settings{.test_cases = 1});
}
