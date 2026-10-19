//! Port of Python's `unicodedata` module for General Category queries,
//! and of the simple case mappings CPython's `re` matches by.
//!
//! Backed by three text data files generated from
//! `src/unicodedata/UnicodeData.txt` and `SpecialCasing.txt` (vendored
//! Unicode 17.0.0 UCD). The files are embedded with `include_str!` and
//! parsed once on first lookup into a `Lazy`-backed `Vec`, keeping
//! `cargo build` from having to parse ~6000 tuple literals per compile.
//!
//! - `categories.txt`: contiguous category runs covering every codepoint in
//!   `0..=0x10FFFF`. Codepoints not listed in UnicodeData.txt are reported
//!   as `Cn` (Unassigned), matching Python.
//! - `nfd_bases.txt`: recursive NFD base for each canonically-decomposable
//!   codepoint.
//! - `case.txt`: the lowercase and uppercase form of each codepoint that
//!   has one other than itself, as CPython's `_PyUnicode_ToLowercase` and
//!   `_PyUnicode_ToUppercase` map it (see [`to_lower`]).
//!
//! To refresh the vendored data, replace the two UCD files and run
//! `scripts/generate_unicodedata_tables.py`, then regenerate the test
//! reference with `scripts/generate_unicodedata_python_reference.py`
//! (`just unicodedata` does both).

use crate::control::{InternalError, hegel_internal_assert};
use crate::sys::sync::Lazy;
use alloc::vec::Vec;

/// Unicode General Category.
///
/// Names match the two-character abbreviations returned by Python's
/// `unicodedata.category(c)`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Category {
    Lu,
    Ll,
    Lt,
    Lm,
    Lo,
    Mn,
    Mc,
    Me,
    Nd,
    Nl,
    No,
    Pc,
    Pd,
    Ps,
    Pe,
    Pi,
    Pf,
    Po,
    Sm,
    Sc,
    Sk,
    So,
    Zs,
    Zl,
    Zp,
    Cc,
    Cf,
    Cs,
    Co,
    Cn,
}

impl Category {
    /// Two-character category code, e.g. `"Lu"` or `"Cn"`. Matches Python's
    /// `unicodedata.category` string.
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Lu => "Lu",
            Category::Ll => "Ll",
            Category::Lt => "Lt",
            Category::Lm => "Lm",
            Category::Lo => "Lo",
            Category::Mn => "Mn",
            Category::Mc => "Mc",
            Category::Me => "Me",
            Category::Nd => "Nd",
            Category::Nl => "Nl",
            Category::No => "No",
            Category::Pc => "Pc",
            Category::Pd => "Pd",
            Category::Ps => "Ps",
            Category::Pe => "Pe",
            Category::Pi => "Pi",
            Category::Pf => "Pf",
            Category::Po => "Po",
            Category::Sm => "Sm",
            Category::Sc => "Sc",
            Category::Sk => "Sk",
            Category::So => "So",
            Category::Zs => "Zs",
            Category::Zl => "Zl",
            Category::Zp => "Zp",
            Category::Cc => "Cc",
            Category::Cf => "Cf",
            Category::Cs => "Cs",
            Category::Co => "Co",
            Category::Cn => "Cn",
        }
    }

    /// Inverse of [`as_str`]. Returns `None` for unknown codes.
    fn from_code(s: &str) -> Option<Category> {
        Some(match s {
            "Lu" => Category::Lu,
            "Ll" => Category::Ll,
            "Lt" => Category::Lt,
            "Lm" => Category::Lm,
            "Lo" => Category::Lo,
            "Mn" => Category::Mn,
            "Mc" => Category::Mc,
            "Me" => Category::Me,
            "Nd" => Category::Nd,
            "Nl" => Category::Nl,
            "No" => Category::No,
            "Pc" => Category::Pc,
            "Pd" => Category::Pd,
            "Ps" => Category::Ps,
            "Pe" => Category::Pe,
            "Pi" => Category::Pi,
            "Pf" => Category::Pf,
            "Po" => Category::Po,
            "Sm" => Category::Sm,
            "Sc" => Category::Sc,
            "Sk" => Category::Sk,
            "So" => Category::So,
            "Zs" => Category::Zs,
            "Zl" => Category::Zl,
            "Zp" => Category::Zp,
            "Cc" => Category::Cc,
            "Cf" => Category::Cf,
            "Cs" => Category::Cs,
            "Co" => Category::Co,
            "Cn" => Category::Cn,
            _ => return None,
        })
    }
}

/// Embedded raw data; parsed lazily into [`ranges`], [`nfd_bases`] and
/// [`case_mappings`].
const CATEGORIES_DATA: &str = include_str!("categories.txt");
const NFD_BASES_DATA: &str = include_str!("nfd_bases.txt");
const CASE_DATA: &str = include_str!("case.txt");

/// Run-length-encoded General Category table covering all codepoints
/// in `0..=0x10FFFF`. Entries are non-overlapping, contiguous, and sorted
/// by `end`; lookup is binary search for the first entry with `end >= cp`.
fn ranges() -> &'static [(u32, Category)] {
    static RANGES: Lazy<Vec<(u32, Category)>> = Lazy::new(|| {
        CATEGORIES_DATA
            .lines()
            .map(|line| {
                let (end, cat) = line
                    .split_once(' ')
                    .unwrap_or_else(|| panic!("malformed categories.txt line: {line:?}"));
                let end = u32::from_str_radix(end, 16)
                    .unwrap_or_else(|_| panic!("bad hex in categories.txt: {end:?}"));
                let cat = Category::from_code(cat)
                    .unwrap_or_else(|| panic!("unknown category in categories.txt: {cat:?}"));
                (end, cat)
            })
            .collect()
    });
    &RANGES
}

/// Recursive NFD base for codepoints that have a canonical decomposition,
/// sorted by codepoint. Codepoints not in this table either have no
/// canonical decomposition or are already their own base.
fn nfd_bases() -> &'static [(u32, u32)] {
    static BASES: Lazy<Vec<(u32, u32)>> = Lazy::new(|| {
        NFD_BASES_DATA
            .lines()
            .map(|line| {
                let (cp, base) = line
                    .split_once(' ')
                    .unwrap_or_else(|| panic!("malformed nfd_bases.txt line: {line:?}"));
                let cp = u32::from_str_radix(cp, 16)
                    .unwrap_or_else(|_| panic!("bad hex in nfd_bases.txt: {cp:?}"));
                let base = u32::from_str_radix(base, 16)
                    .unwrap_or_else(|_| panic!("bad hex in nfd_bases.txt: {base:?}"));
                (cp, base)
            })
            .collect()
    });
    &BASES
}

/// The `(codepoint, lowercase, uppercase)` of every codepoint with a
/// lowercase or uppercase form other than itself, sorted by codepoint.
fn case_mappings() -> &'static [(u32, u32, u32)] {
    static MAPPINGS: Lazy<Vec<(u32, u32, u32)>> = Lazy::new(|| {
        CASE_DATA
            .lines()
            .map(|line| {
                let mut fields = line.split(' ').map(|field| {
                    u32::from_str_radix(field, 16)
                        .unwrap_or_else(|_| panic!("bad hex in case.txt: {field:?}"))
                });
                let mut next = || {
                    fields
                        .next()
                        .unwrap_or_else(|| panic!("malformed case.txt line: {line:?}"))
                };
                (next(), next(), next())
            })
            .collect()
    });
    &MAPPINGS
}

fn case_mapping(c: char) -> Option<(u32, u32)> {
    let mappings = case_mappings();
    let i = mappings
        .binary_search_by_key(&(c as u32), |&(cp, _, _)| cp)
        .ok()?;
    let (_, lower, upper) = mappings[i];
    Some((lower, upper))
}

fn char_of(cp: u32, c: char) -> char {
    char::from_u32(cp).unwrap_or(c)
}

/// The lowercase form of `c`, or `c` itself when it has none: CPython's
/// `_PyUnicode_ToLowercase`, which `re.IGNORECASE` compares characters
/// by. It is the simple lowercase mapping, except for a character whose
/// full lowercase mapping is special-cased, which maps to that mapping's
/// first codepoint (U+0130 to `i`).
pub fn to_lower(c: char) -> char {
    case_mapping(c).map_or(c, |(lower, _)| char_of(lower, c))
}

/// The uppercase form of `c`, or `c` itself when it has none: CPython's
/// `_PyUnicode_ToUppercase`, the first codepoint of a special-cased full
/// mapping as [`to_lower`] takes it (`ß` to `S`).
pub fn to_upper(c: char) -> char {
    case_mapping(c).map_or(c, |(_, upper)| char_of(upper, c))
}

/// The General Category table as `(start, end, category)` runs of equal
/// category, in increasing order and together covering every codepoint in
/// `0..=0x10FFFF`.
pub fn category_runs() -> impl Iterator<Item = (u32, u32, Category)> {
    let mut start = 0;
    ranges().iter().map(move |&(end, cat)| {
        let run = (start, end, cat);
        start = end + 1;
        run
    })
}

/// The runs of [`category_runs`] restricted to the codepoints `lo..=hi`:
/// the first and last runs are clipped to that range, codepoints above
/// `0x10FFFF` are left out, and an empty range yields nothing. Walking
/// these answers a category question about every codepoint of a range
/// without a table lookup per codepoint.
pub fn category_runs_in(lo: u32, hi: u32) -> impl Iterator<Item = (u32, u32, Category)> {
    let table = ranges();
    let hi = hi.min(0x10FFFF);
    let first = if lo > hi {
        table.len()
    } else {
        table.partition_point(|&(end, _)| end < lo)
    };
    let mut start = if first == 0 {
        0
    } else {
        table[first - 1].0 + 1
    };
    table[first..].iter().map_while(move |&(end, cat)| {
        let run_start = start;
        start = end + 1;
        (run_start <= hi).then(|| (run_start.max(lo), end.min(hi), cat))
    })
}

/// Return the Unicode General Category for `cp`.
///
/// Matches Python's `unicodedata.category(chr(cp))` for every codepoint in
/// `0..=0x10FFFF`. Codepoints outside that range are an internal error,
/// mirroring Python's `chr(cp)` which rejects `cp > 0x10FFFF`.
pub fn general_category(cp: u32) -> Result<Category, InternalError> {
    hegel_internal_assert!(cp <= 0x10FFFF, "codepoint {:#x} out of range", cp);
    Ok(category_lookup(cp))
}

/// [`general_category`] for a `char`, whose type already guarantees a
/// codepoint in range — so the lookup cannot fail.
#[cfg(test)]
pub fn general_category_char(c: char) -> Category {
    category_lookup(c as u32)
}

/// Table lookup behind [`general_category`]. Callers must pass
/// `cp <= 0x10FFFF` (the binary search indexes past the table otherwise).
fn category_lookup(cp: u32) -> Category {
    let table = ranges();
    let idx = table
        .binary_search_by(|&(end, _)| {
            if end < cp {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Greater
            }
        })
        .unwrap_err();
    table[idx].1
}

/// Test whether `cp` belongs to the category `group`.
///
/// `group` may be a full two-character category code (`"Lu"`, `"Cn"`, ...)
/// or a single-letter major class (`"L"`, `"N"`, ...) in which case all
/// subcategories with that prefix match. Mirrors Hypothesis's
/// `as_general_categories` prefix semantics (see
/// `hypothesis/internal/charmap.py`).
///
/// Unknown `group` strings match nothing.
pub fn is_in_group(cp: u32, group: &str) -> Result<bool, InternalError> {
    Ok(group_matches(general_category(cp)?.as_str(), group))
}

/// Shared group-matching rule behind [`is_in_group`].
fn group_matches(cat: &str, group: &str) -> bool {
    match group.len() {
        2 => cat == group,
        1 => cat.starts_with(group) && MAJOR_CLASSES.contains(&group),
        _ => false,
    }
}

const MAJOR_CLASSES: &[&str] = &["L", "M", "N", "P", "S", "Z", "C"];

/// Return the recursive NFD base codepoint for `cp`, if it has a canonical
/// decomposition that resolves to a different starting codepoint.
///
/// For example, `À` (U+00C0) → `A` (U+0041), `Ǻ` (U+01FA) → `A`, but `A`
/// itself returns `None`. The returned codepoint is itself non-decomposable
/// (i.e. applying `nfd_base` to the result yields `None`).
///
/// Codepoints with no canonical decomposition (including emoji, CJK
/// ideographs, and ASCII) return `None`. Compatibility decompositions
/// (NFKD, e.g. `Ⅰ` → `I`) are *not* applied — they're not part of NFD.
pub fn nfd_base(cp: u32) -> Option<u32> {
    let table = nfd_bases();
    table
        .binary_search_by_key(&cp, |&(c, _)| c)
        .ok()
        .map(|idx| table[idx].1)
}

#[cfg(test)]
#[path = "../../tests/embedded/native/unicodedata_tests.rs"]
mod tests;
