package hegel_test

import (
	"strings"
	"testing"

	"hegel.dev/go/hegel"
)

func TestDistinctPanicSitesHaveDistinctOrigins(t *testing.T) {
	options := []hegel.Option{hegel.WithTestCases(1), hegel.WithDatabase("")}

	first := runWithSinglePanic(panicAtFirstSite, options...)
	second := runWithSinglePanic(panicAtSecondSite, options...)
	if first == nil || second == nil {
		t.Fatalf("panicking properties must fail: first=%v second=%v", first, second)
	}

	firstReport, secondReport := first.Error(), second.Error()
	if !strings.Contains(firstReport, "runner_external_test.go:") || !strings.Contains(secondReport, "runner_external_test.go:") {
		t.Fatalf("panic origins should point to user code:\nfirst: %v\nsecond: %v", first, second)
	}
	if firstReport == secondReport {
		t.Fatalf("distinct panic sites should have distinct failure origins, got %v twice", first)
	}
}

func runWithSinglePanic(panicSite func(), options ...hegel.Option) error {
	calls := 0
	return hegel.Run(func(hegel.TestCase) {
		calls++
		if calls == 1 {
			panicSite()
		}
	}, options...)
}

func panicAtFirstSite() { panic("same panic") }

func panicAtSecondSite() { panic("same panic") }
