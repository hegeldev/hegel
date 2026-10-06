//! Direct-equivalent tests for `src/native/re/width.rs`. Expected widths
//! come from CPython's `re._parser.parse(p).getwidth()`, with CPython's
//! `MAXWIDTH` shown here as `None`.

use super::*;
use crate::native::re::parser::parse_pattern;

fn width(pattern: &str) -> (u64, Option<u64>) {
    get_width(&parse_pattern(pattern, 0).unwrap().pattern)
}

#[test]
fn literals_and_classes_are_one_wide() {
    assert_eq!(width("abc"), (3, Some(3)));
    assert_eq!(width(r"[a-z]\d."), (3, Some(3)));
    assert_eq!(width("[^x]"), (1, Some(1)));
}

#[test]
fn anchors_and_lookarounds_are_zero_wide() {
    assert_eq!(width(r"^a$\b"), (1, Some(1)));
    assert_eq!(width("(?=abc)(?!x)(?<=y)(?<!zz)"), (0, Some(0)));
    assert_eq!(width("(?!)"), (0, Some(0)));
}

#[test]
fn branches_take_min_and_max() {
    assert_eq!(width("a|bc|def"), (1, Some(3)));
    assert_eq!(width("a|b*"), (0, None));
}

#[test]
fn repeats_scale_the_item() {
    assert_eq!(width("a{2,5}"), (2, Some(5)));
    assert_eq!(width("(ab){3}"), (6, Some(6)));
    assert_eq!(width("a+"), (1, None));
    assert_eq!(width("a*?"), (0, None));
    assert_eq!(width("a?+"), (0, Some(1)));
    assert_eq!(width("(?:)*"), (0, Some(0)));
}

#[test]
fn groups_and_atomic_groups_pass_through() {
    assert_eq!(width("(?:ab)(c)(?>de)"), (5, Some(5)));
    assert_eq!(width("(?i:ab)"), (2, Some(2)));
}

#[test]
fn backreferences_use_the_group_width() {
    assert_eq!(width(r"(ab)\1"), (4, Some(4)));
    assert_eq!(width(r"(a+)\1"), (2, None));
}

#[test]
fn conditional_groups() {
    assert_eq!(width("(a)?(?(1)bc|d)"), (1, Some(3)));
    assert_eq!(width("(a)?(?(1)bc)"), (0, Some(3)));
}

#[test]
fn saturates_instead_of_overflowing() {
    assert_eq!(
        width("(?:(?:a{4294967294}){4294967294}){4294967294}"),
        (u64::MAX, Some(u64::MAX))
    );
}
