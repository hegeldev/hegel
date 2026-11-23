RELEASE_TYPE: patch

This patch makes shrinking faster for state machines and other tests whose random continuations rarely fit the remaining choice budget. The mutation shrink pass now stops trying random continuations of a candidate after five overruns in a row.
