---
title: Why use Hegel?
---

We think property-based testing is extremely important, and that by making high-quality and ergonomic property-based testing available in every language, Hegel is going to be a huge part of the future of how we do software development. This would be true even if we weren't in the middle of a huge upsurge in code written by LLM-based coding agents, but those agents increase the importance of property-based testing massively.
AI written code is simultaneously incredibly impressive and, for want of a better word, sloppy, and we need tools to compensate for that. Property-based testing has a proven track record of catching humans' silly errors, and it does a great job of doing the same for LLMs.

Conversely, it's also never been easier to get started with property-based testing, because agents are actually pretty good at writing them!  We provide a [Hegel skill](https://github.com/hegeldev/hegel-skill) for getting agents to write property-based tests for you. It can't - and shouldn't - replace you writing your own property-based tests, but the hardest part of property-based testing for people has always seemed to be writing the first test, because it forces you to think a lot more about how to generate data for testing your code, and letting an agent get you over that initial hump is a huge win.

All of this is, of course, an argument that you should be using *property-based testing*, rather than Hegel in particular. Why should you use Hegel in particular?

Well, if you've already got great property-based tests that you're happy with, you probably shouldn't. Hegel is still early days and while we *want* it to be the best property-based testing library in every language, and are confident that we'll get it there, we can't deny that it's got some rough edges. That being said, if you want to check it out anyway, I bet Claude will one-shot porting over your existing tests to it, and you can decide for yourself which you prefer (and if it's the existing ones, we would *really* appreciate your telling us why so we can fix it!).

If, on the other hand, you'd like to get started on some green field property-based testing, we think Hegel is a great place to do it. It inherits a lot of power from its Hypothesis core, and we've made it as easy to use as possible.

Right now, Hegel is more or less a "developer preview". We expect the underlying logic to be pretty rock solid, because Hypothesis is pretty rock solid. But there are definitely going to be some rough edges in how we interact with it. We're pretty happy with the API but expect we've not got it 100% right.

We still think it's worth your time to give it a try. If this is your first time using property-based testing on a project, you'll almost certainly find bugs if you use it - hopefully *mostly* in your code - and that's useful even if you ultimately don't decide to adopt it until it's more stable. If you do, let us know how it goes!
