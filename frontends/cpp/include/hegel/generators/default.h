#pragma once

#include "hegel/config.h"

// default_generator (type-directed derivation) is the one feature that needs
// reflect-cpp. Building with HEGEL_REFLECTION=OFF drops it so the rest of the
// library can be consumed from C++17.
#if HEGEL_HAS_REFLECTION

#include <map>
#include <optional>
#include <set>
#include <variant>
#include <vector>

#include <rfl.hpp>

#include "hegel/core.h"
#include "hegel/generators/builds.h"
#include "hegel/generators/collections.h"
#include "hegel/generators/combinators.h"
#include "hegel/generators/numeric.h"
#include "hegel/generators/primitives.h"
#include "hegel/generators/strings.h"

namespace hegel::generators {

    /// @name Typing
    /// @{

    template <typename T, bool Printable = true> class DerivedGenerator;

    // Forward declaration — defined below after all specializations
    template <typename T> DerivedGenerator<T> default_generator();

    /// @cond INTERNAL
    namespace detail {

        // =================================================================
        // Type traits for container detection
        // =================================================================

        template <typename T> struct is_vector : std::false_type {};
        template <typename T>
        struct is_vector<std::vector<T>> : std::true_type {};

        template <typename T> struct is_set : std::false_type {};
        template <typename T> struct is_set<std::set<T>> : std::true_type {};

        template <typename T> struct is_map : std::false_type {};
        template <typename K, typename V>
        struct is_map<std::map<K, V>> : std::true_type {};

        template <typename T> struct is_optional : std::false_type {};
        template <typename T>
        struct is_optional<std::optional<T>> : std::true_type {};

        template <typename T> struct is_tuple : std::false_type {};
        template <typename... Ts>
        struct is_tuple<std::tuple<Ts...>> : std::true_type {};

        template <typename T> struct is_variant : std::false_type {};
        template <typename... Ts>
        struct is_variant<std::variant<Ts...>> : std::true_type {};

        // =================================================================
        // Reflectable struct detection
        // =================================================================

        template <typename T, typename = void>
        struct is_reflectable_struct : std::false_type {};

        template <typename T>
        struct is_reflectable_struct<
            T, std::void_t<decltype(rfl::to_view(std::declval<T&>()))>>
            : std::true_type {};

        // =================================================================
        // DefaultGenerator trait — primary template (struct fallback)
        // =================================================================

        template <typename Needle, typename T, typename Seen = std::tuple<>>
        struct mentions;

        template <typename T>
        using FieldOverride = std::function<bool(const TestCase&, T& result,
                                                 const void* field, Printer&)>;

        template <typename T>
        using FieldOverrides = std::vector<FieldOverride<T>>;

        template <typename T, bool Printable>
        class StructGenerator : public Structural<T, Printable> {
          public:
            explicit StructGenerator(
                std::shared_ptr<const FieldOverrides<T>> overrides)
                : overrides_(std::move(overrides)),
                  label_(internal::label(internal::LabelName::Composite)) {}

            uint64_t label() const override { return label_; }

          protected:
            T draw_printing(const TestCase& tc,
                            Printer& printer) const override {
                T result{};
                printer.begin_block(open_text<T>(printer, "{"));
                bool first = true;
                auto view = rfl::to_view(result);
                view.apply([&](const auto& field) {
                    using PtrType =
                        typename std::remove_cvref_t<decltype(field)>::Type;
                    using FieldType = std::remove_pointer_t<PtrType>;
                    if (!first) {
                        printer.separator();
                    }
                    first = false;
                    if (printer.should_print()) {
                        printer.text("." + std::string(field.name()) + " = ");
                    }
                    if (!draw_override(tc, result, field.value(), printer)) {
                        if constexpr (mentions<T, FieldType>::value) {
                            throw internal::UsageError(
                                "default_generator<" +
                                internal::cpp_type_name<T>() +
                                ">(): the field " + std::string(field.name()) +
                                " refers back to " +
                                internal::cpp_type_name<T>() +
                                ", so deriving its generator would recurse "
                                "without bound. Build the generator with "
                                "recursive() or deferred<T>(), or give the "
                                "field a generator with .override(field<&" +
                                internal::cpp_type_name<T>() +
                                "::" + std::string(field.name()) + ">(...))");
                        } else {
                            *field.value() = tc.draw_and_print(
                                default_generator<FieldType>(), printer);
                        }
                    }
                });
                printer.end_block("}");
                return result;
            }

          private:
            bool draw_override(const TestCase& tc, T& result, const void* field,
                               Printer& printer) const {
                for (auto it = overrides_->rbegin(); it != overrides_->rend();
                     ++it) {
                    if ((*it)(tc, result, field, printer)) {
                        return true;
                    }
                }
                return false;
            }

            std::shared_ptr<const FieldOverrides<T>> overrides_;
            uint64_t label_;
        };

        template <typename T, typename Enable = void> struct DefaultGenerator {
            static_assert(
                is_reflectable_struct<T>::value,
                "default_generator<T>(): T must be a supported primitive, "
                "container, or reflectable struct");

            static constexpr bool is_struct = true;

            static Generator<T> generator() {
                return Generator<T>(std::make_shared<StructGenerator<T, true>>(
                    std::make_shared<const FieldOverrides<T>>()));
            }
        };

        // =================================================================
        // Specializations for primitive types
        // =================================================================

        template <> struct DefaultGenerator<bool> {
            static Generator<bool> generator() { return booleans(); }
        };

        template <> struct DefaultGenerator<std::string> {
            static Generator<std::string> generator() { return text(); }
        };

        template <> struct DefaultGenerator<std::monostate> {
            static Generator<std::monostate> generator() {
                return just(std::monostate{});
            }
        };

        template <typename T>
        struct DefaultGenerator<T, std::enable_if_t<std::is_integral_v<T> &&
                                                    !std::is_same_v<T, bool>>> {
            static Generator<T> generator() { return integers<T>(); }
        };

        template <typename T>
        struct DefaultGenerator<T,
                                std::enable_if_t<std::is_floating_point_v<T>>> {
            static Generator<T> generator() { return floats<T>(); }
        };

        // =================================================================
        // Specializations for standard containers
        // =================================================================

        template <typename T>
        struct DefaultGenerator<T, std::enable_if_t<is_vector<T>::value>> {
            static Generator<T> generator() {
                using Elem = typename T::value_type;
                return vectors(default_generator<Elem>());
            }
        };

        template <typename T>
        struct DefaultGenerator<T, std::enable_if_t<is_set<T>::value>> {
            static Generator<T> generator() {
                using Elem = typename T::value_type;
                return sets(default_generator<Elem>());
            }
        };

        template <typename T>
        struct DefaultGenerator<T, std::enable_if_t<is_map<T>::value>> {
            static Generator<T> generator() {
                using K = typename T::key_type;
                using V = typename T::mapped_type;
                return maps(default_generator<K>(), default_generator<V>());
            }
        };

        template <typename T>
        struct DefaultGenerator<T, std::enable_if_t<is_optional<T>::value>> {
            static Generator<T> generator() {
                using Inner = typename T::value_type;
                return optional(default_generator<Inner>());
            }
        };

        // --- std::tuple<Ts...> ---

        template <typename Tuple, size_t... Is>
        auto make_default_tuple_gen(std::index_sequence<Is...>) {
            return tuples(
                default_generator<std::tuple_element_t<Is, Tuple>>()...);
        }

        template <typename T>
        struct DefaultGenerator<T, std::enable_if_t<is_tuple<T>::value>> {
            static Generator<T> generator() {
                return make_default_tuple_gen<T>(
                    std::make_index_sequence<std::tuple_size_v<T>>{});
            }
        };

        // --- std::variant<Ts...> ---

        template <typename Variant, size_t... Is>
        auto make_default_variant_gen(std::index_sequence<Is...>) {
            return variant(default_generator<
                           std::variant_alternative_t<Is, Variant>>()...);
        }

        template <typename T>
        struct DefaultGenerator<T, std::enable_if_t<is_variant<T>::value>> {
            static Generator<T> generator() {
                return make_default_variant_gen<T>(
                    std::make_index_sequence<std::variant_size_v<T>>{});
            }
        };

        template <typename T, typename = void>
        struct is_struct_default : std::false_type {};
        template <typename T>
        struct is_struct_default<
            T, std::void_t<decltype(DefaultGenerator<T>::is_struct)>>
            : std::true_type {};

        template <typename T, typename Seen> struct seen_contains;
        template <typename T, typename... Ss>
        struct seen_contains<T, std::tuple<Ss...>>
            : std::bool_constant<(std::is_same_v<T, Ss> || ...)> {};

        template <typename T, typename Seen> struct seen_with;
        template <typename T, typename... Ss>
        struct seen_with<T, std::tuple<Ss...>> {
            using type = std::tuple<T, Ss...>;
        };

        template <typename Needle, typename Seen, typename Pack>
        struct any_mentions;
        template <typename Needle, typename Seen, typename... Ts>
        struct any_mentions<Needle, Seen, std::tuple<Ts...>>
            : std::bool_constant<(mentions<Needle, Ts, Seen>::value || ...)> {};
        template <typename Needle, typename Seen, typename... Ts>
        struct any_mentions<Needle, Seen, std::variant<Ts...>>
            : std::bool_constant<(mentions<Needle, Ts, Seen>::value || ...)> {};
        template <typename Needle, typename Seen, typename... Fields>
        struct any_mentions<Needle, Seen, rfl::NamedTuple<Fields...>>
            : std::bool_constant<(
                  mentions<Needle, std::remove_pointer_t<typename Fields::Type>,
                           Seen>::value ||
                  ...)> {};

        template <typename Needle, typename T, typename Seen, typename = void>
        struct mentions_inside : std::false_type {};
        template <typename Needle, typename E, typename Seen>
        struct mentions_inside<Needle, std::vector<E>, Seen>
            : mentions<Needle, E, Seen> {};
        template <typename Needle, typename E, typename Seen>
        struct mentions_inside<Needle, std::set<E>, Seen>
            : mentions<Needle, E, Seen> {};
        template <typename Needle, typename E, typename Seen>
        struct mentions_inside<Needle, std::optional<E>, Seen>
            : mentions<Needle, E, Seen> {};
        template <typename Needle, typename K, typename V, typename Seen>
        struct mentions_inside<Needle, std::map<K, V>, Seen>
            : std::bool_constant<mentions<Needle, K, Seen>::value ||
                                 mentions<Needle, V, Seen>::value> {};
        template <typename Needle, typename... Ts, typename Seen>
        struct mentions_inside<Needle, std::tuple<Ts...>, Seen>
            : any_mentions<Needle, Seen, std::tuple<Ts...>> {};
        template <typename Needle, typename... Ts, typename Seen>
        struct mentions_inside<Needle, std::variant<Ts...>, Seen>
            : any_mentions<Needle, Seen, std::variant<Ts...>> {};
        template <typename Needle, typename T, typename Seen>
        struct mentions_inside<Needle, T, Seen,
                               std::enable_if_t<is_struct_default<T>::value>>
            : std::conditional_t<
                  seen_contains<T, Seen>::value, std::false_type,
                  any_mentions<Needle, typename seen_with<T, Seen>::type,
                               decltype(rfl::to_view(std::declval<T&>()))>> {};

        template <typename Needle, typename T, typename Seen>
        struct mentions
            : std::conditional_t<std::is_same_v<Needle, T>, std::true_type,
                                 mentions_inside<Needle, T, Seen>> {};

    } // namespace detail
    /// @endcond

    // =============================================================================
    // Public API
    // =============================================================================

    /**
     * @brief A Generator produced by default_generator<T>().
     *
     * Behaves exactly like a Generator<T> (it publicly derives from one), but
     * additionally exposes an override() method for supplying per-field
     * generator overrides on struct types.
     *
     * A struct prints field by field, as `T{.a = <field>, .b = <field>}`,
     * with each field as its generator prints it.
     *
     * @code{.cpp}
     * auto gen = default_generator<Rectangle>()
     *     .override(field<&Rectangle::width>(integers<int>({.min_value = 1})));
     * @endcode
     *
     * @tparam T The type produced by the generator
     * @tparam Printable Whether the generator is printable. It derives from
     *   Generator<T> when true, else from AnyGenerator<T>.
     */
    template <typename T, bool Printable>
    class DerivedGenerator : public detail::handle_t<Printable, T> {
      public:
        /// @cond INTERNAL
        DerivedGenerator(detail::handle_t<Printable, T> base,
                         std::shared_ptr<const detail::FieldOverrides<T>>
                             overrides = nullptr)
            : detail::handle_t<Printable, T>(std::move(base)),
              overrides_(std::move(overrides)) {}
        /// @endcond

        /**
         * @brief Override default per-field generators.
         *
         * Each field specification (from `field<&T::member>(gen)`) replaces
         * the default generator for that member. Other fields keep the
         * defaults from default_generator<T>(). T must be a struct. The
         * result is printable when this generator and every override
         * generator are.
         *
         * @code{.cpp}
         * auto gen = default_generator<Person>()
         *     .override(field<&Person::age>(
         *         integers<int>({.min_value = 0, .max_value = 120})));
         * @endcode
         *
         * @tparam Fields Field specification types (from field<>())
         * @param fields Field specifications for overridden members
         * @return A DerivedGenerator<T> with the overrides applied
         */
        template <typename... Fields> auto override(Fields... fields) const {
            static_assert(detail::is_struct_default<T>::value,
                          "override() needs default_generator<T>() of a "
                          "reflectable struct T");
            constexpr bool printable =
                Printable &&
                (detail::is_printable_handle_v<decltype(fields.generator)> &&
                 ...);
            auto overrides =
                std::make_shared<detail::FieldOverrides<T>>(*overrides_);
            (overrides->push_back(field_override(std::move(fields))), ...);
            return DerivedGenerator<T, printable>(
                detail::handle_t<printable, T>(
                    std::make_shared<detail::StructGenerator<T, printable>>(
                        overrides)),
                overrides);
        }

      private:
        template <typename Field>
        static detail::FieldOverride<T> field_override(Field field) {
            return [generator = std::move(field.generator)](
                       const TestCase& tc, T& result, const void* address,
                       Printer& printer) {
                auto& member = result.*(Field::member_ptr);
                if (static_cast<const void*>(&member) != address) {
                    return false;
                }
                member = detail::draw_component(tc, generator, printer);
                return true;
            };
        }

        std::shared_ptr<const detail::FieldOverrides<T>> overrides_;
    };

    /**
     * @brief Create a default generator for type T.
     *
     * Dispatches to the appropriate built-in generator based on the type:
     * - Primitive types: bool, integers, floats, std::string
     * - Containers: vector, set, map, optional, tuple, variant
     * - Reflected structs: any struct with public fields (via reflect-cpp)
     *
     * For structs, each field is generated using default_generator for its
     * type. Call `.override(...)` on the returned generator to customize
     * individual fields:
     *
     * @code{.cpp}
     * struct Person { std::string name; int age; };
     *
     * auto gen = default_generator<Person>()
     *     .override(field<&Person::age>(
     *         integers<int>({.min_value = 0, .max_value = 120})));
     * @endcode
     *
     * @tparam T The type to generate
     * @return A DerivedGenerator<T> (usable anywhere a Generator<T> is)
     */
    template <typename T> DerivedGenerator<T> default_generator() {
        if constexpr (detail::is_struct_default<T>::value) {
            return DerivedGenerator<T>(
                detail::DefaultGenerator<T>::generator(),
                std::make_shared<const detail::FieldOverrides<T>>());
        } else {
            return DerivedGenerator<T>(
                detail::DefaultGenerator<T>::generator());
        }
    }

    /// @}

} // namespace hegel::generators

#endif // HEGEL_HAS_REFLECTION
