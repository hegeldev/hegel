RELEASE_TYPE: patch

This patch makes the nondeterminism caveat per failure. A run that switched to nondeterministic handling used to attach a `note: nondeterministic failure, confirmed: failed 20 of 20 replays` caveat to every failure it reported, including one that reproduced at every replay and was only caught up in another failure's flakiness. Such a failure is now reported as a plain failure; a failure the run did see pass keeps its caveat quoting that evidence.
