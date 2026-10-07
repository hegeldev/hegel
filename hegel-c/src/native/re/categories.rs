//! The Unicode-mode character classes `\d`, `\s` and `\w` as tables over
//! the codespace, for the matcher's per-character tests and for the
//! lexer simulation's first-character sets. CPython defines the classes
//! through `unicodedata` (decimal digits; the `Z` categories plus a few
//! controls; letters, numbers and `_`), which here become one bitmap over
//! the Basic Multilingual Plane and sorted ranges above it, built once
//! from the General Category runs.

use super::constants::ChCode;
use crate::sys::sync::Lazy;
use crate::unicodedata::{self, Category};
use alloc::vec::Vec;

const BMP_END: u32 = 0x10000;

/// One class: a bit per codepoint below [`BMP_END`], ranges above it, and
/// the ranges of the whole class.
struct ClassTable {
    bmp: Vec<u64>,
    high: Vec<(u32, u32)>,
    ranges: Vec<(u32, u32)>,
}

impl ClassTable {
    fn new(ranges: Vec<(u32, u32)>) -> ClassTable {
        let mut bmp = alloc::vec![0u64; (BMP_END / 64) as usize];
        let mut high = Vec::new();
        for &(start, end) in &ranges {
            for cp in start..=end.min(BMP_END - 1) {
                bmp[(cp / 64) as usize] |= 1 << (cp % 64);
            }
            if end >= BMP_END {
                high.push((start.max(BMP_END), end));
            }
        }
        ClassTable { bmp, high, ranges }
    }

    fn contains(&self, c: char) -> bool {
        let cp = c as u32;
        if cp < BMP_END {
            return self.bmp[(cp / 64) as usize] & (1 << (cp % 64)) != 0;
        }
        self.high
            .binary_search_by(|&(start, end)| {
                if end < cp {
                    core::cmp::Ordering::Less
                } else if start > cp {
                    core::cmp::Ordering::Greater
                } else {
                    core::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }
}

/// Whether CPython counts the character `cp` of General Category
/// `category` as a digit, a space and a word character, in Unicode mode.
fn classify(cp: u32, category: Category) -> [bool; 3] {
    let major = category.as_str().as_bytes()[0];
    let digit = category == Category::Nd;
    let space = matches!(cp, 0x9..=0xd | 0x1c..=0x20 | 0x85) || major == b'Z';
    let word = cp == u32::from(b'_') || major == b'L' || major == b'N';
    [digit, space, word]
}

static TABLES: Lazy<[ClassTable; 3]> = Lazy::new(|| {
    let mut ranges: [Vec<(u32, u32)>; 3] = Default::default();
    for (start, end, category) in unicodedata::category_runs() {
        let mut cp = start;
        while cp <= end {
            let classes = classify(cp, category);
            let run_end = if cp < 0x100 { cp } else { end };
            for (i, ranges) in ranges.iter_mut().enumerate() {
                if classes[i] {
                    match ranges.last_mut() {
                        Some(last) if last.1 + 1 == cp => last.1 = run_end,
                        _ => ranges.push((cp, run_end)),
                    }
                }
            }
            cp = run_end + 1;
        }
    }
    ranges.map(ClassTable::new)
});

fn table(category: ChCode) -> Option<&'static ClassTable> {
    let tables = Lazy::force(&TABLES);
    match category {
        ChCode::Digit => Some(&tables[0]),
        ChCode::Space => Some(&tables[1]),
        ChCode::Word => Some(&tables[2]),
        ChCode::NotDigit | ChCode::NotSpace | ChCode::NotWord => None,
    }
}

/// Whether `c` is in the positive class `category` in Unicode mode;
/// `false` for the negated classes, which have no table.
pub fn in_class(c: char, category: ChCode) -> bool {
    table(category).is_some_and(|table| table.contains(c))
}

/// The codepoint ranges of the positive class `category` in Unicode
/// mode, in increasing order with gaps between them, or `None` for the
/// negated classes.
pub fn class_ranges(category: ChCode) -> Option<&'static [(u32, u32)]> {
    table(category).map(|table| table.ranges.as_slice())
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/re_categories_tests.rs"]
mod tests;
