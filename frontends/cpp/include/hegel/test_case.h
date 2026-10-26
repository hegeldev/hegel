#pragma once

#include <cstddef>
#include <functional>
#include <future>
#include <memory>
#include <string_view>
#include <thread>
#include <type_traits>
#include <utility>

namespace hegel::impl::test_case {
    struct TestCaseData;
}

namespace hegel::generators {
    template <typename T> class Generator;
}

namespace hegel {

    /**
     * @brief Handle to the currently-executing test case.
     *
     * A @c TestCase is passed as the sole argument to the callback given to
     * hegel::test(). It is the main way a test definition interacts with Hegel.
     *
     * @c TestCase owns its underlying libhegel handle and is move-only (not
     * copyable). The callback receives it by reference; a @c clone() returns a
     * fresh owning @c TestCase. It must not outlive the test-case callback.
     *
     * @code{.cpp}
     * HEGEL_TEST(example)(hegel::TestCase& tc) {
     *     namespace gs = hegel::generators;
     *     auto x = tc.draw("x", gs::integers<int>({.min_value = 0}));
     *     tc.assume(x != 0);
     *     tc.note("x = " + std::to_string(x));
     * }
     * @endcode
     */
    class TestCase {
      public:
        TestCase(const TestCase&) = delete;
        TestCase& operator=(const TestCase&) = delete;
        /// Move-constructs, transferring ownership of the underlying handle.
        TestCase(TestCase&&) noexcept;
        /**
         * @brief Move-assigns, transferring ownership of the underlying handle.
         * @return Reference to this test case.
         */
        TestCase& operator=(TestCase&&) noexcept;
        ~TestCase();

        /**
         * @brief Draw a random value from a generator.
         *
         * Each draw is printed as a C++ declaration, `auto <name> =
         * <value>;`, on the final replay of a failing test case and on
         * every case at Verbosity::Verbose and above. A value too wide for
         * one line is broken across lines. Draws made through this overload
         * print as `draw_1`, `draw_2`, ... in draw order. Use the named
         * overload to print under a variable name.
         *
         * @tparam T The value type produced by @p gen
         * @param gen The generator to draw from
         * @return A generated value of type T
         */
        template <typename T> T draw(const generators::Generator<T>& gen) const;

        /**
         * @brief Draw a random value from a generator, printed under
         *        @p name.
         *
         * @tparam T The value type produced by @p gen
         * @param name Variable name for the printed declaration.
         * @param gen The generator to draw from
         * @param repeatable Pass true to print @p name with a 1-based
         *             suffix per use (`x_1`, `x_2`, ...). Useful when the
         *             same draw runs in a loop. A non-repeatable name
         *             prints bare on every use.
         * @return A freshly generated value of type T
         */
        template <typename T>
        T draw(std::string_view name, const generators::Generator<T>& gen,
               bool repeatable = false) const;

        /**
         * @brief Reject the current test case if @p condition is false.
         *
         * @code{.cpp}
         * auto age = tc.draw(gs::integers<int>());
         * tc.assume(age >= 18);
         * @endcode
         *
         * @param condition Value that must be true for the test case to
         *                  continue. If false, the current test case is
         *                  rejected.
         */
        void assume(bool condition) const;

        /**
         * @brief Reject the current test case unconditionally.
         *
         * Equivalent to @c assume(false), but marked @c [[noreturn]] so code
         * after the call is statically known to be unreachable.
         *
         * @code{.cpp}
         * auto n = tc.draw(gs::integers<int>());
         * unsigned u = n >= 0 ? static_cast<unsigned>(n) : tc.reject();
         * @endcode
         */
        [[noreturn]] void reject() const;

        /**
         * @brief Record a score for the engine's targeted-search phase to
         *        maximize.
         *
         * Higher scores are treated as "more interesting." The engine biases
         * later test cases toward inputs that produced higher scores under the
         * same @p label. Has no effect unless the Target phase is enabled.
         *
         * @param score The observation to maximize. Must be finite.
         * @param label Distinguishes independent targeting goals. Each label
         *              may be recorded at most once per test case.
         *
         * @code{.cpp}
         * auto n = tc.draw(gs::integers<int>({.min_value = 0}));
         * tc.target(static_cast<double>(n));
         * @endcode
         */
        void target(double score, std::string_view label = "") const;

        /**
         * @brief Record an event for the end-of-run statistics report.
         *
         * With Settings::show_statistics on, the end of the run reports, for
         * each label, the fraction of generated test cases that recorded it
         * at least once. Without it, events report nothing.
         *
         * @param label The event to record.
         *
         * @code{.cpp}
         * auto xs = tc.draw(gs::vectors(gs::integers<int>()));
         * if (xs.empty()) {
         *     tc.event("empty");
         * }
         * @endcode
         */
        void event(std::string_view label) const;

        /**
         * @brief Record a numeric observation for the end-of-run statistics
         *        report.
         *
         * With Settings::show_statistics on, the end of the run reports, for
         * each label, a summary of the observed values (count, min, median,
         * mean, p90, max) over the generated test cases. Unlike target(), a
         * label can be observed any number of times per test case, and the
         * observations do not steer generation.
         *
         * @param label Names the observed quantity.
         * @param value The observation. Must be finite.
         *
         * @code{.cpp}
         * auto xs = tc.draw(gs::vectors(gs::integers<int>()));
         * tc.event_value("size", static_cast<double>(xs.size()));
         * @endcode
         */
        void event_value(std::string_view label, double value) const;

        /**
         * @brief Run @p body in an engine-managed loop.
         *
         * The engine decides how many iterations to run, exploring and
         * shrinking the count like any other drawn quantity. Control returns to
         * the caller once the loop completes. A rejected iteration (via
         * @c assume / @c reject) is discarded and the loop continues; any other
         * exception propagates out as a failure.
         *
         * @param body Callable invoked once per iteration.
         *
         * @code{.cpp}
         * int total = 0;
         * tc.repeat([&] {
         *     total += tc.draw(gs::integers<int>({.min_value = 0}));
         *     if (total < 0) throw std::runtime_error("overflow");
         * });
         * @endcode
         */
        void repeat(const std::function<void()>& body) const;

        /**
         * @brief Record a message that will be printed on the final replay
         *        of a failing test case.
         *
         * The message prints in order with the drawn values, and on every
         * test case at Verbosity::Verbose and above. Each line of a
         * multi-line message prints as its own line.
         *
         * @param message The message to record.
         */
        void note(std::string_view message) const;

        /**
         * @brief Creates a clone of this test case with an independent choice
         * sequence forked from this test case.
         *
         * The returned @c TestCase draws from its own choice sequence but
         * shares this case's outcome and budget. A single test case must not be
         * drawn from concurrently, so give each thread its own clone.
         *
         * What the clone prints (its notes and drawn values) appears in the
         * output at the point where the clone was made, however the threads
         * are scheduled.
         *
         * @return A @c TestCase.
         *
         * @code{.cpp}
         * auto worker = tc.clone();
         * auto a = worker.draw(gs::integers<int>());
         * auto b = tc.draw(gs::integers<int>());
         * @endcode
         */
        TestCase clone() const;

        /**
         * @brief Attribute what this test case prints from now on to a
         *        worker.
         *
         * When several threads each draw from their own clone, give each
         * clone a worker index so the output says which thread printed what.
         * Every line this test case prints after the call, and every line
         * of the blocks and clones made from it after the call, is prefixed
         * with `[worker N +X.XXXms]`: the worker index and the time since
         * the test case started.
         *
         * @param worker_index The worker to attribute the output to.
         * @throws std::invalid_argument in an explicit test case.
         *
         * @code{.cpp}
         * std::vector<std::thread> threads;
         * for (std::size_t i = 0; i < 2; i++) {
         *     threads.emplace_back([worker = tc.clone(), i]() mutable {
         *         worker.set_worker(i);
         *         auto n = worker.draw(gs::integers<int>());
         *     });
         * }
         * for (auto& thread : threads) {
         *     thread.join();
         * }
         * @endcode
         */
        void set_worker(std::size_t worker_index);

        /**
         * @brief Run @p fn on a clone of this test case in a new thread.
         *
         * Clones this test case on the calling thread, then invokes @p fn with
         * the clone (as a @c TestCase&) on a @c std::thread. The returned
         * @c Worker awaits it. @c Worker::join() returns @p fn's result and
         * re-raises any exception it threw. Join every worker before the test
         * body returns.
         *
         * @tparam F Callable invocable as @c fn(TestCase&).
         * @param fn The work to run on the cloned stream.
         * @return A @c Worker handle to the running thread.
         *
         * @code{.cpp}
         * auto w = tc.spawn([](hegel::TestCase& c) {
         *     return c.draw(gs::integers<int>());
         * });
         * auto mine = tc.draw(gs::integers<int>());
         * auto theirs = w.join();
         * @endcode
         */
        template <typename F> auto spawn(F fn) const;

        /// @cond INTERNAL
        explicit TestCase(std::unique_ptr<impl::test_case::TestCaseData> data);

        // Generators reach through this accessor to talk to the backend.
        // Not part of the user-facing API.
        impl::test_case::TestCaseData* data() const { return data_.get(); }
        /// @endcond

      private:
        std::unique_ptr<impl::test_case::TestCaseData> data_;
    };

    /**
     * @brief Handle to a worker started by TestCase::spawn().
     *
     * Move-only. @c join() awaits the worker and returns its result,
     * re-raising any exception it threw. The destructor joins if @c join() was
     * never called.
     */
    template <typename T> class Worker {
      public:
        /// @cond INTERNAL
        Worker(std::thread thread, std::future<T> future)
            : thread_(std::move(thread)), future_(std::move(future)) {}
        /// @endcond
        /// Move-constructs, transferring ownership of the running worker.
        Worker(Worker&&) noexcept = default;
        Worker(const Worker&) = delete;
        Worker& operator=(const Worker&) = delete;
        ~Worker() {
            if (thread_.joinable()) {
                thread_.join();
            }
        }

        /**
         * @brief Await the worker and return its result.
         *
         * Re-raises any exception thrown on the worker thread. Call at most
         * once, before the test body returns.
         *
         * @return The value returned by the spawned callable.
         */
        T join() {
            if (thread_.joinable()) {
                thread_.join();
            }
            return future_.get();
        }

      private:
        std::thread thread_;
        std::future<T> future_;
    };

    template <typename F> auto TestCase::spawn(F fn) const {
        using R = std::invoke_result_t<F, TestCase&>;
        std::packaged_task<R()> task(
            [fn = std::move(fn), cloned_tc = clone()]() mutable {
                return fn(cloned_tc);
            });
        std::future<R> future = task.get_future();
        return Worker<R>(std::thread(std::move(task)), std::move(future));
    }

} // namespace hegel
