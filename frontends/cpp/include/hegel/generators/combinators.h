#pragma once

#include <cstdint>
#include <functional>
#include <initializer_list>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <tuple>
#include <variant>
#include <vector>

#include "hegel/core.h"

namespace hegel::generators {

    /// @cond INTERNAL
    // Concrete IGenerator for sampled_from(). Draws an index into the
    // captured `elements_` vector and returns that element.
    template <typename T>
    class SampledFromGenerator
        : public detail::value_base_t<T, is_representable_v<T>> {
      public:
        explicit SampledFromGenerator(std::vector<T> elements)
            : elements_(std::move(elements)) {
            if (elements_.empty()) {
                throw std::invalid_argument(
                    "sampled_from requires a non-empty vector");
            }
        }

        T do_draw(const TestCase& tc) const override {
            int64_t index = hegel::internal::draw_integer(
                tc, 0, static_cast<int64_t>(elements_.size() - 1));
            return elements_[static_cast<size_t>(index)];
        }

        uint64_t label() const override {
            return hegel::internal::label(
                hegel::internal::LabelName::SampledFrom);
        }

      private:
        std::vector<T> elements_;
    };

    // Concrete IGenerator for one_of(). Draws a branch index inside a
    // one_of span, then delegates to that branch.
    template <typename T, bool Printable>
    class OneOfGenerator : public detail::Structural<T, Printable> {
      public:
        explicit OneOfGenerator(
            std::vector<detail::handle_t<Printable, T>> gens)
            : gens_(std::move(gens)) {
            if (gens_.empty()) {
                throw std::invalid_argument(
                    "one_of requires a non-empty vector of generators");
            }
            std::vector<uint64_t> branches;
            branches.reserve(gens_.size());
            for (const auto& gen : gens_) {
                branches.push_back(gen.label());
            }
            namespace hi = hegel::internal;
            label_ = hi::combine_labels({hi::label(hi::LabelName::OneOf),
                                         hi::combine_labels(branches)});
        }

        uint64_t label() const override { return label_; }

      protected:
        T draw_printing(const TestCase& tc, Printer& printer) const override {
            namespace hi = hegel::internal;
            int64_t index =
                hi::draw_integer(tc, 0, static_cast<int64_t>(gens_.size() - 1));
            return detail::draw_component(tc, gens_[static_cast<size_t>(index)],
                                          printer);
        }

      private:
        std::vector<detail::handle_t<Printable, T>> gens_;
        uint64_t label_;
    };
    /// @endcond

    /// @name Misc
    /// @{

    /**
     * @brief Sample from a fixed set of values.
     *
     * @code{.cpp}
     * auto color = sampled_from({"red", "green", "blue"});
     * auto digit = sampled_from({1, 2, 3, 4, 5});
     * @endcode
     *
     * The result prints the chosen value by its C++ representation, so it
     * is printable when hegel::is_representable_v is true for T.
     *
     * @tparam T Element type
     * @param elements Vector of values to sample from (must not be empty)
     * @return Generator that picks from elements
     */
    template <typename T> auto sampled_from(const std::vector<T>& elements) {
        return detail::handle_t<is_representable_v<T>, T>(
            std::make_shared<SampledFromGenerator<T>>(elements));
    }

    /**
     * @brief Sample from a fixed set of values (initializer list).
     * @tparam T Element type
     * @param elements Values to sample from (must not be empty)
     * @return Generator that picks from elements
     */
    template <typename T> auto sampled_from(std::initializer_list<T> elements) {
        return sampled_from(std::vector<T>(elements));
    }

    /**
     * @brief Sample from a fixed set of C-string literals.
     * @param elements String literals to sample from (must not be empty)
     * @return Generator of std::string picking from elements
     */
    inline Generator<std::string>
    sampled_from(std::initializer_list<const char*> elements) {
        std::vector<std::string> strings;
        strings.reserve(elements.size());
        for (const char* s : elements) {
            strings.push_back(s);
        }
        return sampled_from(strings);
    }

    /// @}

    /// @name Combinators
    /// @{

    /**
     * @brief Choose from multiple generators of the same type.
     *
     * Randomly selects one generator and uses it to produce a value. The
     * value prints as the chosen generator prints it, so the result is
     * printable when every generator is.
     *
     * @code{.cpp}
     * auto range = one_of({
     *     integers<int>({.min_value = 0, .max_value = 10}),
     *     integers<int>({.min_value = 100, .max_value = 110})
     * });
     * @endcode
     *
     * @tparam T Value type (all generators must produce this type)
     * @param gens Vector of generators to choose from (must not be empty)
     * @return Generator that delegates to a randomly chosen generator
     */
    template <typename T> Generator<T> one_of(std::vector<Generator<T>> gens) {
        return Generator<T>(new OneOfGenerator<T, true>(std::move(gens)));
    }

    /**
     * @brief Choose from generators that are not all printable.
     *
     * A braced list of generators that are not all printable, such as
     * `one_of<int>({printable, silent})`, converts to this overload.
     *
     * @tparam T Value type (all generators must produce this type)
     * @param gens Vector of generators to choose from (must not be empty)
     * @return AnyGenerator that delegates to a randomly chosen generator
     */
    template <typename T>
    AnyGenerator<T> one_of(std::vector<AnyGenerator<T>> gens) {
        return AnyGenerator<T>(new OneOfGenerator<T, false>(std::move(gens)));
    }

    /**
     * @brief Choose from a list of printable generators (initializer list).
     * @tparam T Value type produced by each generator
     * @param gens Generators to choose from (must not be empty)
     * @return Generator that picks from gens and forwards do_draw
     */
    template <typename T>
    Generator<T> one_of(std::initializer_list<Generator<T>> gens) {
        return one_of(std::vector<Generator<T>>(gens));
    }

    /// @cond INTERNAL
    namespace detail {

        template <typename Variant, typename GenTuple, size_t I = 0>
        Variant draw_variant_impl(const GenTuple& gens, size_t idx,
                                  const TestCase& tc, Printer& printer) {
            if constexpr (I + 1 < std::tuple_size_v<GenTuple>) {
                if (idx != I) {
                    return draw_variant_impl<Variant, GenTuple, I + 1>(
                        gens, idx, tc, printer);
                }
            }
            return Variant{std::in_place_index<I>,
                           draw_component(tc, std::get<I>(gens), printer)};
        }

    } // namespace detail

    // Concrete IGenerator for variant(). Draws a branch index inside a
    // one_of span; branches can have heterogeneous types.
    template <bool Printable, typename... Ts>
    class VariantGenerator
        : public detail::Structural<std::variant<Ts...>, Printable> {
      public:
        using Result = std::variant<Ts...>;

        explicit VariantGenerator(detail::handle_t<Printable, Ts>... gens)
            : gens_(std::move(gens)...) {
            namespace hi = hegel::internal;
            std::vector<uint64_t> branches = std::apply(
                [](const auto&... g) {
                    return std::vector<uint64_t>{g.label()...};
                },
                gens_);
            label_ = hi::combine_labels({hi::label(hi::LabelName::OneOf),
                                         hi::combine_labels(branches)});
        }

        uint64_t label() const override { return label_; }

      protected:
        Result draw_printing(const TestCase& tc,
                             Printer& printer) const override {
            namespace hi = hegel::internal;
            constexpr size_t N = sizeof...(Ts);
            int64_t index =
                hi::draw_integer(tc, 0, static_cast<int64_t>(N - 1));
            printer.begin_block(detail::open_text<Result>(printer, "{"));
            if (printer.should_print()) {
                printer.text("std::in_place_index<" + std::to_string(index) +
                             ">");
            }
            printer.separator();
            Result result = detail::draw_variant_impl<Result, decltype(gens_)>(
                gens_, static_cast<size_t>(index), tc, printer);
            printer.end_block("}");
            return result;
        }

      private:
        std::tuple<detail::handle_t<Printable, Ts>...> gens_;
        uint64_t label_;
    };

    // Concrete IGenerator for optional(). A boolean draw inside an
    // optional span gates presence; false shrinks toward nullopt.
    template <typename T, bool Printable>
    class OptionalGenerator
        : public detail::Structural<std::optional<T>, Printable> {
      public:
        explicit OptionalGenerator(detail::handle_t<Printable, T> gen)
            : gen_(std::move(gen)),
              label_(hegel::internal::combine_labels(
                  {hegel::internal::label(hegel::internal::LabelName::Optional),
                   gen_.label()})) {}

        uint64_t label() const override { return label_; }

      protected:
        std::optional<T> draw_printing(const TestCase& tc,
                                       Printer& printer) const override {
            namespace hi = hegel::internal;
            std::optional<T> result;
            if (hi::draw_boolean(tc, 0.5)) {
                printer.begin_block(
                    detail::open_text<std::optional<T>>(printer, "{"));
                result = detail::draw_component(tc, gen_, printer);
                printer.end_block("}");
            } else {
                printer.text(
                    detail::open_text<std::optional<T>>(printer, "{}"));
            }
            return result;
        }

      private:
        detail::handle_t<Printable, T> gen_;
        uint64_t label_;
    };
    /// @endcond

    /**
     * @brief Generate std::variant from heterogeneous generators.
     *
     * Each generator produces one possible variant alternative. The
     * variant prints the alternative as its generator prints it, so it is
     * printable when every generator is.
     *
     * @code{.cpp}
     * auto value = variant(integers<int>(), text(), booleans());
     * // Returns std::variant<int, std::string, bool>
     * @endcode
     *
     * @tparam Ts Variant alternative types
     * @param gens Generators for each alternative
     * @return Generator producing variant
     */
    template <typename... Ts>
    Generator<std::variant<Ts...>> variant(Generator<Ts>... gens) {
        return Generator<std::variant<Ts...>>(
            new VariantGenerator<true, Ts...>(std::move(gens)...));
    }

    /**
     * @brief Generate std::variant from generators that are not all
     *        printable.
     *
     * @tparam Ts Variant alternative types
     * @param gens Generators for each alternative
     * @return AnyGenerator producing variant
     */
    template <typename... Ts>
    AnyGenerator<std::variant<Ts...>> variant(AnyGenerator<Ts>... gens) {
        return AnyGenerator<std::variant<Ts...>>(
            new VariantGenerator<false, Ts...>(std::move(gens)...));
    }

    /**
     * @brief Generate optional values (present or absent).
     *
     * Randomly produces either a value from the given generator or
     * std::nullopt. It is printable when @p gen is.
     *
     * @code{.cpp}
     * auto maybe_int = optional(integers<int>());
     * // Returns std::optional<int>, may be nullopt
     * @endcode
     *
     * @tparam T Value type
     * @param gen Generator for the value when present
     * @return Generator producing optional values
     */
    template <typename T>
    Generator<std::optional<T>> optional(Generator<T> gen) {
        return Generator<std::optional<T>>(
            new OptionalGenerator<T, true>(std::move(gen)));
    }

    /**
     * @brief Generate optional values from a generator that is not
     *        printable.
     *
     * @tparam T Value type
     * @param gen Generator for the value when present
     * @return AnyGenerator producing optional values
     */
    template <typename T>
    AnyGenerator<std::optional<T>> optional(AnyGenerator<T> gen) {
        return AnyGenerator<std::optional<T>>(
            new OptionalGenerator<T, false>(std::move(gen)));
    }

    /// @}

    /// @cond INTERNAL
    // Generator that delegates to an implementation supplied later through a
    // shared slot. Handed out by DeferredGeneratorDefinition::generator().
    template <typename T, bool Printable>
    class DeferredGenerator : public detail::Structural<T, Printable> {
      public:
        using Slot = std::optional<detail::handle_t<Printable, T>>;

        explicit DeferredGenerator(std::shared_ptr<Slot> slot)
            : slot_(std::move(slot)) {}

        uint64_t label() const override {
            return hegel::internal::label(hegel::internal::LabelName::Deferred);
        }

      protected:
        T draw_printing(const TestCase& tc, Printer& printer) const override {
            if (!*slot_) {
                throw std::runtime_error(
                    "deferred generator drawn before set() was called");
            }
            return detail::draw_component(tc, **slot_, printer);
        }

      private:
        std::shared_ptr<Slot> slot_;
    };
    /// @endcond

    /**
     * @brief A forward reference to a generator whose definition is provided
     *        later.
     *
     * Created by deferred() or deferred_silent(). Call generator() to obtain
     * handles that can be embedded in other generators before the
     * implementation is known, then call set() to install it. This enables
     * self-recursive and mutually recursive generators.
     *
     * @tparam T Value type produced by the eventual generator.
     * @tparam Printable Whether the generator is printable: true for
     *   deferred(), false for deferred_silent().
     */
    template <typename T, bool Printable = true>
    class DeferredGeneratorDefinition {
      public:
        /// The handle type of the generator.
        using Handle = detail::handle_t<Printable, T>;

        DeferredGeneratorDefinition()
            : slot_(std::make_shared<
                    typename DeferredGenerator<T, Printable>::Slot>()) {}

        /**
         * @brief Return a handle that delegates to whatever is later passed to
         *        set(). May be called multiple times. Drawing from it before
         * set() is called throws.
         *
         * The handle prints what the installed generator prints.
         *
         * @return A Generator (an AnyGenerator for deferred_silent()) that
         *   delegates to the implementation installed by set().
         */
        Handle generator() const {
            return Handle(
                std::make_shared<DeferredGenerator<T, Printable>>(slot_));
        }

        /**
         * @brief Set the implementation for this deferred generator.
         *
         * All handles previously returned by generator() delegate to @p gen.
         * May be called only once. A second call throws. Drawing from a handle
         * before set() is called throws.
         *
         * @param gen The generator to install as the implementation.
         */
        void set(Handle gen) {
            if (*slot_) {
                throw std::runtime_error(
                    "deferred generator set() called more than once");
            }
            *slot_ = std::move(gen);
        }

      private:
        std::shared_ptr<typename DeferredGenerator<T, Printable>::Slot> slot_;
    };

    /**
     * @brief Create a deferred generator definition for forward references.
     *
     * The generator is printable: set() takes a Generator\<T\>. For a
     * definition that is not printable, use deferred_silent().
     *
     * @tparam T Value type produced by the eventual generator.
     * @return A DeferredGeneratorDefinition whose generator() handles can be
     *   used before set() supplies the implementation.
     *
     * @code{.cpp}
     * struct Tree { int leaf; std::vector<Tree> children; };
     * auto tree = gs::deferred<Tree>();
     * auto leaf = gs::integers<int>().map([](int v) { return Tree{v, {}}; });
     * auto branch = gs::compose([tree](const hegel::TestCase& tc) {
     *     return Tree{0, {tc.draw(tree.generator()),
     * tc.draw(tree.generator())}};
     * });
     * tree.set(gs::one_of<Tree>({leaf, branch}));
     * @endcode
     */
    template <typename T> DeferredGeneratorDefinition<T, true> deferred() {
        return DeferredGeneratorDefinition<T, true>();
    }

    /**
     * @brief Create a deferred generator definition that is not printable.
     *
     * As deferred(), but generator() returns an AnyGenerator\<T\> and set()
     * takes one.
     *
     * @tparam T Value type produced by the eventual generator.
     * @return A DeferredGeneratorDefinition whose generator() handles can be
     *   used before set() supplies the implementation.
     */
    template <typename T>
    DeferredGeneratorDefinition<T, false> deferred_silent() {
        return DeferredGeneratorDefinition<T, false>();
    }

    /**
     * @brief Parameters for recursive() generator.
     */
    struct RecursiveParams {
        /// Branches nest at most this deep. Sub-values at this depth are
        /// always leaves, so 0 generates only leaves.
        uint64_t max_depth = 32;
        /// The most leaves one generated value can have. Each value steers
        /// toward a size drawn from this range. An attempt that draws more
        /// leaves is discarded and tried again, and the test case is
        /// rejected when several attempts in a row do not fit.
        uint64_t max_leaves = 100;
    };

    /// @cond INTERNAL
    namespace detail {

        // What the generators of one recursive() share: its leaf generator,
        // its branch function and its label.
        template <typename T, typename Sub, typename Branch>
        struct RecursiveDefinition {
            Sub leaf;
            std::function<Branch(Sub)> branch;
            uint64_t label;
        };

    } // namespace detail

    // The generator a recursive() branch function receives. Each value it
    // draws is a sub-value of the value under construction, one level
    // deeper: a leaf, or a further branch.
    template <typename T, typename Sub, typename Branch>
    class SubtreeGenerator
        : public detail::Structural<T, detail::is_printable_handle_v<Sub>> {
      public:
        using Definition = detail::RecursiveDefinition<T, Sub, Branch>;

        SubtreeGenerator(
            std::shared_ptr<const Definition> definition,
            std::shared_ptr<hegel::internal::RecursionHandle> recursion,
            uint64_t depth)
            : definition_(std::move(definition)),
              recursion_(std::move(recursion)), depth_(depth) {}

        // Every sub-value has the label of the whole value, so the engine
        // can replace a value with one of its own sub-values when it
        // shrinks.
        uint64_t label() const override { return definition_->label; }

        // Draws the sub-value without a span of its own. The root draws
        // this way, inside the span of the recursive generator.
        T draw_subtree(const TestCase& tc, Printer& printer) const {
            T result = recursion_->branch(tc, depth_)
                           ? detail::draw_component(
                                 tc,
                                 definition_->branch(
                                     Sub(std::make_shared<SubtreeGenerator>(
                                         definition_, recursion_, depth_ + 1))),
                                 printer)
                           : draw_leaf(tc, printer);
            if (depth_ == 0) {
                recursion_->finish(tc);
            }
            return result;
        }

      protected:
        T draw_printing(const TestCase& tc, Printer& printer) const override {
            return draw_subtree(tc, printer);
        }

      private:
        T draw_leaf(const TestCase& tc, Printer& printer) const {
            recursion_->leaf(tc);
            return detail::draw_component(tc, definition_->leaf, printer);
        }

        std::shared_ptr<const Definition> definition_;
        std::shared_ptr<hegel::internal::RecursionHandle> recursion_;
        uint64_t depth_;
    };

    // Concrete IGenerator for recursive(). Draws attempts at the whole value
    // until the engine accepts one.
    template <typename T, typename Sub, typename Branch>
    class RecursiveGenerator
        : public detail::Structural<T, detail::is_printable_handle_v<Branch> &&
                                           detail::is_printable_handle_v<Sub>> {
      public:
        using Definition = detail::RecursiveDefinition<T, Sub, Branch>;

        RecursiveGenerator(Sub leaf, std::function<Branch(Sub)> branch,
                           RecursiveParams params)
            : params_(params) {
            namespace hi = hegel::internal;
            uint64_t label = hi::combine_labels(
                {hi::label(hi::LabelName::Recursive), leaf.label()});
            definition_ = std::make_shared<const Definition>(
                Definition{std::move(leaf), std::move(branch), label});
        }

        uint64_t label() const override { return definition_->label; }

      protected:
        T draw_printing(const TestCase& tc, Printer& printer) const override {
            namespace hi = hegel::internal;
            auto recursion = std::make_shared<hi::RecursionHandle>(
                tc, params_.max_depth, params_.max_leaves);
            SubtreeGenerator<T, Sub, Branch> root(definition_, recursion, 0);
            while (true) {
                try {
                    Speculation attempt = printer.speculate();
                    T result = root.draw_subtree(tc, printer);
                    attempt.commit();
                    return result;
                } catch (const hi::LeafBudgetExceeded&) {
                    recursion->retry(tc);
                } catch (const hi::AttemptMispriced&) {
                    // The engine discarded the attempt already.
                    continue;
                }
            }
        }

      private:
        std::shared_ptr<const Definition> definition_;
        RecursiveParams params_;
    };
    /// @endcond

    /**
     * @brief Generate recursively defined values, such as trees.
     *
     * @p leaf generates the values that do not recurse. @p branch builds one
     * level of recursive structure: it receives a generator of sub-values
     * of the same type, and returns a generator that combines some number
     * of them into one value. Hegel calls it again for each branch it
     * generates.
     *
     * The engine decides where a value branches, so values range in size
     * from one leaf up to the limits in @p params, and recursive values
     * are distributed the same way in every Hegel library. Where the
     * structure is not a tree of one type, for example for mutually
     * recursive types, use deferred().
     *
     * The value prints as @p leaf and the generators @p branch returns
     * print it, so the result is printable when @p leaf is printable and
     * @p branch returns a Generator\<T\>.
     *
     * @code{.cpp}
     * struct Tree {
     *     int value;
     *     std::vector<Tree> children;
     * };
     * auto trees = gs::recursive(
     *     gs::integers<int>().map([](int v) { return Tree{v, {}}; }),
     *     [](gs::Generator<Tree> subtrees) {
     *         return gs::vectors(subtrees, {.max_size = 3})
     *             .map([](std::vector<Tree> children) {
     *                 return Tree{0, std::move(children)};
     *             });
     *     });
     * @endcode
     *
     * @tparam T Value type
     * @tparam F Callable taking a `Generator<T>` of sub-values and returning
     *   a generator of T
     * @param leaf Generator for the values that do not recurse
     * @param branch Function that builds a branch from a generator of
     *   sub-values
     * @param params Limits on the depth and the number of leaves
     * @return Generator producing recursive values
     */
    template <typename T, typename F>
    auto recursive(Generator<T> leaf, F branch, RecursiveParams params = {}) {
        using Returned = std::decay_t<std::invoke_result_t<F, Generator<T>>>;
        using Branch =
            detail::handle_t<detail::is_printable_handle_v<Returned>, T>;
        return Branch(
            std::make_shared<RecursiveGenerator<T, Generator<T>, Branch>>(
                std::move(leaf),
                std::function<Branch(Generator<T>)>(std::move(branch)),
                params));
    }

    /**
     * @brief Generate recursively defined values from a leaf generator that
     *        is not printable.
     *
     * As recursive(), but @p branch receives an AnyGenerator\<T\>.
     *
     * @tparam T Value type
     * @tparam F Callable taking an `AnyGenerator<T>` of sub-values and
     *   returning a generator of T
     * @param leaf Generator for the values that do not recurse
     * @param branch Function that builds a branch from a generator of
     *   sub-values
     * @param params Limits on the depth and the number of leaves
     * @return AnyGenerator producing recursive values
     */
    template <typename T, typename F>
    AnyGenerator<T> recursive(AnyGenerator<T> leaf, F branch,
                              RecursiveParams params = {}) {
        return AnyGenerator<T>(
            std::make_shared<
                RecursiveGenerator<T, AnyGenerator<T>, AnyGenerator<T>>>(
                std::move(leaf),
                std::function<AnyGenerator<T>(AnyGenerator<T>)>(
                    std::move(branch)),
                params));
    }

} // namespace hegel::generators
