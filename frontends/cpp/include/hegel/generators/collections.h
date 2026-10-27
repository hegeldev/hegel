#pragma once

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <initializer_list>
#include <map>
#include <optional>
#include <set>
#include <tuple>
#include <type_traits>
#include <utility>
#include <vector>

#include "hegel/core.h"

namespace hegel::generators {

    // =============================================================================
    // Parameter structs
    // =============================================================================

    /**
     * @brief Parameters for vectors() generator.
     */
    struct VectorsParams {
        size_t min_size = 0;            ///< Minimum vector size
        std::optional<size_t> max_size; ///< Maximum vector size. Default: no
                                        ///< limit (engine-chosen sizes)
        bool unique = false; ///< If true, all elements must be unique
    };

    /**
     * @brief Parameters for sets() generator.
     */
    struct SetsParams {
        size_t min_size = 0;            ///< Minimum set size
        std::optional<size_t> max_size; ///< Maximum set size. Default: no
                                        ///< limit (engine-chosen sizes)
    };

    /**
     * @brief Parameters for maps() generator.
     */
    struct MapsParams {
        size_t min_size = 0;            ///< Minimum number of entries
        std::optional<size_t> max_size; ///< Maximum number of entries.
                                        ///< Default: no limit (engine-chosen
                                        ///< sizes)
    };

    /// @cond INTERNAL
    namespace detail {

        // True iff `a == b` is well-formed for const T. vectors() only
        // needs operator== when unique=true, so its availability is checked
        // at runtime rather than required of every element type.
        template <typename T, typename = void>
        struct is_equality_comparable : std::false_type {};
        template <typename T>
        struct is_equality_comparable<
            T, std::void_t<decltype(std::declval<const T&>() ==
                                    std::declval<const T&>())>>
            : std::true_type {};

        inline uint64_t
        collection_max_size(const std::optional<size_t>& max_size) {
            return max_size ? static_cast<uint64_t>(*max_size)
                            : hegel::internal::no_max_size;
        }

    } // namespace detail

    // Concrete IGenerator for vectors(). An engine-managed collection
    // decides how many elements to draw; uniqueness is enforced by
    // rejecting duplicate elements back to the engine.
    template <typename T, bool Printable>
    class VectorsGenerator
        : public detail::Structural<std::vector<T>, Printable> {
      public:
        // Two elements are duplicates when `same` returns true for them.
        // An empty `same` allows duplicates.
        using Same = std::function<bool(const T&, const T&)>;

        VectorsGenerator(detail::handle_t<Printable, T> elements,
                         VectorsParams params = {}, Same same = nullptr)
            : elements_(std::move(elements)), params_(params),
              same_(std::move(same)),
              label_(hegel::internal::combine_labels(
                  {hegel::internal::label(hegel::internal::LabelName::Lists),
                   elements_.label()})) {
            if (params_.max_size && params_.min_size > *params_.max_size) {
                throw std::invalid_argument("Cannot have max_size < min_size");
            }
            if (params_.unique && same_) {
                throw std::invalid_argument(
                    "Cannot have both unique = true and unique_by");
            }
            if (params_.unique) {
                if constexpr (detail::is_equality_comparable<T>::value) {
                    same_ = [](const T& a, const T& b) { return a == b; };
                } else {
                    throw std::invalid_argument(
                        "vectors(..., {.unique = true}) requires an "
                        "equality-comparable element type");
                }
            }
        }

        uint64_t label() const override { return label_; }

      protected:
        std::vector<T> draw_printing(const TestCase& tc,
                                     Printer& printer) const override {
            namespace hi = hegel::internal;
            hi::CollectionHandle collection(
                tc, params_.min_size,
                detail::collection_max_size(params_.max_size));
            std::vector<T> result;
            printer.begin_block(
                detail::open_text<std::vector<T>>(printer, "{"));
            while (collection.more(tc)) {
                Speculation element = printer.speculate();
                if (!result.empty()) {
                    printer.separator();
                }
                T value = detail::draw_component(tc, elements_, printer);
                if (same_ && std::any_of(result.begin(), result.end(),
                                         [this, &value](const T& existing) {
                                             return same_(existing, value);
                                         })) {
                    element.abort();
                    collection.reject(tc, "duplicate element");
                    continue;
                }
                element.commit();
                result.push_back(std::move(value));
            }
            printer.end_block("}");
            return result;
        }

      private:
        detail::handle_t<Printable, T> elements_;
        VectorsParams params_;
        Same same_;
        uint64_t label_;
    };

    // Concrete IGenerator for sets(). Duplicate elements are rejected back
    // to the engine so it draws a replacement.
    template <typename T, bool Printable>
    class SetsGenerator : public detail::Structural<std::set<T>, Printable> {
      public:
        SetsGenerator(detail::handle_t<Printable, T> elements,
                      SetsParams params = {})
            : elements_(std::move(elements)), params_(params),
              label_(hegel::internal::combine_labels(
                  {hegel::internal::label(hegel::internal::LabelName::Sets),
                   elements_.label()})) {
            if (params_.max_size && params_.min_size > *params_.max_size) {
                throw std::invalid_argument("Cannot have max_size < min_size");
            }
        }

        uint64_t label() const override { return label_; }

      protected:
        std::set<T> draw_printing(const TestCase& tc,
                                  Printer& printer) const override {
            namespace hi = hegel::internal;
            hi::CollectionHandle collection(
                tc, params_.min_size,
                detail::collection_max_size(params_.max_size));
            std::set<T> result;
            printer.begin_block(detail::open_text<std::set<T>>(printer, "{"));
            while (collection.more(tc)) {
                Speculation element = printer.speculate();
                if (!result.empty()) {
                    printer.separator();
                }
                if (!result
                         .insert(detail::draw_component(tc, elements_, printer))
                         .second) {
                    element.abort();
                    collection.reject(tc, "duplicate element");
                    continue;
                }
                element.commit();
            }
            printer.end_block("}");
            return result;
        }

      private:
        detail::handle_t<Printable, T> elements_;
        SetsParams params_;
        uint64_t label_;
    };

    // Concrete IGenerator for maps(). Draws the key first and only draws a
    // value once the key is known to be fresh; duplicate keys are rejected
    // back to the engine.
    template <typename K, typename V, bool Printable>
    class MapsGenerator : public detail::Structural<std::map<K, V>, Printable> {
      public:
        MapsGenerator(detail::handle_t<Printable, K> keys,
                      detail::handle_t<Printable, V> values,
                      MapsParams params = {})
            : keys_(std::move(keys)), values_(std::move(values)),
              params_(params),
              label_(hegel::internal::combine_labels(
                  {hegel::internal::label(hegel::internal::LabelName::Dicts),
                   keys_.label(), values_.label()})) {
            if (params_.max_size && params_.min_size > *params_.max_size) {
                throw std::invalid_argument("Cannot have max_size < min_size");
            }
        }

        uint64_t label() const override { return label_; }

      protected:
        std::map<K, V> draw_printing(const TestCase& tc,
                                     Printer& printer) const override {
            namespace hi = hegel::internal;
            hi::CollectionHandle collection(
                tc, params_.min_size,
                detail::collection_max_size(params_.max_size));
            std::map<K, V> result;
            printer.begin_block(
                detail::open_text<std::map<K, V>>(printer, "{"));
            while (collection.more(tc)) {
                Speculation entry = printer.speculate();
                if (!result.empty()) {
                    printer.separator();
                }
                printer.begin_block("{");
                K key = detail::draw_component(tc, keys_, printer);
                if (result.find(key) != result.end()) {
                    entry.abort();
                    collection.reject(tc, "duplicate key");
                    continue;
                }
                printer.separator();
                V value = detail::draw_component(tc, values_, printer);
                printer.end_block("}");
                entry.commit();
                result.emplace(std::move(key), std::move(value));
            }
            printer.end_block("}");
            return result;
        }

      private:
        detail::handle_t<Printable, K> keys_;
        detail::handle_t<Printable, V> values_;
        MapsParams params_;
        uint64_t label_;
    };
    /// @endcond

    /// @name Collections
    /// @{

    /**
     * @brief Generate vectors with elements from another generator.
     *
     * The vector prints each element as @p elements prints it, so it is
     * printable when @p elements is.
     *
     * @code{.cpp}
     * auto int_vec = vectors(integers<int>());
     * auto bounded = vectors(integers<int>(), {.min_size = 1, .max_size = 10});
     * auto unique_vec = vectors(integers<int>(), {.unique = true});
     * @endcode
     *
     * @tparam T Element type
     * @param elements Generator for vector elements
     * @param params Size and uniqueness constraints
     * @return Generator producing vectors
     */
    template <typename T>
    Generator<std::vector<T>> vectors(Generator<T> elements,
                                      VectorsParams params = {}) {
        return Generator<std::vector<T>>(
            new VectorsGenerator<T, true>(std::move(elements), params));
    }

    /**
     * @brief Generate vectors with elements from a generator that is not
     *        printable.
     *
     * @tparam T Element type
     * @param elements Generator for vector elements
     * @param params Size and uniqueness constraints
     * @return AnyGenerator producing vectors
     */
    template <typename T>
    AnyGenerator<std::vector<T>> vectors(AnyGenerator<T> elements,
                                         VectorsParams params = {}) {
        return AnyGenerator<std::vector<T>>(
            new VectorsGenerator<T, false>(std::move(elements), params));
    }

    /// @cond INTERNAL
    namespace detail {

        template <typename T, typename F>
        std::function<bool(const T&, const T&)> same_key(F unique_by) {
            return [key = std::move(unique_by)](const T& a, const T& b) {
                return key(a) == key(b);
            };
        }

    } // namespace detail
    /// @endcond

    /**
     * @brief Generate vectors whose elements have distinct keys.
     *
     * Like vectors(), but no two elements of a generated vector have equal
     * keys. @p unique_by computes the key of an element, and the keys are
     * compared with `==`. Do not also set VectorsParams::unique.
     *
     * @code{.cpp}
     * struct User { int id; std::string name; };
     * auto users = vectors(
     *     builds<User>(integers<int>(), text()), {.max_size = 10},
     *     [](const User& user) { return user.id; });
     * // No two users have the same id.
     * @endcode
     *
     * @tparam T Element type
     * @tparam F Callable taking `const T&` and returning the element's key
     * @param elements Generator for vector elements
     * @param params Size constraints
     * @param unique_by Function from an element to its key
     * @return Generator producing vectors with distinct keys
     */
    template <typename T, typename F,
              typename = std::enable_if_t<std::is_invocable_v<F, const T&>>>
    Generator<std::vector<T>> vectors(Generator<T> elements,
                                      VectorsParams params, F unique_by) {
        return Generator<std::vector<T>>(new VectorsGenerator<T, true>(
            std::move(elements), params,
            detail::same_key<T>(std::move(unique_by))));
    }

    /**
     * @brief Generate vectors whose elements have distinct keys, from a
     *        generator that is not printable.
     *
     * @tparam T Element type
     * @tparam F Callable taking `const T&` and returning the element's key
     * @param elements Generator for vector elements
     * @param params Size constraints
     * @param unique_by Function from an element to its key
     * @return AnyGenerator producing vectors with distinct keys
     */
    template <typename T, typename F,
              typename = std::enable_if_t<std::is_invocable_v<F, const T&>>>
    AnyGenerator<std::vector<T>> vectors(AnyGenerator<T> elements,
                                         VectorsParams params, F unique_by) {
        return AnyGenerator<std::vector<T>>(new VectorsGenerator<T, false>(
            std::move(elements), params,
            detail::same_key<T>(std::move(unique_by))));
    }

    /**
     * @brief Generate sets with elements from another generator.
     *
     * The set prints its elements in the order they were drawn, each as
     * @p elements prints it, so it is printable when @p elements is.
     *
     * @code{.cpp}
     * auto int_set = sets(integers<int>());
     * auto bounded = sets(integers<int>(), {.min_size = 1, .max_size = 5});
     * @endcode
     *
     * @tparam T Element type (must be comparable)
     * @param elements Generator for set elements
     * @param params Size constraints
     * @return Generator producing sets
     */
    template <typename T>
    Generator<std::set<T>> sets(Generator<T> elements, SetsParams params = {}) {
        return Generator<std::set<T>>(
            new SetsGenerator<T, true>(std::move(elements), params));
    }

    /**
     * @brief Generate sets with elements from a generator that is not
     *        printable.
     *
     * @tparam T Element type (must be comparable)
     * @param elements Generator for set elements
     * @param params Size constraints
     * @return AnyGenerator producing sets
     */
    template <typename T>
    AnyGenerator<std::set<T>> sets(AnyGenerator<T> elements,
                                   SetsParams params = {}) {
        return AnyGenerator<std::set<T>>(
            new SetsGenerator<T, false>(std::move(elements), params));
    }

    /**
     * @brief Generate maps with configurable key and value types.
     *
     * The map prints its entries in the order they were drawn, each key
     * and value as its generator prints it, so it is printable when both
     * generators are.
     *
     * @code{.cpp}
     * // String keys
     * auto strMap = maps(text(), integers<int>());
     *
     * // Integer keys
     * auto intMap = maps(integers<int>(), text());
     *
     * // With size bounds
     * auto bounded = maps(text(), integers<int>(), {.min_size = 1,
     * .max_size = 3});
     * @endcode
     *
     * @tparam K Key type
     * @tparam V Value type
     * @param keys Generator for map keys
     * @param values Generator for map values
     * @param params Size constraints
     * @return Generator producing maps
     */
    template <typename K, typename V>
    Generator<std::map<K, V>> maps(Generator<K> keys, Generator<V> values,
                                   MapsParams params = {}) {
        return Generator<std::map<K, V>>(new MapsGenerator<K, V, true>(
            std::move(keys), std::move(values), params));
    }

    /**
     * @brief Generate maps from generators that are not both printable.
     *
     * @tparam K Key type
     * @tparam V Value type
     * @param keys Generator for map keys
     * @param values Generator for map values
     * @param params Size constraints
     * @return AnyGenerator producing maps
     */
    template <typename K, typename V>
    AnyGenerator<std::map<K, V>>
    maps(AnyGenerator<K> keys, AnyGenerator<V> values, MapsParams params = {}) {
        return AnyGenerator<std::map<K, V>>(new MapsGenerator<K, V, false>(
            std::move(keys), std::move(values), params));
    }

    /// @cond INTERNAL
    // Concrete IGenerator for tuples(). Draws each element in order inside
    // one tuple span.
    template <bool Printable, typename... Ts>
    class TuplesGenerator
        : public detail::Structural<std::tuple<Ts...>, Printable> {
      public:
        using ResultTuple = std::tuple<Ts...>;

        explicit TuplesGenerator(detail::handle_t<Printable, Ts>... gens)
            : gens_(std::move(gens)...),
              label_(make_label(std::index_sequence_for<Ts...>{})) {}

        uint64_t label() const override { return label_; }

      protected:
        ResultTuple draw_printing(const TestCase& tc,
                                  Printer& printer) const override {
            namespace hi = hegel::internal;
            printer.begin_block(detail::open_text<ResultTuple>(printer, "{"));
            ResultTuple result =
                draw_all(tc, printer, std::index_sequence_for<Ts...>{});
            printer.end_block("}");
            return result;
        }

      private:
        template <size_t... Is>
        ResultTuple draw_all(const TestCase& tc, Printer& printer,
                             std::index_sequence<Is...>) const {
            return ResultTuple{draw_element<Is>(tc, printer)...};
        }

        template <size_t I>
        std::tuple_element_t<I, ResultTuple>
        draw_element(const TestCase& tc, Printer& printer) const {
            if constexpr (I > 0) {
                printer.separator();
            }
            return detail::draw_component(tc, std::get<I>(gens_), printer);
        }

        template <size_t... Is>
        uint64_t make_label(std::index_sequence<Is...>) const {
            namespace hi = hegel::internal;
            return hi::combine_labels({hi::label(hi::LabelName::Tuples),
                                       std::get<Is>(gens_).label()...});
        }

        std::tuple<detail::handle_t<Printable, Ts>...> gens_;
        uint64_t label_;
    };
    /// @endcond

    /**
     * @brief Generate tuples from multiple generators.
     *
     * Each generator produces one element of the resulting tuple. The tuple
     * prints each element as its generator prints it, so it is printable
     * when every generator is.
     *
     * @code{.cpp}
     * auto pair = tuples(integers<int>(), text());
     * auto triple = tuples(booleans(), integers<int>(), floats<double>());
     * @endcode
     *
     * @tparam Ts Element types
     * @param gens Generators for each tuple element
     * @return Generator producing tuples
     */
    template <typename... Ts>
    Generator<std::tuple<Ts...>> tuples(Generator<Ts>... gens) {
        return Generator<std::tuple<Ts...>>(
            new TuplesGenerator<true, Ts...>(std::move(gens)...));
    }

    /**
     * @brief Generate tuples from generators that are not all printable.
     *
     * @tparam Ts Element types
     * @param gens Generators for each tuple element
     * @return AnyGenerator producing tuples
     */
    template <typename... Ts>
    AnyGenerator<std::tuple<Ts...>> tuples(AnyGenerator<Ts>... gens) {
        return AnyGenerator<std::tuple<Ts...>>(
            new TuplesGenerator<false, Ts...>(std::move(gens)...));
    }

    /// @cond INTERNAL
    // Concrete IGenerator for arrays(). Draws exactly N elements from one
    // element generator inside a single tuple span (fixed arity, not the
    // engine-managed collection protocol vectors() uses).
    template <typename T, size_t N, bool Printable>
    class ArrayGenerator
        : public detail::Structural<std::array<T, N>, Printable> {
      public:
        explicit ArrayGenerator(detail::handle_t<Printable, T> element)
            : element_(std::move(element)),
              label_(hegel::internal::combine_labels(
                  {hegel::internal::label(hegel::internal::LabelName::Array),
                   element_.label()})) {}

        uint64_t label() const override { return label_; }

      protected:
        std::array<T, N> draw_printing(const TestCase& tc,
                                       Printer& printer) const override {
            namespace hi = hegel::internal;
            printer.begin_block(
                detail::open_text<std::array<T, N>>(printer, "{"));
            std::array<T, N> result =
                draw_all(tc, printer, std::make_index_sequence<N>{});
            printer.end_block("}");
            return result;
        }

      private:
        // Braced-init evaluates its elements left to right, so the N draws
        // happen in order.
        template <size_t... Is>
        std::array<T, N> draw_all(const TestCase& tc, Printer& printer,
                                  std::index_sequence<Is...>) const {
            return std::array<T, N>{draw_element(tc, printer, Is)...};
        }

        T draw_element(const TestCase& tc, Printer& printer,
                       size_t index) const {
            if (index > 0) {
                printer.separator();
            }
            return detail::draw_component(tc, element_, printer);
        }

        detail::handle_t<Printable, T> element_;
        uint64_t label_;
    };
    /// @endcond

    /**
     * @brief Generate fixed-size arrays.
     *
     * Draws exactly @p N elements from @p element, producing a
     * @c std::array<T, N>. Unlike vectors(), the length is fixed at compile
     * time. @p N cannot be deduced, so specify it explicitly. The array is
     * printable when @p element is.
     *
     * @code{.cpp}
     * auto rgb = arrays<int, 3>(integers<int>({.min_value = 0, .max_value =
     * 255}));
     * // Draws std::array<int, 3>
     * @endcode
     *
     * @tparam T Element type
     * @tparam N Number of elements
     * @param element Generator for each element
     * @return Generator producing std::array<T, N>
     */
    template <typename T, size_t N>
    Generator<std::array<T, N>> arrays(Generator<T> element) {
        return Generator<std::array<T, N>>(
            new ArrayGenerator<T, N, true>(std::move(element)));
    }

    /**
     * @brief Generate fixed-size arrays from a generator that is not
     *        printable.
     *
     * @tparam T Element type
     * @tparam N Number of elements
     * @param element Generator for each element
     * @return AnyGenerator producing std::array<T, N>
     */
    template <typename T, size_t N>
    AnyGenerator<std::array<T, N>> arrays(AnyGenerator<T> element) {
        return AnyGenerator<std::array<T, N>>(
            new ArrayGenerator<T, N, false>(std::move(element)));
    }

    /// @cond INTERNAL
    namespace detail {

        inline std::vector<size_t> draw_index_sample(const TestCase& tc,
                                                     size_t n,
                                                     uint64_t min_size,
                                                     uint64_t max_size) {
            namespace hi = hegel::internal;
            std::vector<size_t> remaining(n);
            for (size_t i = 0; i < n; ++i) {
                remaining[i] = i;
            }
            hi::CollectionHandle collection(tc, min_size, max_size);
            std::vector<size_t> chosen;
            while (!remaining.empty() && collection.more(tc)) {
                auto index = static_cast<size_t>(hi::draw_integer(
                    tc, 0, static_cast<int64_t>(remaining.size() - 1)));
                chosen.push_back(remaining[index]);
                remaining.erase(remaining.begin() +
                                static_cast<std::ptrdiff_t>(index));
            }
            return chosen;
        }

        inline void check_sizes(uint64_t min_size,
                                const std::optional<size_t>& max_size) {
            if (max_size && min_size > *max_size) {
                throw std::invalid_argument("Cannot have max_size < min_size");
            }
        }

    } // namespace detail

    // Concrete IGenerator for permutations(). Draws, for each position, the
    // index of one of the values not yet placed, inside an engine-managed
    // collection of exactly that many elements.
    template <typename T>
    class PermutationsGenerator
        : public detail::value_base_t<std::vector<T>, is_representable_v<T>> {
      public:
        explicit PermutationsGenerator(std::vector<T> values)
            : values_(std::move(values)) {}

        std::vector<T> do_draw(const TestCase& tc) const override {
            std::vector<T> result;
            result.reserve(values_.size());
            for (size_t index : detail::draw_index_sample(
                     tc, values_.size(), values_.size(), values_.size())) {
                result.push_back(values_[index]);
            }
            return result;
        }

        uint64_t label() const override {
            return hegel::internal::label(
                hegel::internal::LabelName::Permutation);
        }

      private:
        std::vector<T> values_;
    };
    /// @endcond

    /**
     * @brief Generate permutations of a fixed list of values.
     *
     * Each generated vector holds all of @p values in some order. It shrinks
     * toward the original order.
     *
     * @code{.cpp}
     * auto order = permutations(std::vector<int>{1, 2, 3, 4, 5});
     * @endcode
     *
     * The result prints the reordered vector by its C++ representation, so
     * it is printable when hegel::is_representable_v is true for T.
     *
     * @tparam T Element type
     * @param values The values to reorder
     * @return Generator producing reorderings of @p values
     */
    template <typename T> auto permutations(std::vector<T> values) {
        return detail::handle_t<is_representable_v<T>, std::vector<T>>(
            std::make_shared<PermutationsGenerator<T>>(std::move(values)));
    }

    /**
     * @brief Generate permutations of a fixed list of values (initializer
     * list).
     *
     * @tparam T Element type
     * @param values The values to reorder
     * @return Generator producing reorderings of @p values
     */
    template <typename T> auto permutations(std::initializer_list<T> values) {
        return permutations(std::vector<T>(values));
    }

    /**
     * @brief Parameters for subsequences().
     */
    struct SubsequencesParams {
        size_t min_size = 0;            ///< Minimum subsequence length
        std::optional<size_t> max_size; ///< Maximum subsequence length.
                                        ///< Default: the number of values
    };

    /**
     * @brief Parameters for samples().
     */
    struct SamplesParams {
        size_t min_size = 0;            ///< Minimum sample length
        std::optional<size_t> max_size; ///< Maximum sample length. Default:
                                        ///< no limit (engine-chosen sizes),
                                        ///< or the number of values without
                                        ///< replacement
        bool replacement = true; ///< Whether a value may be chosen more than
                                 ///< once
    };

    /// @cond INTERNAL
    template <typename T>
    class SubsequencesGenerator
        : public detail::value_base_t<std::vector<T>, is_representable_v<T>> {
      public:
        SubsequencesGenerator(std::vector<T> values, SubsequencesParams params)
            : values_(std::move(values)), params_(params) {
            detail::check_sizes(params_.min_size, params_.max_size);
            if (params_.min_size > values_.size()) {
                throw std::invalid_argument(
                    "Cannot generate a subsequence: min_size " +
                    std::to_string(params_.min_size) + " is larger than the " +
                    std::to_string(values_.size()) +
                    " elements in the sequence");
            }
        }

        std::vector<T> do_draw(const TestCase& tc) const override {
            size_t max_size = params_.max_size
                                  ? std::min(*params_.max_size, values_.size())
                                  : values_.size();
            std::vector<size_t> indices = detail::draw_index_sample(
                tc, values_.size(), params_.min_size, max_size);
            std::sort(indices.begin(), indices.end());
            std::vector<T> result;
            result.reserve(indices.size());
            for (size_t index : indices) {
                result.push_back(values_[index]);
            }
            return result;
        }

        uint64_t label() const override {
            return hegel::internal::label(
                hegel::internal::LabelName::Subsequence);
        }

      private:
        std::vector<T> values_;
        SubsequencesParams params_;
    };

    template <typename T>
    class SamplesGenerator
        : public detail::value_base_t<std::vector<T>, is_representable_v<T>> {
      public:
        SamplesGenerator(std::vector<T> values, SamplesParams params)
            : values_(std::move(values)), params_(params) {
            detail::check_sizes(params_.min_size, params_.max_size);
            if (params_.replacement) {
                if (values_.empty() && params_.min_size > 0) {
                    throw std::invalid_argument(
                        "Cannot generate a non-empty sample from an empty "
                        "sequence");
                }
            } else if (params_.min_size > values_.size()) {
                throw std::invalid_argument(
                    "Cannot generate a sample without replacement: min_size " +
                    std::to_string(params_.min_size) + " is larger than the " +
                    std::to_string(values_.size()) +
                    " elements in the sequence");
            }
        }

        std::vector<T> do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            std::vector<T> result;
            if (params_.replacement) {
                uint64_t max_size =
                    values_.empty()
                        ? 0
                        : detail::collection_max_size(params_.max_size);
                hi::CollectionHandle collection(tc, params_.min_size, max_size);
                while (collection.more(tc)) {
                    auto index = static_cast<size_t>(hi::draw_integer(
                        tc, 0, static_cast<int64_t>(values_.size() - 1)));
                    result.push_back(values_[index]);
                }
            } else {
                size_t max_size = params_.max_size ? std::min(*params_.max_size,
                                                              values_.size())
                                                   : values_.size();
                for (size_t index : detail::draw_index_sample(
                         tc, values_.size(), params_.min_size, max_size)) {
                    result.push_back(values_[index]);
                }
            }
            return result;
        }

        uint64_t label() const override {
            return hegel::internal::label(hegel::internal::LabelName::Sample);
        }

      private:
        std::vector<T> values_;
        SamplesParams params_;
    };
    /// @endcond

    /**
     * @brief Generate subsequences of a fixed list of values.
     *
     * Each generated vector holds some of @p values, in their original
     * order. It shrinks toward fewer values, taken from earlier in the list.
     *
     * @code{.cpp}
     * auto some = subsequences(std::vector<int>{1, 2, 3, 4, 5},
     *                          {.min_size = 1});
     * // e.g. {2, 5}
     * @endcode
     *
     * The result prints the vector by its C++ representation, so it is
     * printable when hegel::is_representable_v is true for T.
     *
     * @tparam T Element type
     * @param values The values to choose from
     * @param params Length constraints
     * @return Generator producing subsequences of @p values
     * @throws std::invalid_argument if the size bounds are inconsistent or
     *   `min_size` exceeds the number of values
     */
    template <typename T>
    auto subsequences(std::vector<T> values, SubsequencesParams params = {}) {
        return detail::handle_t<is_representable_v<T>, std::vector<T>>(
            std::make_shared<SubsequencesGenerator<T>>(std::move(values),
                                                       params));
    }

    /**
     * @brief Generate subsequences of a fixed list of values (initializer
     * list).
     *
     * @tparam T Element type
     * @param values The values to choose from
     * @param params Length constraints
     * @return Generator producing subsequences of @p values
     */
    template <typename T>
    auto subsequences(std::initializer_list<T> values,
                      SubsequencesParams params = {}) {
        return subsequences(std::vector<T>(values), params);
    }

    /**
     * @brief Generate samples of a fixed list of values.
     *
     * With replacement (the default) each element of the generated vector is
     * chosen independently from the whole of @p values, so a value can
     * appear any number of times. With `replacement = false` each of
     * @p values is used at most once, so the result is a reordered subset of
     * them. Either way it shrinks toward fewer elements, taken from earlier
     * in the list.
     *
     * @code{.cpp}
     * auto rolls = samples(std::vector<int>{1, 2, 3, 4, 5, 6},
     *                      {.min_size = 2, .max_size = 2});
     * // e.g. {4, 4}
     * auto picks = samples(std::vector<int>{1, 2, 3, 4, 5, 6},
     *                      {.max_size = 3, .replacement = false});
     * // e.g. {5, 1, 3}
     * @endcode
     *
     * The result prints the vector by its C++ representation, so it is
     * printable when hegel::is_representable_v is true for T.
     *
     * @tparam T Element type
     * @param values The values to choose from
     * @param params Length constraints and whether to sample with
     *   replacement
     * @return Generator producing samples of @p values
     * @throws std::invalid_argument if the size bounds are inconsistent,
     *   `min_size` is positive and @p values is empty, or, without
     *   replacement, `min_size` exceeds the number of values
     */
    template <typename T>
    auto samples(std::vector<T> values, SamplesParams params = {}) {
        return detail::handle_t<is_representable_v<T>, std::vector<T>>(
            std::make_shared<SamplesGenerator<T>>(std::move(values), params));
    }

    /**
     * @brief Generate samples of a fixed list of values (initializer list).
     *
     * @tparam T Element type
     * @param values The values to choose from
     * @param params Length constraints and whether to sample with
     *   replacement
     * @return Generator producing samples of @p values
     */
    template <typename T>
    auto samples(std::initializer_list<T> values, SamplesParams params = {}) {
        return samples(std::vector<T>(values), params);
    }

    /// @}

} // namespace hegel::generators
