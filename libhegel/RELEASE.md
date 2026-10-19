RELEASE_TYPE: patch

This patch fixes a shrinking regression introduced in 0.46.0. A run that reports a single failure shrinks only the smallest failure it found, but shrinking that failure can run into a different failure with a smaller example than any seen before, and the run then reported that example without shrinking it. Such a failure is now shrunk before it is reported, so the reported failure is minimal again.
