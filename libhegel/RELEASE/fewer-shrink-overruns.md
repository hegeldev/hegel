RELEASE_TYPE: patch

This patch makes shrinking run the test function much less often. The mutation shrink pass now gives up on a candidate after five of its random continuations in a row run out of data.
