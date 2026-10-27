#include <hegel/explicit.h>
#include <hegel/internal.h>
#include <hegel/printer.h>
#include <hegel/test_case.h>

#include <engine.h>
#include <test_case.h>

#include <hegel.h>

#include <any>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <memory>
#include <mutex>
#include <optional>
#include <sstream>
#include <string>
#include <string_view>
#include <typeinfo>
#include <utility>
#include <vector>

namespace hegel::impl::test_case {

    TestCaseData::~TestCaseData() {
        hegel_context_t* ctx = thread_context();
        printer_free(ctx, printer);
        if (tc != nullptr) {
            hegel_test_case_free(ctx, tc);
        }
    }

    std::vector<std::string> rendered_output(const TestCaseData& data) {
        struct RootPrinter {
            hegel_context_t* ctx;
            hegel_printer_t* handle;
            ~RootPrinter() { printer_free(ctx, handle); }
        };
        hegel_context_t* ctx = thread_context();
        RootPrinter root{ctx, test_case_printer(ctx, data.tc)};
        std::string document = read_document(ctx, root.handle);
        std::vector<std::string> lines;
        std::istringstream in(document);
        std::string line;
        while (std::getline(in, line)) {
            lines.push_back(line);
        }
        return lines;
    }

} // namespace hegel::impl::test_case

namespace hegel {

    TestCase::TestCase(std::unique_ptr<impl::test_case::TestCaseData> data)
        : data_(std::move(data)) {}

    TestCase::TestCase(TestCase&&) noexcept = default;
    TestCase& TestCase::operator=(TestCase&&) noexcept = default;
    TestCase::~TestCase() = default;

    TestCase TestCase::clone() const {
        if (data_->explicit_state) {
            throw internal::UsageError(
                "clone() is not supported in an explicit test case");
        }
        hegel_test_case_t* handle =
            impl::test_case_clone(impl::thread_context(), data_->tc);
        auto cloned = std::make_unique<impl::test_case::TestCaseData>(
            handle, data_->emit);
        cloned->draw_names = data_->draw_names;
        return TestCase(std::move(cloned));
    }

    void TestCase::set_worker(std::size_t worker_index) {
        if (data_->explicit_state) {
            throw internal::UsageError(
                "set_worker() is not supported in an explicit test case");
        }
        impl::test_case_set_worker(impl::thread_context(), data_->tc,
                                   static_cast<int64_t>(worker_index));
    }

    void TestCase::assume(bool condition) const {
        if (!condition) {
            throw internal::HegelReject();
        }
    }

    void TestCase::reject() const { throw internal::HegelReject(); }

    void TestCase::target(double score, std::string_view label) const {
        if (data_->explicit_state) {
            return;
        }
        impl::target(*this, score, label);
    }

    void TestCase::event(std::string_view label) const {
        if (data_->explicit_state) {
            return;
        }
        impl::event(*this, label);
    }

    void TestCase::event_value(std::string_view label, double value) const {
        if (data_->explicit_state) {
            return;
        }
        impl::event_value(*this, label, value);
    }

    void TestCase::repeat(const std::function<void()>& body) const {
        if (data_->explicit_state) {
            note("// Repetition #1");
            body();
            return;
        }
        // Seed the loop's minimum length, then let the engine decide each
        // iteration: the count is drawn like any collection, so it shrinks.
        int64_t min_size = internal::draw_integer(*this, 0, int64_t{1} << 20);
        internal::CollectionHandle collection(
            *this, static_cast<uint64_t>(min_size), internal::no_max_size);
        uint64_t iteration = 0;
        while (collection.more(*this)) {
            note("// Repetition #" + std::to_string(++iteration));
            try {
                body();
            } catch (const internal::HegelReject&) {
                // Iteration rejected; discard it and keep looping.
                continue;
            }
        }
    }

    void TestCase::note(std::string_view message) const {
        if (data_->explicit_state) {
            std::istringstream lines{std::string(message)};
            std::string line;
            while (std::getline(lines, line)) {
                data_->explicit_state->lines.push_back(line);
            }
            return;
        }
        if (!data_->emit) {
            return;
        }
        impl::note(impl::thread_context(), data_->tc, message);
    }

    namespace internal {

        NestedDraw::NestedDraw(const TestCase& tc)
            : tc_(tc), outermost_(tc.data()->draw_depth == 0) {
            tc_.data()->draw_depth++;
        }

        NestedDraw::~NestedDraw() { tc_.data()->draw_depth--; }

        bool records_output(const TestCase& tc) { return tc.data()->emit; }

        bool is_explicit(const TestCase& tc) {
            return tc.data()->explicit_state != nullptr;
        }

        void check_not_explicit(const TestCase& tc, const char* what) {
            if (is_explicit(tc)) {
                throw UsageError(std::string(what) +
                                 " is not supported in an explicit test case");
            }
        }

        const std::any& take_explicit_value(const TestCase& tc,
                                            std::string_view name,
                                            bool repeatable,
                                            const std::type_info& type) {
            impl::test_case::ExplicitState& state = *tc.data()->explicit_state;
            const std::vector<ExplicitTestCase::Value>& values =
                state.test_case.values();
            std::string display = draw_display_name(tc, name, repeatable);
            std::optional<size_t> chosen;
            for (size_t i = 0; i < values.size() && !chosen; ++i) {
                if (!state.used[i] && values[i].name == display) {
                    chosen = i;
                }
            }
            for (size_t i = 0; i < values.size() && !chosen; ++i) {
                if (!state.used[i] && !values[i].name) {
                    chosen = i;
                }
            }
            if (!chosen) {
                throw UsageError(
                    "The explicit test case has no value for the draw " +
                    display + ". Give one with hegel::named(\"" + display +
                    "\", value), or add a positional value.");
            }
            const ExplicitTestCase::Value& value = values[*chosen];
            if (value.value.type() != type) {
                throw UsageError("The explicit test case value for the draw " +
                                 display + " has type " +
                                 impl::demangle(value.value.type().name()) +
                                 ", but the draw produces " +
                                 impl::demangle(type.name()) + ".");
            }
            state.used[*chosen] = true;
            state.lines.push_back("auto " + display + " = " +
                                  value.render(value.value) + ";");
            return value.value;
        }

        hegel::Printer test_case_printer(const TestCase& tc) {
            impl::test_case::TestCaseData* data = tc.data();
            if (data->printer == nullptr) {
                data->printer =
                    impl::test_case_printer(impl::thread_context(), data->tc);
            }
            return hegel::Printer(data->printer, false);
        }

        std::string draw_display_name(const TestCase& tc, std::string_view name,
                                      bool repeatable) {
            impl::test_case::DrawNames& names = *tc.data()->draw_names;
            std::lock_guard<std::mutex> lock(names.mutex);
            std::string base(name.empty() ? "draw" : name);
            int count = ++names.counts[base];
            if (!name.empty() && !repeatable) {
                // A bare name prints bare on every use; the surrounding
                // output (e.g. a stateful step header) disambiguates.
                names.taken.insert(base);
                return base;
            }
            for (int candidate = count;; candidate++) {
                std::string attempt = base + "_" + std::to_string(candidate);
                if (names.taken.insert(attempt).second) {
                    return attempt;
                }
            }
        }

        TestCase block(const TestCase& tc, uint64_t indent) {
            const impl::test_case::TestCaseData* data = tc.data();
            hegel_test_case_t* handle =
                impl::test_case_block(impl::thread_context(), data->tc, indent);
            auto block = std::make_unique<impl::test_case::TestCaseData>(
                handle, data->emit);
            block->draw_names = data->draw_names;
            return TestCase(std::move(block));
        }

    } // namespace internal

} // namespace hegel
