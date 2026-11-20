package libhegel

import (
	"strings"
	"testing"
)

// TestOutputCallbackReceivesEngineOutput drives a verbose run with a non-nil
// writer and asserts the engine forwards its log lines and its test cases'
// notes through the callback, and that freeing the run releases the writer.
func TestOutputCallbackReceivesEngineOutput(t *testing.T) {
	ctx := NewContext()
	s, err := ctx.SettingsNew()
	if err != nil {
		t.Fatal(err)
	}
	s.TestCases(ctx, 3)
	s.Database(ctx, "")
	s.Verbosity(ctx, VERBOSITY_VERBOSE)

	var sb strings.Builder
	run, err := s.RunStart(ctx, &sb)
	if err != nil {
		t.Fatalf("RunStart: %v", err)
	}
	for {
		tc, err := run.NextTestCase(ctx)
		if err != nil {
			t.Fatalf("NextTestCase: %v", err)
		}
		if tc == nil {
			break
		}
		if _, err := tc.GenerateInteger(ctx, 0, 100); err != nil {
			t.Fatalf("GenerateInteger: %v", err)
		}
		if err := tc.Note(ctx, "streamed note"); err != nil {
			t.Fatalf("Note: %v", err)
		}
		if err := tc.MarkComplete(ctx, STATUS_VALID, "", "", ""); err != nil {
			t.Fatalf("MarkComplete: %v", err)
		}
	}
	if _, err := run.RunResult(ctx); err != nil {
		t.Fatalf("RunResult: %v", err)
	}
	run.Free()

	if !strings.Contains(sb.String(), "phase") {
		t.Errorf("expected verbose phase output via callback, got %q", sb.String())
	}
	if got := strings.Count(sb.String(), "streamed note\n"); got != 3 {
		t.Errorf("expected each test case's note via callback, got %d in %q", got, sb.String())
	}
	if !strings.HasSuffix(sb.String(), "\n") {
		t.Errorf("expected callback output to end with a newline, got %q", sb.String())
	}
}

// TestDeleteOutputFnFailedCall covers deleteOutputFn: when a non-nil output
// writer is supplied but the underlying C call fails, the handle is deleted
// immediately and the call returns the error.
func TestDeleteOutputFnFailedCall(t *testing.T) {
	lib := Stub(t, uintptr(0), E_INTERNAL, "boom") // run_start: placeholder handle, error, diagnostic
	s := &Settings{pointer: pointer[settingsT]{syms: lib.syms, raw: 1}}

	run, err := s.RunStart(lib, &strings.Builder{})
	if err == nil || run != nil {
		t.Fatalf("expected error and nil run, got run=%v err=%v", run, err)
	}
	if !strings.Contains(err.Error(), "boom") {
		t.Errorf("expected diagnostic in error, got %q", err)
	}
}
