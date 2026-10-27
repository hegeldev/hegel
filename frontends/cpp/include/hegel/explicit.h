#pragma once

#include <any>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

#include "repr.h"

namespace hegel {

    /**
     * @brief A value of an ExplicitTestCase that a named draw takes.
     *
     * Create one with hegel::named().
     *
     * @tparam T The value type.
     */
    template <typename T> struct NamedValue {
        /// The name of the draw that takes the value.
        std::string name;
        /// The value.
        T value;
    };

    /**
     * @brief Give a value of an ExplicitTestCase to the draw named @p name.
     *
     * @tparam T The value type. It must be the type the draw produces.
     * @param name The name the draw prints under: `x_1` for the first
     *             `tc.draw("x", ...)`, `x_2` for the second, `draw_1` for the
     *             first unnamed draw.
     * @param value The value the draw returns.
     * @return The named value.
     */
    template <typename T>
    NamedValue<std::decay_t<T>> named(std::string name, T&& value) {
        return {std::move(name), std::forward<T>(value)};
    }

    /**
     * @brief Values to run a test with before Hegel generates any.
     *
     * An explicit test case runs the test body once, in the Explicit phase
     * (Phase::Explicit), before any generated test case. Each draw of the
     * body returns a value of the explicit test case instead of a generated
     * one: the value named for the draw (see hegel::named()), else the next
     * positional value. Each value must have the exact type the draw
     * produces, so write `int64_t{5}` for a draw of `integers<int64_t>()`.
     * A string literal is a `std::string`.
     *
     * Every value must be drawn. TestCase::target() and TestCase::event()
     * do nothing, TestCase::repeat() runs its body once, a TestCase::clone()
     * draws the same values and names its draws in the same sequence as the
     * test case it was cloned from, and TestCase::set_worker() does nothing.
     * Anything else is a usage error, which makes hegel::test() throw
     * `std::invalid_argument` without reporting a failure: a value that no
     * draw takes, a draw with no value or a value of another type, a
     * rejection (TestCase::assume() or TestCase::reject(), whose message
     * begins "Explicit test cases must not be rejected"), and anything that
     * needs generated data other than a draw (TestCase::draw_silent(),
     * stateful testing).
     *
     * If an explicit test case fails, Hegel prints what it drew and noted,
     * and the test's exception propagates; no test case is generated. With
     * Settings::report_multiple_failures, every explicit test case runs, the
     * generated test cases follow, and every distinct failure is reported
     * together, the explicit test cases' first.
     *
     * @code{.cpp}
     * HEGEL_EXPLICIT_TEST_CASE(division, 7, 0)
     * HEGEL_EXPLICIT_TEST_CASE(division, hegel::named("d", 1), 0)
     * HEGEL_TEST(division)(hegel::TestCase& tc) {
     *     auto n = tc.draw(gs::integers<int>());
     *     auto d = tc.draw("d", gs::integers<int>());
     *     if (d != 0 && n / d * d + n % d != n) {
     *         throw std::runtime_error("division is wrong");
     *     }
     * }
     * @endcode
     *
     * @see HEGEL_EXPLICIT_TEST_CASE, hegel::test()
     */
    class ExplicitTestCase {
      public:
        /// @cond INTERNAL
        // One value: its name if a named draw takes it, the value, and how
        // to print it.
        struct Value {
            std::optional<std::string> name;
            std::any value;
            std::string (*render)(const std::any&);
        };
        /// @endcond

        /**
         * @brief An explicit test case with the given values.
         *
         * @tparam Ts The value types. A hegel::named() value is named, and
         *            every other value is positional.
         * @param values The values, in the order the draws take the
         *               positional ones.
         */
        template <
            typename... Ts,
            typename = std::enable_if_t<
                !(std::is_same_v<std::decay_t<Ts>, ExplicitTestCase> || ...)>>
        explicit ExplicitTestCase(Ts&&... values) {
            (add(std::forward<Ts>(values)), ...);
        }

        /// @cond INTERNAL
        const std::vector<Value>& values() const { return values_; }
        /// @endcond

      private:
        template <typename T> static std::string render(const std::any& value) {
            return internal::repr(std::any_cast<const T&>(value));
        }

        template <typename T> static Value make_value(T value) {
            return Value{std::nullopt, std::any(std::move(value)),
                         &ExplicitTestCase::render<T>};
        }

        template <typename T> struct is_named : std::false_type {};
        template <typename T>
        struct is_named<NamedValue<T>> : std::true_type {};

        template <typename T> static Value positional(T&& value) {
            using V = std::decay_t<T>;
            if constexpr (std::is_same_v<V, const char*> ||
                          std::is_same_v<V, char*>) {
                return make_value(std::string(value));
            } else {
                return make_value(V(std::forward<T>(value)));
            }
        }

        template <typename T> void add(T&& value) {
            if constexpr (is_named<std::decay_t<T>>::value) {
                Value named_value = positional(std::forward<T>(value).value);
                named_value.name = value.name;
                values_.push_back(std::move(named_value));
            } else {
                values_.push_back(positional(std::forward<T>(value)));
            }
        }

        std::vector<Value> values_;
    };

} // namespace hegel
