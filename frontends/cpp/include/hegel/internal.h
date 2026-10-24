#pragma once

#include "printer.h"
#include "test_case.h"

#include <cstddef>
#include <cstdint>
#include <exception>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

/// @cond INTERNAL
// opaque libhegel handle types.
struct hegel_collection_t;
struct hegel_pool_t;
struct hegel_state_machine_t;
/// @endcond

/**
 * @brief Internal utilities exposed in public headers.
 */
namespace hegel::internal {
    /// @cond INTERNAL

    /* Typed draw primitives over libhegel's C ABI. Declared here (rather
     * than in a private header) because the generator templates in the
     * public headers call them from their do_draw implementations. Each
     * throws HegelStopTest when the engine's choice budget is exhausted,
     * HegelReject when the draw rejects itself, and std::runtime_error on
     * any other engine failure.
     */

    // The names the generators derive their span labels from. The engine
    // identifies a span only by its label: spans with the same label come
    // from the same generator, which makes them candidates for swapping and
    // reordering when it shrinks. src/engine.cpp maps each name to a
    // `hegel.<kind>` string, the same one every Hegel frontend uses for that
    // generator, and derives its label with hegel_label_from_name.
    enum class LabelName {
        Generator,
        Composite,
        Map,
        FlatMap,
        Filter,
        FilterAttempt,
        Just,
        SampledFrom,
        OneOf,
        Optional,
        Deferred,
        Integers,
        Floats,
        Booleans,
        Text,
        Characters,
        FromRegex,
        Binary,
        Emails,
        Urls,
        Domains,
        IpAddressesV4,
        IpAddressesV6,
        Uuids,
        Lists,
        Sets,
        Dicts,
        Tuples,
        Array,
        Permutation,
        Recursive,
        StatefulRule,
        PoolValuesReusable,
        PoolValuesConsumed,
    };
    inline constexpr std::size_t label_name_count =
        static_cast<std::size_t>(LabelName::PoolValuesConsumed) + 1;

    // The label of a generator with no component generators, derived once
    // per process.
    uint64_t label(LabelName name);

    // The label of a generator built from other generators: its own label
    // first, then its components' labels (hegel_label_combine).
    uint64_t combine_labels(const std::vector<uint64_t>& labels);

    // Open / close a labeled span around a group of draws so the shrinker
    // can treat them as a unit. stop_span(tc, true) marks the span rejected
    // (e.g. a filter predicate failed) so the engine retries from before it.
    void start_span(const TestCase& tc, uint64_t label);
    void stop_span(const TestCase& tc, bool discard = false);

    int64_t draw_integer(const TestCase& tc, int64_t min_value,
                         int64_t max_value);
    // Uses the engine's big-integer draw when the range exceeds int64_t.
    uint64_t draw_integer_unsigned(const TestCase& tc, uint64_t min_value,
                                   uint64_t max_value);
    // Draws an integer between `min_value` and `max_value` inclusive. All
    // three buffers hold `len` bytes of two's-complement little-endian
    // integer, and the drawn value is sign-filled to `len` bytes.
    void draw_integer_bytes(const TestCase& tc, const uint8_t* min_value,
                            const uint8_t* max_value, std::size_t len,
                            uint8_t* out);
    bool draw_boolean(const TestCase& tc, double p,
                      std::optional<bool> forced = std::nullopt);
    double draw_float(const TestCase& tc, uint32_t width, double min_value,
                      double max_value, bool allow_nan, bool allow_infinity,
                      bool exclude_min, bool exclude_max,
                      double smallest_nonzero_magnitude);

    inline constexpr uint64_t no_max_size = ~uint64_t{0};
    class CollectionHandle {
      public:
        CollectionHandle(const TestCase& tc, uint64_t min_size,
                         uint64_t max_size);
        ~CollectionHandle();
        CollectionHandle(const CollectionHandle&) = delete;
        CollectionHandle& operator=(const CollectionHandle&) = delete;

        bool more(const TestCase& tc);

        void reject(const TestCase& tc, const char* why);

      private:
        hegel_collection_t* handle_ = nullptr;
    };

    class PoolHandle {
      public:
        explicit PoolHandle(const TestCase& tc);
        ~PoolHandle();
        PoolHandle(const PoolHandle&) = delete;
        PoolHandle& operator=(const PoolHandle&) = delete;

        // Returns a fresh variable id.
        int64_t add(const TestCase& tc);
        // Returns the variable id the engine picked, removing it from the
        // pool when consume is true.
        int64_t draw_variable(const TestCase& tc, bool consume);

      private:
        hegel_pool_t* handle_ = nullptr;
    };

    inline constexpr int64_t state_machine_done = INT64_MIN;
    class StateMachineHandle {
      public:
        StateMachineHandle(const TestCase& tc,
                           const std::vector<std::string>& rule_names,
                           const std::vector<int64_t>& rule_groups,
                           const std::vector<std::string>& invariant_names,
                           int64_t min_concurrency, int64_t max_concurrency);
        ~StateMachineHandle();
        StateMachineHandle(const StateMachineHandle&) = delete;
        StateMachineHandle& operator=(const StateMachineHandle&) = delete;

        // Starts the next round and returns its concurrency group, or
        // state_machine_done once the engine's step budget for this test case
        // is used up.
        int64_t next_group(const TestCase& tc);
        // Returns the index of the next rule to run this round, or
        // state_machine_done once the round's rule stream is exhausted.
        int64_t next_rule(const TestCase& tc, int64_t worker_index);
        // Report that an assumption failed in the last rule, so it does not
        // count against the step budget.
        void rule_rejected(const TestCase& tc, int64_t worker_index);
        bool should_check_invariant(const TestCase& tc,
                                    int64_t invariant_index);

        int64_t concurrency() const { return concurrency_; }

      private:
        hegel_state_machine_t* handle_ = nullptr;
        int64_t concurrency_ = 0;
    };

    struct CapturedException {
        std::exception_ptr exception;
        std::string origin;

        [[noreturn]] void rethrow() const;
    };

    CapturedException capture_current_exception();

    // Marks a draw in progress on `tc`. A draw made inside another one
    // (a generator that draws from the test case itself) prints nothing:
    // the outermost draw prints the whole value.
    class NestedDraw {
      public:
        explicit NestedDraw(const TestCase& tc);
        ~NestedDraw();
        NestedDraw(const NestedDraw&) = delete;
        NestedDraw& operator=(const NestedDraw&) = delete;

        bool outermost() const { return outermost_; }

      private:
        const TestCase& tc_;
        bool outermost_;
    };

    // Whether notes and drawn values of `tc` are recorded in the engine's
    // document (see TestCase::note).
    bool records_output(const TestCase& tc);

    // `tc`'s printer onto its region of the test case's document.
    Printer test_case_printer(const TestCase& tc);

    // The variable name a draw prints under: `name`, or `name_<n>` for a
    // repeatable name (and `draw_<n>` for an unnamed draw), skipping names
    // already taken.
    std::string draw_display_name(const TestCase& tc, std::string_view name,
                                  bool repeatable);

    // A handle onto `tc`'s own choice sequence whose output is a block
    // nested `indent` columns further in, at the current position of `tc`'s
    // output (hegel_test_case_block). Draw through either, never both at
    // once.
    TestCase block(const TestCase& tc, uint64_t indent);

    // Attributes everything `tc` prints from now on to concurrent worker
    // `worker_index` (hegel_test_case_set_worker).
    void set_concurrent_worker(TestCase& tc, std::size_t worker_index);

    /* Exception thrown when a test case is rejected and should be
     * discarded (e.g. by `TestCase::assume(false)`, an exhausted
     * `filter()`, or an `UnsatisfiedAssumption` from the engine).
     * The runner records the case as INVALID and continues.
     */
    class HegelReject : public std::exception {
      public:
        const char* what() const noexcept override {
            return "test case rejected";
        }
    };

    /* Exception thrown when the backend tells us to abandon the current
     * test iteration entirely (StopTest, Overflow, FlakyStrategyDefinition,
     * FlakyReplay). The runner unwinds the test body and skips
     * `mark_complete` — the engine already knows the iteration is over.
     */
    class HegelStopTest : public std::exception {
      public:
        const char* what() const noexcept override {
            return "test case stopped by backend";
        }
    };

} // namespace hegel::internal

/// @endcond
