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
        gs::floats<float>({.min_value = 0.0f, .exclude_min = true}),
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
