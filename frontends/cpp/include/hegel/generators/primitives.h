#pragma once

#include "hegel/core.h"

#include <variant>

namespace hegel::generators {

    /// @name Primitives
    /// @{

    /**
     * @brief Generate random boolean values.
     * @return Generator producing `true` or `false`.
     */
    Generator<bool> booleans();

    /// @cond INTERNAL
    // Concrete IGenerator<T> subclass produced by just(). A constant draws
    // zero entropy, so it returns the captured value without touching the
    // engine at all.
    template <typename T> class JustGenerator : public IGenerator<T> {
      public:
        explicit JustGenerator(T value) : value_(std::move(value)) {}

        T do_draw(const TestCase&) const override { return value_; }

        uint64_t label() const override {
            return hegel::internal::label(hegel::internal::LabelName::Just);
        }

      private:
        T value_;
    };
    /// @endcond

    /**
     * @brief Generate a constant value.
     *
     * @code{.cpp}
     * auto answer = just(42);
     * auto greeting = just("hello");
     * @endcode
     *
     * @tparam T The value type
     * @param value The constant value to generate
     * @return Generator that always produces value
     */
    template <typename T> Generator<T> just(T value) {
        return Generator<T>(new JustGenerator<T>(std::move(value)));
    }

    /**
     * @brief Overload for `just` so that `just("a string literal")` has type
     * `Generator<std::string>` rather than `Generator<const char*>`.
     * @param value The constant value to generator
     * @return Generator that always produces value
     */
    inline Generator<std::string> just(const char* value) {
        return just(std::string(value));
    }

    /**
     * @brief Generate the unit value, `std::monostate{}`.
     *
     * Draws nothing. Useful where a generator is required but carries no
     * information, for example as an alternative of variant().
     *
     * @code{.cpp}
     * auto maybe = variant(none(), integers<int>());
     * // Generator<std::variant<std::monostate, int>>
     * @endcode
     *
     * @return Generator that always produces `std::monostate{}`
     */
    inline Generator<std::monostate> none() { return just(std::monostate{}); }

    /// @}

} // namespace hegel::generators
