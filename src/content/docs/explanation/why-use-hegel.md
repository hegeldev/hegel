---
title: Why use Hegel?
---

Hegel is an attempt to bring the power of property-based testing in Hypothesis to every language.

If you're not familiar with it, Hypothesis is the most widely used property-based testing library in the world.

Some of why Hypothesis is the most widely used library of this sort is because it's written in Python, which is quite popular, but Hypothesis wasn't the first property-based testing library in Python, only the first that achieved widespread use. This is because it has a lot of benefits over other property-based testing libraries.

The main ones are:

* Hypothesis has a great library of high-quality generators, and flexible tools for building on them.
* Hypothesis has "internal shrinking", which means that it will basically always give you a high-quality and readable final example. It avoids many of the pitfalls of shrinking in other property-based testing libraries, such as producing invalid test cases, requiring manually writing shrinkers, and poor quality out-of-the-box shrinking.<sup class='aside-link'>4</sup>
* Hypothesis has a test database, which means that when a test fails, if you rerun it it will automatically fail fast in the same way.

All of these benefits follow from the underlying model of Hypothesis, which is relatively simple, but the reality is that the real competitive advantage of Hypothesis is that the Hypothesis developers (including those of us involved in Hegel) put an unreasonable amount of work into it. As a result, not many other libraries come close, because most people are only willing to put a reasonable amount of work in. Go's [Rapid library](https://github.com/flyingmutant/rapid) is probably the most credible port we've seen, but most of the other libraries that claim to be Hypothesis inspired didn't adopt the core model, and as a result don't get the benefits of it.
And, to be honest, we're not willing to put that much work in again for new languages either! We'd love it if every language had a Hypothesis-quality property-based testing library, but not as much as we'd love not to have to maintain that for every language.

Hegel is our attempt to solve this: We implement a reusable core that can be used from any language, and a variety of language specific libraries that can act as thin frontends to that core. This makes it possible to do the unreasonable amount of work once, rather than once per language, and opens up the possibility of great property-based testing in every language.
We think this is extremely important, especially now.

Property-based testing has always been a great tool for reliable software development, but it's become even more important with the huge upsurge in code written by LLM-based coding agents.
AI written code is simultaneously incredibly impressive and, for want of a better word, sloppy, and we need tools to compensate for that. Property-based testing has a proven track record of catching humans' silly errors, and it does a great job of doing the same for LLMs.

Conversely, it's also never been easier to get started with property-based testing, because agents are actually pretty good at writing them!  We provide a [Hegel skill](https://github.com/hegeldev/hegel-skill) for getting agents to write property-based tests for you. It can't - and shouldn't - replace you writing your own property-based tests, but the hardest part of property-based testing for people has always seemed to be writing the first test, because it forces you to think a lot more about how to generate data for testing your code, and letting an agent get you over that initial hump is a huge win.

All of this is, of course, an argument that you should be using *property-based testing*, rather than Hegel in particular. Why should you use Hegel in particular?

Well, if you've already got great property-based tests that you're happy with, you probably shouldn't. Hegel is still early days and while we *want* it to be the best property-based testing library in every language, and are confident that we'll get it there, we can't deny that it's got some rough edges. That being said, if you want to check it out anyway, I bet Claude will one-shot porting over your existing tests to it, and you can decide for yourself which you prefer (and if it's the existing ones, we would *really* appreciate your telling us why so we can fix it!).

If, on the other hand, you'd like to get started on some green field property-based testing, we think Hegel is a great place to do it. It inherits a lot of power from its Hypothesis core, and we've made it as easy to use as possible.

Right now, Hegel is more or less a "developer preview". We expect the underlying logic to be pretty rock solid, because Hypothesis is pretty rock solid. But there are definitely going to be some rough edges in how we interact with it. We're pretty happy with the API but expect we've not got it 100% right.

We still think it's worth your time to give it a try. If this is your first time using property-based testing on a project, you'll almost certainly find bugs if you use it - hopefully *mostly* in your code - and that's useful even if you ultimately don't decide to adopt it until it's more stable. If you do, let us know how it goes!
