#pragma once

#include <any>
#include <cstdint>
#include <functional>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <type_traits>
#include <typeinfo>

#include "config.h"
#include "internal.h"
#include "repr.h"
#include "test_case.h"

/**
 * @brief Hegel generators.
 */
namespace hegel::generators {

    template <typename T> class CompositeGenerator;
    template <typename T, typename U> class MappedGenerator;

    /**
     * @brief Base interface for generators.
     *
     * IGenerator is not part of the public API. You should not implement it.
     *
     * @tparam T The type to generate values for
     */
    template <typename T> struct IGenerator {
        IGenerator() {}
        virtual ~IGenerator() = default;

        /// @cond INTERNAL
        // Every generator produces its value by driving the engine's typed
        // draw primitives (hegel::internal::draw_*) against the test case.
        virtual T do_draw(const TestCase& tc) const = 0;

        // The span label that identifies this generator to the engine. A
        // generator built from others combines its own label with theirs.
        virtual uint64_t label() const {
            return internal::label(internal::LabelName::Generator);
        }

        // Draws a value that satisfies `predicate`, as filter() does: up to
        // internal::filter_attempts attempts, each in a span of its own that
        // is discarded when the predicate rejects its value, then the test
        // case is rejected. A generator whose draws change state outside
        // the choice sequence (a consuming pool draw) overrides this to undo
        // what its rejected attempts did.
        virtual T
        do_draw_filtered(const TestCase& tc,
                         const std::function<bool(const T&)>& predicate) const {
            uint64_t attempt =
                internal::label(internal::LabelName::FilterAttempt);
            for (int i = 0; i < internal::filter_attempts; ++i) {
                internal::start_span(tc, attempt);
                T value = do_draw(tc);
                if (predicate(value)) {
                    internal::stop_span(tc);
                    return value;
                }
                internal::stop_span(tc, true);
            }
            tc.reject();
        }
        /// @endcond
    };

    /**
     * @brief The base class of all generators.
     *
     * @code{.cpp}
     * namespace gs = hegel::generators;
     *
     * HEGEL_TEST(generator_tour)(hegel::TestCase& tc) {
     *     // Create a generator and draw a value
     *     auto int_gen = gs::integers<int>({.min_value = 0, .max_value = 100});
     *     auto value = tc.draw("value", int_gen);
     *
     *     // Transform with map
     *     auto squared = int_gen.map([](int x) { return x * x; });
     *
     *     // Filter values
     *     auto even = int_gen.filter([](int x) { return x % 2 == 0; });
     *
     *     // Dependent generation with flat_map
     *     auto sized = gs::integers<size_t>({.min_value = 1, .max_value = 10})
     *         .flat_map([](size_t len) {
     *             return gs::text({.min_size = len, .max_size = len});
     *         });
     * }
     * @endcode
     *
     * @tparam T The type to generate values for
     */
    template <typename T> class Generator {
      public:
        /// The type of value this generator produces.
        using value_type = T;

        /// @cond INTERNAL
        Generator(IGenerator<T>* p) : inner_(p) {}
        Generator(std::shared_ptr<IGenerator<T>> p) : inner_(std::move(p)) {}

        T do_draw(const TestCase& tc) const { return inner_->do_draw(tc); }

        uint64_t label() const { return inner_->label(); }
        /// @endcond

        /**
         * @brief Transform generated values with a function.
         *
         * Given a Generator\<T\> and a function T -> S, creates a
         * Generator\<S\>.
         *
         * This works by generating values from the Generator&lt;T&gt; and
         * applying a transformation to each value.
         *
         * Here's an example of how you'd use this:
         *
         * @code{.cpp}
         * auto halved = integers<int>().map(
         *     [](int x) { return double(x) / 2.0; }
         * );
         * // halved is Generator<double>
         * @endcode
         *
         * @tparam F Function type (T -> S)
         * @param f Transformation function with signature S f(T)
         * @return Generator\<S\> producing transformed values
         * @see flat_map()
         */
        template <typename F>
        Generator<std::invoke_result_t<F, T>> map(F&& f) const {
            using ResultType = std::invoke_result_t<F, T>;
            return Generator<ResultType>(
                new MappedGenerator<T, ResultType>(inner_, std::forward<F>(f)));
        }

        /**
         * @brief Chain generators for dependent generation.
         *
         * Given a Generator\<T\> and a function T -> Generator\<S\>, creates a
         * Generator\<S\>. Useful when generation parameters depend on
         * previously generated values.
         *
         * @code{.cpp}
         * auto sized_string =
         *     integers<size_t>({.min_value = 1, .max_value = 10})
         *         .flat_map([](size_t len) {
         *             return text({.min_size = len, .max_size = len});
         *         });
         * // sized_string is Generator<std::string>
         * @endcode
         *
         * @tparam F Function type (T -> Generator\<S\>)
         * @param f Function that takes a T and returns a Generator\<S\>
         * @return Generator\<S\> producing values from the chained generator
         * @see map(), text()
         */
        template <typename F> std::invoke_result_t<F, T> flat_map(F&& f) const {
            // F: T -> Generator<ResultType>; flat_map returns the same
            // Generator<ResultType>.
            using ResultType = typename std::invoke_result_t<F, T>::value_type;
            auto inner = inner_;
            uint64_t label = internal::combine_labels(
                {internal::label(internal::LabelName::FlatMap),
                 inner->label()});
            return Generator<ResultType>(new CompositeGenerator<ResultType>(
                [inner, label,
                 f = std::forward<F>(f)](const TestCase& tc) -> ResultType {
                    internal::start_span(tc, label);
                    ResultType result = f(inner->do_draw(tc)).do_draw(tc);
                    internal::stop_span(tc);
                    return result;
                },
                label));
        }

        /**
         * @brief Filter generated values by a predicate.
         *
         * Creates a Generator that only produces values satisfying the
         * predicate. The new Generator has the same type as this Generator.
         *
         * Each draw tries a few values. If none satisfies the predicate, the
         * test case is rejected, as by TestCase::reject(). So a predicate that
         * few values satisfy makes most test cases fail to generate. For
         * example, if you want sorted lists of length N, you should
         * generate lists of length N and sort them, not generate random lists
         * and filter by a predicate of 'length == N && is_sorted'.
         *
         * @code{.cpp}
         * auto even = integers<int>({.min_value = 0, .max_value = 100})
         *     .filter([](int x) { return x % 2 == 0; });
         * // even is Generator<int>
         * @endcode
         *
         * @param pred Predicate that values must satisfy
         * @return Generator<T> producing only values satisfying pred
         */
        Generator<T> filter(std::function<bool(const T&)> pred) const {
            auto inner = inner_;
            uint64_t label = internal::combine_labels(
                {internal::label(internal::LabelName::Filter), inner->label()});
            auto draw = [inner, pred](const TestCase& tc) -> T {
                return inner->do_draw_filtered(tc, pred);
            };
            return Generator<T>(new CompositeGenerator<T>(draw, label));
        }

      private:
        std::shared_ptr<IGenerator<T>> inner_;
    };

    /// @cond INTERNAL
    // Generator that produces values by invoking a user-provided function.
    // Users never construct or reference this type directly; it's produced
    // internally by compose() and by map()/flat_map()/filter().
    template <typename T> class CompositeGenerator : public IGenerator<T> {
      public:
        explicit CompositeGenerator(std::function<T(const TestCase&)> fn)
            : gen_fn_(std::move(fn)),
              label_(internal::label(internal::LabelName::Composite)) {}

        CompositeGenerator(std::function<T(const TestCase&)> fn, uint64_t label)
            : gen_fn_(std::move(fn)), label_(label) {}

        T do_draw(const TestCase& tc) const override { return gen_fn_(tc); }

        uint64_t label() const override { return label_; }

      private:
        std::function<T(const TestCase&)> gen_fn_;
        uint64_t label_;
    };
    /// @endcond

    /// @cond INTERNAL
    // Generator that applies a client-side transformation to values drawn
    // from a source generator. Produced internally by Generator<T>::map().
    template <typename T, typename U>
    class MappedGenerator : public IGenerator<U> {
      public:
        MappedGenerator(std::shared_ptr<IGenerator<T>> source,
                        std::function<U(T)> f)
            : source_(std::move(source)), f_(std::move(f)),
              label_(internal::combine_labels(
                  {internal::label(internal::LabelName::Map),
                   source_->label()})) {}

        U do_draw(const TestCase& tc) const override {
            internal::start_span(tc, label_);
            U result = f_(source_->do_draw(tc));
            internal::stop_span(tc);
            return result;
        }

        uint64_t label() const override { return label_; }

      private:
        std::shared_ptr<IGenerator<T>> source_;
        std::function<U(T)> f_;
        uint64_t label_;
    };
    /// @endcond

    /**
     * @brief Build a generator from imperative code that draws from a
     *        TestCase.
     *
     * The element type is deduced from @p fn's return type. To force a specific
     * type, give the lambda an explicit trailing return type.
     *
     * The draws @p fn makes form one span, so the engine shrinks them as a
     * unit, as it does for map(), flat_map() and filter().
     *
     * @code{.cpp}
     * auto generate_person() {
     *     return gs::compose([](const hegel::TestCase& tc) {
     *         int age = tc.draw(gs::integers<int>());
     *         std::string name = tc.draw(gs::text());
     *         return Person{age, name};
     *     });
     * }
     * @endcode
     *
     * @tparam F A callable taking `const TestCase&`
     * @param fn Function that draws from the TestCase and returns a value
     * @return A Generator whose element type is the return type of @p fn
     */
    template <typename F> auto compose(F&& fn) {
        using T = std::invoke_result_t<F, const TestCase&>;
        uint64_t label = internal::label(internal::LabelName::Composite);
        return Generator<T>(new CompositeGenerator<T>(
            [label, fn = std::forward<F>(fn)](const TestCase& tc) mutable -> T {
                internal::start_span(tc, label);
                T result = fn(tc);
                internal::stop_span(tc);
                return result;
            },
            label));
    }

} // namespace hegel::generators

namespace hegel::internal {

    // Prints `prefix`, `value` and `suffix` as one line of `tc`'s output,
    // when `tc` records output.
    template <typename T>
    void print_line(const TestCase& tc, const std::string& prefix,
                    const T& value, std::string_view suffix = "") {
        if (!records_output(tc)) {
            return;
        }
        Printer printer = test_case_printer(tc);
        Speculation speculation(printer);
        printer.text(prefix);
        print_value(printer, value);
        printer.text(suffix);
        printer.hard_break();
        speculation.commit();
    }

} // namespace hegel::internal

namespace hegel {

    template <typename T>
    T TestCase::draw(const generators::Generator<T>& gen) const {
        return draw(std::string_view(), gen);
    }

    template <typename T>
    T TestCase::draw(std::string_view name, const generators::Generator<T>& gen,
                     bool repeatable) const {
        if (internal::is_explicit(*this)) {
            return std::any_cast<T>(internal::take_explicit_value(
                *this, name, repeatable, typeid(T)));
        }
        internal::NestedDraw nested(*this);
        T value = gen.do_draw(*this);
        if (nested.outermost() && internal::records_output(*this)) {
            internal::print_line(
                *this,
                "auto " + internal::draw_display_name(*this, name, repeatable) +
                    " = ",
                value, ";");
        }
        return value;
    }

} // namespace hegel
