#pragma once

/**
 * @cond INTERNAL
 */

// Wrappers over the engine's pretty-printer. Hegel for C++ chooses what to
// print (C++ syntax) and where the groups and break points go; the engine
// lays the result out within the line width.

#include <cstdint>
#include <limits>
#include <string>
#include <string_view>

/// @cond INTERNAL
struct hegel_printer_t;
/// @endcond

namespace hegel::internal {

    // A width no value reaches, for rendering on one line.
    inline constexpr uint64_t unbounded_width =
        std::numeric_limits<uint32_t>::max();

    // A borrowed handle onto one region of an engine document. Writes to a
    // region whose document was already read are ignored.
    class Printer {
      public:
        explicit Printer(hegel_printer_t* handle) : handle_(handle) {}

        // Unbreakable text. Must not contain a newline.
        void text(std::string_view text);
        // A point that renders as `separator` when its group fits on one
        // line, and as a newline when the group breaks.
        void breakable(std::string_view separator);
        // Emits `open` and starts a group whose broken lines are indented
        // `indent` columns further.
        void begin_group(uint64_t indent, std::string_view open);
        // Ends the innermost group and emits `close`.
        void end_group(std::string_view close);
        // Changes the indentation of the break points that follow by
        // `delta` columns.
        void shift_indent(int64_t delta);
        // An unconditional newline.
        void hard_break();
        // Text from a formatter that knows nothing of the printer: the engine
        // recovers its bracket structure, so it can break like the rest.
        void reflow(std::string_view text);

      private:
        hegel_printer_t* handle_;
    };

    // A standalone engine document, for rendering a value outside a test
    // case.
    class Document {
      public:
        explicit Document(uint64_t width);
        ~Document();
        Document(const Document&) = delete;
        Document& operator=(const Document&) = delete;

        Printer printer() const { return Printer(handle_); }
        // Lays out everything printed and returns it. Seals the document.
        std::string finish();

      private:
        hegel_printer_t* handle_ = nullptr;
    };

} // namespace hegel::internal

/// @endcond
