// Numeric generators: integers, floats, booleans — parameter validation and
// IEEE-754 special-value behavior.

#include <gtest/gtest.h>

#include <cmath>
#include <cstdint>
#include <limits>
#include <stdexcept>

#include <hegel/hegel.h>

#include "common/utils.h"

namespace gs = hegel::generators;

TEST(Validation, WeightedBooleansProbability) {
    EXPECT_THROW(gs::weighted_booleans(-0.1), std::invalid_argument);
    EXPECT_THROW(gs::weighted_booleans(1.1), std::invalid_argument);
    EXPECT_THROW(gs::weighted_booleans(std::nan("")), std::invalid_argument);
    EXPECT_NO_THROW(gs::weighted_booleans(0.0));
    EXPECT_NO_THROW(gs::weighted_booleans(1.0));
}

TEST(WeightedBooleans, FollowTheirProbability) {
    int trues = 0;
    int cases = 0;
    hegel::test(
        [&](hegel::TestCase& tc) {
            EXPECT_FALSE(tc.draw(gs::weighted_booleans(0.0)));
            EXPECT_TRUE(tc.draw(gs::weighted_booleans(1.0)));
            trues += tc.draw(gs::weighted_booleans(0.9)) ? 1 : 0;
            ++cases;
        },
        hegel::Settings{.test_cases = 100,
                        .database = hegel::Database::disabled()});
    EXPECT_GT(trues, cases / 2) << trues << " of " << cases;
    EXPECT_EQ(gs::weighted_booleans(0.9).label(), gs::booleans().label());
}

TEST(Validation, IntegersMinGreaterThanMax) {
    EXPECT_THROW(gs::integers<int>({.min_value = 10, .max_value = 5}),
                 std::invalid_argument);
}

TEST(Validation, IntegersMinEqualMaxDoesNotThrow) {
    EXPECT_NO_THROW(gs::integers<int>({.min_value = 5, .max_value = 5}));
}

TEST(Validation, FloatsAllowNanWithMinValue) {
    EXPECT_THROW(gs::floats({.min_value = 0.0, .allow_nan = true}),
                 std::invalid_argument);
}

TEST(Validation, FloatsAllowNanWithMaxValue) {
    EXPECT_THROW(gs::floats({.max_value = 1.0, .allow_nan = true}),
                 std::invalid_argument);
}

TEST(Validation, FloatsMinGreaterThanMax) {
    EXPECT_THROW(gs::floats({.min_value = 10.0, .max_value = 5.0}),
                 std::invalid_argument);
}

TEST(Validation, FloatsAllowInfinityWithBothBounds) {
    EXPECT_THROW(
        gs::floats(
            {.min_value = 0.0, .max_value = 1.0, .allow_infinity = true}),
        std::invalid_argument);
}

TEST(Validation, FloatsBothFormsOfOneBound) {
    EXPECT_THROW(gs::floats({.min_value = 0.0, .min_value_exclusive = 0.0}),
                 std::invalid_argument);
    EXPECT_THROW(gs::floats({.max_value = 1.0, .max_value_exclusive = 1.0}),
                 std::invalid_argument);
}

TEST(Validation, FloatsAllowSubnormalAgainstTheBounds) {
    EXPECT_THROW(gs::floats({.min_value = 1.0, .allow_subnormal = true}),
                 std::invalid_argument);
    EXPECT_THROW(gs::floats({.max_value = -1.0, .allow_subnormal = true}),
                 std::invalid_argument);
    EXPECT_THROW(gs::floats({.min_value = 1e-310,
                             .max_value = 2e-310,
                             .allow_subnormal = false}),
                 std::invalid_argument);
    EXPECT_NO_THROW(gs::floats(
        {.min_value = -1e-310, .max_value = 0.0, .allow_subnormal = false}));
    EXPECT_NO_THROW(gs::floats(
        {.min_value = 1e-310, .max_value = 1.0, .allow_subnormal = false}));
}

TEST(Floats, ExclusiveBoundsExcludeTheBoundItself) {
    hegel::tests::common::assert_all_examples<double>(
        gs::floats<double>(
            {.min_value_exclusive = 0.0, .max_value_exclusive = 1.0}),
        [](const double& x) { return x > 0.0 && x < 1.0; }, 500);
    hegel::tests::common::assert_all_examples<double>(
        gs::floats<double>({.max_value = 0.0, .min_value_exclusive = -1.0}),
        [](const double& x) { return x > -1.0 && x <= 0.0; }, 500);
}

TEST(Floats, SubnormalsAreAllowedWhenTheBoundsAdmitAny) {
    const double normal = std::numeric_limits<double>::min();
    hegel::tests::common::assert_all_examples<double>(
        gs::floats<double>({.min_value = 1e-310, .max_value = 1e-310}),
        [](const double& x) { return x == 1e-310; }, 20);
    hegel::tests::common::assert_all_examples<double>(
        gs::floats<double>({.min_value = 1.0, .max_value = 1.0}),
        [](const double& x) { return x == 1.0; }, 20);
    hegel::tests::common::assert_all_examples<double>(
        gs::floats<double>({.min_value = -1e-310, .max_value = 1.0}),
        [](const double& x) { return x >= -1e-310 && x <= 1.0; }, 100);
    hegel::tests::common::assert_all_examples<double>(
        gs::floats<double>({.min_value = 1.0}),
        [](const double& x) { return x >= 1.0; }, 100);
    hegel::tests::common::assert_all_examples<double>(
        gs::floats<double>({.max_value = -1.0}),
        [](const double& x) { return x <= -1.0; }, 100);
    hegel::tests::common::assert_all_examples<double>(
        gs::floats<double>(
            {.min_value = -1.0, .max_value = 1.0, .allow_subnormal = false}),
        [normal](const double& x) {
            return x == 0.0 || std::fabs(x) >= normal;
        },
        500);
    hegel::tests::common::assert_all_examples<float>(
        gs::floats<float>(
            {.min_value_exclusive = 0.0f, .allow_subnormal = false}),
        [](const float& x) { return x >= std::numeric_limits<float>::min(); },
        500);
}

// An unbounded float draw (allow_nan / allow_infinity default to true) must
// actually produce the IEEE-754 special values, not flatten them somewhere
// between the engine and the caller. Observing each special value across the
// run pins this property down.
TEST(Floats, NanAndInfinitySurviveGeneration) {
    bool saw_nan = false;
    bool saw_pos_inf = false;
    bool saw_neg_inf = false;

    auto gen = gs::floats<double>();
    hegel::test(
        [&](hegel::TestCase& tc) {
            double x = tc.draw(gen);
            if (std::isnan(x)) {
                saw_nan = true;
            } else if (std::isinf(x)) {
                (x > 0 ? saw_pos_inf : saw_neg_inf) = true;
            }
        },
        hegel::Settings{.test_cases = 2000,
                        .seed = 1,
                        .database = hegel::Database::disabled()});

    EXPECT_TRUE(saw_nan) << "NaN never generated";
    EXPECT_TRUE(saw_pos_inf) << "+infinity never generated";
    EXPECT_TRUE(saw_neg_inf) << "-infinity never generated";
}

#ifdef __SIZEOF_INT128__
// Integer types wider than 64 bits draw across their whole range through the
// engine's big-integer draw.
TEST(WideIntegers, SignedDrawsOutsideTheInt64Range) {
    auto wide = hegel::tests::common::find_any<__int128>(
        gs::integers<__int128>(), [](const __int128& x) {
            return x > static_cast<__int128>(INT64_MAX) ||
                   x < static_cast<__int128>(INT64_MIN);
        });
    EXPECT_TRUE(wide > static_cast<__int128>(INT64_MAX) ||
                wide < static_cast<__int128>(INT64_MIN));
}

TEST(WideIntegers, SignedRespectsBounds) {
    __int128 low = -(static_cast<__int128>(1) << 100);
    __int128 high = -(static_cast<__int128>(1) << 99);
    hegel::tests::common::assert_all_examples<__int128>(
        gs::integers<__int128>({.min_value = low, .max_value = high}),
        [low, high](const __int128& x) { return x >= low && x <= high; });
}

TEST(WideIntegers, SignedShrinksTowardZero) {
    __int128 bound = -(static_cast<__int128>(1) << 70);
    __int128 found = hegel::tests::common::minimal<__int128>(
        gs::integers<__int128>(),
        [bound](const __int128& x) { return x < bound; });
    EXPECT_TRUE(found < bound && found > 4 * bound);
}

TEST(WideIntegers, UnsignedReachesTheTopOfTheRange) {
    unsigned __int128 top = ~static_cast<unsigned __int128>(0);
    unsigned __int128 found = hegel::tests::common::minimal<unsigned __int128>(
        gs::integers<unsigned __int128>(),
        [top](const unsigned __int128& x) { return x > top - 10; });
    EXPECT_TRUE(found == top - 9);
}

TEST(WideIntegers, PrintAsDecimal) {
    EXPECT_EQ(hegel::internal::repr(static_cast<__int128>(1) << 100),
              "static_cast<__int128>(1267650600228229401496703205376)");
    EXPECT_EQ(hegel::internal::repr(-(static_cast<__int128>(1) << 100)),
              "static_cast<__int128>(-1267650600228229401496703205376)");
}
#endif

// A float is drawn at 32 bits, so a lower bound excluded from the range is
// never reached by rounding a smaller double to a float.
TEST(Floats, SinglePrecisionDrawsAtFloatWidth) {
    hegel::tests::common::assert_all_examples<float>(
        gs::floats<float>({.min_value_exclusive = 0.0f}),
        [](const float& x) { return x > 0.0f; }, 500);
    hegel::tests::common::assert_all_examples<float>(
        gs::floats<float>({.min_value = -1.0f, .max_value = 1.0f}),
        [](const float& x) { return x >= -1.0f && x <= 1.0f; });
}

// A long double is drawn as a double, whatever its size on the platform.
TEST(Floats, LongDoubleDrawsAsDouble) {
    hegel::tests::common::assert_all_examples<long double>(
        gs::floats<long double>(), [](const long double& x) {
            return std::isnan(x) || std::isinf(x) ||
                   std::fabs(x) <= std::numeric_limits<double>::max();
        });
}

TEST(Floats, NarrowingABoundMovesItInward) {
    using gs::detail::narrow_down;
    using gs::detail::narrow_up;
    float up = narrow_up<float>(0.1);
    float down = narrow_down<float>(0.1);
    EXPECT_GT(static_cast<double>(up), 0.1);
    EXPECT_LT(static_cast<double>(down), 0.1);
    EXPECT_EQ(std::nextafter(down, 1.0f), up);
    EXPECT_EQ(narrow_up<float>(0.5), 0.5f);
    EXPECT_EQ(narrow_down<float>(0.5), 0.5f);
    EXPECT_EQ(narrow_up<float>(1e300), std::numeric_limits<float>::infinity());
    EXPECT_EQ(narrow_down<float>(1e300), std::numeric_limits<float>::max());
    EXPECT_EQ(narrow_up<float>(-1e300), std::numeric_limits<float>::lowest());
    EXPECT_EQ(narrow_down<float>(-1e300),
              -std::numeric_limits<float>::infinity());
}

TEST(Floats, LongDoubleBoundsBetweenDoublesAreRespected) {
    double above_one = std::nextafter(1.0, 2.0);
    double third = std::nextafter(std::nextafter(above_one, 2.0), 2.0);
    long double tiny = std::ldexp(1.0L, -54);
    long double min_value = 1.0L + tiny;
    long double max_value = static_cast<long double>(third) - tiny;
    hegel::tests::common::assert_all_examples<long double>(
        gs::floats<long double>(
            {.min_value = min_value, .max_value = max_value}),
        [&](const long double& x) { return x >= min_value && x <= max_value; });
}

TEST(Floats, LongDoubleExcludesOnlyBoundsThatAreDoubles) {
    double above_one = std::nextafter(1.0, 2.0);
    double second = std::nextafter(above_one, 2.0);
    long double tiny = std::ldexp(1.0L, -54);
    long double max_value = static_cast<long double>(second) - tiny;
    for (long double min_value : {1.0L, 1.0L + tiny}) {
        hegel::tests::common::assert_all_examples<long double>(
            gs::floats<long double>({.min_value_exclusive = min_value,
                                     .max_value_exclusive = max_value}),
            [&](const long double& x) {
                return x == static_cast<long double>(above_one);
            });
    }
}
