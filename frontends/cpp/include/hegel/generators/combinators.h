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
    template <typename T> class SampledFromGenerator : public IGenerator<T> {
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
    template <typename T> class OneOfGenerator : public IGenerator<T> {
      public:
        explicit OneOfGenerator(std::vector<Generator<T>> gens)
            : gens_(std::move(gens)) {
            if (gens_.empty()) {
                throw std::invalid_argument(
                    "one_of requires a non-empty vector of generators");
            }
            std::vector<uint64_t> branches;
            branches.reserve(gens_.size());
            for (const Generator<T>& gen : gens_) {
                branches.push_back(gen.label());
            }
            namespace hi = hegel::internal;
            label_ = hi::combine_labels({hi::label(hi::LabelName::OneOf),
                                         hi::combine_labels(branches)});
        }

        T do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            hi::start_span(tc, label_);
            int64_t index =
                hi::draw_integer(tc, 0, static_cast<int64_t>(gens_.size() - 1));
            T result = gens_[static_cast<size_t>(index)].do_draw(tc);
            hi::stop_span(tc);
            return result;
        }

        uint64_t label() const override { return label_; }

      private:
        std::vector<Generator<T>> gens_;
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
     * @tparam T Element type
     * @param elements Vector of values to sample from (must not be empty)
     * @return Generator that picks from elements
     */
    template <typename T>
    Generator<T> sampled_from(const std::vector<T>& elements) {
        return Generator<T>(new SampledFromGenerator<T>(elements));
    }

    /**
     * @brief Sample from a fixed set of values (initializer list).
     * @tparam T Element type
     * @param elements Values to sample from (must not be empty)
     * @return Generator that picks from elements
     */
    template <typename T>
    Generator<T> sampled_from(std::initializer_list<T> elements) {
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
     * Randomly selects one generator and uses it to produce a value.
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
        return Generator<T>(new OneOfGenerator<T>(std::move(gens)));
    }

    /**
     * @brief Choose from a list of generators (initializer list).
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
                                  const TestCase& tc) {
            if constexpr (I < std::tuple_size_v<GenTuple>) {
                if (idx == I) {
                    return Variant{std::in_place_index<I>,
                                   std::get<I>(gens).do_draw(tc)};
                }
                return draw_variant_impl<Variant, GenTuple, I + 1>(gens, idx,
                                                                   tc);
            } else {
                // Unreachable: idx is always in [0, N), so an earlier branch
                // matches before the recursion bottoms out.
                return Variant{}; // GCOVR_EXCL_LINE
            }
        }

    } // namespace detail

    // Concrete IGenerator for variant(). Draws a branch index inside a
    // one_of span; branches can have heterogeneous types.
    template <typename... Ts>
    class VariantGenerator : public IGenerator<std::variant<Ts...>> {
      public:
        using Result = std::variant<Ts...>;

        explicit VariantGenerator(Generator<Ts>... gens)
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

        Result do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            constexpr size_t N = sizeof...(Ts);
            hi::start_span(tc, label_);
            int64_t index =
                hi::draw_integer(tc, 0, static_cast<int64_t>(N - 1));
            Result result = detail::draw_variant_impl<Result, decltype(gens_)>(
                gens_, static_cast<size_t>(index), tc);
            hi::stop_span(tc);
            return result;
        }

        uint64_t label() const override { return label_; }

      private:
        std::tuple<Generator<Ts>...> gens_;
        uint64_t label_;
    };

    // Concrete IGenerator for optional(). A boolean draw inside an
    // optional span gates presence; false shrinks toward nullopt.
    template <typename T>
    class OptionalGenerator : public IGenerator<std::optional<T>> {
      public:
        explicit OptionalGenerator(Generator<T> gen)
            : gen_(std::move(gen)),
              label_(hegel::internal::combine_labels(
                  {hegel::internal::label(hegel::internal::LabelName::Optional),
                   gen_.label()})) {}

        std::optional<T> do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            hi::start_span(tc, label_);
            std::optional<T> result;
            if (hi::draw_boolean(tc, 0.5)) {
                result = gen_.do_draw(tc);
            }
            hi::stop_span(tc);
            return result;
        }

        uint64_t label() const override { return label_; }

      private:
        Generator<T> gen_;
        uint64_t label_;
    };
    /// @endcond

    /**
     * @brief Generate std::variant from heterogeneous generators.
     *
     * Each generator produces one possible variant alternative.
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
            new VariantGenerator<Ts...>(std::move(gens)...));
    }

    /**
     * @brief Generate optional values (present or absent).
     *
     * Randomly produces either a value from the given generator or
     * std::nullopt.
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
            new OptionalGenerator<T>(std::move(gen)));
    }

    /// @}

    /// @cond INTERNAL
    // Generator that delegates to an implementation supplied later through a
    // shared slot. Handed out by DeferredGeneratorDefinition::generator().
    template <typename T> class DeferredGenerator : public IGenerator<T> {
      public:
        explicit DeferredGenerator(
            std::shared_ptr<std::optional<Generator<T>>> slot)
            : slot_(std::move(slot)) {}

        T do_draw(const TestCase& tc) const override {
            if (!*slot_) {
                throw std::runtime_error(
                    "deferred generator drawn before set() was called");
            }
            return (*slot_)->do_draw(tc);
        }

        uint64_t label() const override {
            return hegel::internal::label(hegel::internal::LabelName::Deferred);
        }

      private:
        std::shared_ptr<std::optional<Generator<T>>> slot_;
    };
    /// @endcond

    /**
     * @brief A forward reference to a generator whose definition is provided
     *        later.
     *
     * Created by deferred(). Call generator() to obtain handles that can be
     * embedded in other generators before the implementation is known, then
     * call set() to install it. This enables self-recursive and mutually
     * recursive generators.
     *
     * @tparam T Value type produced by the eventual generator.
     */
    template <typename T> class DeferredGeneratorDefinition {
      public:
        DeferredGeneratorDefinition()
            : slot_(std::make_shared<std::optional<Generator<T>>>()) {}

        /**
         * @brief Return a handle that delegates to whatever is later passed to
         *        set(). May be called multiple times. Drawing from it before
         * set() is called throws.
         *
         * @return A Generator that delegates to the implementation installed by
         *   set().
         */
        Generator<T> generator() const {
            return Generator<T>(new DeferredGenerator<T>(slot_));
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
        void set(Generator<T> gen) {
            if (*slot_) {
                throw std::runtime_error(
                    "deferred generator set() called more than once");
            }
            *slot_ = std::move(gen);
        }

      private:
        std::shared_ptr<std::optional<Generator<T>>> slot_;
    };

    /**
     * @brief Create a deferred generator definition for forward references.
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
    template <typename T> DeferredGeneratorDefinition<T> deferred() {
        return DeferredGeneratorDefinition<T>();
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
        template <typename T> struct RecursiveDefinition {
            Generator<T> leaf;
            std::function<Generator<T>(Generator<T>)> branch;
            uint64_t label;
        };

    } // namespace detail

    // The generator a recursive() branch function receives. Each value it
    // draws is a sub-value of the value under construction, one level
    // deeper: a leaf, or a further branch.
    template <typename T> class SubtreeGenerator : public IGenerator<T> {
      public:
        SubtreeGenerator(
            std::shared_ptr<const detail::RecursiveDefinition<T>> definition,
            std::shared_ptr<hegel::internal::RecursionHandle> recursion,
            uint64_t depth)
            : definition_(std::move(definition)),
              recursion_(std::move(recursion)), depth_(depth) {}

        T do_draw(const TestCase& tc) const override {
            hegel::internal::start_span(tc, definition_->label);
            T result = draw_subtree(tc);
            hegel::internal::stop_span(tc);
            return result;
        }

        // Every sub-value has the label of the whole value, so the engine
        // can replace a value with one of its own sub-values when it
        // shrinks.
        uint64_t label() const override { return definition_->label; }

        // Draws the sub-value without a span of its own. The root draws
        // this way, inside the span of the recursive generator.
        T draw_subtree(const TestCase& tc) const {
            T result = recursion_->branch(tc, depth_)
                           ? definition_
                                 ->branch(Generator<T>(new SubtreeGenerator<T>(
                                     definition_, recursion_, depth_ + 1)))
                                 .do_draw(tc)
                           : draw_leaf(tc);
            if (depth_ == 0) {
                recursion_->finish(tc);
            }
            return result;
        }

      private:
        T draw_leaf(const TestCase& tc) const {
            recursion_->leaf(tc);
            return definition_->leaf.do_draw(tc);
        }

        std::shared_ptr<const detail::RecursiveDefinition<T>> definition_;
        std::shared_ptr<hegel::internal::RecursionHandle> recursion_;
        uint64_t depth_;
    };

    // Concrete IGenerator for recursive(). Draws attempts at the whole value
    // until the engine accepts one.
    template <typename T> class RecursiveGenerator : public IGenerator<T> {
      public:
        RecursiveGenerator(Generator<T> leaf,
                           std::function<Generator<T>(Generator<T>)> branch,
                           RecursiveParams params)
            : params_(params) {
            namespace hi = hegel::internal;
            uint64_t label = hi::combine_labels(
                {hi::label(hi::LabelName::Recursive), leaf.label()});
            definition_ =
                std::make_shared<const detail::RecursiveDefinition<T>>(
                    detail::RecursiveDefinition<T>{std::move(leaf),
                                                   std::move(branch), label});
        }

        T do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            hi::start_span(tc, definition_->label);
            auto recursion = std::make_shared<hi::RecursionHandle>(
                tc, params_.max_depth, params_.max_leaves);
            SubtreeGenerator<T> root(definition_, recursion, 0);
            while (true) {
                try {
                    T result = root.draw_subtree(tc);
                    hi::stop_span(tc);
                    return result;
                } catch (const hi::LeafBudgetExceeded&) {
                    recursion->retry(tc);
                } catch (
                    const hi::
                        AttemptMispriced&) { // NOLINT(bugprone-empty-catch)
                    // The engine discarded the attempt already.
                }
            }
        }

        uint64_t label() const override { return definition_->label; }

      private:
        std::shared_ptr<const detail::RecursiveDefinition<T>> definition_;
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
     *   a `Generator<T>`
     * @param leaf Generator for the values that do not recurse
     * @param branch Function that builds a branch from a generator of
     *   sub-values
     * @param params Limits on the depth and the number of leaves
     * @return Generator producing recursive values
     */
    template <typename T, typename F>
    Generator<T> recursive(Generator<T> leaf, F branch,
                           RecursiveParams params = {}) {
        return Generator<T>(new RecursiveGenerator<T>(
            std::move(leaf),
            std::function<Generator<T>(Generator<T>)>(std::move(branch)),
            params));
    }

} // namespace hegel::generators
