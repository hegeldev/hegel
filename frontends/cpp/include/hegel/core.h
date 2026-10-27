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
#include <utility>
#include <vector>

#include "config.h"
#include "internal.h"
#include "printer.h"
#include "repr.h"
#include "test_case.h"

/**
 * @brief Hegel generators.
 */
namespace hegel::generators {

    template <typename T> class AnyGenerator;
    template <typename T> class Generator;
    template <typename U, bool SourcePrintable> class Mapped;

    /**
     * @brief The interface of a generator that draws without printing.
     *
     * Implement it to write a generator of your own. A generator that only
     * implements IGenerator is not printable: make_generator() wraps it in
     * an AnyGenerator, which TestCase::draw_silent() accepts and
     * TestCase::draw() does not. To make it printable, implement
     * IPrintableGenerator instead, or call AnyGenerator::print_with() or
     * AnyGenerator::print_as_repr() on the handle.
     *
     * do_draw() makes the value from draws on the test case: through other
     * generators, with TestCase::draw_silent().
     *
     * Every draw from a generator runs in a span (a group of draws that the
     * engine shrinks as a unit) labelled with its label(), opened by the
     * TestCase draw it is made through. A generator of your own is labelled
     * with the name of its type unless it overrides label().
     *
     * @tparam T The type of value the generator makes
     */
    template <typename T> class IGenerator {
      public:
        /// The type of value this generator makes.
        using value_type = T;

        IGenerator() = default;
        virtual ~IGenerator() = default;

        /**
         * @brief Draw a value.
         * @param tc The test case to draw from.
         * @return The value.
         */
        virtual T do_draw(const TestCase& tc) const = 0;

        /**
         * @brief The label that identifies this generator to the engine.
         *
         * A generator built from other generators combines its own label
         * with theirs (combine_labels()).
         *
         * @return The label.
         */
        virtual uint64_t label() const {
            return internal::type_label(typeid(*this));
        }

        /**
         * @brief Draw a value that @p predicate accepts, for filter().
         *
         * Draws at most three values, each in a span that is discarded when
         * @p predicate rejects its value, then rejects the test case. A
         * generator whose draws change state outside the choice sequence
         * overrides this to undo what its rejected attempts did.
         *
         * @param tc The test case to draw from.
         * @param predicate The condition the value must satisfy.
         * @return The value.
         */
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
    };

    /**
     * @brief The interface of a printable generator: one that can print what
     *        it draws, as it draws it.
     *
     * A printable generator writes the C++ representation of its value to
     * a Printer while it draws. TestCase::draw() calls do_draw_and_print()
     * with the printer of the failure report, so the report says what was
     * drawn. A container prints each element as the element generator
     * prints it, so printing that one generator chooses shows inside every
     * value built from it.
     *
     * Implement do_draw_and_print() only: do_draw() calls it with
     * Printer::noop(), so the silent draw and the printed draw make the same
     * choices. Draw other generators with TestCase::draw_and_print() (a
     * printable generator) or TestCase::draw_silent().
     *
     * @code{.cpp}
     * struct Point { int x, y; };
     *
     * class PointGenerator : public gs::IPrintableGenerator<Point> {
     *   public:
     *     Point do_draw_and_print(const hegel::TestCase& tc,
     *                             hegel::Printer& p) const override {
     *         p.begin_group(6, "Point(");
     *         int x = tc.draw_and_print(coordinate_, p);
     *         p.separator();
     *         int y = tc.draw_and_print(coordinate_, p);
     *         p.end_group(")");
     *         return {x, y};
     *     }
     *
     *   private:
     *     gs::Generator<int> coordinate_ =
     *         gs::integers<int>({.min_value = 0, .max_value = 9});
     * };
     *
     * auto points = gs::make_generator<PointGenerator>();
     * // A failure report prints `auto p = Point(0, 0);`
     * @endcode
     *
     * @tparam T The type of value the generator makes
     */
    template <typename T> class IPrintableGenerator : public IGenerator<T> {
      public:
        /**
         * @brief Draw a value and print it to @p printer.
         *
         * Must make the same draws as do_draw().
         *
         * @param tc The test case to draw from.
         * @param printer The printer to write the value's representation to.
         * @return The value.
         */
        virtual T do_draw_and_print(const TestCase& tc,
                                    Printer& printer) const = 0;

        /**
         * @brief Draw a value without printing it.
         *
         * Calls do_draw_and_print() with Printer::noop().
         *
         * @param tc The test case to draw from.
         * @return The value.
         */
        T do_draw(const TestCase& tc) const override {
            Printer silent = Printer::noop();
            return do_draw_and_print(tc, silent);
        }

        /**
         * @brief Draw a value that @p predicate accepts and print it, for
         *        filter().
         *
         * Makes the same attempts as IGenerator::do_draw_filtered(), each
         * in a speculative region of @p printer that is kept only for the
         * accepted value.
         *
         * @param tc The test case to draw from.
         * @param predicate The condition the value must satisfy.
         * @param printer The printer to write the value's representation to.
         * @return The value.
         */
        virtual T do_draw_filtered_and_print(
            const TestCase& tc, const std::function<bool(const T&)>& predicate,
            Printer& printer) const {
            uint64_t attempt =
                internal::label(internal::LabelName::FilterAttempt);
            for (int i = 0; i < internal::filter_attempts; ++i) {
                internal::start_span(tc, attempt);
                Speculation speculation = printer.speculate();
                T value = do_draw_and_print(tc, printer);
                if (predicate(value)) {
                    speculation.commit();
                    internal::stop_span(tc);
                    return value;
                }
                speculation.abort();
                internal::stop_span(tc, true);
            }
            tc.reject();
        }
    };

    /// @cond INTERNAL
    namespace detail {

        template <typename T>
        class ValuePrinted : public IPrintableGenerator<T> {
          public:
            T do_draw(const TestCase& tc) const override = 0;

            T do_draw_and_print(const TestCase& tc,
                                Printer& printer) const override {
                T value = this->do_draw(tc);
                printer.repr(value);
                return value;
            }

            T do_draw_filtered_and_print(
                const TestCase& tc,
                const std::function<bool(const T&)>& predicate,
                Printer& printer) const override {
                T value = this->do_draw_filtered(tc, predicate);
                printer.repr(value);
                return value;
            }
        };

        template <typename T, bool Printable>
        using value_base_t =
            std::conditional_t<Printable, ValuePrinted<T>, IGenerator<T>>;

        template <typename T, bool Printable> class Structural;

        template <typename T>
        class Structural<T, true> : public IPrintableGenerator<T> {
          public:
            T do_draw_and_print(const TestCase& tc,
                                Printer& printer) const override {
                return draw_printing(tc, printer);
            }

          protected:
            virtual T draw_printing(const TestCase& tc,
                                    Printer& printer) const = 0;
        };

        template <typename T>
        class Structural<T, false> : public IGenerator<T> {
          public:
            T do_draw(const TestCase& tc) const override {
                Printer silent = Printer::noop();
                return draw_printing(tc, silent);
            }

          protected:
            virtual T draw_printing(const TestCase& tc,
                                    Printer& printer) const = 0;
        };

        template <typename H>
        inline constexpr bool is_printable_handle_v =
            std::is_base_of_v<Generator<typename H::value_type>, H>;

        template <bool Printable, typename T>
        using handle_t =
            std::conditional_t<Printable, Generator<T>, AnyGenerator<T>>;

        template <typename H>
        typename H::value_type draw_component(const TestCase& tc, const H& gen,
                                              Printer& printer) {
            if constexpr (is_printable_handle_v<H>) {
                return tc.draw_and_print(gen, printer);
            } else {
                return tc.draw_silent(gen);
            }
        }

        template <typename T>
        std::string open_text(const Printer& printer, std::string_view open) {
            if (!printer.should_print()) {
                return std::string();
            }
            return internal::cpp_type_name<T>() + std::string(open);
        }

    } // namespace detail
    /// @endcond

    /**
     * @brief A generator: a handle to an IGenerator.
     *
     * Copies share the generator. An AnyGenerator may not be printable:
     * draw it with TestCase::draw_silent(), or make it printable with
     * print_with() or print_as_repr(). Generator, the printable handle,
     * derives from it, so every generator is an AnyGenerator.
     *
     * A function that takes or returns a generator that may not be
     * printable spells its type AnyGenerator<T>. Converting a Generator<T>
     * to AnyGenerator<T> drops its printing.
     *
     * @tparam T The type to generate values for
     */
    template <typename T> class AnyGenerator {
      public:
        /// The type of value this generator produces.
        using value_type = T;

        /// @cond INTERNAL
        AnyGenerator(std::shared_ptr<IGenerator<T>> impl)
            : inner_(std::move(impl)) {}
        AnyGenerator(IGenerator<T>* impl) : inner_(impl) {}

        T do_draw(const TestCase& tc) const { return inner_->do_draw(tc); }

        T
        do_draw_filtered(const TestCase& tc,
                         const std::function<bool(const T&)>& predicate) const {
            return inner_->do_draw_filtered(tc, predicate);
        }

        uint64_t label() const { return inner_->label(); }
        /// @endcond

        /**
         * @brief Transform generated values with a function.
         *
         * Given a generator of T and a function T -> U, makes a generator of
         * U. The result prints the value it made, by its C++ representation,
         * so it is a printable Generator\<U\> when hegel::is_representable_v
         * is true for U.
         *
         * @code{.cpp}
         * auto halved = integers<int>().map(
         *     [](int x) { return double(x) / 2.0; }
         * );
         * @endcode
         *
         * @tparam F Function type (T -> U)
         * @param f Transformation function with signature U f(T)
         * @return A Mapped handle, which derives from Generator\<U\> or
         *   AnyGenerator\<U\>
         * @see Mapped::print_as_call(), flat_map()
         */
        template <typename F>
        Mapped<std::invoke_result_t<F, T>, false> map(F&& f) const;

        /**
         * @brief Chain generators for dependent generation.
         *
         * Given a function T -> generator of U, makes a generator of U.
         * Useful when generation parameters depend on previously generated
         * values. The value drawn from this generator is not printed. The
         * result prints what the generator that @p f returns prints, so it
         * is a Generator\<U\> when @p f returns a Generator\<U\>, and an
         * AnyGenerator\<U\> otherwise.
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
         * @tparam F Function type (T -> generator of U)
         * @param f Function that takes a T and returns a generator
         * @return A generator of the values drawn from the generators @p f
         *   returns
         * @see map(), text()
         */
        template <typename F> auto flat_map(F&& f) const;

        /**
         * @brief Filter generated values by a predicate.
         *
         * Each draw tries a few values. If none satisfies the predicate, the
         * test case is rejected, as by TestCase::reject(). So a predicate that
         * few values satisfy makes most test cases fail to generate. For
         * example, if you want sorted lists of length N, you should
         * generate lists of length N and sort them, not generate random lists
         * and filter by a predicate of 'length == N && is_sorted'.
         *
         * The result prints the accepted value as this generator prints it.
         *
         * @code{.cpp}
         * auto even = integers<int>({.min_value = 0, .max_value = 100})
         *     .filter([](int x) { return x % 2 == 0; });
         * // even is Generator<int>
         * @endcode
         *
         * @param pred Predicate that values must satisfy
         * @return A generator producing only values satisfying pred
         */
        AnyGenerator<T> filter(std::function<bool(const T&)> pred) const;

        /**
         * @brief Make a printable generator that prints its values with
         *        @p print.
         *
         * Draws as this generator does, then calls @p print with the value
         * and the printer. Use it for a type with no C++ representation, to
         * print a value another way, or to hide a secret.
         *
         * @code{.cpp}
         * auto ids = integers<int>({.min_value = 0}).print_with(
         *     [](int id, hegel::Printer& p) {
         *         p.text("UserId(" + std::to_string(id) + ")");
         *     });
         * @endcode
         *
         * @tparam F Callable taking `const T&` and `hegel::Printer&`
         * @param print Writes the representation of a value.
         * @return A Generator\<T\>
         */
        template <typename F> Generator<T> print_with(F&& print) const;

        /**
         * @brief Make a printable generator that prints its values by their
         *        C++ representation (Printer::repr()).
         *
         * Requires hegel::is_representable_v for T.
         *
         * @return A Generator\<T\>
         */
        Generator<T> print_as_repr() const;

      protected:
        /// @cond INTERNAL
        std::shared_ptr<IGenerator<T>> inner_;
        /// @endcond
    };

    /**
     * @brief A printable generator: TestCase::draw() accepts it.
     *
     * Every built-in generator returns one when it can print its values.
     * Converting a Generator\<T\> to AnyGenerator\<T\> drops its printing,
     * so a helper function that returns a generator should return
     * Generator\<T\>:
     *
     * @code{.cpp}
     * namespace gs = hegel::generators;
     *
     * gs::Generator<std::vector<int>> small_lists() {
     *     return gs::vectors(gs::integers<int>({.min_value = 0}),
     *                        {.max_size = 5});
     * }
     *
     * HEGEL_TEST(generator_tour)(hegel::TestCase& tc) {
     *     auto xs = tc.draw("xs", small_lists());
     *     auto even = gs::integers<int>().filter(
     *         [](int x) { return x % 2 == 0; });
     *     auto n = tc.draw("n", even);
     * }
     * @endcode
     *
     * @tparam T The type to generate values for
     */
    template <typename T> class Generator : public AnyGenerator<T> {
      public:
        /// @cond INTERNAL
        Generator(std::shared_ptr<IPrintableGenerator<T>> impl)
            : AnyGenerator<T>(impl), printable_(std::move(impl)) {}
        Generator(IPrintableGenerator<T>* impl)
            : Generator(std::shared_ptr<IPrintableGenerator<T>>(impl)) {}

        T do_draw_and_print(const TestCase& tc, Printer& printer) const {
            return printable_->do_draw_and_print(tc, printer);
        }

        T do_draw_filtered_and_print(
            const TestCase& tc, const std::function<bool(const T&)>& predicate,
            Printer& printer) const {
            return printable_->do_draw_filtered_and_print(tc, predicate,
                                                          printer);
        }
        /// @endcond

        /**
         * @brief Transform generated values with a function.
         *
         * As AnyGenerator::map(). The result also has
         * Mapped::print_as_call(), which prints the value drawn from this
         * generator.
         *
         * @tparam F Function type (T -> U)
         * @param f Transformation function with signature U f(T)
         * @return A Mapped handle
         */
        template <typename F>
        Mapped<std::invoke_result_t<F, T>, true> map(F&& f) const;

        /**
         * @brief Filter generated values by a predicate.
         *
         * As AnyGenerator::filter(). The result is printable.
         *
         * @param pred Predicate that values must satisfy
         * @return A Generator\<T\> producing only values satisfying pred
         */
        Generator<T> filter(std::function<bool(const T&)> pred) const;

      private:
        std::shared_ptr<IPrintableGenerator<T>> printable_;
    };

    /// @cond INTERNAL
    namespace detail {

        template <typename T, typename U, bool Printable>
        class MapImpl : public value_base_t<U, Printable> {
          public:
            MapImpl(AnyGenerator<T> source, std::function<U(T)> f)
                : source_(std::move(source)), f_(std::move(f)),
                  label_(internal::combine_labels(
                      {internal::label(internal::LabelName::Map),
                       source_.label()})) {}

            U do_draw(const TestCase& tc) const override {
                return f_(tc.draw_silent(source_));
            }

            uint64_t label() const override { return label_; }

          private:
            AnyGenerator<T> source_;
            std::function<U(T)> f_;
            uint64_t label_;
        };

        template <typename T, typename U>
        class PrintedAsCallImpl : public IPrintableGenerator<U> {
          public:
            PrintedAsCallImpl(Generator<T> source, std::function<U(T)> f,
                              std::string open, uint64_t label)
                : source_(std::move(source)), f_(std::move(f)),
                  open_(std::move(open)), label_(label) {}

            U do_draw_and_print(const TestCase& tc,
                                Printer& printer) const override {
                printer.begin_group(open_.size(), open_);
                T input = tc.draw_and_print(source_, printer);
                printer.end_group(")");
                return f_(std::move(input));
            }

            uint64_t label() const override { return label_; }

          private:
            Generator<T> source_;
            std::function<U(T)> f_;
            std::string open_;
            uint64_t label_;
        };

        template <typename T> class SilentFilterImpl : public IGenerator<T> {
          public:
            using Predicate = std::function<bool(const T&)>;

            SilentFilterImpl(AnyGenerator<T> source, Predicate predicate)
                : source_(std::move(source)), predicate_(std::move(predicate)),
                  label_(internal::combine_labels(
                      {internal::label(internal::LabelName::Filter),
                       source_.label()})) {}

            T do_draw(const TestCase& tc) const override {
                return source_.do_draw_filtered(tc, predicate_);
            }

            T do_draw_filtered(const TestCase& tc,
                               const Predicate& predicate) const override {
                return source_.do_draw_filtered(tc, both(predicate));
            }

            uint64_t label() const override { return label_; }

          protected:
            Predicate both(const Predicate& predicate) const {
                return [first = predicate_, predicate](const T& value) {
                    return first(value) && predicate(value);
                };
            }

            AnyGenerator<T> source_;
            Predicate predicate_;
            uint64_t label_;
        };

        template <typename T>
        class PrintedFilterImpl : public IPrintableGenerator<T> {
          public:
            using Predicate = std::function<bool(const T&)>;

            PrintedFilterImpl(Generator<T> source, Predicate predicate)
                : silent_(source, predicate), source_(std::move(source)),
                  predicate_(std::move(predicate)) {}

            T do_draw(const TestCase& tc) const override {
                return silent_.do_draw(tc);
            }

            T do_draw_filtered(const TestCase& tc,
                               const Predicate& predicate) const override {
                return silent_.do_draw_filtered(tc, predicate);
            }

            T do_draw_and_print(const TestCase& tc,
                                Printer& printer) const override {
                return source_.do_draw_filtered_and_print(tc, predicate_,
                                                          printer);
            }

            T do_draw_filtered_and_print(const TestCase& tc,
                                         const Predicate& predicate,
                                         Printer& printer) const override {
                return source_.do_draw_filtered_and_print(
                    tc,
                    [first = predicate_, predicate](const T& value) {
                        return first(value) && predicate(value);
                    },
                    printer);
            }

            uint64_t label() const override { return silent_.label(); }

          private:
            SilentFilterImpl<T> silent_;
            Generator<T> source_;
            Predicate predicate_;
        };

        template <typename T>
        class PrintedWithImpl : public IPrintableGenerator<T> {
          public:
            using Print = std::function<void(const T&, Printer&)>;

            PrintedWithImpl(AnyGenerator<T> source, Print print)
                : source_(std::move(source)), print_(std::move(print)) {}

            T do_draw(const TestCase& tc) const override {
                return source_.do_draw(tc);
            }

            T do_draw_filtered(
                const TestCase& tc,
                const std::function<bool(const T&)>& predicate) const override {
                return source_.do_draw_filtered(tc, predicate);
            }

            T do_draw_and_print(const TestCase& tc,
                                Printer& printer) const override {
                T value = source_.do_draw(tc);
                print(value, printer);
                return value;
            }

            T do_draw_filtered_and_print(
                const TestCase& tc,
                const std::function<bool(const T&)>& predicate,
                Printer& printer) const override {
                T value = source_.do_draw_filtered(tc, predicate);
                print(value, printer);
                return value;
            }

            uint64_t label() const override { return source_.label(); }

          private:
            void print(const T& value, Printer& printer) const {
                if (printer.should_print()) {
                    print_(value, printer);
                }
            }

            AnyGenerator<T> source_;
            Print print_;
        };

        template <typename T> class ReprPrintedImpl : public ValuePrinted<T> {
          public:
            explicit ReprPrintedImpl(AnyGenerator<T> source)
                : source_(std::move(source)) {}

            T do_draw(const TestCase& tc) const override {
                return source_.do_draw(tc);
            }

            T do_draw_filtered(
                const TestCase& tc,
                const std::function<bool(const T&)>& predicate) const override {
                return source_.do_draw_filtered(tc, predicate);
            }

            uint64_t label() const override { return source_.label(); }

          private:
            AnyGenerator<T> source_;
        };

        template <typename T, typename U, bool Printable>
        class FlatMapImpl : public Structural<U, Printable> {
          public:
            using Next = std::function<handle_t<Printable, U>(T)>;

            FlatMapImpl(AnyGenerator<T> source, Next f)
                : source_(std::move(source)), f_(std::move(f)),
                  label_(internal::combine_labels(
                      {internal::label(internal::LabelName::FlatMap),
                       source_.label()})) {}

            uint64_t label() const override { return label_; }

          protected:
            U draw_printing(const TestCase& tc,
                            Printer& printer) const override {
                handle_t<Printable, U> next = f_(tc.draw_silent(source_));
                return draw_component(tc, next, printer);
            }

          private:
            AnyGenerator<T> source_;
            Next f_;
            uint64_t label_;
        };

        template <typename T, bool Printable>
        class CompositeImpl : public value_base_t<T, Printable> {
          public:
            CompositeImpl(std::function<T(const TestCase&)> fn, uint64_t label)
                : fn_(std::move(fn)), label_(label) {}

            T do_draw(const TestCase& tc) const override { return fn_(tc); }

            uint64_t label() const override { return label_; }

          private:
            std::function<T(const TestCase&)> fn_;
            uint64_t label_;
        };

    } // namespace detail
    /// @endcond

    /**
     * @brief The generator that map() returns.
     *
     * It derives from Generator\<U\> when hegel::is_representable_v is
     * true for U, and from AnyGenerator\<U\> otherwise, so it can be used
     * wherever those can.
     *
     * @tparam U The type of the values it makes
     * @tparam SourcePrintable Whether the mapped generator is printable
     */
    template <typename U, bool SourcePrintable>
    class Mapped : public detail::handle_t<is_representable_v<U>, U> {
      public:
        /// @cond INTERNAL
        using Base = detail::handle_t<is_representable_v<U>, U>;
        using AsCall = std::function<Generator<U>(std::string, uint64_t)>;

        Mapped(Base base, AsCall as_call)
            : Base(std::move(base)), as_call_(std::move(as_call)) {}
        /// @endcond

        /**
         * @brief Make a generator that prints a call of @p function with
         *        the drawn input.
         *
         * A map() prints the value it made. This prints the value drawn
         * from the mapped generator instead, as that generator prints it,
         * in a call of @p function, so a type with no C++ representation
         * can be printed. The mapped generator must be printable.
         *
         * @code{.cpp}
         * auto keys = gs::integers<uint64_t>()
         *     .map(KeyData::from_ffi)
         *     .print_as_call("KeyData::from_ffi");
         * // A failure report prints `auto key = KeyData::from_ffi(3);`
         * @endcode
         *
         * @param function The name of the mapping function.
         * @return A Generator\<U\>
         * @throws std::invalid_argument if @p function is empty.
         */
        Generator<U> print_as_call(std::string_view function) const {
            static_assert(
                SourcePrintable,
                "print_as_call() needs a printable mapped generator (a "
                "hegel::generators::Generator<T>), but this map() is of an "
                "AnyGenerator<T>. Make the mapped generator printable, or use "
                ".print_with(...) on the result of map().");
            if (function.empty()) {
                throw std::invalid_argument(
                    "print_as_call requires a non-empty function name");
            }
            return as_call_(std::string(function) + "(", this->label());
        }

      private:
        AsCall as_call_;
    };

    /// @cond INTERNAL
    namespace detail {

        template <typename T, typename U, bool SourcePrintable>
        Mapped<U, SourcePrintable>
        make_mapped(handle_t<SourcePrintable, T> source,
                    std::function<U(T)> f) {
            using Result = Mapped<U, SourcePrintable>;
            typename Result::Base base(
                std::make_shared<MapImpl<T, U, is_representable_v<U>>>(source,
                                                                       f));
            typename Result::AsCall as_call;
            if constexpr (SourcePrintable) {
                as_call = [source, f](std::string open, uint64_t label) {
                    return Generator<U>(
                        std::make_shared<PrintedAsCallImpl<T, U>>(
                            source, f, std::move(open), label));
                };
            }
            return Result(std::move(base), std::move(as_call));
        }

    } // namespace detail
    /// @endcond

    template <typename T>
    template <typename F>
    Mapped<std::invoke_result_t<F, T>, false>
    AnyGenerator<T>::map(F&& f) const {
        using U = std::invoke_result_t<F, T>;
        return detail::make_mapped<T, U, false>(
            *this, std::function<U(T)>(std::forward<F>(f)));
    }

    template <typename T>
    template <typename F>
    Mapped<std::invoke_result_t<F, T>, true> Generator<T>::map(F&& f) const {
        using U = std::invoke_result_t<F, T>;
        return detail::make_mapped<T, U, true>(
            *this, std::function<U(T)>(std::forward<F>(f)));
    }

    template <typename T>
    template <typename F>
    auto AnyGenerator<T>::flat_map(F&& f) const {
        using Result = std::decay_t<std::invoke_result_t<F, T>>;
        using U = typename Result::value_type;
        constexpr bool printable = detail::is_printable_handle_v<Result>;
        using Next = typename detail::FlatMapImpl<T, U, printable>::Next;
        return detail::handle_t<printable, U>(
            std::make_shared<detail::FlatMapImpl<T, U, printable>>(
                *this, Next(std::forward<F>(f))));
    }

    template <typename T>
    AnyGenerator<T>
    AnyGenerator<T>::filter(std::function<bool(const T&)> pred) const {
        return AnyGenerator<T>(std::make_shared<detail::SilentFilterImpl<T>>(
            *this, std::move(pred)));
    }

    template <typename T>
    Generator<T>
    Generator<T>::filter(std::function<bool(const T&)> pred) const {
        return Generator<T>(std::make_shared<detail::PrintedFilterImpl<T>>(
            *this, std::move(pred)));
    }

    template <typename T>
    template <typename F>
    Generator<T> AnyGenerator<T>::print_with(F&& print) const {
        return Generator<T>(std::make_shared<detail::PrintedWithImpl<T>>(
            *this, typename detail::PrintedWithImpl<T>::Print(
                       std::forward<F>(print))));
    }

    template <typename T> Generator<T> AnyGenerator<T>::print_as_repr() const {
        static_assert(is_representable_v<T>,
                      "print_as_repr() needs a type with a C++ "
                      "representation (hegel::is_representable_v). Use "
                      ".print_with(...) to print it another way.");
        return Generator<T>(
            std::make_shared<detail::ReprPrintedImpl<T>>(*this));
    }

    /**
     * @brief Wrap a generator implementation in a handle.
     *
     * Returns a Generator\<T\> when @p Impl implements
     * IPrintableGenerator\<T\>, else an AnyGenerator\<T\>.
     *
     * @code{.cpp}
     * auto points = gs::make_generator<PointGenerator>();
     * @endcode
     *
     * @tparam Impl The implementation, an IGenerator\<T\>
     * @tparam Args The types of its constructor arguments
     * @param args The arguments to construct @p Impl with.
     * @return A handle to a new @p Impl
     */
    template <typename Impl, typename... Args>
    auto make_generator(Args&&... args) {
        using T = typename Impl::value_type;
        std::shared_ptr<Impl> impl =
            std::make_shared<Impl>(std::forward<Args>(args)...);
        if constexpr (std::is_base_of_v<IPrintableGenerator<T>, Impl>) {
            return Generator<T>(std::move(impl));
        } else {
            return AnyGenerator<T>(std::move(impl));
        }
    }

    /**
     * @brief The label of a generator with no component generators.
     *
     * Two generators with the same label are the same kind of generator,
     * for the engine. Derive the label from a name that no other generator
     * uses.
     *
     * @param name The generator's name.
     * @return The label.
     */
    uint64_t label_from_name(std::string_view name);

    /**
     * @brief The label of a generator built from other generators.
     * @param labels Its own label, then the labels of its components.
     * @return The combined label.
     */
    inline uint64_t combine_labels(const std::vector<uint64_t>& labels) {
        return internal::combine_labels(labels);
    }

    /**
     * @brief Build a generator from imperative code that draws from a
     *        TestCase.
     *
     * The element type is deduced from @p fn's return type. To force a specific
     * type, give the lambda an explicit trailing return type.
     *
     * The draws @p fn makes form one span, so the engine shrinks them as a
     * unit, as it does for map(), flat_map() and filter(). They do not
     * print: the result prints the value @p fn returns, by its C++
     * representation, so it is a printable Generator\<T\> when
     * hegel::is_representable_v is true for T, and an AnyGenerator\<T\>
     * otherwise.
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
     * @return A generator whose element type is the return type of @p fn
     */
    template <typename F> auto compose(F&& fn) {
        using T = std::invoke_result_t<F, const TestCase&>;
        constexpr bool printable = is_representable_v<T>;
        return detail::handle_t<printable, T>(
            std::make_shared<detail::CompositeImpl<T, printable>>(
                std::function<T(const TestCase&)>(std::forward<F>(fn)),
                internal::label(internal::LabelName::Composite)));
    }

} // namespace hegel::generators

namespace hegel {

    template <typename T>
    T TestCase::draw(const generators::Generator<T>& gen) const {
        return draw(std::string_view(), gen);
    }

    template <typename T>
    T TestCase::draw(std::string_view name,
                     const generators::Generator<T>& gen) const {
        if (internal::is_explicit(*this)) {
            return std::any_cast<T>(
                internal::take_explicit_value(*this, name, typeid(T)));
        }
        internal::NestedDraw nested(*this);
        if (!nested.outermost() || !internal::records_output(*this)) {
            return draw_silent(gen);
        }
        std::string display = internal::draw_display_name(*this, name);
        Printer printer = internal::test_case_printer(*this);
        Speculation line = printer.speculate();
        printer.text("auto " + display + " = ");
        T value = draw_and_print(gen, printer);
        printer.text(";");
        printer.hard_break();
        line.commit();
        return value;
    }

    /// @cond INTERNAL
    template <typename T>
    T TestCase::draw(const generators::AnyGenerator<T>&) const {
        static_assert(
            internal::always_false_v<T>,
            "tc.draw() needs a printable generator "
            "(hegel::generators::Generator<T>), but this is an "
            "AnyGenerator<T>. If T is your own type, give it a C++ "
            "representation: a reflectable aggregate (C++20), operator<<, or "
            "a print_to(hegel::Printer&) const member. Otherwise use "
            ".print_with(...), .print_as_repr(), or .print_as_call(\"f\") on "
            "a map() of a printable generator, or draw with "
            "tc.draw_silent(...). A helper that returns AnyGenerator<T> drops "
            "printing, so return Generator<T> instead.");
    }

    template <typename T>
    T TestCase::draw(std::string_view,
                     const generators::AnyGenerator<T>&) const {
        static_assert(
            internal::always_false_v<T>,
            "tc.draw() needs a printable generator "
            "(hegel::generators::Generator<T>), but this is an "
            "AnyGenerator<T>. If T is your own type, give it a C++ "
            "representation: a reflectable aggregate (C++20), operator<<, or "
            "a print_to(hegel::Printer&) const member. Otherwise use "
            ".print_with(...), .print_as_repr(), or .print_as_call(\"f\") on "
            "a map() of a printable generator, or draw with "
            "tc.draw_silent(...). A helper that returns AnyGenerator<T> drops "
            "printing, so return Generator<T> instead.");
    }
    /// @endcond

    template <typename T>
    T TestCase::draw_silent(const generators::AnyGenerator<T>& gen) const {
        internal::check_not_explicit(*this, "draw_silent");
        internal::NestedDraw nested(*this);
        internal::start_span(*this, gen.label());
        T value = gen.do_draw(*this);
        internal::stop_span(*this);
        return value;
    }

    template <typename T>
    T TestCase::draw_and_print(const generators::Generator<T>& gen,
                               Printer& printer) const {
        internal::check_not_explicit(*this, "draw_and_print");
        internal::NestedDraw nested(*this);
        internal::start_span(*this, gen.label());
        T value = gen.do_draw_and_print(*this, printer);
        internal::stop_span(*this);
        return value;
    }

} // namespace hegel
