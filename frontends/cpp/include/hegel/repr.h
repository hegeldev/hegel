#pragma once

/**
 * @cond INTERNAL
 */

// Prints drawn values as C++ expressions through the engine's printer, for
// draw-output lines. Types the library cannot render as a compilable
// expression fall back to the text of their stream operator, which the
// engine reflows, then to an "<unprintable Type>" placeholder.

#include "config.h"
#include "printer.h"

#include <array>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <limits>
#include <map>
#include <optional>
#include <set>
#include <sstream>
#include <string>
#include <string_view>
#include <tuple>
#include <type_traits>
#include <utility>
#include <variant>
#include <vector>

#if HEGEL_HAS_REFLECTION
#include <rfl.hpp>
#endif

namespace hegel::internal {

    template <typename T> std::string cpp_type_name();
    template <typename T> void print_value(Printer& printer, const T& value);

    // Best-effort type name sliced from the compiler's pretty function
    // string. Used for placeholder output and for types with no clean
    // spelling of their own.
    template <typename T> constexpr std::string_view compiler_type_name() {
#if defined(__clang__)
        std::string_view p = __PRETTY_FUNCTION__;
        auto start = p.find("T = ");
        auto end = p.rfind(']');
        if (start == std::string_view::npos || end == std::string_view::npos) {
            return "?"; // GCOVR_EXCL_LINE - guards a compiler format change
        }
        start += 4;
        return p.substr(start, end - start);
#elif defined(__GNUC__)
        std::string_view p = __PRETTY_FUNCTION__;
        auto start = p.find("T = ");
        if (start == std::string_view::npos) {
            return "?";
        }
        start += 4;
        auto end = p.find(';', start);
        if (end == std::string_view::npos) {
            end = p.rfind(']');
        }
        if (end == std::string_view::npos) {
            return "?";
        }
        return p.substr(start, end - start);
#elif defined(_MSC_VER)
        std::string_view p = __FUNCSIG__;
        constexpr std::string_view marker = "compiler_type_name<";
        auto start = p.find(marker);
        auto end = p.rfind(">(");
        if (start == std::string_view::npos || end == std::string_view::npos) {
            return "?";
        }
        start += marker.size();
        return p.substr(start, end - start);
#else
        return "?";
#endif
    }

    namespace repr_detail {

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

        template <typename T> struct is_pair : std::false_type {};
        template <typename A, typename B>
        struct is_pair<std::pair<A, B>> : std::true_type {};

        template <typename T> struct is_tuple : std::false_type {};
        template <typename... Ts>
        struct is_tuple<std::tuple<Ts...>> : std::true_type {};

        template <typename T> struct is_variant : std::false_type {};
        template <typename... Ts>
        struct is_variant<std::variant<Ts...>> : std::true_type {};

        template <typename T> struct is_std_array : std::false_type {};
        template <typename E, std::size_t N>
        struct is_std_array<std::array<E, N>> : std::true_type {};

        template <typename T> struct array_traits;
        template <typename E, std::size_t N>
        struct array_traits<std::array<E, N>> {
            using element_type = E;
            static constexpr std::size_t size = N;
        };

        template <typename T, typename = void>
        struct prints_itself : std::false_type {};
        template <typename T>
        struct prints_itself<
            T, std::void_t<decltype(std::declval<const T&>().print_to(
                   std::declval<Printer&>()))>> : std::true_type {};

        template <typename T, typename = void>
        struct has_ostream_operator : std::false_type {};
        template <typename T>
        struct has_ostream_operator<
            T, std::void_t<decltype(std::declval<std::ostream&>()
                                    << std::declval<const T&>())>>
            : std::true_type {};

#if HEGEL_HAS_REFLECTION
        template <typename T, typename = void>
        struct is_reflectable_impl : std::false_type {};
        template <typename T>
        struct is_reflectable_impl<
            T, std::void_t<decltype(rfl::to_view(std::declval<T&>()))>>
            : std::true_type {};

        // The aggregate check must come first: rfl::to_view hard-errors on
        // non-aggregates instead of failing substitution.
        template <typename T>
        struct is_reflectable
            : std::conjunction<std::is_aggregate<T>, is_reflectable_impl<T>> {};
#endif

        template <typename... Ts> std::string type_name_list() {
            if constexpr (sizeof...(Ts) == 0) {
                return "";
            } else {
                std::string names[] = {cpp_type_name<Ts>()...};
                std::string out;
                for (std::size_t i = 0; i < sizeof...(Ts); ++i) {
                    if (i != 0) {
                        out += ", ";
                    }
                    out += names[i];
                }
                return out;
            }
        }

        template <typename T> struct template_args;
        template <typename... Ts> struct template_args<std::tuple<Ts...>> {
            static std::string names() { return type_name_list<Ts...>(); }
        };
        template <typename... Ts> struct template_args<std::variant<Ts...>> {
            static std::string names() { return type_name_list<Ts...>(); }
        };

        // Appends one byte as it must appear inside a char or string
        // literal: named escapes, printable ASCII verbatim, all other
        // bytes as a fixed three-digit octal escape (a following literal
        // digit can never extend a three-digit octal escape).
        inline void append_escaped(std::string& out, unsigned char c,
                                   bool in_string) {
            switch (c) {
            case '\n':
                out += "\\n";
                return;
            case '\t':
                out += "\\t";
                return;
            case '\r':
                out += "\\r";
                return;
            case '\\':
                out += "\\\\";
                return;
            case '"':
                if (in_string) {
                    out += "\\\"";
                    return;
                }
                break;
            case '\'':
                if (!in_string) {
                    out += "\\'";
                    return;
                }
                break;
            default:
                break;
            }
            if (c >= 0x20 && c < 0x7f) {
                out += static_cast<char>(c);
                return;
            }
            char buf[8];
            std::snprintf(buf, sizeof(buf), "\\%03o", c);
            out += buf;
        }

    } // namespace repr_detail

    // C++ source spelling of T, for use inside rendered expressions.
    template <typename T> std::string cpp_type_name() {
        if constexpr (std::is_same_v<T, bool>) {
            return "bool";
        } else if constexpr (std::is_same_v<T, char>) {
            return "char";
        } else if constexpr (std::is_same_v<T, signed char>) {
            return "signed char";
        } else if constexpr (std::is_same_v<T, unsigned char>) {
            return "unsigned char";
        } else if constexpr (std::is_same_v<T, short>) {
            return "short";
        } else if constexpr (std::is_same_v<T, unsigned short>) {
            return "unsigned short";
        } else if constexpr (std::is_same_v<T, int>) {
            return "int";
        } else if constexpr (std::is_same_v<T, unsigned int>) {
            return "unsigned int";
        } else if constexpr (std::is_same_v<T, long>) {
            return "long";
        } else if constexpr (std::is_same_v<T, unsigned long>) {
            return "unsigned long";
        } else if constexpr (std::is_same_v<T, long long>) {
            return "long long";
        } else if constexpr (std::is_same_v<T, unsigned long long>) {
            return "unsigned long long";
        } else if constexpr (std::is_same_v<T, float>) {
            return "float";
        } else if constexpr (std::is_same_v<T, double>) {
            return "double";
        } else if constexpr (std::is_same_v<T, long double>) {
            return "long double";
        } else if constexpr (std::is_same_v<T, std::string>) {
            return "std::string";
        } else if constexpr (std::is_same_v<T, std::monostate>) {
            return "std::monostate";
        } else if constexpr (repr_detail::is_vector<T>::value) {
            return "std::vector<" + cpp_type_name<typename T::value_type>() +
                   ">";
        } else if constexpr (repr_detail::is_set<T>::value) {
            return "std::set<" + cpp_type_name<typename T::value_type>() + ">";
        } else if constexpr (repr_detail::is_map<T>::value) {
            return "std::map<" + cpp_type_name<typename T::key_type>() + ", " +
                   cpp_type_name<typename T::mapped_type>() + ">";
        } else if constexpr (repr_detail::is_optional<T>::value) {
            return "std::optional<" + cpp_type_name<typename T::value_type>() +
                   ">";
        } else if constexpr (repr_detail::is_pair<T>::value) {
            return "std::pair<" + cpp_type_name<typename T::first_type>() +
                   ", " + cpp_type_name<typename T::second_type>() + ">";
        } else if constexpr (repr_detail::is_tuple<T>::value) {
            return "std::tuple<" + repr_detail::template_args<T>::names() + ">";
        } else if constexpr (repr_detail::is_variant<T>::value) {
            return "std::variant<" + repr_detail::template_args<T>::names() +
                   ">";
        } else if constexpr (repr_detail::is_std_array<T>::value) {
            using traits = repr_detail::array_traits<T>;
            return "std::array<" +
                   cpp_type_name<typename traits::element_type>() + ", " +
                   std::to_string(traits::size) + ">";
        } else {
            return std::string(compiler_type_name<T>());
        }
    }

    namespace repr_detail {

        // Separates the items of a group with a comma and a break point.
        class ItemSeparator {
          public:
            explicit ItemSeparator(Printer& printer) : printer_(printer) {}

            // Call before each item.
            void operator()() {
                if (!first_) {
                    printer_.text(",");
                    printer_.breakable(" ");
                }
                first_ = false;
            }

          private:
            Printer& printer_;
            bool first_ = true;
        };

        // Prints `open`, the items `body` prints, and `close`: on one line
        // when they fit, else with one item per line, indented four columns,
        // and `close` on a line of its own.
        template <typename F>
        void print_block(Printer& printer, const std::string& open,
                         const std::string& close, F&& body) {
            constexpr int64_t indent = 4;
            printer.begin_group(static_cast<uint64_t>(indent), open);
            printer.breakable("");
            ItemSeparator separator(printer);
            body(separator);
            printer.shift_indent(-indent);
            printer.breakable("");
            printer.shift_indent(indent);
            printer.end_group(close);
        }

        template <typename Items>
        void print_sequence(Printer& printer, const std::string& open,
                            const Items& items) {
            print_block(printer, open, "}",
                        [&printer, &items](ItemSeparator& separator) {
                            for (const auto& item : items) {
                                separator();
                                print_value(printer, item);
                            }
                        });
        }

        template <typename Tuple>
        void print_tuple(Printer& printer, const std::string& open,
                         const Tuple& value) {
            print_block(printer, open, "}",
                        [&printer, &value](ItemSeparator& separator) {
                            std::apply(
                                [&](const auto&... elems) {
                                    ((separator(), print_value(printer, elems)),
                                     ...);
                                },
                                value);
                        });
        }

        // Decimal digits of an integer of any width.
        template <typename T> std::string integer_digits(T value) {
            using U = std::make_unsigned_t<T>;
            auto magnitude = static_cast<U>(value);
            bool negative = false;
            if constexpr (std::is_signed_v<T>) {
                negative = value < 0;
                if (negative) {
                    magnitude = static_cast<U>(U{0} - magnitude);
                }
            }
            std::string reversed;
            do {
                reversed.push_back(static_cast<char>('0' + magnitude % 10));
                magnitude /= 10;
            } while (magnitude != 0);
            if (negative) {
                reversed.push_back('-');
            }
            return std::string(reversed.rbegin(), reversed.rend());
        }

        // The text of a value with no inner structure.
        template <typename T> std::string scalar_text(const T& value) {
            if constexpr (std::is_same_v<T, bool>) {
                return value ? "true" : "false";
            } else if constexpr (std::is_same_v<T, char>) {
                std::string out = "'";
                append_escaped(out, static_cast<unsigned char>(value),
                               /*in_string=*/false);
                out += "'";
                return out;
            } else if constexpr (std::is_same_v<T, signed char> ||
                                 std::is_same_v<T, unsigned char>) {
                return "static_cast<" + cpp_type_name<T>() + ">(" +
                       std::to_string(static_cast<int>(value)) + ")";
            } else if constexpr (std::is_same_v<T, int>) {
                return std::to_string(value);
            } else if constexpr (std::is_same_v<T, unsigned int>) {
                return std::to_string(value) + "u";
            } else if constexpr (std::is_same_v<T, long>) {
                return std::to_string(value) + "l";
            } else if constexpr (std::is_same_v<T, unsigned long>) {
                return std::to_string(value) + "ul";
            } else if constexpr (std::is_same_v<T, long long>) {
                return std::to_string(value) + "ll";
            } else if constexpr (std::is_same_v<T, unsigned long long>) {
                return std::to_string(value) + "ull";
            } else if constexpr (std::is_integral_v<T>) {
                // Integral types with no literal suffix of their own.
                return "static_cast<" + cpp_type_name<T>() + ">(" +
                       integer_digits(value) + ")";
            } else if constexpr (std::is_enum_v<T>) {
                using underlying = std::underlying_type_t<T>;
                return "static_cast<" + cpp_type_name<T>() + ">(" +
                       scalar_text(static_cast<underlying>(value)) + ")";
            } else if constexpr (std::is_floating_point_v<T>) {
                if (std::isnan(value)) {
                    return "std::numeric_limits<" + cpp_type_name<T>() +
                           ">::quiet_NaN()";
                }
                if (std::isinf(value)) {
                    std::string inf = "std::numeric_limits<" +
                                      cpp_type_name<T>() + ">::infinity()";
                    return value < 0 ? "-" + inf : inf;
                }
                char buf[64];
                constexpr int digits = std::numeric_limits<T>::max_digits10;
                if constexpr (std::is_same_v<T, long double>) {
                    std::snprintf(buf, sizeof(buf), "%.*Lg", digits, value);
                } else {
                    std::snprintf(buf, sizeof(buf), "%.*g", digits,
                                  static_cast<double>(value));
                }
                std::string out = buf;
                // keep integral-valued float rendering as float since %g
                // drops decimal when it isn't needed.
                if (out.find('.') == std::string::npos &&
                    out.find('e') == std::string::npos &&
                    out.find('E') == std::string::npos) {
                    out += ".0";
                }
                if constexpr (std::is_same_v<T, float>) {
                    out += "f";
                } else if constexpr (std::is_same_v<T, long double>) {
                    out += "L";
                }
                return out;
            } else if constexpr (std::is_same_v<T, std::string>) {
                bool has_nul = value.find('\0') != std::string::npos;
                std::string out = "std::string(\"";
                for (char c : value) {
                    append_escaped(out, static_cast<unsigned char>(c),
                                   /*in_string=*/true);
                }
                out += "\"";
                if (has_nul) {
                    // The pointer constructor would stop at the first NUL.
                    out += ", " + std::to_string(value.size());
                }
                out += ")";
                return out;
            } else {
                static_assert(std::is_same_v<T, std::monostate>);
                return "std::monostate{}";
            }
        }

        template <typename T>
        inline constexpr bool is_scalar_v =
            std::is_arithmetic_v<T> || std::is_enum_v<T> ||
            std::is_same_v<T, std::string> || std::is_same_v<T, std::monostate>;

    } // namespace repr_detail

    // Prints a value as one C++ expression, or as a pseudo-expression for
    // types with no renderable form. A compound value gives the printer its
    // structure, so the engine can lay it out across lines.
    template <typename T> void print_value(Printer& printer, const T& value) {
        using repr_detail::ItemSeparator;
        if constexpr (repr_detail::is_scalar_v<T>) {
            printer.text(repr_detail::scalar_text(value));
        } else if constexpr (repr_detail::prints_itself<T>::value) {
            value.print_to(printer);
        } else if constexpr (repr_detail::is_pair<T>::value ||
                             repr_detail::is_tuple<T>::value) {
            repr_detail::print_tuple(printer, cpp_type_name<T>() + "{", value);
        } else if constexpr (repr_detail::is_optional<T>::value) {
            std::string name = cpp_type_name<T>();
            if (!value.has_value()) {
                printer.text(name + "{}");
                return;
            }
            repr_detail::print_block(printer, name + "{", "}",
                                     [&printer, &value](ItemSeparator&) {
                                         print_value(printer, *value);
                                     });
        } else if constexpr (repr_detail::is_variant<T>::value) {
            // in_place_index keeps variants with repeated alternative types
            // unambiguous.
            repr_detail::print_block(
                printer, cpp_type_name<T>() + "{", "}",
                [&printer, &value](ItemSeparator& separator) {
                    separator();
                    printer.text("std::in_place_index<" +
                                 std::to_string(value.index()) + ">");
                    separator();
                    std::visit(
                        [&printer](const auto& v) { print_value(printer, v); },
                        value);
                });
        } else if constexpr (repr_detail::is_vector<T>::value ||
                             repr_detail::is_set<T>::value ||
                             repr_detail::is_std_array<T>::value) {
            repr_detail::print_sequence(printer, cpp_type_name<T>() + "{",
                                        value);
        } else if constexpr (repr_detail::is_map<T>::value) {
            repr_detail::print_block(
                printer, cpp_type_name<T>() + "{", "}",
                [&printer, &value](ItemSeparator& separator) {
                    for (const auto& entry : value) {
                        separator();
                        repr_detail::print_block(
                            printer, "{", "}",
                            [&printer, &entry](ItemSeparator& inner) {
                                inner();
                                print_value(printer, entry.first);
                                inner();
                                print_value(printer, entry.second);
                            });
                    }
                });
        }
#if HEGEL_HAS_REFLECTION
        else if constexpr (repr_detail::is_reflectable<T>::value) {
            repr_detail::print_block(
                printer, cpp_type_name<T>() + "{", "}",
                [&printer, &value](ItemSeparator& separator) {
                    auto view = rfl::to_view(const_cast<T&>(value));
                    view.apply([&printer, &separator](const auto& field) {
                        separator();
                        printer.text("." + std::string(field.name()) + " = ");
                        print_value(printer, *field.value());
                    });
                });
        }
#endif
        else if constexpr (repr_detail::has_ostream_operator<T>::value) {
            std::ostringstream os;
            os << value;
            printer.reflow(os.str());
        } else {
            printer.text("<unprintable " +
                         std::string(compiler_type_name<T>()) + ">");
        }
    }

    // Renders a value as one C++ expression laid out within `width`
    // columns.
    template <typename T> std::string render(const T& value, uint64_t width) {
        Document document(width);
        Printer printer = document.printer();
        print_value(printer, value);
        return document.finish();
    }

    // Renders a value as one C++ expression on a single line.
    template <typename T> std::string repr(const T& value) {
        return render(value, unbounded_width);
    }

} // namespace hegel::internal

/// @endcond
