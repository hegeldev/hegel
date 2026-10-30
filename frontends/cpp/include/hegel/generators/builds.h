#pragma once

#include <tuple>

#include "hegel/core.h"

namespace hegel::generators {

    /// @cond INTERNAL
    template <typename T, bool Printable, typename... Gens>
    class BuildsGenerator : public detail::Structural<T, Printable> {
      public:
        explicit BuildsGenerator(Gens... gens)
            : gens_(std::move(gens)...),
              label_(internal::label(internal::LabelName::Composite)) {}

        uint64_t label() const override { return label_; }

      protected:
        T draw_printing(const TestCase& tc, Printer& printer) const override {
            std::string open;
            if (printer.should_print()) {
                open = internal::cpp_type_name<T>() + "(";
            }
            printer.begin_group(open.size(), open);
            Arguments arguments = std::apply(
                [&](const auto&... gens) {
                    std::size_t index = 0;
                    return Arguments{
                        draw_argument(tc, printer, gens, index++)...};
                },
                gens_);
            printer.end_group(")");
            return std::make_from_tuple<T>(std::move(arguments));
        }

      private:
        using Arguments = std::tuple<typename Gens::value_type...>;

        template <typename G>
        static typename G::value_type
        draw_argument(const TestCase& tc, Printer& printer, const G& gen,
                      std::size_t index) {
            if (index > 0) {
                printer.separator();
            }
            return detail::draw_component(tc, gen, printer);
        }

        std::tuple<Gens...> gens_;
        uint64_t label_;
    };
    /// @endcond

    /// @name Combinators
    /// @{

    /**
     * @brief Construct objects using positional constructor arguments.
     *
     * Generates constructor arguments from the provided generators and
     * calls T's constructor. The value prints as a constructor call,
     * `T(<arg>, ...)`, with each argument as its generator prints it, so
     * the result is printable when every argument generator is.
     *
     * @code{.cpp}
     * struct Point {
     *     Point(double x, double y) : x(x), y(y) {}
     *     double x, y;
     * };
     *
     * auto point = builds<Point>(
     *     floats<double>({.min_value = 0, .max_value = 100}),
     *     floats<double>({.min_value = 0, .max_value = 100})
     * );
     * @endcode
     *
     * @tparam T Type to construct
     * @tparam Gens Generator types for constructor arguments
     * @param gens Generators providing constructor arguments
     * @return Generator producing T instances
     */
    template <typename T, typename... Gens> auto builds(Gens... gens) {
        constexpr bool printable = (detail::is_printable_handle_v<Gens> && ...);
        return detail::handle_t<printable, T>(
            std::make_shared<BuildsGenerator<T, printable, Gens...>>(
                std::move(gens)...));
    }

    /**
     * @brief Helper for named field initialization in builds_agg().
     *
     * @tparam MemberPtr Pointer-to-member for the field
     * @tparam Gen Generator type for the field value
     */
    template <auto MemberPtr, typename Gen> struct Field {
        static constexpr auto member_ptr = MemberPtr; ///< The member pointer
        Gen generator; ///< Generator for the field value
    };

    /**
     * @brief Create a field specification for builds_agg().
     *
     * @code{.cpp}
     * auto rect = builds_agg<Rectangle>(
     *     field<&Rectangle::width>(
     *      integers<int>({.min_value = 1, .max_value = 100})),
     *     field<&Rectangle::height>(
     *      integers<int>({.min_value = 1, .max_value = 100}))
     * );
     * @endcode
     *
     * @tparam MemberPtr Pointer-to-member specifying which field
     * @tparam Gen Generator type
     * @param gen Generator for the field value
     * @return A Field specification for use with builds_agg()
     */
    template <auto MemberPtr, typename Gen>
    Field<MemberPtr, Gen> field(Gen gen) {
        return Field<MemberPtr, Gen>{std::move(gen)};
    }

    /**
     * @brief Construct aggregates using named field initialization.
     *
     * Useful for structs where you want to specify fields by name rather
     * than position. Each field() specifies a member pointer and generator.
     * The field draws do not print: the result prints the value it built,
     * by its C++ representation, so it is printable when
     * hegel::is_representable_v is true for T.
     * @code{.cpp}
     * struct Rectangle {
     *     int width;
     *     int height;
     * };
     *
     * auto rect = builds_agg<Rectangle>(
     *     field<&Rectangle::width>(
     *      integers<int>({.min_value = 1, .max_value = 100})),
     *     field<&Rectangle::height>(
     *      integers<int>({.min_value = 1, .max_value = 100}))
     * );
     * @endcode
     *
     * @tparam T Aggregate type to construct
     * @tparam Fields Field specification types
     * @param fields Field specifications from field()
     * @return Generator producing T instances
     */
    template <typename T, typename... Fields>
    auto builds_agg(Fields... fields) {
        auto gen_tuple = std::make_tuple(std::move(fields)...);

        return compose([gen_tuple](const TestCase& tc) mutable -> T {
            T result{};
            std::apply(
                [&result, &tc](auto&... fs) {
                    ((result.*
                          (std::remove_reference_t<decltype(fs)>::member_ptr) =
                          tc.draw_silent(fs.generator)),
                     ...);
                },
                gen_tuple);
            return result;
        });
    }

    /// @}

} // namespace hegel::generators
