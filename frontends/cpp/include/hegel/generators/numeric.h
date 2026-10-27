#pragma once

#include <array>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <type_traits>

#include "hegel/config.h"
#include "hegel/core.h"

namespace hegel::generators {

    // =============================================================================
    // Parameter structs
    // =============================================================================

    /**
     * @brief Parameters for integers() generator.
     * @tparam T The integer type
     */
    template <typename T> struct IntegersParams {
        std::optional<T>
            min_value; ///< Minimum value (inclusive). Default: type minimum
        std::optional<T>
            max_value; ///< Maximum value (inclusive). Default: type maximum
    };

    /**
     * @brief Parameters for floats() generator.
     * @tparam T The floating point type
     */
    template <typename T> struct FloatsParams {
        std::optional<T> min_value; ///< Inclusive minimum. Default: no minimum
        std::optional<T> max_value; ///< Inclusive maximum. Default: no maximum
        std::optional<T>
            min_value_exclusive; ///< A minimum that is itself excluded. Not
                                 ///< together with min_value
        std::optional<T>
            max_value_exclusive; ///< A maximum that is itself excluded. Not
                                 ///< together with max_value
        std::optional<bool>
            allow_nan; ///< Allow NaN. Default: true if unbounded
        std::optional<bool>
            allow_infinity; ///< Allow infinity. Default: true if unbounded
        std::optional<bool>
            allow_subnormal; ///< Allow subnormal values. Default: true when
                             ///< the bounds admit any
    };

    /// @cond INTERNAL
    namespace detail {

        // Draws an integer of a type wider than 64 bits through the engine's
        // big-integer draw. The buffers have one byte more than T, so the
        // sign of an unsigned bound is always clear.
        template <typename T>
        T draw_wide_integer(const TestCase& tc, T min_value, T max_value) {
            using U = std::make_unsigned_t<T>;
            constexpr std::size_t len = sizeof(T) + 1;
            auto encode = [](T value) {
                std::array<uint8_t, len> bytes{};
                auto bits = static_cast<U>(value);
                for (std::size_t i = 0; i < sizeof(T); ++i) {
                    bytes[i] = static_cast<uint8_t>(bits >> (8 * i));
                }
                if constexpr (std::is_signed_v<T>) {
                    bytes[sizeof(T)] = value < 0 ? 0xFF : 0;
                }
                return bytes;
            };
            std::array<uint8_t, len> min_bytes = encode(min_value);
            std::array<uint8_t, len> max_bytes = encode(max_value);
            std::array<uint8_t, len> out{};
            hegel::internal::draw_integer_bytes(
                tc, min_bytes.data(), max_bytes.data(), len, out.data());
            U bits = 0;
            for (std::size_t i = 0; i < sizeof(T); ++i) {
                bits |= static_cast<U>(out[i]) << (8 * i);
            }
            return static_cast<T>(bits);
        }

        template <typename To, typename From> To narrow_up(From value) {
            if (value > static_cast<From>(std::numeric_limits<To>::max())) {
                return std::numeric_limits<To>::infinity();
            }
            if (value < static_cast<From>(std::numeric_limits<To>::lowest())) {
                return std::numeric_limits<To>::lowest();
            }
            auto narrowed = static_cast<To>(value);
            if (static_cast<From>(narrowed) < value) {
                narrowed = std::nextafter(narrowed,
                                          std::numeric_limits<To>::infinity());
            }
            return narrowed;
        }

        template <typename To, typename From> To narrow_down(From value) {
            return -narrow_up<To>(-value);
        }

    } // namespace detail

    // Concrete IGenerator<T> subclass produced by integers().
    template <typename T>
    HEGEL_REQUIRES(std::is_integral_v<T>)
    class IntegerGenerator : public detail::ValuePrinted<T> {
        static_assert(std::is_integral_v<T>,
                      "integers<T> requires an integral type T");

      public:
        explicit IntegerGenerator(const IntegersParams<T>& params = {})
            : min_(params.min_value.value_or(std::numeric_limits<T>::min())),
              max_(params.max_value.value_or(std::numeric_limits<T>::max())) {
            if (min_ > max_) {
                throw std::invalid_argument(
                    "Cannot have max_value < min_value");
            }
        }

        T do_draw(const TestCase& tc) const override {
            if constexpr (sizeof(T) > sizeof(int64_t)) {
                return detail::draw_wide_integer<T>(tc, min_, max_);
            } else if constexpr (std::is_signed_v<T>) {
                return static_cast<T>(hegel::internal::draw_integer(
                    tc, static_cast<int64_t>(min_),
                    static_cast<int64_t>(max_)));
            } else {
                return static_cast<T>(hegel::internal::draw_integer_unsigned(
                    tc, static_cast<uint64_t>(min_),
                    static_cast<uint64_t>(max_)));
            }
        }

        uint64_t label() const override {
            return hegel::internal::label(hegel::internal::LabelName::Integers);
        }

      private:
        T min_;
        T max_;
    };

    // Concrete IGenerator<T> subclass produced by floats(). Resolves the
    // engine draw parameters once at construction.
    template <typename T>
    HEGEL_REQUIRES(std::is_floating_point_v<T>)
    class FloatGenerator : public detail::ValuePrinted<T> {
        static_assert(std::is_floating_point_v<T>,
                      "floats<T> requires a floating-point type T");

        // The engine draws 32-bit and 64-bit floats. A float is drawn at 32
        // bits, and every other type at 64.
        using Drawn =
            std::conditional_t<std::is_same_v<T, float>, float, double>;

      public:
        explicit FloatGenerator(const FloatsParams<T>& params = {}) {
            if (params.min_value && params.min_value_exclusive) {
                throw std::invalid_argument(
                    "Cannot give both min_value and min_value_exclusive");
            }
            if (params.max_value && params.max_value_exclusive) {
                throw std::invalid_argument(
                    "Cannot give both max_value and max_value_exclusive");
            }
            bool exclude_min = params.min_value_exclusive.has_value();
            bool exclude_max = params.max_value_exclusive.has_value();
            std::optional<T> min_value =
                exclude_min ? params.min_value_exclusive : params.min_value;
            std::optional<T> max_value =
                exclude_max ? params.max_value_exclusive : params.max_value;
            bool has_min = min_value.has_value();
            bool has_max = max_value.has_value();
            allow_nan_ = params.allow_nan.value_or(!has_min && !has_max);
            allow_infinity_ =
                params.allow_infinity.value_or(!has_min || !has_max);
            if (allow_nan_ && (has_min || has_max)) {
                throw std::invalid_argument(
                    "Cannot have allow_nan=true with min_value or max_value");
            }
            if (has_min && has_max && *min_value > *max_value) {
                throw std::invalid_argument(
                    "Cannot have max_value < min_value");
            }
            if (allow_infinity_ && has_min && has_max) {
                throw std::invalid_argument(
                    "Cannot have allow_infinity=true with both min_value and "
                    "max_value");
            }

            // An unset bound is the widest the draw allows: infinite when
            // infinities may be drawn, else the finite extreme of the width
            // drawn at.
            min_ =
                has_min ? detail::narrow_up<double>(*min_value)
                : allow_infinity_
                    ? -std::numeric_limits<double>::infinity()
                    : static_cast<double>(std::numeric_limits<Drawn>::lowest());
            max_ = has_max ? detail::narrow_down<double>(*max_value)
                   : allow_infinity_
                       ? std::numeric_limits<double>::infinity()
                       : static_cast<double>(std::numeric_limits<Drawn>::max());
            exclude_min_ = exclude_min && static_cast<T>(min_) == *min_value;
            exclude_max_ = exclude_max && static_cast<T>(max_) == *max_value;

            const auto smallest_normal =
                static_cast<double>(std::numeric_limits<Drawn>::min());
            bool allow_subnormal = params.allow_subnormal.value_or(
                subnormal_default(has_min, has_max, smallest_normal));
            if (allow_subnormal) {
                if (has_min && min_ >= smallest_normal) {
                    throw std::invalid_argument(
                        "allow_subnormal=true, but min_value excludes all "
                        "values below the smallest positive normal");
                }
                if (has_max && max_ <= -smallest_normal) {
                    throw std::invalid_argument(
                        "allow_subnormal=true, but max_value excludes all "
                        "values above the smallest negative normal");
                }
            } else if (has_min && has_max) {
                bool contains_zero = min_ <= 0.0 && max_ >= 0.0;
                if (!contains_zero && max_ < smallest_normal &&
                    min_ > -smallest_normal) {
                    throw std::invalid_argument(
                        "allow_subnormal=false leaves no floating-point "
                        "values between min_value and max_value");
                }
            }
            smallest_nonzero_magnitude_ =
                allow_subnormal ? static_cast<double>(
                                      std::numeric_limits<Drawn>::denorm_min())
                                : smallest_normal;
        }

        T do_draw(const TestCase& tc) const override {
            constexpr uint32_t width = std::is_same_v<T, float> ? 32 : 64;
            return static_cast<T>(hegel::internal::draw_float(
                tc, width, min_, max_, allow_nan_, allow_infinity_,
                exclude_min_, exclude_max_, smallest_nonzero_magnitude_));
        }

        uint64_t label() const override {
            return hegel::internal::label(hegel::internal::LabelName::Floats);
        }

      private:
        bool subnormal_default(bool has_min, bool has_max,
                               double smallest_normal) const {
            if (has_min && has_max) {
                if (min_ == max_) {
                    return -smallest_normal < min_ && min_ < smallest_normal;
                }
                return min_ < smallest_normal && max_ > -smallest_normal;
            }
            if (has_min) {
                return min_ < smallest_normal;
            }
            if (has_max) {
                return max_ > -smallest_normal;
            }
            return true;
        }

        double min_;
        double max_;
        bool allow_nan_;
        bool allow_infinity_;
        bool exclude_min_;
        bool exclude_max_;
        double smallest_nonzero_magnitude_;
    };
    /// @endcond

    /// @name Numeric
    /// @{

    /**
     * @brief Generate random integers. For a given integral type T, produces
     * values in the range [std::numeric_limits<T>::min(),
     * std::numeric_limits<T>::max()] by default.
     *
     * @code{.cpp}
     * auto any_int = integers<int>();
     * auto bounded = integers<int>({.min_value = 0, .max_value = 100});
     * auto positive = integers<int>({.min_value = 1});
     * @endcode
     *
     * Integer types wider than 64 bits, such as `__int128` where the compiler
     * provides it, are supported.
     *
     * @tparam T Integer type (default: int64_t)
     * @param params Bounds constraints
     * @return Generator producing integers in the specified range
     */
    template <typename T = int64_t>
    HEGEL_REQUIRES(std::is_integral_v<T>)
    Generator<T> integers(IntegersParams<T> params = {}) {
        return Generator<T>(new IntegerGenerator<T>(std::move(params)));
    }

    /**
     * @brief Generate random floating point numbers.
     *
     * @code{.cpp}
     * auto any_float = floats<double>();
     * auto unit = floats<double>({.min_value = 0.0, .max_value = 1.0});
     * auto open = floats<double>({
     *     .min_value_exclusive = 0.0, .max_value_exclusive = 1.0
     * });
     * auto flush_to_zero_safe = floats<double>({.allow_subnormal = false});
     * @endcode
     *
     * A bound is inclusive (`min_value`, `max_value`) or exclusive
     * (`min_value_exclusive`, `max_value_exclusive`); giving both forms of
     * one bound is an error. Subnormal values are drawn whenever the bounds
     * admit any, unless `allow_subnormal` is `false`; asking for them with
     * bounds that exclude them all, or excluding them from a range that
     * holds nothing else, is an error.
     *
     * A `float` is drawn as a 32-bit value, so it shrinks and hits edge
     * cases as a `float`. Every other type is drawn as a `double`. Where
     * `long double` is wider than `double`, a bound that is not a `double`
     * is moved inward to the nearest `double`, so every value is still
     * within the bounds. Bounds with no `double` between them are an
     * error.
     *
     * @tparam T Floating point type (default: double)
     * @param params Bounds, exclusion and special-value constraints
     * @return Generator producing floats in the specified range
     */
    template <typename T = double>
    HEGEL_REQUIRES(std::is_floating_point_v<T>)
    Generator<T> floats(FloatsParams<T> params = {}) {
        return Generator<T>(new FloatGenerator<T>(std::move(params)));
    }

    /// @}

} // namespace hegel::generators
