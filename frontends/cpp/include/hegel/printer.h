#pragma once

#include <cstddef>
#include <cstdint>
#include <functional>
#include <limits>
#include <string>
#include <string_view>
#include <utility>

/// @cond INTERNAL
struct hegel_printer_t;
/// @endcond

namespace hegel {

    class Speculation;

    /**
     * @brief Writes to the document that a failure report shows.
     *
     * When a test fails, Hegel prints each drawn value as a C++ declaration,
     * `auto x = <value>;`. A printable generator writes the `<value>` part
     * itself, through a Printer, while it draws: see
     * generators::IPrintableGenerator and generators::AnyGenerator::print_with.
     *
     * Hegel for C++ only says what to print and where a line can break.
     * libhegel lays the document out: a group fits on one line, or each of
     * its break points becomes a new line.
     *
     * - text() writes text that does not break.
     * - breakable() writes a point that is @p separator when its group fits
     *   on one line, and a new line when the group breaks.
     * - begin_group() / end_group() (or group()) make a group.
     *
     * @code{.cpp}
     * hegel::Document document(12);
     * document.printer().seq("{", "}", std::vector<int>{1000, 2000, 3000},
     *                        [](hegel::Printer& p, int n) { p.repr(n); });
     * std::string out = std::move(document).finish();
     * // {
     * //     1000,
     * //     2000,
     * //     3000
     * // }
     * @endcode
     *
     * A Printer is move-only and has one owner (one thread) at a time. A
     * no-op printer (noop()) discards everything, which lets one drawing
     * body serve a silent draw and a printed draw. A printer whose region
     * is dead (see deferred()) also discards everything. A misuse of the
     * protocol, such as end_group() with no open group, throws
     * std::logic_error.
     */
    class Printer {
      public:
        /**
         * @brief A printer that discards everything.
         * @return A printer whose should_print() is false.
         */
        static Printer noop();

        /**
         * @brief Move-constructs, transferring the region.
         * @param other The printer to move from. It becomes a no-op printer.
         */
        Printer(Printer&& other) noexcept;
        /**
         * @brief Move-assigns, transferring the region.
         * @param other The printer to move from. It takes the region this
         *   printer had, and releases it when it is destroyed.
         * @return Reference to this printer.
         */
        Printer& operator=(Printer&& other) noexcept;
        Printer(const Printer&) = delete;
        Printer& operator=(const Printer&) = delete;
        ~Printer();

        /**
         * @brief Whether printing to this printer gives output.
         *
         * False for noop() and for a dead region. Use it to skip work, such
         * as formatting, whose only purpose is to be printed.
         *
         * @return Whether writes to this printer are kept.
         */
        bool should_print() const;

        /**
         * @brief Write text that does not break.
         *
         * Each newline in @p text is a hard_break().
         *
         * @param text The text to write.
         */
        void text(std::string_view text);

        /**
         * @brief Write the C++ representation of a value.
         *
         * The representation is a C++ expression (`std::vector<int>{1, 2}`,
         * `std::string("a")`, `1.5f`) with groups and break points, so the
         * engine can break a wide value across lines. A type prints itself
         * through a `void print_to(hegel::Printer&) const` member, a
         * reflectable aggregate (C++20) prints its fields, and other types
         * print the text of their `operator<<`, through reflow(). See
         * hegel::is_representable_v. Nothing is formatted unless
         * should_print().
         *
         * @tparam T The type of @p value
         * @param value The value to write.
         */
        template <typename T> void repr(const T& value);

        /**
         * @brief Write text that some other formatter made, with its bracket
         *        structure recovered.
         *
         * The engine finds the groups of `(...)`, `[...]` and `{...}` in
         * @p text and their `, ` separated items, so the text can break like
         * structured output.
         *
         * @param text The formatted text. It can contain newlines.
         */
        void reflow(std::string_view text);

        /**
         * @brief Write a point where the line can break.
         * @param separator The text written when the group fits on one line.
         */
        void breakable(std::string_view separator = " ");

        /**
         * @brief Write text only when the innermost group breaks.
         *
         * Use it for text that only the broken form needs, such as a
         * trailing comma.
         *
         * @param text The text. It must not contain a newline.
         */
        void if_break(std::string_view text);

        /// Write a new line, then the current indentation.
        void hard_break();

        /**
         * @brief Write @p open and start a group.
         *
         * The break points of the group indent their new lines @p indent
         * columns more.
         *
         * @param indent Extra indentation of the group's broken lines.
         * @param open Text written before the group.
         */
        void begin_group(std::size_t indent, std::string_view open);

        /**
         * @brief End the innermost group and write @p close.
         * @param close Text written after the group.
         * @throws std::logic_error if no group is open.
         */
        void end_group(std::string_view close);

        /**
         * @brief Write a group: @p open, what @p body writes, then @p close.
         *
         * The scoped form of begin_group() / end_group().
         *
         * @code{.cpp}
         * p.group(5, "make(", ")", [](hegel::Printer& p) {
         *     p.text("1");
         *     p.separator();
         *     p.text("2");
         * });
         * @endcode
         *
         * @tparam F A callable taking `hegel::Printer&`
         * @param indent Extra indentation of the group's broken lines.
         * @param open Text written before the group.
         * @param close Text written after the group.
         * @param body Writes the content of the group.
         */
        template <typename F>
        void group(std::size_t indent, std::string_view open,
                   std::string_view close, F&& body) {
            begin_group(indent, open);
            std::invoke(std::forward<F>(body), *this);
            end_group(close);
        }

        /**
         * @brief Write a list of items, separated by commas.
         *
         * On one line when it fits (`{1, 2}`). Else each item is on a line
         * of its own, indented four columns, and @p close is on a line of
         * its own. This is the layout of the containers that Hegel prints.
         *
         * @tparam Range A range of items
         * @tparam F A callable taking `hegel::Printer&` and an item
         * @param open Text written before the items.
         * @param close Text written after the items.
         * @param items The items.
         * @param print_item Writes one item.
         */
        template <typename Range, typename F>
        void seq(std::string_view open, std::string_view close,
                 const Range& items, F&& print_item) {
            begin_block(open);
            bool first = true;
            for (const auto& item : items) {
                if (!first) {
                    separator();
                }
                first = false;
                std::invoke(print_item, *this, item);
            }
            end_block(close);
        }

        /// Write the separator between two items: `,` and breakable(" ").
        void separator();

        /**
         * @brief Change the indentation of the break points that follow.
         * @param delta Columns to add. Can be negative.
         */
        void shift_indent(std::ptrdiff_t delta);

        /**
         * @brief Write `  // <text>` at the end of the current line.
         *
         * Every group open at this point breaks, so nothing else is written
         * after the comment on its line.
         *
         * @param text The comment. It must not contain a newline.
         */
        void comment(std::string_view text);

        /**
         * @brief Start a speculative region on this printer.
         *
         * What is written to this printer while the region is open is kept
         * only if Speculation::commit() is called. Speculation::abort(), or
         * destroying the Speculation without a commit, discards it.
         * Speculations nest. Close them in the reverse order of opening.
         *
         * @return The open region.
         */
        Speculation speculate();

        /**
         * @brief Make a child region at the current position.
         *
         * What the child gets later, from any thread that owns it, shows at
         * this position when the document is read. A generator uses this to
         * print a value whose representation is known only after the draw,
         * as `randoms()` does with the values it gives. The child is dead
         * (its writes are discarded) after the document is read, or when a
         * speculative region that it is in is aborted. The child of a no-op
         * or dead printer is a no-op printer.
         *
         * @return The child region.
         */
        Printer deferred();

        /// @cond INTERNAL
        Printer(hegel_printer_t* handle, bool owned)
            : handle_(handle), owned_(owned) {}
        hegel_printer_t* handle() const { return handle_; }
        void begin_block(std::string_view open);
        void end_block(std::string_view close);
        /// @endcond

      private:
        hegel_printer_t* handle_ = nullptr;
        bool owned_ = false;
    };

    /**
     * @brief An open speculative region; see Printer::speculate().
     *
     * Move-only. Destroying it without commit() or abort() aborts it, so an
     * exception from a draw discards what the draw wrote.
     */
    class Speculation {
      public:
        /**
         * @brief Move-constructs, transferring the open region.
         * @param other The speculation to move from. It no longer closes the
         *   region.
         */
        Speculation(Speculation&& other) noexcept;
        Speculation(const Speculation&) = delete;
        Speculation& operator=(const Speculation&) = delete;
        Speculation& operator=(Speculation&&) = delete;
        ~Speculation();

        /// End the region and keep what was written in it.
        void commit();
        /// End the region and discard what was written in it.
        void abort();

        /// @cond INTERNAL
        explicit Speculation(hegel_printer_t* handle) : handle_(handle) {}
        /// @endcond

      private:
        void close(bool keep);

        hegel_printer_t* handle_ = nullptr;
        bool resolved_ = false;
    };

    /**
     * @brief A document of its own, outside a test case.
     *
     * Use it to check what a printing function writes.
     *
     * @code{.cpp}
     * hegel::Document document(20);
     * document.printer().repr(std::vector<int>{1, 2});
     * std::string out = std::move(document).finish();
     * // std::vector<int>{1, 2}
     * @endcode
     */
    class Document {
      public:
        /**
         * @brief Make an empty document.
         * @param max_width The width the engine lays the document out in.
         * @throws std::runtime_error if @p max_width is 0.
         */
        explicit Document(std::size_t max_width = 79);
        Document(const Document&) = delete;
        Document& operator=(const Document&) = delete;

        /**
         * @brief The printer that writes to this document.
         * @return The document's printer.
         */
        Printer& printer() { return printer_; }

        /**
         * @brief Lay out the document and return it.
         *
         * Later writes to the document's printers are discarded.
         *
         * @return The text of the document.
         */
        std::string finish() &&;

      private:
        Printer printer_;
    };

} // namespace hegel

namespace hegel::internal {
    /// @cond INTERNAL
    // A width no value reaches, for rendering on one line.
    inline constexpr std::size_t unbounded_width =
        std::numeric_limits<uint32_t>::max();
    /// @endcond
} // namespace hegel::internal
