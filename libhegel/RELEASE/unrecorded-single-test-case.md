RELEASE_TYPE: patch

This patch stops a run of a single test case on the urandom backend, with no database to save a failure to, from recording the choices its test case makes. This is the configuration a long-running workload has under Antithesis, where the record of an hours-long test case grew without bound.

A failure of such a run is now reported from its one execution: it is not shrunk or replayed, the test case is not stamped for capture, and the failure has no reproduce blob.
