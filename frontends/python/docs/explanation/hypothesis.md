# Relationship to Hypothesis

The short version is "we're still figuring that out". Hegel and
[Hypothesis](https://hypothesis.readthedocs.io) are very similar libraries,
built on very similar ideas: Hegel's engine is a descendant of Hypothesis's,
and Hegel for Python's API is modelled on Hypothesis's. Hegel is more
optimised for the sorts of workloads that you are likely to want to run on
[Antithesis](https://antithesis.com/), and we are providing it partly because
many people want to write their workloads in Python. Avoiding writing a Hegel
for Python just because Hypothesis already existed seemed silly.

As a rule of thumb: if you just want to have tests that run as part of your CI
and already have Hypothesis tests, you probably want to keep using
Hypothesis. If you want to write a workload for Antithesis, you definitely
want to use Hegel. Otherwise, probably stick to Hypothesis for now, but
there's not a huge amount in it, in some cases Hegel will be better, and you
should feel free to experiment.

The answer may change in future, but if and when it does it will be very easy
to get an LLM to port your tests from Hypothesis to Hegel.
{doc}`../how-to/port-from-hypothesis` describes the translation, and
{doc}`comparison-with-hypothesis` lists the differences in detail.
