# Engine issues found while building Hegel for Python

Places where libhegel (`hegeltest-c` 0.43.7) behaves differently from what
Python's `re` module or Hypothesis would do. These are recorded here rather
than worked around in the bindings, so they can be fixed in the engine.

## Regex generation

- **A NUL in a pattern is lost.** `hegel_string_generator_regex` takes the
  pattern as a NUL-terminated C string, so `from_regex("a\x00b")` cannot
  express the literal NUL (the bindings substitute U+FFFD, and the generated
  strings do not match the pattern). Python's `re` accepts NULs in patterns.
  `tests/test_generators_strings.py::test_from_regex_with_literal_nul` is
  marked xfail.
- **Case-insensitive matching ignores the ASCII flag.** Under `(?ai)` Python
  folds case only for `a`-`z`, so `(?ai)À` does not match `à`; the engine
  generates `à` for it. The same applies to bytes patterns, which the
  bindings pass with `(?a)` since bytes `\w` and case folding are ASCII-only
  in Python. `tests/hypothesis_port/nocover/test_regex.py::test_fuzz_stuff`
  is a non-strict xfail for this reason.
- **The ASCII flag excludes explicit non-ASCII characters.** Under `re.ASCII`
  Python still matches literal characters and ranges outside ASCII, so
  `(?a)[Ï-İ]` and `(?a)[^\u0000-\u00ff]` have matches; the engine finds no
  string for them. `test_ascii_flag_does_not_restrict_characters_or_ranges` in
  `tests/hypothesis_port/cover/test_regex.py` is xfailed.
- **Repetitions of an unproducible subpattern are rejected, not skipped.**
  For `(?-i:Ā)*k` with an alphabet of code points up to 127, the engine keeps
  choosing one or more repetitions of `Ā`, which the alphabet cannot supply,
  and rejects the attempt; about a third of runs then trip `FilterTooMuch`
  instead of producing `k`. `test_inline_flags_do_not_leak_out_of_an_incompatible_subpattern`
  is a non-strict xfail.
- **Case-insensitive negated sets** sometimes produce strings that do not
  match (`tests/hypothesis_port/cover/test_regex.py`, non-strict xfail from
  the port).

## Other

- `sets(sampled_from(range(n)), min_size=n)` fails `FilterTooMuch` because the
  engine's collection generation does not sample without replacement
  (`tests/hypothesis_port/nocover/test_sampled_from.py`).
- Bounded integers are drawn with a heavy weight on huge values:
  `integers(0, 10**100)` produced values above `10**25` about two thirds of
  the time (`tests/hypothesis_port/nocover/test_integer_ranges.py`).
- Identical inputs are generated repeatedly; there is no duplicate suppression
  (`tests/hypothesis_port/nocover/test_duplication.py`).
- A `wide | just(needle)` strategy shrinks to the needle rather than the
  minimal wide value (`tests/hypothesis_port/quality/test_widening_shrinks.py`).
