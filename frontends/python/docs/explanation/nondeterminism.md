# Nondeterministic tests

Hegel assumes that a test is *deterministic*: run it twice with the same
choices and it does the same thing. Shrinking relies on this. The engine
changes a failing input a little, reruns the test, and checks whether it still
fails. If the test can fail or pass on the same input, shrinking cannot tell
whether a change made a difference.

Some tests aren't deterministic: they depend on timing, threads, the current
time or an external service. Concurrent state machines are nondeterministic by
nature. Hegel notices when replaying the same choices gives a different
result, and handles it according to the `nondeterminism_strictness` setting.

## Quiet handling (the default)

With `nondeterminism_strictness="quiet"`, Hegel switches to handling the test
as nondeterministic. It confirms each failure by replaying it several times,
and reports it with a caveat saying how reliably it reproduced. A failure that
reproduced only once is dropped if another failure is confirmed.

This is the right setting for tests that are nondeterministic on purpose.

## Warnings and errors

`nondeterminism_strictness="warn"` handles the test the same way, but prints
a one-line notice when Hegel detects the nondeterminism, so you find out
about it.

`nondeterminism_strictness="error"` treats nondeterminism as a bug in the
test: the run stops with a {class}`hegel.Flaky` error. This is what Hypothesis
does.

## Nondeterministic generation

A different kind of nondeterminism is a test that makes different *requests*
of the engine when given the same choices: drawing from a different generator,
or a different number of times, depending on something other than the values
it has drawn. Hegel cannot replay such a test at all, and reports it as an
error whatever the setting.
