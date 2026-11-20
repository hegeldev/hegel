package hegel

import (
	"errors"
	"io"
	"regexp"
	"strings"
	"testing"

	"hegel.dev/go/hegel/internal/libhegel"
)

// newEmittingTestCase returns the first test case of a real verbose run, which
// the engine streams to out. The run's own output before the test case starts
// is dropped.
func newEmittingTestCase(t *testing.T, out io.Writer) *testCase {
	t.Helper()
	return newEmittingTestCaseAt(t, out, libhegel.VERBOSITY_VERBOSE)
}

func newEmittingTestCaseAt(t *testing.T, out io.Writer, verbosity libhegel.Verbosity) *testCase {
	t.Helper()
	ctx := libhegel.NewContext()
	s, err := ctx.SettingsNew()
	if err != nil {
		t.Fatal(err)
	}
	if err := s.Database(ctx, ""); err != nil {
		t.Fatal(err)
	}
	if err := s.Verbosity(ctx, verbosity); err != nil {
		t.Fatal(err)
	}
	gate := &gatedWriter{w: out}
	run, err := s.RunStart(ctx, gate)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(run.Free)
	tc, err := run.NextTestCase(ctx)
	if err != nil {
		t.Fatal(err)
	}
	gate.open = true
	state, err := newTestCase(ctx, tc, captureUserPanics)
	if err != nil {
		t.Fatal(err)
	}
	if !state.wantsOutput {
		t.Fatal("a verbose run's test case does not want output")
	}
	return state
}

type gatedWriter struct {
	w    io.Writer
	open bool
}

func (g *gatedWriter) Write(p []byte) (int, error) {
	if !g.open {
		return len(p), nil
	}
	return g.w.Write(p)
}

func TestNativeOutputLifecycle(t *testing.T) {
	var out strings.Builder
	s := newEmittingTestCase(t, &out)
	err := s.run(func(tc TestCase) {
		tc.Note("first\nsecond")
		if got := out.String(); got != "first\nsecond\n" {
			t.Fatalf("streamed output = %q", got)
		}
		clone, err := tc.clone()
		if err != nil {
			t.Fatal(err)
		}
		tc.Note("parent after clone")
		clone.Note("child")
		clone.log("framework %d", 3)
		grandchild, err := clone.clone()
		if err != nil {
			t.Fatal(err)
		}
		if !grandchild.(*testCase).wantsOutput || !clone.(*testCase).wantsOutput {
			t.Fatal("a clone lost the output gate")
		}
		grandchild.Note("grandchild")
		if got := out.String(); got != "first\nsecond\n" {
			t.Fatalf("output past a live clone = %q", got)
		}
	})
	if err != nil {
		t.Fatal(err)
	}
	if got, want := out.String(), "first\nsecond\nchild\nframework 3\ngrandchild\nparent after clone\n"; got != want {
		t.Fatalf("output = %q, want %q", got, want)
	}
	if err := s.tc.Note(s.ctx, "late"); !errors.Is(err, libhegel.E_INVALID_HANDLE) {
		t.Fatalf("late write = %v", err)
	}
}

func TestNativeOutputStreamsBeforePanic(t *testing.T) {
	var out strings.Builder
	s := newEmittingTestCase(t, &out)
	s.panicPolicy = propagateUserPanics
	defer func() {
		if got := recover(); got != "boom" {
			t.Fatalf("panic = %v", got)
		}
		if got := out.String(); got != "before panic\n" {
			t.Fatalf("output = %q", got)
		}
	}()
	_ = s.run(func(tc TestCase) { tc.Note("before panic"); panic("boom") })
}

// A verbose run's rejected and overrun cases print their output followed by
// the engine's reason for stopping them.
func TestNativeOutputVerboseStoppedCases(t *testing.T) {
	for name, tt := range map[string]struct {
		reject func(TestCase)
		reason string
	}{
		"assume": {func(tc TestCase) { tc.Assume(false) }, "failed assumption"},
		"overrun": {func(tc TestCase) {
			tc.(*testCase).abort(libhegel.E_STOP_TEST)
		}, "out of data"},
	} {
		t.Run(name, func(t *testing.T) {
			var out strings.Builder
			s := newEmittingTestCase(t, &out)
			err := s.run(func(tc TestCase) {
				tc.Note("rejected")
				tt.reject(tc)
			})
			if err != nil {
				t.Fatal(err)
			}
			if got, want := out.String(), "rejected\nTest case stopped: "+tt.reason+"\n"; got != want {
				t.Fatalf("output = %q, want %q", got, want)
			}
		})
	}
}

// A verbose run prints a failing case's one-line summary after its output:
// the first frame outside Hegel (here the testing package, since this test is
// inside it), then the failure's first line.
func TestNativeOutputSummary(t *testing.T) {
	var out strings.Builder
	s := newEmittingTestCase(t, &out)
	if s.wantsDetail {
		t.Fatal("an unstamped verbose test case wants detail")
	}
	err := s.run(func(tc TestCase) {
		tc.Note("drawn")
		tc.Errorf("bad %d\nmore", 7)
	})
	if err != nil {
		t.Fatal(err)
	}
	if got := out.String(); !regexp.MustCompile(`^drawn\ntesting\.go:\d+: failure: bad 7 …\n$`).MatchString(got) {
		t.Fatalf("output = %q", got)
	}
}

// A debug run prints a failing case's full diagnostic, stack included.
func TestNativeOutputDetail(t *testing.T) {
	var out strings.Builder
	s := newEmittingTestCaseAt(t, &out, libhegel.VERBOSITY_DEBUG)
	if !s.wantsDetail {
		t.Fatal("a debug test case does not want detail")
	}
	err := s.run(func(tc TestCase) {
		tc.Note("drawn")
		tc.Errorf("bad %d", 7)
	})
	if err != nil {
		t.Fatal(err)
	}
	got := out.String()
	if !strings.HasPrefix(got, "drawn\nfailure: bad 7\n\n") || !strings.Contains(got, "TestNativeOutputDetail") || strings.Contains(got, ": failure: bad 7") {
		t.Fatalf("output = %q", got)
	}
}

// The engine refuses a failure without the summary it wants, and the run
// surfaces that refusal.
func TestNativeOutputMissingSummaryIsAnError(t *testing.T) {
	s := newEmittingTestCase(t, io.Discard)
	s.wantsOutput = false
	if err := s.run(func(tc TestCase) { tc.Fail() }); !errors.Is(err, libhegel.E_INVALID_ARG) || !strings.Contains(err.Error(), "summary") {
		t.Fatalf("run = %v", err)
	}
}

func TestNativeOutputMarkCompleteError(t *testing.T) {
	s := newStubTestCase(t, libhegel.E_BACKEND, "mark_complete failed")
	s.wantsOutput = true
	s.wantsDetail = true
	if err := s.run(func(tc TestCase) { tc.Fail() }); !errors.Is(err, libhegel.E_BACKEND) {
		t.Fatalf("run = %v", err)
	}
}

func TestNativeOutputWantsDetailError(t *testing.T) {
	ctx := libhegel.Stub(t,
		uintptr(1), libhegel.OK, // settings_new
		uintptr(1), libhegel.OK, // run_start
		uintptr(1), libhegel.OK, // next_test_case
		true, libhegel.OK, // wants_output
		false, libhegel.E_BACKEND, "wants_detail failed",
	)
	s, _ := ctx.SettingsNew()
	run, _ := s.RunStart(ctx, nil)
	tc, _ := run.NextTestCase(ctx)
	if _, err := newTestCase(ctx, tc, captureUserPanics); !errors.Is(err, libhegel.E_BACKEND) {
		t.Fatal(err)
	}
}

func TestNativeOutputWantsOutputError(t *testing.T) {
	ctx := libhegel.Stub(t,
		uintptr(1), libhegel.OK, // settings_new
		uintptr(1), libhegel.OK, // run_start
		uintptr(1), libhegel.OK, // next_test_case
		false, libhegel.E_BACKEND, "wants_output failed",
	)
	s, _ := ctx.SettingsNew()
	run, _ := s.RunStart(ctx, nil)
	tc, _ := run.NextTestCase(ctx)
	if _, err := newTestCase(ctx, tc, captureUserPanics); !errors.Is(err, libhegel.E_BACKEND) {
		t.Fatal(err)
	}
}

func TestNativeNoteError(t *testing.T) {
	s := newEmittingTestCase(t, io.Discard)
	err := s.invoke(func(tc TestCase) { tc.Note(string([]byte{255})) })
	if !errors.Is(err, libhegel.E_INVALID_ARG) {
		t.Fatalf("note = %v", err)
	}
}

func TestNativeOutputDisabled(t *testing.T) {
	s := newStubTestCase(t,
		uintptr(2), libhegel.OK, // clone
		uintptr(2),  // context clone
		libhegel.OK, // mark complete
	)
	err := s.run(func(tc TestCase) {
		clone, err := tc.clone()
		if err != nil {
			t.Fatal(err)
		}
		if clone.(*testCase).wantsOutput {
			t.Fatal("a clone wants output its root does not")
		}
		clone.Note("silent child")
		tc.Note("silent root")
		tc.Fail()
	})
	if err != nil {
		t.Fatal(err)
	}
}

func TestRunWantsOutputErrors(t *testing.T) {
	for _, replay := range []bool{false, true} {
		t.Run(map[bool]string{false: "run", true: "replay"}[replay], func(t *testing.T) {
			ops := []any{
				uintptr(1), libhegel.OK, // settings
				libhegel.OK,             // derandomize
				uintptr(1), libhegel.OK, // run start
			}
			if replay {
				ops = append(ops,
					uintptr(0), libhegel.OK, // no next case
					uintptr(1), libhegel.OK, // result
					libhegel.RUN_STATUS_FAILED, libhegel.OK,
					uint64(1), libhegel.OK, // failure count
					uintptr(1), libhegel.OK, // failure
					"blob", libhegel.OK,
					"", libhegel.OK, // deterministic failure: no caveat
					false, libhegel.OK, // print blob
					uintptr(1), libhegel.OK, // case from blob
				)
			} else {
				ops = append(ops, uintptr(1), libhegel.OK)
			}
			ops = append(ops, false, libhegel.E_BACKEND, "wants_output failed")
			ctx := libhegel.Stub(t, ops...)
			opts := applyOpts([]Option{WithDerandomize(false)})
			opts.output = io.Discard
			err := runWithContext(ctx, func(TestCase) { t.Fatal("body ran after constructor failed") }, opts)
			if !errors.Is(err, libhegel.E_BACKEND) {
				t.Fatal(err)
			}
		})
	}
}

// A verbose run streams every test case's draws and notes to the run's output,
// and says why the engine stopped a rejected case.
func TestVerboseRunStreamsTestCases(t *testing.T) {
	t.Parallel()
	var out strings.Builder
	opts := applyOpts([]Option{WithTestCases(20), WithDerandomize(true), WithDatabase(""), WithVerbosity(VerbosityVerbose)})
	opts.output = &out
	err := runWithContext(libhegel.NewContext(), func(tc TestCase) {
		x := Draw(tc, Integers(0, 1000))
		tc.Note("streamed")
		tc.Assume(x%2 == 0)
	}, opts)
	if err != nil {
		t.Fatal(err)
	}
	got := out.String()
	if n := strings.Count(got, "streamed\n"); n < 20 {
		t.Fatalf("streamed %d test cases, want at least 20:\n%s", n, got)
	}
	if !strings.Contains(got, "streamed\nTest case stopped: failed assumption\n") {
		t.Fatalf("missing the engine's stop reason:\n%s", got)
	}
}

// At default verbosity the report of a failure is its minimal replay: its
// draws, notes and diagnostic, printed once.
func TestFailureReportShowsMinimalReplay(t *testing.T) {
	t.Parallel()
	var out strings.Builder
	opts := applyOpts([]Option{WithDerandomize(true), WithDatabase("")})
	opts.output = &out
	err := runWithContext(libhegel.NewContext(), func(tc TestCase) {
		x := Draw(tc, Integers(0, 1000))
		tc.Note("checked")
		if x >= 10 {
			tc.Fail()
		}
	}, opts)
	if !errors.Is(err, errPropTestFailed) {
		t.Fatal(err)
	}
	got := out.String()
	if !strings.Contains(got, " = 10\nchecked\nfailure: test case aborted with Fail\n\n") || strings.Count(got, "checked") != 1 {
		t.Fatalf("report:\n%s", got)
	}
}

// A run without an output destination passes the engine no callback, prints
// nothing, and still reports its failure, at any verbosity.
func TestRunWithoutOutputReportsFailure(t *testing.T) {
	t.Parallel()
	opts := applyOpts([]Option{WithDerandomize(true), WithDatabase(""), WithVerbosity(VerbosityDebug), WithReproductionBlob(true)})
	err := runWithContext(libhegel.NewContext(), func(tc TestCase) {
		if Draw(tc, Integers(0, 1000)) >= 10 {
			tc.Fail()
		}
	}, opts)
	if !errors.Is(err, errPropTestFailed) {
		t.Fatal(err)
	}
}

// WithReproductionBlob(false) keeps the blob out of a replayed failure's report.
func TestReplayedFailureHidesBlob(t *testing.T) {
	t.Parallel()
	var out strings.Builder
	opts := applyOpts([]Option{WithDerandomize(true), WithDatabase(""), WithReproductionBlob(false)})
	opts.output = &out
	err := runWithContext(libhegel.NewContext(), func(tc TestCase) {
		if Draw(tc, Integers(0, 1000)) >= 10 {
			tc.Fail()
		}
	}, opts)
	if !errors.Is(err, errPropTestFailed) || !strings.Contains(out.String(), "test case aborted with Fail") || strings.Contains(out.String(), "reproduction blob") {
		t.Fatalf("run = %v, report:\n%s", err, out.String())
	}
}

// A quiet run prints no test-case output for its failure, and still re-raises
// the panic of the minimal failing example.
func TestQuietRunRaisesMinimalFailure(t *testing.T) {
	t.Parallel()
	var out strings.Builder
	opts := applyOpts([]Option{WithDerandomize(true), WithDatabase(""), WithVerbosity(VerbosityQuiet)})
	opts.output = &out
	defer func() {
		if got := recover(); got != 10 {
			t.Fatalf("panic = %v, want the minimal failing value 10", got)
		}
		if strings.Contains(out.String(), " = ") {
			t.Fatalf("quiet output = %q", out.String())
		}
	}()
	_ = runWithContext(libhegel.NewContext(), func(tc TestCase) {
		if x := Draw(tc, Integers(0, 1000)); x >= 10 {
			panic(x)
		}
	}, opts)
}

// A verbose run's summary of a failing case names the line in the user's test
// that failed; the full stack trace is left to the final report.
func TestVerboseSummaryNamesUserLine(t *testing.T) {
	out := newTempGoProject(t).
		writeFile("hegel_test.go", `package temptest

import (
	"testing"

	"hegel.dev/go/hegel"
)

func TestErrorf(t *testing.T) {
	hegel.Test(t, func(ht *hegel.T) {
		x := hegel.Draw(ht, hegel.Integers(0, 1000))
		if x > 300 {
			ht.Errorf("x too big: %d", x)
		}
	}, hegel.WithVerbosity(hegel.VerbosityVerbose), hegel.WithDatabase(""), hegel.WithDerandomize(true))
}

// A panic ends the test binary, so it runs last.
func TestPanics(t *testing.T) {
	hegel.Test(t, func(ht *hegel.T) {
		if hegel.Draw(ht, hegel.Integers(0, 1000)) > 300 {
			panic("x too big\nsecond line")
		}
	}, hegel.WithVerbosity(hegel.VerbosityVerbose), hegel.WithDatabase(""), hegel.WithDerandomize(true))
}
`).
		expectFailure(`"Action":"fail"`).
		goTest()
	for name, want := range map[string]string{
		"TestPanics": `(?m)^\s+hegel_test\.go:22: panic: x too big …$`,
		"TestErrorf": `(?m)^\s+hegel_test\.go:13: failure: x too big: \d+$`,
	} {
		result := out.Tests[name]
		if matched, _ := regexp.MatchString(want, result.Output); !matched {
			t.Errorf("%s output did not match %q:\n%s", name, want, result.Output)
		}
		if strings.Count(result.Output, "hegel.dev/go/hegel.(*testCase).invoke(...)") > 1 {
			t.Errorf("%s printed a stack trace per test case below debug verbosity:\n%s", name, result.Output)
		}
	}
}

// A run that ends without a verdict on the property reports why through
// testing.T, since nothing else prints its message.
func TestRunErrorIsReported(t *testing.T) {
	out := newTempGoProject(t).
		writeFile("hegel_test.go", `package temptest

import (
	"testing"

	"hegel.dev/go/hegel"
)

func TestFilterTooMuch(t *testing.T) {
	hegel.Test(t, func(ht *hegel.T) {
		hegel.Draw(ht, hegel.Booleans())
		ht.Assume(false)
	}, hegel.WithDatabase(""), hegel.WithDerandomize(true))
}
`).
		expectFailure(`"Action":"fail"`).
		goTest()
	result := out.Tests["TestFilterTooMuch"]
	want := `(?m)^\s+hegel_test\.go:10: property test run failed: FailedHealthCheck: FilterTooMuch`
	if matched, _ := regexp.MatchString(want, result.Output); !matched {
		t.Errorf("output did not match %q:\n%s", want, result.Output)
	}
}
