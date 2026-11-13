RELEASE_TYPE: patch

This patch fixes two cases where a failing test reported a recursive value that was not fully shrunk.

A recursive value whose branches may hold a single child, such as a tree of lists of one or two subtrees, could shrink to a branch of two children when a branch of one was also a failure. And a value found after the engine had abandoned earlier attempts at drawing it, most often one with large collections at every level, could be reported with much of its original data intact, because shrinking spent its whole budget on the abandoned attempts.
