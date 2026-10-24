#include <hegel/internal.h>
#include <hegel/printer.h>
#include <hegel/test_case.h>

#include <engine.h>
#include <test_case.h>

#include <hegel.h>

#include <cstddef>
#include <cstdint>
#include <functional>
#include <memory>
#include <sstream>
#include <string>
#include <string_view>
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
        hegel_test_case_t* handle =
            impl::test_case_clone(impl::thread_context(), data_->tc);
        return TestCase(std::make_unique<impl::test_case::TestCaseData>(
            handle, data_->emit, data_->stateful_step_count));
    }

    void TestCase::assume(bool condition) const {
        if (!condition) {
            throw internal::HegelReject();
        }
    }

    void TestCase::reject() const { throw internal::HegelReject(); }

    void TestCase::target(double score, std::string_view label) const {
        impl::target(*this, score, label);
    }

    void TestCase::repeat(const std::function<void()>& body) const {
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

        Printer test_case_printer(const TestCase& tc) {
            impl::test_case::TestCaseData* data = tc.data();
            if (data->printer == nullptr) {
                data->printer =
                    impl::test_case_printer(impl::thread_context(), data->tc);
            }
            return Printer(data->printer);
        }

        std::string draw_display_name(const TestCase& tc, std::string_view name,
                                      bool repeatable) {
            impl::test_case::DrawNames& names = *tc.data()->draw_names;
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
                handle, data->emit, data->stateful_step_count);
            block->draw_names = data->draw_names;
            return TestCase(std::move(block));
        }

        void set_concurrent_worker(TestCase& tc, std::size_t worker_index) {
            impl::test_case_set_worker(impl::thread_context(), tc.data()->tc,
                                       static_cast<int64_t>(worker_index));
        }

    } // namespace internal

} // namespace hegel
