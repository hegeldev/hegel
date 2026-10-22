#pragma once

#include <condition_variable>
#include <cstddef>
#include <cstdint>
#include <exception>
#include <functional>
#include <map>
#include <memory>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <string>
#include <thread>
#include <type_traits>
#include <utility>
#include <vector>

#include "hegel/core.h"

/**
 * @brief Stateful (model-based) property testing.
 *
 * A stateful test exercises a system through a sequence of randomly chosen
 * actions ("rules") applied to a state machine. Derive a machine from
 * @ref StateMachine and define its @c rules(). Each @ref Rule is a name and a
 * step function that performs one application of the rule, drawing any
 * arguments it needs from the test case and mutating the machine. Invariants
 * are predicates on the machine evaluated in full before any step and after
 * the final step, and sampled at intermediate join points. An invariant must
 * throw when violated.
 *
 * To run a state machine, pass an instance to @ref run inside a
 * @ref hegel::test. Examples in this documentation assume the alias
 * `namespace gs = hegel::generators;`.
 *
 * Example: an integer stack.
 *
 * @code{.cpp}
    struct IntegerStack
        : hegel::stateful::StateMachine<IntegerStack, std::vector<int>> {
        IntegerStack() : StateMachine({.initial_state = {}}) {}

        std::vector<hegel::stateful::Rule<IntegerStack>> rules() {
            return {
                hegel::stateful::Rule<IntegerStack>(
                    "push", [](hegel::TestCase& tc, IntegerStack& m) {
                        m.state.push_back(tc.draw(gs::integers<int>(
                            {.min_value = 0, .max_value = 100})));
                    }),
                hegel::stateful::Rule<IntegerStack>(
                    "pop", [](hegel::TestCase& tc, IntegerStack& m) {
                        tc.assume(!m.state.empty());
                        m.state.pop_back();
                    }),
            };
        }
    };

    HEGEL_TEST(stack_operations)(hegel::TestCase& tc) {
        IntegerStack machine;
        hegel::stateful::run(machine, tc);
    }
 * @endcode
 */
namespace hegel::stateful {

    using hegel::generators::Generator;
    using hegel::generators::IGenerator;

    template <typename T> class VariablesGenerator;
    template <typename T> class Invariant;

    /// The group name for rules that do not specify a group.
    inline constexpr const char* anonymous_group = "<anonymous>";

    /**
     * @brief An action that a worker can apply to a concurrent state machine.
     *
     * Rules in the same group can run concurrently. Rules in different groups
     * cannot. A rule that does not specify a group uses @ref anonymous_group.
     *
     * @tparam M The state-machine type that the rule acts on
     */
    template <typename M> class ConcurrentRule {
      public:
        /// The type of the function that applies the rule.
        using Function = std::function<void(TestCase&, M&)>;

        /**
         * @brief Creates a rule in @ref anonymous_group.
         *
         * @param name The name of the rule
         * @param function The function that applies the rule
         */
        ConcurrentRule(std::string name, Function function)
            : ConcurrentRule(std::move(name), anonymous_group,
                             std::move(function)) {}

        /**
         * @brief Creates a rule in the specified group.
         *
         * @param name The name of the rule
         * @param group The concurrency group of the rule
         * @param function The function that applies the rule
         */
        ConcurrentRule(std::string name, std::string group, Function function)
            : name_(std::move(name)), group_(std::move(group)),
              function_(std::move(function)) {}

        /**
         * @brief Returns the name of the rule.
         *
         * @return The name of the rule
         */
        const std::string& name() const { return name_; }

        /**
         * @brief Returns the concurrency group of the rule.
         *
         * @return The concurrency group of the rule
         */
        const std::string& group() const { return group_; }

        /**
         * @brief Returns the function that applies the rule.
         *
         * @return The function that applies the rule
         */
        const Function& function() const { return function_; }

      private:
        std::string name_;
        std::string group_;
        Function function_;
    };

    /**
     * @brief Base class for a concurrent state machine.
     *
     * Derive a machine from this class. Define a @c rules() member that returns
     * the rules for the machine. Override @ref invariants to add invariant
     * checks.
     *
     * This example tests a key-value store that has a lost-update bug. The
     * @c increment operation reads a value and then writes the incremented
     * value in a separate operation. Thus, two workers can overwrite an
     * update. The invariant compares the stored total with the number of
     * completed increments.
     *
     * The @c operations group contains the register, increment, and read
     * rules. These rules can run concurrently. The @c snapshot group does not
     * overlap with the @c operations group.
     *
     * @code{.cpp}
        class ConcurrentKVStore {
          public:
            std::optional<int64_t> get(int key) const {
                std::lock_guard<std::mutex> lock(mutex_);
                auto value = values_.find(key);
                if (value == values_.end()) {
                    return std::nullopt;
                }
                return value->second;
            }

            void put(int key, int64_t value) {
                std::lock_guard<std::mutex> lock(mutex_);
                values_[key] = value;
            }

            bool put_if_absent(int key, int64_t value) {
                std::lock_guard<std::mutex> lock(mutex_);
                return values_.emplace(key, value).second;
            }

            void increment(int key) {
                int64_t value = get(key).value_or(0);
                std::this_thread::yield();
                put(key, value + 1);
            }

            std::map<int, int64_t> snapshot() const {
                std::lock_guard<std::mutex> lock(mutex_);
                return values_;
            }

          private:
            mutable std::mutex mutex_;
            std::map<int, int64_t> values_;
        };

        struct ConcurrentKV
            : hegel::stateful::ConcurrentStateMachine<ConcurrentKV> {
            ConcurrentKVStore store;
            hegel::stateful::ConcurrentPool<int> keys;
            std::atomic<int64_t> increments{0};

            explicit ConcurrentKV(hegel::TestCase& tc) : keys(tc) {}

            std::vector<hegel::stateful::ConcurrentRule<ConcurrentKV>>
            rules() {
                return {
                    {"register", "operations",
                     [](hegel::TestCase& tc, ConcurrentKV& m) {
                         int key = tc.draw(hegel::generators::integers<int>(
                             {.min_value = 0, .max_value = 3}));
                         if (m.store.put_if_absent(key, 0)) {
                             m.keys.add(tc, key);
                         }
                     }},
                    {"increment", "operations",
                     [](hegel::TestCase& tc, ConcurrentKV& m) {
                         int key = tc.draw(
                             hegel::stateful::values_reusable(m.keys));
                         m.store.increment(key);
                         m.increments.fetch_add(1);
                     }},
                    {"read", "operations",
                     [](hegel::TestCase& tc, ConcurrentKV& m) {
                         int key = tc.draw(
                             hegel::stateful::values_reusable(m.keys));
                         int64_t value = m.store.get(key).value_or(0);
                         tc.note("read " + std::to_string(key) + " -> " +
                                 std::to_string(value));
                     }},
                    {"snapshot", "snapshot",
                     [](hegel::TestCase& tc, ConcurrentKV& m) {
                         tc.note("snapshot holds " +
                                 std::to_string(m.store.snapshot().size()) +
                                 " keys");
                     }},
                };
            }

            std::vector<hegel::stateful::Invariant<ConcurrentKV>> invariants()
                const {
                return {{"no_lost_updates", [](const ConcurrentKV& m) {
                             int64_t stored = 0;
                             for (const auto& entry : m.store.snapshot()) {
                                 stored += entry.second;
                             }
                             int64_t performed = m.increments.load();
                             if (stored != performed) {
                                 throw std::runtime_error(
                                     "the store lost an increment");
                             }
                         }}};
            }
        };

        HEGEL_TEST(concurrent_kv)(hegel::TestCase& tc) {
            ConcurrentKV machine(tc);
            hegel::stateful::run_concurrent(machine, tc, 1, 4);
        }
     * @endcode
     *
     * @tparam Derived The deriving state-machine type
     */
    template <typename Derived> class ConcurrentStateMachine {
      public:
        /**
         * @brief Returns the invariants of the machine.
         *
         * Override this member to add invariants. There are no invariants by
         * default.
         *
         * @return The invariants of the machine
         */
        std::vector<Invariant<Derived>> invariants() const { return {}; }
    };

    /**
     * @brief A pool of previously generated values.  A pool lets data flow from
     * one rule to another, so a rule can act on a handle or identifier that an
     * earlier rule produced rather than on a freshly drawn value.
     *
     * Create one with its constructor and populate it with @ref add. To draw
     * from the pool, use the generators @ref values_reusable (returns a value
     * without removing it) and @ref values_consumed (removes and returns a
     * value).
     *
     * A @ref Pool is neither copyable nor movable. A state that embeds a pool
     * is passed to @ref run by reference and never copied.
     *
     * Example: a resource allocator. The @c alloc rule creates a fresh resource
     * and deposits it in the pool. The @c free rule draws one of those resource
     * back out and releases it.
     *
     * @code{.cpp}
        struct Allocator
            : hegel::stateful::StateMachine<Allocator, std::set<int>> {
            // The pool is tied to a test case and cannot be copied, so it is
            // a member of the machine rather than part of the state.
            hegel::stateful::Pool<int> handles;
            int next_handle = 0;

            explicit Allocator(hegel::TestCase& tc)
                : StateMachine({.initial_state = {}}), handles(tc) {}

            std::vector<hegel::stateful::Rule<Allocator>> rules() {
                return {
                    hegel::stateful::Rule<Allocator>(
                        "alloc", [](hegel::TestCase&, Allocator& m) {
                            int h = m.next_handle++;
                            m.handles.add(h);
                            m.state.insert(h);
                        }),
                    hegel::stateful::Rule<Allocator>(
                        "free", [](hegel::TestCase& tc, Allocator& m) {
                            // draws a handle a prior alloc put in the pool
                            auto h = tc.draw(
                                "h",
                                hegel::stateful::values_consumed(m.handles));
                            m.state.erase(h);
                        }),
                };
            }
        };

        HEGEL_TEST(allocator_state_machine)(hegel::TestCase& tc) {
            Allocator machine(tc);
            hegel::stateful::run(machine, tc);
        }
     * @endcode
     *
     * @tparam T The type of variables in the pool.
     */
    template <typename T> class Pool {
      public:
        /**
         * @brief Creates an empty pool. Pools are tied to a test case. Do not
         * reuse one across test cases.
         *
         * @param tc The test case tied to the pool
         */
        explicit Pool(const TestCase& tc) : tc_(tc), pool_handle_(tc) {}

        /**
         * @brief Adds @p element to the pool. Overload for copying lvalues.
         *
         * @param element The element to add to the pool
         */
        void add(const T& element) {
            int64_t var_id = pool_handle_.add(tc_);
            // GCOVR_EXCL_START
            if (pool_.find(var_id) != pool_.end()) {
                throw std::runtime_error("unexpected variable id in map");
            }
            // GCOVR_EXCL_STOP
            pool_.emplace(var_id, element);
        }

        /**
         * @brief Adds @p element to the pool. Overload for moving rvalues.
         *
         * @param element The element to add to the pool
         */
        void add(T&& element) {
            int64_t var_id = pool_handle_.add(tc_);
            // GCOVR_EXCL_START
            if (pool_.find(var_id) != pool_.end()) {
                throw std::runtime_error("unexpected variable id in map");
            }
            // GCOVR_EXCL_STOP
            pool_.emplace(var_id, std::move(element));
        }

        /**
         * @brief Returns the number of variables in the pool.
         *
         * @return The number of variables in the pool
         */
        std::size_t size() const { return pool_.size(); }

        Pool(const Pool&) = delete;

      private:
        const TestCase& tc_;
        hegel::internal::PoolHandle pool_handle_;
        std::map<int64_t, T> pool_;

        friend class VariablesGenerator<T>;
    };

    /// @cond INTERNAL
    // Concrete IGenerator for variables.
    template <typename T> class VariablesGenerator : public IGenerator<T> {
      public:
        explicit VariablesGenerator(Pool<T>& p, bool consume)
            : p_(p), consume_(consume) {}

        T do_draw(const TestCase& tc) const override {
            int64_t variable = p_.pool_handle_.draw_variable(tc, consume_);

            auto it = p_.pool_.find(variable);
            // GCOVR_EXCL_START
            if (it == p_.pool_.end()) {
                throw std::runtime_error(
                    "Pool state diverged between the engine and the "
                    "client, or a bug in the pool bookkeeping.");
            }
            // GCOVR_EXCL_STOP
            if (consume_) {
                auto val = std::move(it->second);
                p_.pool_.erase(it);
                return val;
            }
            return it->second;
        }

      private:
        Pool<T>& p_;
        bool consume_;
    };
    /// @endcond

    /// @name Variable pools
    /// @{

    /**
     * @brief Returns a value from the pool and removes it.
     *
     * @code{.cpp}
        HEGEL_TEST(pool_round_trip)(hegel::TestCase& tc) {
            hegel::stateful::Pool<int> pool(tc);
            auto original_set = tc.draw(
                "original_set", gs::sets(gs::integers<int>(), {.max_size =
     10}));

            for (int num : original_set) {
                pool.add(num);
            }

            std::set<int> returned_set;
            for (int i = 0; i < original_set.size(); i++) {
                returned_set.insert(
                    tc.draw(hegel::stateful::values_consumed(pool)));
            }

            assert(original_set == returned_set);
        }
     * @endcode
     *
     * @tparam T Element type
     * @param p The pool to draw from
     * @return An element of the pool
     */
    template <typename T> Generator<T> values_consumed(Pool<T>& p) {
        return Generator<T>(new VariablesGenerator<T>(p, true));
    }

    /**
     * @brief Returns a value from the pool without removing it.
     *
     * @code{.cpp}
        HEGEL_TEST(pool_reuse)(hegel::TestCase& tc) {
            hegel::stateful::Pool<int> pool(tc);
            auto sz = tc.draw("sz", gs::integers<uint8_t>());
            auto original_set = tc.draw(
                "original_set", gs::sets(gs::integers<int>(), {.max_size =
     sz}));

            for (int num : original_set) {
                pool.add(num);
            }

            for (int i = 0; i < original_set.size(); i++) {
                tc.draw(hegel::stateful::values_reusable(pool));
            }

            assert(pool.size() == original_set.size());
        }
     * @endcode
     *
     * @tparam T Element type
     * @param p The pool to draw from
     * @return An element of the pool
     */
    template <typename T> Generator<T> values_reusable(Pool<T>& p) {
        return Generator<T>(new VariablesGenerator<T>(p, false));
    }

    /// @cond INTERNAL
    template <typename T> class ConcurrentVariablesGenerator;
    /// @endcond

    /**
     * @brief A pool of values that concurrent workers can share.
     *
     * Store the pool in a concurrent state machine. Use @ref add to add a
     * value from a worker, @ref values_reusable to draw a copy without
     * removal, and @ref values_consumed to draw and remove a value.
     *
     * libhegel selects a value and updates the pool as an atomic operation.
     * A draw rejects the test case if the pool is empty.
     *
     * A @ref ConcurrentPool is not copyable.
     *
     * @tparam T The type of the values in the pool
     */
    template <typename T> class ConcurrentPool {
      public:
        /**
         * @brief Creates an empty pool.
         *
         * @param tc The test case
         */
        explicit ConcurrentPool(const TestCase& tc) : pool_handle_(tc) {}

        /**
         * @brief Adds a value to the pool.
         *
         * Call with the test case of the current worker.
         *
         * @param tc The test case of the current worker
         * @param value The value to add
         */
        void add(const TestCase& tc, T value) {
            std::lock_guard<std::mutex> lock(mutex_);
            int64_t variable = pool_handle_.add(tc);
            values_.emplace(variable, std::move(value));
        }

        /**
         * @brief Returns true if the pool contains no values.
         *
         * @return True if the pool contains no values
         */
        bool empty() const {
            std::lock_guard<std::mutex> lock(mutex_);
            return values_.empty();
        }

        /**
         * @brief Returns the number of values in the pool.
         *
         * @return The number of values in the pool
         */
        std::size_t size() const {
            std::lock_guard<std::mutex> lock(mutex_);
            return values_.size();
        }

        ConcurrentPool(const ConcurrentPool&) = delete;
        ConcurrentPool& operator=(const ConcurrentPool&) = delete;

      private:
        internal::PoolHandle pool_handle_;
        mutable std::mutex mutex_;
        std::map<int64_t, T> values_;

        friend class ConcurrentVariablesGenerator<T>;
    };

    /// @cond INTERNAL
    template <typename T>
    class ConcurrentVariablesGenerator : public IGenerator<T> {
      public:
        ConcurrentVariablesGenerator(ConcurrentPool<T>& pool, bool consume)
            : pool_(pool), consume_(consume) {}

        T do_draw(const TestCase& tc) const override {
            std::lock_guard<std::mutex> lock(pool_.mutex_);
            int64_t variable = pool_.pool_handle_.draw_variable(tc, consume_);
            if (consume_) {
                T result = std::move(pool_.values_.at(variable));
                pool_.values_.erase(variable);
                return result;
            } else {
                return pool_.values_.at(variable);
            }
        }

      private:
        ConcurrentPool<T>& pool_;
        bool consume_;
    };
    /// @endcond

    /**
     * @brief Returns a value from a concurrent pool and removes it.
     *
     * The draw rejects the test case if the pool is empty.
     *
     * @tparam T The type of the values in the pool
     * @param pool The pool to draw from
     * @return A generator that consumes values from the pool
     */
    template <typename T>
    Generator<T> values_consumed(ConcurrentPool<T>& pool) {
        return Generator<T>(new ConcurrentVariablesGenerator<T>(pool, true));
    }

    /**
     * @brief Returns a copy of a value from a concurrent pool.
     *
     * The draw does not remove the value. It rejects the test case if the pool
     * is empty.
     *
     * @tparam T The type of the values in the pool
     * @param pool The pool to draw from
     * @return A generator that reuses values from the pool
     */
    template <typename T>
    Generator<T> values_reusable(ConcurrentPool<T>& pool) {
        return Generator<T>(new ConcurrentVariablesGenerator<T>(pool, false));
    }

    /**
     * @brief A rule is one possible action in a stateful test.
     *
     * @code{.cpp}
        auto push = hegel::stateful::Rule<std::vector<int>>(
            "push", [](hegel::TestCase& tc, std::vector<int>& stack) {
                stack.push_back(tc.draw(gs::integers<int>()));
            });
     * @endcode
     *
     * @tparam T The state-machine type the rule acts on
     */
    template <typename T> class Rule {
      public:
        /**
         * @brief Declares a new Rule
         *
         * @param name name of the rule
         * @param step function representing the step a rule takes. It mutates
         * the state in place, drawing any arguments it needs from the test
         * case. Always check preconditions with @c assume() before mutating. A
         * rule that rejects after mutating leaves the state partially modified.
         */
        explicit Rule(std::string name, std::function<void(TestCase&, T&)> step)
            : name_(std::move(name)), step_(std::move(step)) {}

        /**
         * @brief Returns the name of the rule.
         *
         * @return const std::string&
         */
        const std::string& name() const { return name_; }
        /**
         * @brief Returns the underlying step function of the rule.
         *
         * @return const std::function<void(TestCase&, T&)>&
         */
        const std::function<void(TestCase&, T&)>& step() const { return step_; }

      private:
        std::string name_;
        std::function<void(TestCase&, T&)> step_;
    };

    /**
     * @brief An invariant is a predicate that must hold at any point in the
     * stateful test. They are evaluated in full on the initial and final state
     * and sampled at intermediate join points. The invariant function should
     * throw when the invariant is violated.
     *
     * @tparam T The state-machine type the invariant checks
     */
    template <typename T> class Invariant {
      public:
        /**
         * @brief Declare a new invariant.
         *
         * @param name
         * @param invariant
         */
        explicit Invariant(std::string name,
                           std::function<void(const T&)> invariant)
            : name_(std::move(name)), invariant_(std::move(invariant)) {}

        /**
         * @brief Returns the name of the invariant.
         *
         * @return const std::string&
         */
        const std::string& name() const { return name_; }
        /**
         * @brief Returns the function representing the predicate of the
         * invariant.
         *
         * @return const std::function<void(const T&)>&
         */
        const std::function<void(const T&)>& invariant() const {
            return invariant_;
        }

      private:
        std::string name_;
        std::function<void(const T&)> invariant_;
    };

    /**
     * @brief Arguments for the @ref StateMachine constructor.
     *
     * @tparam State The type of the state the rules act on
     */
    template <typename State> struct StateMachineParams {
        /// The state the first rule acts on.
        State initial_state;
    };

    /**
     * @brief Base class for a state machine. Holds the state and declares the
     * rules and the invariants that act on it.
     *
     * Derive from it with the deriving type as @p Derived and the state's type
     * as @p State, pass the initial state to this constructor, and define
     *
     * @code{.cpp}
        std::vector<Rule<Derived>> rules();
     * @endcode
     *
     * returning the actions the test may apply. Optionally override
     * @ref invariants to add predicates checked in full before the first step
     * and after the final step, and sampled between steps. Rules mutate
     * @ref state in place. Pass an instance to @ref run.
     *
     * @code{.cpp}
        struct Counter : hegel::stateful::StateMachine<Counter, int> {
            Counter() : StateMachine({.initial_state = 0}) {}

            std::vector<hegel::stateful::Rule<Counter>> rules() {
                return {hegel::stateful::Rule<Counter>(
                    "inc", [](hegel::TestCase&, Counter& m) { m.state++; })};
            }
        };
     * @endcode
     *
     * A machine may hold members besides the state, for anything the state's
     * type cannot carry. A @ref Pool, for instance, is tied to a test case and
     * is neither copyable nor movable.
     *
     * @tparam Derived The deriving state-machine type
     * @tparam State The type of the state the rules act on
     */
    template <typename Derived, typename State> class StateMachine {
      public:
        /// The type of the state the rules act on.
        using state_type = State;
        /**
         * @brief Builds a machine holding the given initial state.
         *
         * @code{.cpp}
         * Counter() : StateMachine({.initial_state = 0}) {}
         * @endcode
         *
         * @param params The initial state. See StateMachineParams.
         */
        explicit StateMachine(StateMachineParams<State> params)
            : state(std::move(params.initial_state)) {}

        /// The state the rules act on. Rules mutate it in place, and a
        /// failing run prints it. See RunParams::print_state.
        State state;

        /**
         * @brief Invariants checked in full before the first step and after the
         * final step, and sampled between steps. Override to add them. Defaults
         * to none.
         *
         * @return const std::vector<Invariant<Derived>>&
         */
        std::vector<Invariant<Derived>> invariants() { return {}; }

      protected:
        ~StateMachine() = default;
    };

    /**
     * @brief Options for @ref run.
     */
    struct RunParams {
        /// If true (the default), a failing sequence prints the machine's
        /// state before its first step and after every step that runs to
        /// completion.
        ///
        /// @code{.txt}
        /// state = 0
        /// Step 1: add
        ///   auto amount = 3;
        /// state = 3
        /// Step 2: add
        ///   auto amount = 9;
        /// @endcode
        ///
        /// A step that throws prints no state, so the last state shown is
        /// the one the failing step started from.
        bool print_state = true;
    };

    /// @cond INTERNAL
    // prints the machine's state.
    template <typename M>
    void print_state(TestCase& tc, const M& machine, const RunParams& params) {
        if (params.print_state) {
            tc.note("state = " + internal::repr(machine.state));
        }
    }

    template <typename T>
    void check_invariants(
        TestCase& tc, const std::string& origin, const T& state,
        const std::vector<Invariant<T>>& invariants,
        std::optional<std::reference_wrapper<internal::StateMachineHandle>>
            machine_handle = std::nullopt) {
        for (std::size_t index = 0; index < invariants.size(); ++index) {
            if (machine_handle.has_value() &&
                !machine_handle->get().should_check_invariant(
                    tc, static_cast<int64_t>(index))) {
                continue;
            }
            const Invariant<T>& inv = invariants[index];
            try {
                inv.invariant()(state);
            } catch (...) {
                tc.note("Invariant " + inv.name() + " violated " + origin);
                throw;
            }
        }
    }
    /// @endcond

    /**
     * @brief Executes a concurrent stateful test with rules from @p machine.
     *
     * The runner selects a concurrency level from @p min_concurrency through
     * @p max_concurrency. Execution proceeds in rounds. For each round,
     * libhegel picks a random concurrency group. Every worker thread then runs
     * a short (possibly empty) random sequence of rules from only that group
     * concurrently with the other workers. The runner waits for all workers to
     * finish before it starts the next round.
     *
     * All invariants run before the first round and after the final round.
     * Between rounds, the runner samples each invariant independently.
     *
     * Raises @c std::invalid_argument if the machine declares no rules or if
     * the concurrency bounds are invalid.
     *
     * On a failing replay, the output shows the concurrency level, each round,
     * the active group, and the rules that each worker applies, but the
     * falsifying test case is not shrunk since the failure may be dependent on
     * thread scheduling.
     *
     * @tparam M The state-machine type, deriving from
     * @ref ConcurrentStateMachine
     * @param machine The state machine that the workers share
     * @param tc The test case object
     * @param min_concurrency The minimum number of workers
     * @param max_concurrency The maximum number of workers
     */
    template <typename M>
    void run_concurrent(M& machine, TestCase& tc, int64_t min_concurrency,
                        int64_t max_concurrency) {
        static_assert(std::is_base_of<ConcurrentStateMachine<M>, M>::value,
                      "run_concurrent() requires a machine deriving from "
                      "ConcurrentStateMachine<M>.");
        if (min_concurrency < 1 || min_concurrency > max_concurrency) {
            throw std::invalid_argument(
                "concurrency bounds must satisfy 1 <= min <= max");
        }

        std::vector<ConcurrentRule<M>> rules = machine.rules();
        std::vector<Invariant<M>> invariants = machine.invariants();
        if (rules.empty()) {
            throw std::invalid_argument(
                "Cannot run a concurrent state machine with no rules.");
        }

        std::vector<std::string> rule_names;
        std::vector<std::string> invariant_names;
        std::vector<std::string> group_names;     // maps group ID to name
        std::map<std::string, int64_t> group_ids; // maps group name to ID
        std::vector<int64_t>
            rule_groups; // ID of group that rule_names[i] belongs to is the ID
                         // at rule_groups[i].
        rule_names.reserve(rules.size());
        rule_groups.reserve(rules.size());
        for (const ConcurrentRule<M>& rule : rules) {
            rule_names.push_back(rule.name());
            auto [group, inserted] = group_ids.emplace(
                rule.group(), static_cast<int64_t>(group_ids.size()));
            if (inserted) {
                group_names.push_back(rule.group());
            }
            rule_groups.push_back(group->second);
        }
        invariant_names.reserve(invariants.size());
        for (const Invariant<M>& invariant : invariants) {
            invariant_names.push_back(invariant.name());
        }

        internal::StateMachineHandle machine_handle(
            tc, rule_names, rule_groups, invariant_names, min_concurrency,
            max_concurrency);
        tc.note("Concurrency level: " +
                std::to_string(machine_handle.concurrency()));
        tc.note("Initial invariant check.");
        check_invariants(tc, "in the initial state", machine, invariants);

        enum class EventKind { RoundDone, Invalid, Overrun, Control, Panicked };
        struct Event {
            EventKind kind = EventKind::RoundDone;
            std::optional<internal::CapturedException> exception;
        };
        struct RoundState {
            std::mutex mutex;
            std::condition_variable start;
            std::condition_variable done;
            uint64_t round_number = 0;
            bool stop = false;
            std::size_t num_workers_completed = 0;
            std::vector<Event> events;
        };

        auto classify = [] {
            Event event;
            event.exception = internal::capture_current_exception();
            try {
                std::rethrow_exception(event.exception->exception);
            } catch (const internal::HegelStopTest&) {
                event.kind = EventKind::Overrun;
            } catch (const internal::HegelReject&) {
                // not the same as rule rejection - entire test case is rejected
                // requires some worker race on cloned test cases with same
                // parent
                event.kind = EventKind::Invalid; // GCOVR_EXCL_LINE
            } catch (const std::invalid_argument&) {
                event.kind = EventKind::Control;
            } catch (...) {
                event.kind = EventKind::Panicked;
            }
            return event;
        };

        auto run_round = [&](std::size_t worker, TestCase& worker_tc) {
            try {
                while (true) {
                    int64_t rule_index = machine_handle.next_rule(
                        worker_tc, static_cast<int64_t>(worker));
                    if (rule_index == internal::state_machine_done) {
                        return Event{};
                    }
                    if (rule_index < 0 ||
                        static_cast<std::size_t>(rule_index) >= rules.size()) {
                        // GCOVR_EXCL_START
                        throw std::runtime_error(
                            "state_machine_next_rule returned out-of-range "
                            "rule index. Please report this as a bug.");
                        // GCOVR_EXCL_STOP
                    }

                    const ConcurrentRule<M>& rule =
                        rules[static_cast<std::size_t>(rule_index)];
                    worker_tc.note("Rule: " + rule.name());
                    try {
                        internal::NoteIndentScope indent(worker_tc);
                        rule.function()(worker_tc, machine);
                    } catch (const internal::HegelReject&) {
                        machine_handle.rule_rejected(
                            worker_tc, static_cast<int64_t>(worker));
                        worker_tc.note(
                            "Rule stopped early due to violated assumption.");
                    }
                }
            } catch (...) {
                return classify();
            }
        };

        std::size_t worker_count =
            static_cast<std::size_t>(machine_handle.concurrency());
        RoundState state;
        state.events.resize(worker_count);
        std::vector<std::thread> workers;
        workers.reserve(worker_count);
        for (std::size_t worker = 0; worker < worker_count; ++worker) {
            TestCase worker_tc = tc.clone();
            internal::set_concurrent_worker(worker_tc, worker);
            workers.emplace_back(
                [&, worker, worker_tc = std::move(worker_tc)]() mutable {
                    uint64_t observed_round_number = 0;
                    while (true) {
                        {
                            std::unique_lock<std::mutex> lock(state.mutex);
                            state.start.wait(lock, [&] {
                                return state.stop ||
                                       state.round_number !=
                                           observed_round_number; // main thread
                                                                  // started a
                                                                  // new round
                            });
                            if (state.stop) {
                                return;
                            }
                            observed_round_number = state.round_number;
                        }

                        {
                            Event event = run_round(worker, worker_tc);
                            std::lock_guard<std::mutex> lock(state.mutex);
                            state.events[worker] = std::move(event);
                            ++state.num_workers_completed;
                        }
                        state.done.notify_one();
                    }
                });
        }

        auto stop_workers = [&] {
            {
                std::lock_guard<std::mutex> lock(state.mutex);
                state.stop = true;
            }
            state.start.notify_all();
            for (std::thread& worker : workers) {
                worker.join();
            }
        };

        try {
            while (true) {
                int64_t group = machine_handle.next_group(tc);
                if (group == internal::state_machine_done) {
                    break;
                }
                if (group < 0 ||
                    static_cast<std::size_t>(group) >= group_names.size()) {
                    // GCOVR_EXCL_START
                    throw std::runtime_error(
                        "state_machine_next_group returned an unknown group "
                        "index. Please report this as a bug.");
                    // GCOVR_EXCL_STOP
                }
                {
                    std::lock_guard<std::mutex> lock(state.mutex);
                    state.num_workers_completed = 0;
                    ++state.round_number;
                    tc.note("---------------- Round " +
                            std::to_string(state.round_number) + ": group \"" +
                            group_names[static_cast<std::size_t>(group)] +
                            "\" ----------------");
                }
                state.start.notify_all();
                {
                    std::unique_lock<std::mutex> lock(state.mutex);
                    state.done.wait(lock, [&] {
                        return state.num_workers_completed == worker_count;
                    });
                }

                std::optional<std::size_t> control;
                bool saw_overrun = false;
                bool saw_invalid = false;
                std::optional<std::size_t> panic;
                // need to collect every event because the precedence is
                // control > overrun > invalid > panic > round complete
                for (std::size_t worker = 0; worker < worker_count; ++worker) {
                    switch (state.events[worker].kind) {
                    case EventKind::RoundDone:
                        break;
                        // GCOVR_EXCL_START
                    case EventKind::Invalid:
                        saw_invalid = true;
                        break;
                        // GCOVR_EXCL_STOP
                    case EventKind::Control:
                        if (!control.has_value()) {
                            control = worker;
                        }
                        break;
                    case EventKind::Overrun:
                        saw_overrun = true;
                        break;
                    case EventKind::Panicked:
                        if (!panic.has_value()) {
                            panic = worker;
                        }
                        break;
                    }
                }
                if (control.has_value()) {
                    state.events[*control].exception->rethrow();
                }
                if (saw_overrun) {
                    throw internal::HegelStopTest();
                }
                if (saw_invalid) {
                    throw internal::HegelReject(); // GCOVR_EXCL_LINE
                }
                if (panic.has_value()) {
                    state.events[*panic].exception->rethrow();
                }
                check_invariants(
                    tc, "after round " + std::to_string(state.round_number),
                    machine, invariants, std::ref(machine_handle));
            }

            tc.note("Final invariant check.");
            check_invariants(tc, "in the final state", machine, invariants);
        } catch (...) {
            stop_workers();
            throw;
        }
        stop_workers();
    }

    /**
     * @brief Executes a stateful test by repeatedly applying randomly chosen
     * rules from @p machine to it. Invariants run in full before the first
     * step and after the final step. Between steps, each invariant is sampled
     * independently. Rules mutate
     * @p machine in place. Raises @p std::invalid_argument if the machine
     * declares no rules.
     *
     * On a failing replay, each applied rule prints as @c "Step N: <name>". A
     * violated invariant identifies whether it was observed after a step or
     * in the initial or final state.
     *
     * @code{.txt}
        Step 1: add
        Step 2: add
        Invariant nonneg violated after step 2
     * @endcode
     *
     * @tparam M The state-machine type, deriving from @ref StateMachine
     * @param machine The state machine, initialized by the caller and mutated
     * by its rules
     * @param tc The test case object
     * @param params Options for the run. See RunParams.
     */
    template <typename M>
    void run(M& machine, TestCase& tc, const RunParams& params = {}) {
        static_assert(
            std::is_base_of<StateMachine<M, typename M::state_type>, M>::value,
            "run() requires a machine deriving from "
            "StateMachine<M, State>.");
        std::vector<Rule<M>> rules = machine.rules();
        std::vector<Invariant<M>> invariants = machine.invariants();
        if (rules.empty()) {
            throw std::invalid_argument(
                "Cannot run a state machine with no rules.");
        }
        std::vector<std::string> rule_names;
        rule_names.reserve(rules.size());
        for (const Rule<M>& rule : rules)
            rule_names.push_back(rule.name());

        std::vector<std::string> invariant_names;
        invariant_names.reserve(invariants.size());
        for (const Invariant<M>& invariant : invariants)
            invariant_names.push_back(invariant.name());

        std::vector<int64_t> rule_groups(rule_names.size(), 0);
        internal::StateMachineHandle machine_handle(tc, rule_names, rule_groups,
                                                    invariant_names, 1, 1);
        tc.note("Initial invariant check.");
        print_state(tc, machine, params);
        check_invariants(tc, "in the initial state", machine, invariants);
        int64_t steps_run = 0;

        // Sequential stateful testing is modeled as a concurrent stateful test
        // with one group and one worker.
        while (machine_handle.next_group(tc) != internal::state_machine_done) {
            while (true) {
                int64_t next_rule_idx = machine_handle.next_rule(tc, 0);
                if (next_rule_idx == internal::state_machine_done) {
                    break;
                }
                // GCOVR_EXCL_START
                if (next_rule_idx < 0 ||
                    static_cast<size_t>(next_rule_idx) >= rules.size()) {
                    throw std::runtime_error(
                        "state_machine_next_rule returned out-of-range "
                        "rule index. Please report this as a bug.");
                }
                // GCOVR_EXCL_STOP
                internal::start_span(tc, internal::SpanLabel::StatefulRule);
                steps_run++;
                const Rule<M>& rule = rules[static_cast<size_t>(next_rule_idx)];
                tc.note("Step " + std::to_string(steps_run) + ": " +
                        rule.name());

                try {
                    // nest the draws the step makes under its "Step N" header.
                    {
                        internal::NoteIndentScope indent(tc);
                        rule.step()(tc, machine);
                    }
                    print_state(tc, machine, params);
                    internal::stop_span(tc);
                } catch (const internal::HegelReject&) {
                    tc.note("Rule stopped early due to violated assumption.");
                    machine_handle.rule_rejected(tc, 0);
                    internal::stop_span(tc, true);
                } catch (...) {
                    internal::stop_span(tc);
                    throw;
                }
            }
            check_invariants(tc, "after step " + std::to_string(steps_run),
                             machine, invariants, std::ref(machine_handle));
        }

        tc.note("Final invariant check.");
        check_invariants(tc, "in the final state", machine, invariants);
    }
    /// @}

} // namespace hegel::stateful
