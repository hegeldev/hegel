#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "hegel/core.h"

namespace hegel::generators {

    /**
     * @brief Parameters for text() generator.
     */
    struct TextParams {
        size_t min_size = 0;            ///< Minimum string length
        std::optional<size_t> max_size; ///< Maximum string length. Default:
                                        ///< 100, or min_size + 100 when
                                        ///< min_size is above 100

        // Character filtering options
        std::optional<std::string> codec;      ///< Restrict to this codec
        std::optional<uint32_t> min_codepoint; ///< Minimum Unicode codepoint
        std::optional<uint32_t> max_codepoint; ///< Maximum Unicode codepoint
        std::optional<std::vector<std::string>>
            categories; ///< Whitelist of Unicode categories
        std::optional<std::vector<std::string>>
            exclude_categories; ///< Blacklist of Unicode categories
        std::optional<std::string>
            include_characters; ///< Always include these characters
        std::optional<std::string>
            exclude_characters; ///< Always exclude these characters
        std::optional<std::string>
            alphabet; ///< Fixed set of allowed characters
    };

    /**
     * @brief Parameters for characters() generator.
     *
     * Same character filtering options as TextParams except no min_size,
     * max_size, or alphabet.
     */
    struct CharactersParams {
        std::optional<std::string> codec;      ///< Restrict to this codec
        std::optional<uint32_t> min_codepoint; ///< Minimum Unicode codepoint
        std::optional<uint32_t> max_codepoint; ///< Maximum Unicode codepoint
        std::optional<std::vector<std::string>>
            categories; ///< Whitelist of Unicode categories
        std::optional<std::vector<std::string>>
            exclude_categories; ///< Blacklist of Unicode categories
        std::optional<std::string>
            include_characters; ///< Always include these characters
        std::optional<std::string>
            exclude_characters; ///< Always exclude these characters
    };

    /**
     * @brief Parameters for binary() generator.
     */
    struct BinaryParams {
        size_t min_size = 0;            ///< Minimum size in bytes
        std::optional<size_t> max_size; ///< Maximum size in bytes. Default:
                                        ///< 100, or min_size + 100 when
                                        ///< min_size is above 100
    };

    /**
     * @brief Parameters for from_lark() generator.
     */
    struct LarkParams {
        /// The rule whose strings to generate. Default: `start`.
        std::optional<std::string> start;
        /// Terminals whose values come from a generator instead of from the
        /// grammar, as pairs of a terminal name and a generator of strings.
        /// A terminal the grammar `%declare`s without a definition needs
        /// one; any other terminal may be given one to override its
        /// pattern. The generators need not be printable: their values
        /// print as part of the whole string.
        std::vector<std::pair<std::string, AnyGenerator<std::string>>>
            explicit_terminals;
        /// The characters that terminals drawn from their patterns may
        /// contain. Default: all of Unicode.
        std::optional<CharactersParams> alphabet;
        /// Maximum string length in characters, explicit terminals' values
        /// included. Default: 8192. At most 1048576.
        std::optional<size_t> max_size;
        /// How deeply rules may nest. Default: 200. At most 500.
        std::optional<uint32_t> max_depth;
        /// The directory `%import`ed grammar files are read from. Default:
        /// none, so only Lark's bundled grammars can be imported.
        std::optional<std::string> import_path;
    };

    /// @name Strings
    /// @{

    /**
     * @brief Generate random text strings.
     * @param params Length and character filtering constraints
     * @return Generator producing random strings
     */
    Generator<std::string> text(const TextParams& params = {});

    /**
     * @brief Generate single-character UTF-8 strings.
     * @param params Character filtering constraints
     * @return Generator producing single-character strings
     */
    Generator<std::string> characters(const CharactersParams& params = {});

    /**
     * @brief Generate random binary data (byte sequences).
     * @param params Size constraints
     * @return Generator producing byte vectors
     */
    Generator<std::vector<uint8_t>> binary(BinaryParams params = {});

    /**
     * @brief Generate strings matching a regular expression.
     *
     * @code{.cpp}
     * // Default: the entire generated string matches the pattern,
     * // as if anchored with ^...$.
     * auto strict = from_regex("[A-Z]{2}-[0-9]{4}");
     * // e.g. "QX-8271"
     *
     * // fullmatch = false: generated string only needs to *contain* a match,
     * // so arbitrary prefix/suffix characters may surround it.
     * auto loose = from_regex("[A-Z]{2}-[0-9]{4}", false);
     * // e.g. "xx QX-8271 yy"
     * @endcode
     *
     * @param pattern Regex pattern
     * @param fullmatch If `true` (default), the entire generated string must
     *   match the pattern (equivalent to anchoring with `^` and `$`). If
     *   `false`, the generated string need only contain a substring that
     *   matches; arbitrary characters may appear before or after the match.
     * @return Generator producing strings that satisfy @p pattern under the
     *   selected match mode.
     */
    Generator<std::string> from_regex(const std::string& pattern,
                                      bool fullmatch = true);

    /**
     * @brief Parameters for the from_regex() overload with an alphabet.
     */
    struct RegexParams {
        /// Whether the entire generated string must match the pattern (as if
        /// anchored with `^` and `$`), or only contain a match.
        bool fullmatch = true;
        /// The characters any part of the string may contain: the
        /// characters a wildcard, a negated class or the padding around a
        /// partial match draws from, and those a literal or class may
        /// produce. Default: all of Unicode.
        std::optional<CharactersParams> alphabet;
    };

    /**
     * @brief Generate strings matching a regular expression, restricted to
     *        an alphabet.
     *
     * @code{.cpp}
     * auto ascii = from_regex(
     *     "a.c", {.alphabet = CharactersParams{.codec = "ascii"}});
     * // e.g. "a!c": the wildcard draws only ASCII characters
     * @endcode
     *
     * @param pattern Regex pattern
     * @param params Match mode and alphabet
     * @return Generator producing strings that satisfy @p pattern, every
     *   character of which the alphabet can generate.
     */
    Generator<std::string> from_regex(const std::string& pattern,
                                      const RegexParams& params);

    /**
     * @brief Generate strings of a grammar written in the syntax of the
     * [Lark](https://lark-parser.readthedocs.io/en/stable/grammar.html)
     * parsing library.
     *
     * The whole grammar language is supported: rules, terminals with
     * string and regex patterns and priorities, `%import` (including Lark's
     * bundled `common`, `python`, `unicode` and `lark` grammars),
     * `%ignore`, `%declare`, `%override`, `%extend` and templates. The
     * generated strings are ones Lark's Earley parser accepts for the
     * grammar, with `%ignore`d terminals such as whitespace and comments
     * inserted where a lexer would skip them. A string that Lark's basic
     * lexer would read differently is avoided where the grammar allows it.
     * A draw that cannot make such a string after several attempts rejects
     * the test case, as a failed TestCase::assume() does.
     *
     * The value of an explicit terminal (LarkParams::explicit_terminals)
     * is drawn from its generator for each place the terminal occurs. The
     * lexing checks do not apply to these values: the generator must make
     * values that read as the terminal where they land (a value inside a
     * quoted string must not contain a quote, for example). The values
     * count toward LarkParams::max_size. When they make a string too long,
     * Hegel draws the string again with less space for the grammar's own
     * text, and after several attempts it rejects the test case.
     *
     * `%import a.b.X` reads `a/b.lark` from LarkParams::import_path, and
     * Lark's bundled grammar of that name is used when the directory has
     * no such file. A relative import (`%import .lib.X`) in `a/b.lark`
     * reads `a/lib.lark`, and in the grammar text itself reads `lib.lark`.
     *
     * @code{.cpp}
     * const char* expressions = R"lark(
     *     ?start: sum
     *     ?sum: product | sum "+" product
     *     ?product: atom | product "*" atom
     *     ?atom: NUMBER | "(" sum ")"
     *     %import common.NUMBER
     *     %import common.WS
     *     %ignore WS
     * )lark";
     *
     * auto expression = tc.draw(from_lark(expressions));
     * @endcode
     *
     * @param grammar The grammar, in Lark's syntax
     * @param params The start rule, explicit terminals, alphabet, size
     *   limits and import directory
     * @return Generator producing strings of the grammar
     * @throws std::invalid_argument for a grammar Lark would reject, an
     *   undefined start rule, a `max_size` below the smallest string the
     *   grammar can produce, a terminal given more than one generator or
     *   that the grammar does not have, or an imported file that exists
     *   but cannot be read
     */
    Generator<std::string> from_lark(const std::string& grammar,
                                     const LarkParams& params = {});

    /// @}

} // namespace hegel::generators
