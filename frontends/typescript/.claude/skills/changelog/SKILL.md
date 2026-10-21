---
name: changelog
description: "Changelog style guide for writing RELEASE.md files. Use when creating or reviewing RELEASE.md, writing changelog entries, or preparing a PR that needs release notes."
---

# Changelog Style Guide

This guide describes the style for writing `RELEASE.md` files for hegel-typescript. The style is modeled on the [Hypothesis changelog](https://hypothesis.readthedocs.io/en/latest/changes.html).

## Choosing `RELEASE_TYPE`

hegel-typescript is currently zerover (`0.x.y`), so the usual semver mapping does **not** apply. While we are pre-1.0:

- **`patch`** — Bug fixes, internal changes, **and new features / non-breaking API additions**. The default choice.
- **`minor`** — **Breaking changes only.** Any change that requires users to update their code (renamed/removed APIs, changed signatures, behavior changes that could break downstream tests) is a minor bump.
- **`major`** — Not used while we are zerover. Reserve for the eventual 1.0 and beyond.

If you find yourself reaching for `minor` because the change feels "big," check whether it actually breaks any caller. A large new feature that adds API surface without removing or changing existing behavior is still a `patch`.

## Opening sentence pattern

Every entry should open with a sentence that signals the scope and nature of the change:

- **Patch (fixes, improvements, new features):** Start with `"This patch ..."`
- **Minor (breaking changes):** Start with `"This release ..."` and explain migration
- **Tiny internal-only changes:** A bare sentence is fine — `"Internal refactoring."` or `"Clean up some internal code."`

The opening verb should tell the reader what _kind_ of change this is:

| Change type     | RELEASE_TYPE | Opening pattern                                                                             |
| --------------- | ------------ | ------------------------------------------------------------------------------------------- |
| Bug fix         | `patch`      | `"This patch fixes ..."` or `"Fix ..."`                                                     |
| New feature     | `patch`      | `"This patch adds ..."`                                                                     |
| Improvement     | `patch`      | `"This patch improves ..."`                                                                 |
| Performance     | `patch`      | `"This patch improves the performance of ..."` or `"Optimize ..."`                          |
| Deprecation     | `minor`      | `"This release deprecates ..."`                                                             |
| Breaking change | `minor`      | `"This release changes ..."` (then explain migration)                                       |
| Internal-only   | `patch`      | `"Internal refactoring."` / `"Refactor some internals."` / `"Clean up some internal code."` |

## Describe the user impact, not the implementation

Bad: "Reworked `NativeDataSource` to reuse a single koffi decode buffer across `hegel_generate` calls."

Good: "This patch improves the performance of data generation, particularly for tests that draw many values per test case. Each draw now does less redundant work at the FFI boundary."

Bad: "Fixed a bug in `arrays()`."

Good: "This patch fixes a bug where `arrays()` with `unique: true` could produce arrays containing duplicate elements when the elements were structurally equal but not identical."

## Length calibration

- **Internal-only changes:** 1 sentence. (`"Refactor some internals."`)
- **Simple bug fixes:** 1-3 sentences. Describe the bug and what changed.
- **New features:** 1-2 short paragraphs. Describe what it does and why it's useful.
- **Breaking changes / API changes:** Multiple paragraphs. Include before/after code examples and migration guidance.

Don't pad entries. If a change can be described in one sentence, use one sentence.

## Code examples

Include fenced code blocks for:

- New API features (show usage)
- Breaking changes (show before/after)
- Anything where seeing the code is clearer than describing it

Don't include code blocks for bug fixes or internal changes.

## References

- Reference GitHub issues when relevant: `([#123](https://github.com/hegeldev/hegel-typescript/issues/123))`
- Reference previous versions when building on prior work
- Reference related libraries/specs when relevant

## Tone

- Third person, present tense for describing behavior
- Professional but conversational — be direct, not formal
- Honest about uncertainty: `"This should improve performance"`, `"We expect this to..."`, `"In some cases this may..."`
- It's okay to briefly explain _why_ a change was made if the motivation isn't obvious

## Things to avoid

- No emojis
- No bullet lists for single-topic entries (use them for multi-topic entries like API cleanups)
- No commit hashes or PR numbers in the text (issue numbers are fine)
- Don't describe the implementation when you can describe the effect
- Don't use vague language like `"various improvements"` — be specific about what changed
- Don't add marketing language or hype

## Examples

**Good patch (bug fix):**

```
RELEASE_TYPE: patch

This patch fixes a bug where `floats()` could pass `excludeMin` to the engine
without a corresponding `minValue`, causing an engine error instead of
generating values.
```

**Good patch (internal):**

```
RELEASE_TYPE: patch

Internal refactoring of the CBOR decoding code.
```

**Good patch (new feature):**

```
RELEASE_TYPE: patch

This patch adds the `reportMultipleFailures` setting. When enabled, a run that
finds several distinct failing examples reports all of them, instead of only
the first.

It defaults to `false`, preserving the existing single-failure behavior.
```

**Good minor (breaking change):**

````
RELEASE_TYPE: minor

This release changes `oneOf()` to take an array of generators instead of
variadic arguments, so that alternatives can be built up programmatically
without spreading.

Before:

```ts
oneOf(integers(), text());
```

After:

```ts
oneOf([integers(), text()]);
```

To migrate, wrap the arguments you were passing to `oneOf()` in a single array.
````
