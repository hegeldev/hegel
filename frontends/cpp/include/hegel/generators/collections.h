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
    template <typename T>
    class VectorsGenerator : public IGenerator<std::vector<T>> {
      public:
        // Two elements are duplicates when `same` returns true for them.
        // An empty `same` allows duplicates.
        using Same = std::function<bool(const T&, const T&)>;

        VectorsGenerator(Generator<T> elements, VectorsParams params = {},
                         Same same = nullptr)
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

        std::vector<T> do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            hi::start_span(tc, label_);
            hi::CollectionHandle collection(
                tc, params_.min_size,
                detail::collection_max_size(params_.max_size));
            std::vector<T> result;
            while (collection.more(tc)) {
                T element = elements_.do_draw(tc);
                if (same_ && std::any_of(result.begin(), result.end(),
                                         [this, &element](const T& existing) {
                                             return same_(existing, element);
                                         })) {
                    collection.reject(tc, "duplicate element");
                    continue;
                }
                result.push_back(std::move(element));
            }
            hi::stop_span(tc);
            return result;
        }

        uint64_t label() const override { return label_; }

      private:
        Generator<T> elements_;
        VectorsParams params_;
        Same same_;
        uint64_t label_;
    };

    // Concrete IGenerator for sets(). Duplicate elements are rejected back
    // to the engine so it draws a replacement.
    template <typename T> class SetsGenerator : public IGenerator<std::set<T>> {
      public:
        SetsGenerator(Generator<T> elements, SetsParams params = {})
            : elements_(std::move(elements)), params_(params),
              label_(hegel::internal::combine_labels(
                  {hegel::internal::label(hegel::internal::LabelName::Sets),
                   elements_.label()})) {
            if (params_.max_size && params_.min_size > *params_.max_size) {
                throw std::invalid_argument("Cannot have max_size < min_size");
            }
        }

        std::set<T> do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            hi::start_span(tc, label_);
            hi::CollectionHandle collection(
                tc, params_.min_size,
                detail::collection_max_size(params_.max_size));
            std::set<T> result;
            while (collection.more(tc)) {
                if (!result.insert(elements_.do_draw(tc)).second) {
                    collection.reject(tc, "duplicate element");
                }
            }
            hi::stop_span(tc);
            return result;
        }

        uint64_t label() const override { return label_; }

      private:
        Generator<T> elements_;
        SetsParams params_;
        uint64_t label_;
    };

    // Concrete IGenerator for maps(). Draws the key first and only draws a
    // value once the key is known to be fresh; duplicate keys are rejected
    // back to the engine.
    template <typename K, typename V>
    class MapsGenerator : public IGenerator<std::map<K, V>> {
      public:
        MapsGenerator(Generator<K> keys, Generator<V> values,
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

        std::map<K, V> do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            hi::start_span(tc, label_);
            hi::CollectionHandle collection(
                tc, params_.min_size,
                detail::collection_max_size(params_.max_size));
            std::map<K, V> result;
            while (collection.more(tc)) {
                K key = keys_.do_draw(tc);
                if (result.find(key) != result.end()) {
                    collection.reject(tc, "duplicate key");
                    continue;
                }
                result.emplace(std::move(key), values_.do_draw(tc));
            }
            hi::stop_span(tc);
            return result;
        }

        uint64_t label() const override { return label_; }

      private:
        Generator<K> keys_;
        Generator<V> values_;
        MapsParams params_;
        uint64_t label_;
    };
    /// @endcond

    /// @name Collections
    /// @{

    /**
     * @brief Generate vectors with elements from another generator.
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
            new VectorsGenerator<T>(std::move(elements), params));
    }

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
        return Generator<std::vector<T>>(new VectorsGenerator<T>(
            std::move(elements), params,
            [key = std::move(unique_by)](const T& a, const T& b) {
                return key(a) == key(b);
            }));
    }

    /**
     * @brief Generate sets with elements from another generator.
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
            new SetsGenerator<T>(std::move(elements), params));
    }

    /**
     * @brief Generate maps with configurable key and value types.
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
        return Generator<std::map<K, V>>(new MapsGenerator<K, V>(
            std::move(keys), std::move(values), params));
    }

    /// @cond INTERNAL
    namespace detail {

        template <typename Tuple, typename GenTuple, size_t... Is>
        Tuple draw_tuple_impl(const GenTuple& gens, std::index_sequence<Is...>,
                              const TestCase& tc) {
            return Tuple{std::get<Is>(gens).do_draw(tc)...};
        }

    } // namespace detail

    // Concrete IGenerator for tuples(). Draws each element in order inside
    // one tuple span.
    template <typename... Ts>
    class TuplesGenerator : public IGenerator<std::tuple<Ts...>> {
      public:
        using ResultTuple = std::tuple<Ts...>;

        explicit TuplesGenerator(Generator<Ts>... gens)
            : gens_(std::move(gens)...),
              label_(make_label(std::index_sequence_for<Ts...>{})) {}

        ResultTuple do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            hi::start_span(tc, label_);
            ResultTuple result = detail::draw_tuple_impl<ResultTuple>(
                gens_, std::index_sequence_for<Ts...>{}, tc);
            hi::stop_span(tc);
            return result;
        }

        uint64_t label() const override { return label_; }

      private:
        template <size_t... Is>
        uint64_t make_label(std::index_sequence<Is...>) const {
            namespace hi = hegel::internal;
            return hi::combine_labels({hi::label(hi::LabelName::Tuples),
                                       std::get<Is>(gens_).label()...});
        }

        std::tuple<Generator<Ts>...> gens_;
        uint64_t label_;
    };
    /// @endcond

    /**
     * @brief Generate tuples from multiple generators.
     *
     * Each generator produces one element of the resulting tuple.
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
            new TuplesGenerator<Ts...>(std::move(gens)...));
    }

    /// @cond INTERNAL
    // Concrete IGenerator for arrays(). Draws exactly N elements from one
    // element generator inside a single tuple span (fixed arity, not the
    // engine-managed collection protocol vectors() uses).
    template <typename T, size_t N>
    class ArrayGenerator : public IGenerator<std::array<T, N>> {
      public:
        explicit ArrayGenerator(Generator<T> element)
            : element_(std::move(element)),
              label_(hegel::internal::combine_labels(
                  {hegel::internal::label(hegel::internal::LabelName::Array),
                   element_.label()})) {}

        std::array<T, N> do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            hi::start_span(tc, label_);
            std::array<T, N> result =
                draw_all(tc, std::make_index_sequence<N>{});
            hi::stop_span(tc);
            return result;
        }

      private:
        // Braced-init evaluates its elements left to right, so the N draws
        // happen in order; the comma operator consumes each index.
        template <size_t... Is>
        std::array<T, N> draw_all(const TestCase& tc,
                                  std::index_sequence<Is...>) const {
            return std::array<T, N>{
                (static_cast<void>(Is), element_.do_draw(tc))...};
        }

      public:
        uint64_t label() const override { return label_; }

      private:
        Generator<T> element_;
        uint64_t label_;
    };
    /// @endcond

    /**
     * @brief Generate fixed-size arrays.
     *
     * Draws exactly @p N elements from @p element, producing a
     * @c std::array<T, N>. Unlike vectors(), the length is fixed at compile
     * time. @p N cannot be deduced, so specify it explicitly.
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
            new ArrayGenerator<T, N>(std::move(element)));
    }

    /// @cond INTERNAL
    // Concrete IGenerator for permutations(). Draws, for each position, the
    // index of one of the values not yet placed, inside an engine-managed
    // collection of exactly that many elements.
    template <typename T>
    class PermutationsGenerator : public IGenerator<std::vector<T>> {
      public:
        explicit PermutationsGenerator(std::vector<T> values)
            : values_(std::move(values)) {}

        std::vector<T> do_draw(const TestCase& tc) const override {
            namespace hi = hegel::internal;
            hi::start_span(tc, label());
            std::vector<size_t> remaining(values_.size());
            for (size_t i = 0; i < remaining.size(); ++i) {
                remaining[i] = i;
            }
            hi::CollectionHandle collection(tc, values_.size(), values_.size());
            std::vector<T> result;
            result.reserve(values_.size());
            while (!remaining.empty() && collection.more(tc)) {
                auto index = static_cast<size_t>(hi::draw_integer(
                    tc, 0, static_cast<int64_t>(remaining.size() - 1)));
                result.push_back(values_[remaining[index]]);
                remaining.erase(remaining.begin() +
                                static_cast<std::ptrdiff_t>(index));
            }
            hi::stop_span(tc);
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
     * @tparam T Element type
     * @param values The values to reorder
     * @return Generator producing reorderings of @p values
     */
    template <typename T>
    Generator<std::vector<T>> permutations(std::vector<T> values) {
        return Generator<std::vector<T>>(
            new PermutationsGenerator<T>(std::move(values)));
    }

    /**
     * @brief Generate permutations of a fixed list of values (initializer
     * list).
     *
     * @tparam T Element type
     * @param values The values to reorder
     * @return Generator producing reorderings of @p values
     */
    template <typename T>
    Generator<std::vector<T>> permutations(std::initializer_list<T> values) {
        return permutations(std::vector<T>(values));
    }

    /// @}

} // namespace hegel::generators
