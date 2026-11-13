RELEASE_TYPE: patch

This patch changes the error `gs::domains().max_length(n)` reports for an invalid `n`. The message is now the engine's own, which says what is wrong with the value: `domain max_length=256 exceeds the RFC 1035 limit of 255`, or `domain max_length=3 leaves no eligible TLDs`. Previously it was `max_length must be between 4 and 255` for both. Which values are accepted has not changed.

It also refactors some internal error handling.
