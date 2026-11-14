---
title: Using Hegel in Antithesis
sidebar:
  label: Hegel in Antithesis
---

To get more bugfinding power and total reproducibility out of your Hegel tests, you can
run them in [Antithesis](https://antithesis.com). 

Any use of concurrent stateful testing is worth running in Antithesis,
since handling nondeterministic failures is obviated by the deterministic
hypervisor of Antithesis. For example, any failure dependent on operating system
thread scheduling, which usually results in flaky failures, will be perfectly
reproducible inside of Antithesis. 

Antithesis can improve the bugfinding power of a Hegel test in three main ways.

First, it can [inject faults](https://antithesis.com/docs/product/writing_tests/controlling_faults/fault_types/) that are otherwise extremely unlikely or impossible outside of Antithesis. For example, Antithesis 
injects thread pauses to simulate one thread taking an unexpectedly long time to 
execute, which can widen the window for a race condition to occur and introduce 
new execution interleavings. 

Second, the coverage-guided fuzzer of Antithesis enables Hegel to explore more of
the program state space. Hegel on its own is a black-box 
test (no knowledge of the system when generating test cases), but Hegel in Antithesis is a gray-box test 
(partial knowledge of internal structure of the system). Inside of Antithesis, Hegel uses the fuzzer-generated 
bytes as the source of randomness, so Antithesis indirectly augments the inputs Hegel generates
based on code coverage, thus increasing the likelihood of exploring new program states. 
Since fuzzers are designed to be long-running processes, it also allows Hegel to run for 
much longer and find deeper bugs.

Third, you can write more granular and complex properties using the [Antithesis SDK](https://antithesis.com/docs/reference/sdk/) in the system under test. For example, you can [assert](https://antithesis.com/docs/product/writing_tests/assertions/) that an event sometimes happens or that code is unreachable. 
And since the assertions reside in your code, you can have properties on intermediate 
states of the system that are otherwise not accessible outside of it. Antithesis 
assertions also provide hints about which states should be explored in order to search for bugs.

We have spent most of this section on when you should use Antithesis and the advantages 
of doing so, but when should you **not** use Hegel in Antithesis? 

When creating a Hegel test, you should initially be iterating on and running the test
locally. Hegel is designed for fast, local iteration and finding shallow bugs quickly.
The local feedback loop will always be faster than uploading then running the test in 
Antithesis. By the time you run a Hegel test in Antithesis, the test should be in 
a state in which you are confident it is covering a significant portion of your system.

Any lightweight Hegel test, such as one that checks a simple function or composition
of a few functions, should probably only run locally. Antithesis is designed for
testing distributed systems and interactions between their components. Using 
Antithesis to test simple functions is overkill.

Finally, the key disadvantage, which we hope to fix in the near future, is
the lack of shrinking. Since Hegel tests in Antithesis often run for a very long
time before finding a bug, a failing test case is too large to shrink (sequentially) 
in a reasonable amount of time, so shrinking is disabled by default. 
