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
        std::optional<T> min_value; ///< Minimum value. Default: no minimum
        std::optional<T> max_value; ///< Maximum value. Default: no maximum
        bool exclude_min =
            false; ///< If true, exclude min_value (exclusive bound)
        bool exclude_max =
            false; ///< If true, exclude max_value (exclusive bound)
        std::optional<bool>
            allow_nan; ///< Allow NaN. Default: true if unbounded
        std::optional<bool>
            allow_infinity; ///< Allow infinity. Default: true if unbounded
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

        // The least value of type To that is not below `value`. A value above
        // the finite range of To narrows to infinity.
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

        // The greatest value of type To that is not above `value`. A value
        // below the finite range of To narrows to minus infinity.
        template <typename To, typename From> To narrow_down(From value) {
            return -narrow_up<To>(-value);
        }

    } // namespace detail

    // Concrete IGenerator<T> subclass produced by integers().
    template <typename T>
    HEGEL_REQUIRES(std::is_integral_v<T>)
    class IntegerGenerator : public IGenerator<T> {
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
    class FloatGenerator : public IGenerator<T> {
        static_assert(std::is_floating_point_v<T>,
                      "floats<T> requires a floating-point type T");

        // The engine draws 32-bit and 64-bit floats. A float is drawn at 32
        // bits, and every other type at 64.
        using Drawn =
            std::conditional_t<std::is_same_v<T, float>, float, double>;

      public:
        explicit FloatGenerator(const FloatsParams<T>& params = {}) {
            bool has_min = params.min_value.has_value();
            bool has_max = params.max_value.has_value();
            allow_nan_ = params.allow_nan.value_or(!has_min && !has_max);
            allow_infinity_ =
                params.allow_infinity.value_or(!has_min || !has_max);
            if (allow_nan_ && (has_min || has_max)) {
                throw std::invalid_argument(
                    "Cannot have allow_nan=true with min_value or max_value");
            }
            if (has_min && has_max && *params.min_value > *params.max_value) {
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
            // drawn at. A bound of a type wider than double narrows inward, to
            // the nearest double inside the range, so every value drawn is in
            // the range. A narrowed bound is no longer a value of the range,
            // so it is never excluded.
            min_ =
                has_min ? detail::narrow_up<double>(*params.min_value)
                : allow_infinity_
                    ? -std::numeric_limits<double>::infinity()
                    : static_cast<double>(std::numeric_limits<Drawn>::lowest());
            max_ = has_max ? detail::narrow_down<double>(*params.max_value)
                   : allow_infinity_
                       ? std::numeric_limits<double>::infinity()
                       : static_cast<double>(std::numeric_limits<Drawn>::max());
            exclude_min_ =
                params.exclude_min && static_cast<T>(min_) == *params.min_value;
            exclude_max_ =
                params.exclude_max && static_cast<T>(max_) == *params.max_value;
        }

        T do_draw(const TestCase& tc) const override {
            constexpr uint32_t width = std::is_same_v<T, float> ? 32 : 64;
            return static_cast<T>(hegel::internal::draw_float(
                tc, width, min_, max_, allow_nan_, allow_infinity_,
                exclude_min_, exclude_max_,
                static_cast<double>(std::numeric_limits<Drawn>::denorm_min())));
        }

        uint64_t label() const override {
            return hegel::internal::label(hegel::internal::LabelName::Floats);
        }

      private:
        double min_;
        double max_;
        bool allow_nan_;
        bool allow_infinity_;
        bool exclude_min_;
        bool exclude_max_;
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
     *     .min_value = 0.0, .max_value = 1.0,
     *     .exclude_min = true, .exclude_max = true
     * });
     * @endcode
     *
     * A `float` is drawn as a 32-bit value, so it shrinks and hits edge
     * cases as a `float`. Every other type is drawn as a `double`. Where
     * `long double` is wider than `double`, a bound that is not a `double`
     * is moved inward to the nearest `double`, so every value is still
     * within the bounds. Bounds with no `double` between them are an
     * error.
     *
     * @tparam T Floating point type (default: double)
     * @param params Bounds and exclusion constraints
     * @return Generator producing floats in the specified range
     */
    template <typename T = double>
    HEGEL_REQUIRES(std::is_floating_point_v<T>)
    Generator<T> floats(FloatsParams<T> params = {}) {
        return Generator<T>(new FloatGenerator<T>(std::move(params)));
    }

    /// @}

} // namespace hegel::generators
