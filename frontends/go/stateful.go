package hegel

import (
	"errors"
	"fmt"
	"reflect"
	"runtime"
	"slices"
	"sort"
	"strings"

	"hegel.dev/go/hegel/internal/libhegel"
)

// stateMachine drives a user-supplied struct's Rule-prefixed and
// Invariant-prefixed methods as a property-tested state machine.
//
// By convention, rules may mutate the machine but invariants must not;
// the framework cannot enforce this, and a mutating invariant will
// produce misleading test runs.
type stateMachine struct {
	rules                []stateMachineRule
	invariants           []stateMachineRule
	maxConcurrency       int
	stepCount            int
	configuredRuleGroups []stateMachineRuleGroup
	alwaysCheckNames     []string
	pauseFaultsNames     []string
	ruleGroups           [][]int64
	invariantAlwaysCheck []bool
}

type stateMachineRuleGroup struct {
	name  string
	rules []string
}

// StateMachineOption configures a state machine run.
type StateMachineOption func(*stateMachine)

// WithConcurrency allows state machine rules to run concurrently. The engine
// chooses a worker count between 1 and [runtime.GOMAXPROCS] for each test case
// and may change it between rounds, so concurrent execution is not guaranteed
// for every round.
//
// Workers invoke rule methods concurrently on the same machine value. Any
// state read or mutated by rules must therefore be safe for concurrent access,
// for example through mutexes or atomics. Rules must not depend on their
// relative ordering. Each worker receives its own TestCase, so the TestCase
// passed to one rule must not be retained or shared with another worker.
//
// Execution proceeds in rounds. After every worker has finished its rules for
// a round, selected invariants run before the next round begins: ordinary
// invariants are sampled, while those configured with
// [WithAlwaysCheckInvariants] always run. Invariants are never executed
// concurrently.
func WithConcurrency() StateMachineOption {
	return WithBoundedConcurrency(runtime.GOMAXPROCS(0))
}

// WithBoundedConcurrency is like [WithConcurrency], but uses n instead of
// [runtime.GOMAXPROCS] as the maximum worker count.
//
// n is an upper bound, not a guarantee that every test case runs n rules at
// once.
func WithBoundedConcurrency(n int) StateMachineOption {
	return func(sm *stateMachine) {
		sm.maxConcurrency = n
	}
}

// WithStatefulStepCount sets the maximum number of rounds for this RunStateful
// invocation. With sequential rules, each round is one completed rule.
// The default is 50. RunStateful panics if n is less than 1.
func WithStatefulStepCount(n int) StateMachineOption {
	return func(sm *stateMachine) {
		sm.stepCount = n
	}
}

// WithAlwaysCheckInvariants marks invariants that must run at every join point
// (after every rule in a sequential machine). Other invariants are sampled at
// intermediate states and run with probability 1 / the configured step count.
// All invariants run unconditionally on the initial and final states.
//
// Pass the full method names, including the Invariant prefix. Calls are
// additive. RunStateful panics if a name is not a discovered invariant.
func WithAlwaysCheckInvariants(invariants ...string) StateMachineOption {
	names := slices.Clone(invariants)
	return func(sm *stateMachine) {
		sm.alwaysCheckNames = append(sm.alwaysCheckNames, names...)
	}
}

// WithPauseFaultsInvariants marks invariants that run with fault injection
// paused. Inside Antithesis the engine pauses the fault injector immediately
// before each such invariant runs and resumes it immediately after, even when
// the invariant fails, so the invariant observes the system under test without
// a fault landing in the middle of the check. Outside Antithesis the option
// has no effect. Invariants not named here never pause fault injection.
//
// Pass the full method names, including the Invariant prefix. Calls are
// additive, and combine with [WithAlwaysCheckInvariants]. RunStateful panics
// if a name is not a discovered invariant.
func WithPauseFaultsInvariants(invariants ...string) StateMachineOption {
	names := slices.Clone(invariants)
	return func(sm *stateMachine) {
		sm.pauseFaultsNames = append(sm.pauseFaultsNames, names...)
	}
}

// WithRuleGroup assigns rules to a named concurrency group. Rules in the same
// group may run concurrently with each other; rules that share no group never
// overlap. Pass the full method names, including the Rule prefix. Calls using
// the same group name add rules to that group.
//
// Each round, the engine picks one group and hands out only that group's
// rules. Rules not mentioned by any call share one implicit default group, so
// they may overlap with each other but not with rules in an explicit group. A
// rule named by several calls belongs to every group named, and runs in the
// rounds of each of them, so it may overlap the rules of every group it
// belongs to while those groups' other rules never overlap each other.
//
// RunStateful panics if a group is empty or names an unknown rule or
// invariant.
func WithRuleGroup(name string, rules ...string) StateMachineOption {
	ruleNames := slices.Clone(rules)
	return func(sm *stateMachine) {
		for i := range sm.configuredRuleGroups {
			if sm.configuredRuleGroups[i].name == name {
				sm.configuredRuleGroups[i].rules = append(sm.configuredRuleGroups[i].rules, ruleNames...)
				return
			}
		}
		sm.configuredRuleGroups = append(sm.configuredRuleGroups, stateMachineRuleGroup{name: name, rules: ruleNames})
	}
}

// stateMachineRule is a discovered rule or invariant: the method name and
// a bound function value with the receiver pre-applied at discovery time.
// pauseFaults is set on invariants named by [WithPauseFaultsInvariants].
type stateMachineRule struct {
	name        string
	fn          func(TestCase)
	pauseFaults bool
}

// newStateMachine inspects machine's method set and returns a runner.
func newStateMachine[M any, T interface{ *M }](machine T, opts ...StateMachineOption) (*stateMachine, error) {
	if machine == nil {
		return nil, fmt.Errorf("state machine pointer must not be nil")
	}
	sm := &stateMachine{maxConcurrency: 1, stepCount: 50}
	for _, opt := range opts {
		opt(sm)
	}
	if sm.maxConcurrency < 1 {
		return nil, fmt.Errorf("state machine maximum concurrency must be positive")
	}

	if sm.stepCount < 1 {
		return nil, fmt.Errorf("state machine step count must be positive")
	}

	rt := reflect.TypeOf(machine)
	rv := reflect.ValueOf(machine)
	tcType := reflect.TypeFor[TestCase]()

	for i := range rt.NumMethod() {
		m := rt.Method(i)
		name := m.Name
		mt := m.Type

		takesTestCase := false
		for j := 1; j < mt.NumIn(); j++ {
			if mt.In(j) == tcType {
				takesTestCase = true
				break
			}
		}

		isRule := strings.HasPrefix(name, "Rule")
		isInvariant := strings.HasPrefix(name, "Invariant")

		if !isRule && !isInvariant {
			if takesTestCase {
				return nil, fmt.Errorf("method %s takes TestCase but is not prefixed with Rule or Invariant", name)
			}
			continue
		}

		fn, ok := rv.Method(i).Interface().(func(TestCase))
		if !ok {
			return nil, fmt.Errorf("method %s: rules and invariants must have signature func(TestCase) with no return", name)
		}

		r := stateMachineRule{name: name, fn: fn}
		if isRule {
			sm.rules = append(sm.rules, r)
		} else {
			sm.invariants = append(sm.invariants, r)
		}
	}

	if len(sm.rules) == 0 {
		return nil, fmt.Errorf("state machine has no rules; at least one method must be prefixed with Rule")
	}
	ruleIndices := make(map[string]int, len(sm.rules))
	for i, rule := range sm.rules {
		ruleIndices[rule.name] = i
	}
	invariantIndices := make(map[string]int, len(sm.invariants))
	sm.invariantAlwaysCheck = make([]bool, len(sm.invariants))
	for i, invariant := range sm.invariants {
		invariantIndices[invariant.name] = i
	}
	for _, name := range sm.alwaysCheckNames {
		index, ok := invariantIndices[name]
		if !ok {
			return nil, fmt.Errorf("state machine always-check configuration references unknown invariant %q; available invariants: %s", name, strings.Join(names(sm.invariants), ", "))
		}
		sm.invariantAlwaysCheck[index] = true
	}
	for _, name := range sm.pauseFaultsNames {
		index, ok := invariantIndices[name]
		if !ok {
			return nil, fmt.Errorf("state machine pause-faults configuration references unknown invariant %q; available invariants: %s", name, strings.Join(names(sm.invariants), ", "))
		}
		sm.invariants[index].pauseFaults = true
	}

	// Group 0 is the "catch all" for rules no option names.
	sm.ruleGroups = make([][]int64, len(sm.rules))
	for groupIndex, group := range sm.configuredRuleGroups {
		if len(group.rules) == 0 {
			return nil, fmt.Errorf("state machine rule group %d is empty", groupIndex+1)
		}
		for _, name := range group.rules {
			ruleIndex, ok := ruleIndices[name]
			if !ok {
				if _, ok := invariantIndices[name]; ok {
					return nil, fmt.Errorf("state machine rule group names invariant %q; only rules can be grouped", name)
				}
				return nil, fmt.Errorf("state machine rule group references unknown rule %q; available rules: %s", name, strings.Join(names(sm.rules), ", "))
			}
			if !slices.Contains(sm.ruleGroups[ruleIndex], int64(groupIndex+1)) {
				sm.ruleGroups[ruleIndex] = append(sm.ruleGroups[ruleIndex], int64(groupIndex+1))
			}
		}
	}
	for i, groups := range sm.ruleGroups {
		if len(groups) == 0 {
			sm.ruleGroups[i] = []int64{0}
		}
	}

	return sm, nil
}

// names returns the method names of the given rules in registration order,
// matching the rule indices the engine draws via [TestCase.stateMachineNextRule].
func names(rules []stateMachineRule) []string {
	out := make([]string, len(rules))
	for i, r := range rules {
		out[i] = r.name
	}
	return out
}

// Run drives a state machine.
//
// It registers the machine with the engine, runs every invariant once, then
// draws a step count and for each step asks the engine which rule to run next
// (the engine owns rule selection, including swarm testing) and invokes it.
// At each join point always-check invariants are run, while the rest are
// sampled by the engine. Every invariant is run again on the final state.
// Each run of an invariant configured with [WithPauseFaultsInvariants] is
// bracketed by a pause and an unpause of fault injection on the test case.
//
// Rules that reject the current pre-state via [TestCase.Assume] are
// skipped and another rule is drawn, up to a retry budget.
func (sm *stateMachine) Run(tc TestCase) {
	machine, concurrency, err := tc.stateMachineNew(names(sm.rules), sm.ruleGroups, names(sm.invariants), sm.invariantAlwaysCheck, sm.maxConcurrency, sm.stepCount)
	if err != nil {
		tc.abort(err)
	}

	tc.log("Initial invariant check.")

	for _, inv := range sm.invariants {
		if err := checkInvariant(tc, inv); err != nil {
			tc.abort(err)
		}
	}

	workersGroup := make(workgroup, concurrency)

	// The engine drives execution in rounds: stateMachineNextGroup is asked at
	// every join point — including before the first rule — whether another
	// round should run, and each round's rule stream is pulled via
	// stateMachineNextRule until the engine signals the join point. With a
	// single group and concurrency 1 this runs the familiar sequential loop;
	// the engine owns the overall step budget.
	notedLevel := int64(0)
	for round := 1; ; round++ {
		group, level, err := tc.stateMachineNextGroup(machine)
		if err != nil {
			tc.abort(err)
		}
		if group == libhegel.StateMachineDone {
			break
		}
		groupName := "<anonymous>"
		if group != 0 {
			groupName = sm.configuredRuleGroups[int(group)-1].name
		}
		if sm.maxConcurrency != 1 && level != notedLevel {
			tc.log("Concurrency level: %d", level)
			notedLevel = level
		}
		tc.log("---------------- Round %d: group %q ----------------", round, groupName)

		// A clone anchors its native output at the current position in the root
		// document. Create fresh handles after the round heading so each round's
		// worker transcripts follow that heading, as they did with Go buffers.
		errs := func() []*workerError {
			workers := make([]TestCase, 0, concurrency)
			defer func() {
				for _, worker := range workers {
					worker.free()
				}
			}()
			for worker := range concurrency {
				clone, err := tc.clone()
				if err != nil {
					tc.abort(err)
				}
				workers = append(workers, clone)
				if concurrency > 1 {
					if err := clone.setWorker(worker); err != nil {
						tc.abort(err)
					}
				}
			}

			for i, worker := range workers {
				workersGroup.Go(i, func() error {
					for {
						idx, err := worker.stateMachineNextRule(machine, int64(i))
						if err != nil {
							return err
						}
						if idx == libhegel.StateMachineDone {
							return nil
						}
						rule := sm.rules[idx]
						worker.log("Rule: %s", rule.name)

						rejected, err := invokeRule(worker, rule.fn)
						if err != nil {
							return err
						}
						if rejected {
							if err := worker.stateMachineRuleRejected(machine, int64(i)); err != nil { // coverage-ignore
								return err
							}
							worker.log("Rule stopped early due to violated assumption.")
						}
					}
				})
			}
			return workersGroup.Wait()
		}()
		if len(errs) != 0 {
			dropped := errs[1:]
			sort.Slice(dropped, func(i, j int) bool {
				return dropped[i].worker < dropped[j].worker
			})
			for _, err := range dropped {
				if outcome, ok := errors.AsType[*invocationError](err.err); ok {
					location := findCallerInPCs(outcome.pcs, isNotHegelFrame)
					tc.log("Dropped concurrent %s from worker %d at %s: %s", outcome.kind, err.worker, location, outcome)
				}
			}
			tc.abort(errs[0])
		}
		for i, inv := range sm.invariants {
			shouldCheck, err := tc.stateMachineShouldCheckInvariant(machine, int64(i))
			if err != nil {
				tc.abort(err)
			}
			if !shouldCheck {
				continue
			}
			if err := checkInvariant(tc, inv); err != nil {
				tc.abort(err)
			}
		}
	}

	tc.log("Final invariant check.")
	for _, inv := range sm.invariants {
		if err := checkInvariant(tc, inv); err != nil {
			tc.abort(err)
		}
	}
}

// checkInvariant runs an invariant once, with fault injection paused around
// it when the invariant was configured with [WithPauseFaultsInvariants]. The
// unpause is deferred so it happens even when the invariant fails; an error
// from the invariant takes precedence over one from unpausing.
func checkInvariant(tc TestCase, inv stateMachineRule) (err error) {
	if inv.pauseFaults {
		if err := tc.pauseFaults(); err != nil {
			return err
		}
		defer func() {
			if unpauseErr := tc.unpauseFaults(); err == nil {
				err = unpauseErr
			}
		}()
	}
	_, err = invokeRule(tc, inv.fn)
	return err
}

// invokeRule gives each rule or invariant its own abort boundary. Assume
// rejects only that invocation; all other outcomes propagate to the outer test
// case, which owns completion of the native stream.
func invokeRule(tc TestCase, fn testBody) (bool, error) {
	err := tc.invoke(fn)
	if errors.Is(err, libhegel.E_ASSUME) {
		return true, nil
	}
	return false, err
}

// RunStateful enables model-based testing.
//
// machine is a pointer to a struct which implements a state-machine in terms
// of rules and invariants.
//
// Methods whose name starts with "Rule" and whose signature is
// func(TestCase) are registered as rules. Methods whose name
// starts with "Invariant" with the same signature are registered as
// invariants.
//
// Rules run sequentially by default. Pass [WithConcurrency] or
// [WithBoundedConcurrency] to allow rules to run concurrently, and
// [WithRuleGroup] to restrict which rules may overlap. Pass
// [WithStatefulStepCount] to change the default limit of 50 rounds, and
// [WithAlwaysCheckInvariants] for invariants that must observe every join
// point. Other invariants are sampled between unconditional initial and final
// checks. Pass [WithPauseFaultsInvariants] for invariants that must run with
// fault injection paused.
//
// It panics if a method takes TestCase but is not prefixed
// with Rule or Invariant, if a Rule- or Invariant-prefixed method has
// the wrong signature, or if the machine has no rules.
func RunStateful[M any, T interface{ *M }](tc TestCase, machine T, opts ...StateMachineOption) {
	sm, err := newStateMachine(machine, opts...)
	if err != nil {
		panic(err)
	}
	sm.Run(tc)
}

type workerResult struct {
	worker int
	err    error
}

type workerError struct {
	worker int
	err    error
}

func (e *workerError) Error() string { return e.err.Error() }
func (e *workerError) Unwrap() error { return e.err }

type workgroup chan workerResult

func (g workgroup) Go(worker int, fn func() error) {
	go func() {
		g <- workerResult{worker: worker, err: fn()}
	}()
}

func (g workgroup) Wait() []*workerError {
	priority := func(err error) int {
		var userFailure *invocationError
		switch {
		case errors.As(err, &userFailure):
			return 3
		case errors.Is(err, libhegel.E_ASSUME):
			return 2
		case errors.Is(err, libhegel.E_STOP_TEST):
			return 1
		default:
			return 0
		}
	}

	results := make([]workerResult, 0, cap(g))
	for range cap(g) {
		result := <-g
		if result.err != nil {
			results = append(results, result)
		}
	}
	sort.Slice(results, func(i, j int) bool {
		left, right := results[i], results[j]
		leftPriority, rightPriority := priority(left.err), priority(right.err)
		if leftPriority != rightPriority {
			return leftPriority < rightPriority
		}
		return left.worker < right.worker
	})
	errs := make([]*workerError, len(results))
	for i, result := range results {
		errs[i] = &workerError{worker: result.worker, err: result.err}
	}
	return errs
}
