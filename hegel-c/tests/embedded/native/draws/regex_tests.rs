//! Embedded tests for the SRE-style matcher in `src/native/draws/regex.rs`.
//!
//! The matcher (`match_seq`) is internal: it's only exercised through
//! negative-lookahead validation, where the body shape determines which
//! arms get evaluated. End-to-end tests cover the literal-only path
//! comfortably, but the more complex arms (`Branch`, `MaxRepeat`,
//! `GroupRef`, etc.) need patterns that the generator may rarely emit
//! against. These direct-call tests pin each arm independently of the
//! generator's draw distribution.

use super::*;
use crate::native::HashMap;
use crate::native::bignum::BigInt;
use crate::native::core::{ChoiceValue, NativeTestCase};
use crate::native::re::constants::{
    AtCode, ChCode, SRE_FLAG_DOTALL, SRE_FLAG_IGNORECASE, SRE_FLAG_MULTILINE,
};
use crate::native::re::parser::{OpCode, SetItem, SubPattern};
use alloc::string::ToString;

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn lit(cp: char) -> OpCode {
    OpCode::Literal(cp as u32)
}

fn sub(ops: Vec<OpCode>) -> SubPattern {
    SubPattern { data: ops }
}

#[test]
fn match_seq_literal_match() {
    let groups = HashMap::default();
    assert_eq!(match_seq(&[lit('a')], 0, &chars("a"), 0, &groups), Some(1));
}

#[test]
fn match_seq_literal_no_match() {
    let groups = HashMap::default();
    assert_eq!(match_seq(&[lit('a')], 0, &chars("b"), 0, &groups), None);
}

#[test]
fn match_seq_not_literal_match() {
    let groups = HashMap::default();
    assert_eq!(
        match_seq(
            &[OpCode::NotLiteral('a' as u32)],
            0,
            &chars("b"),
            0,
            &groups
        ),
        Some(1)
    );
}

#[test]
fn match_seq_not_literal_no_match() {
    let groups = HashMap::default();
    assert_eq!(
        match_seq(
            &[OpCode::NotLiteral('a' as u32)],
            0,
            &chars("a"),
            0,
            &groups
        ),
        None
    );
}

#[test]
fn match_seq_any_matches_non_newline() {
    let groups = HashMap::default();
    assert_eq!(
        match_seq(&[OpCode::Any], 0, &chars("x"), 0, &groups),
        Some(1)
    );
}

#[test]
fn match_seq_any_does_not_match_newline_without_dotall() {
    let groups = HashMap::default();
    assert_eq!(match_seq(&[OpCode::Any], 0, &chars("\n"), 0, &groups), None);
}

#[test]
fn match_seq_any_matches_newline_with_dotall() {
    let groups = HashMap::default();
    assert_eq!(
        match_seq(&[OpCode::Any], 0, &chars("\n"), SRE_FLAG_DOTALL, &groups),
        Some(1)
    );
}

#[test]
fn match_seq_in_set_literal_match() {
    let groups = HashMap::default();
    let items = vec![SetItem::Literal('a' as u32), SetItem::Literal('b' as u32)];
    assert_eq!(
        match_seq(&[OpCode::In(items.clone())], 0, &chars("a"), 0, &groups),
        Some(1)
    );
    assert_eq!(
        match_seq(&[OpCode::In(items)], 0, &chars("c"), 0, &groups),
        None
    );
}

#[test]
fn match_seq_in_set_range_match() {
    let groups = HashMap::default();
    let items = vec![SetItem::Range('a' as u32, 'z' as u32)];
    assert_eq!(
        match_seq(&[OpCode::In(items.clone())], 0, &chars("m"), 0, &groups),
        Some(1)
    );
    assert_eq!(
        match_seq(&[OpCode::In(items)], 0, &chars("A"), 0, &groups),
        None
    );
}

#[test]
fn match_seq_in_set_range_ignorecase() {
    let groups = HashMap::default();
    let items = vec![SetItem::Range('a' as u32, 'z' as u32)];
    assert_eq!(
        match_seq(
            &[OpCode::In(items)],
            0,
            &chars("M"),
            SRE_FLAG_IGNORECASE,
            &groups
        ),
        Some(1)
    );
}

#[test]
fn match_seq_in_set_category_match() {
    let groups = HashMap::default();
    let items = vec![SetItem::Category(ChCode::Digit)];
    assert_eq!(
        match_seq(&[OpCode::In(items.clone())], 0, &chars("5"), 0, &groups),
        Some(1)
    );
    assert_eq!(
        match_seq(&[OpCode::In(items)], 0, &chars("a"), 0, &groups),
        None
    );
}

#[test]
fn match_seq_in_set_negated() {
    let groups = HashMap::default();
    let items = vec![SetItem::Negate, SetItem::Literal('a' as u32)];
    assert_eq!(
        match_seq(&[OpCode::In(items.clone())], 0, &chars("b"), 0, &groups),
        Some(1)
    );
    assert_eq!(
        match_seq(&[OpCode::In(items)], 0, &chars("a"), 0, &groups),
        None
    );
}

#[test]
fn match_seq_at_beginning_string() {
    let groups = HashMap::default();
    assert_eq!(
        match_seq(
            &[OpCode::At(AtCode::BeginningString)],
            0,
            &chars(""),
            0,
            &groups
        ),
        Some(0)
    );
    assert_eq!(
        match_seq(
            &[OpCode::At(AtCode::BeginningString)],
            1,
            &chars("ab"),
            0,
            &groups
        ),
        None
    );
}

#[test]
fn match_seq_at_beginning() {
    let groups = HashMap::default();
    assert_eq!(
        match_seq(&[OpCode::At(AtCode::Beginning)], 0, &chars("a"), 0, &groups),
        Some(0)
    );
    assert_eq!(
        match_seq(
            &[OpCode::At(AtCode::Beginning)],
            1,
            &chars("ab"),
            0,
            &groups
        ),
        None
    );
    assert_eq!(
        match_seq(
            &[OpCode::At(AtCode::Beginning)],
            1,
            &chars("\na"),
            SRE_FLAG_MULTILINE,
            &groups
        ),
        Some(1)
    );
}

#[test]
fn match_seq_at_end() {
    let groups = HashMap::default();
    assert_eq!(
        match_seq(&[OpCode::At(AtCode::End)], 1, &chars("a"), 0, &groups),
        Some(1)
    );
    assert_eq!(
        match_seq(&[OpCode::At(AtCode::End)], 1, &chars("ab"), 0, &groups),
        None
    );
    assert_eq!(
        match_seq(&[OpCode::At(AtCode::End)], 1, &chars("a\n"), 0, &groups),
        Some(1)
    );
    assert_eq!(
        match_seq(
            &[OpCode::At(AtCode::End)],
            1,
            &chars("a\nb"),
            SRE_FLAG_MULTILINE,
            &groups
        ),
        Some(1)
    );
}

#[test]
fn match_seq_at_end_string() {
    let groups = HashMap::default();
    assert_eq!(
        match_seq(&[OpCode::At(AtCode::EndString)], 1, &chars("a"), 0, &groups),
        Some(1)
    );
    assert_eq!(
        match_seq(
            &[OpCode::At(AtCode::EndString)],
            1,
            &chars("ab"),
            0,
            &groups
        ),
        None
    );
}

#[test]
fn match_seq_at_word_boundary() {
    let groups = HashMap::default();
    assert_eq!(
        match_seq(&[OpCode::At(AtCode::Boundary)], 1, &chars("ab"), 0, &groups),
        None
    );
    assert_eq!(
        match_seq(&[OpCode::At(AtCode::Boundary)], 0, &chars("ab"), 0, &groups),
        Some(0)
    );
    assert_eq!(
        match_seq(
            &[OpCode::At(AtCode::NonBoundary)],
            1,
            &chars("ab"),
            0,
            &groups
        ),
        Some(1)
    );
}

#[test]
fn match_seq_branch_first_arm_matches() {
    let groups = HashMap::default();
    let ops = vec![OpCode::Branch(vec![
        sub(vec![lit('a')]),
        sub(vec![lit('b')]),
    ])];
    assert_eq!(match_seq(&ops, 0, &chars("a"), 0, &groups), Some(1));
}

#[test]
fn match_seq_branch_second_arm_matches() {
    let groups = HashMap::default();
    let ops = vec![OpCode::Branch(vec![
        sub(vec![lit('a')]),
        sub(vec![lit('b')]),
    ])];
    assert_eq!(match_seq(&ops, 0, &chars("b"), 0, &groups), Some(1));
}

#[test]
fn match_seq_branch_no_match() {
    let groups = HashMap::default();
    let ops = vec![OpCode::Branch(vec![
        sub(vec![lit('a')]),
        sub(vec![lit('b')]),
    ])];
    assert_eq!(match_seq(&ops, 0, &chars("c"), 0, &groups), None);
}

#[test]
fn match_seq_subpattern() {
    let groups = HashMap::default();
    let ops = vec![OpCode::Subpattern {
        group: Some(1),
        add_flags: 0,
        del_flags: 0,
        p: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("a"), 0, &groups), Some(1));
}

#[test]
fn match_seq_subpattern_inline_flags() {
    let groups = HashMap::default();
    let ops = vec![OpCode::Subpattern {
        group: None,
        add_flags: SRE_FLAG_IGNORECASE,
        del_flags: 0,
        p: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("A"), 0, &groups), Some(1));
}

#[test]
fn match_seq_atomic_group() {
    let groups = HashMap::default();
    let ops = vec![OpCode::AtomicGroup(sub(vec![lit('a')]))];
    assert_eq!(match_seq(&ops, 0, &chars("a"), 0, &groups), Some(1));
    assert_eq!(match_seq(&ops, 0, &chars("b"), 0, &groups), None);
}

#[test]
fn match_seq_groupref_match() {
    let mut groups = HashMap::default();
    groups.insert(1, chars("ab"));
    let ops = vec![OpCode::GroupRef(1)];
    assert_eq!(match_seq(&ops, 0, &chars("ab"), 0, &groups), Some(2));
}

#[test]
fn match_seq_groupref_too_short() {
    let mut groups = HashMap::default();
    groups.insert(1, chars("abc"));
    let ops = vec![OpCode::GroupRef(1)];
    assert_eq!(match_seq(&ops, 0, &chars("ab"), 0, &groups), None);
}

#[test]
fn match_seq_groupref_mismatched() {
    let mut groups = HashMap::default();
    groups.insert(1, chars("ab"));
    let ops = vec![OpCode::GroupRef(1)];
    assert_eq!(match_seq(&ops, 0, &chars("xy"), 0, &groups), None);
}

#[test]
fn match_seq_groupref_unset() {
    let groups = HashMap::default();
    let ops = vec![OpCode::GroupRef(1)];
    assert_eq!(match_seq(&ops, 0, &chars("ab"), 0, &groups), None);
}

#[test]
fn match_seq_groupref_exists_yes_arm() {
    let mut groups = HashMap::default();
    groups.insert(1, chars("x"));
    let ops = vec![OpCode::GroupRefExists {
        cond_group: 1,
        yes: sub(vec![lit('a')]),
        no: Some(sub(vec![lit('b')])),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("a"), 0, &groups), Some(1));
}

#[test]
fn match_seq_groupref_exists_no_arm() {
    let groups = HashMap::default();
    let ops = vec![OpCode::GroupRefExists {
        cond_group: 1,
        yes: sub(vec![lit('a')]),
        no: Some(sub(vec![lit('b')])),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("b"), 0, &groups), Some(1));
}

#[test]
fn match_seq_groupref_exists_no_arm_missing() {
    let groups = HashMap::default();
    let ops = vec![OpCode::GroupRefExists {
        cond_group: 1,
        yes: sub(vec![lit('a')]),
        no: None,
    }];
    assert_eq!(match_seq(&ops, 0, &chars(""), 0, &groups), Some(0));
}

#[test]
fn match_seq_positive_lookaround_match() {
    let groups = HashMap::default();
    let ops = vec![
        OpCode::Assert {
            direction: 1,
            p: sub(vec![lit('a')]),
        },
        lit('a'),
    ];
    assert_eq!(match_seq(&ops, 0, &chars("a"), 0, &groups), Some(1));
}

#[test]
fn match_seq_positive_lookaround_no_match() {
    let groups = HashMap::default();
    let ops = vec![OpCode::Assert {
        direction: 1,
        p: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("b"), 0, &groups), None);
}

#[test]
fn match_seq_negative_lookaround_match() {
    let groups = HashMap::default();
    let ops = vec![
        OpCode::AssertNot {
            direction: 1,
            p: sub(vec![lit('a')]),
        },
        lit('b'),
    ];
    assert_eq!(match_seq(&ops, 0, &chars("b"), 0, &groups), Some(1));
}

#[test]
fn match_seq_negative_lookaround_blocks() {
    let groups = HashMap::default();
    let ops = vec![OpCode::AssertNot {
        direction: 1,
        p: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("a"), 0, &groups), None);
}

#[test]
fn match_seq_failure_never_matches() {
    let groups = HashMap::default();
    assert_eq!(
        match_seq(&[OpCode::Failure], 0, &chars(""), 0, &groups),
        None
    );
}

#[test]
fn match_seq_max_repeat_unbounded() {
    let groups = HashMap::default();
    let ops = vec![OpCode::MaxRepeat {
        min: 0,
        max: u32::MAX,
        item: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("aaa"), 0, &groups), Some(3));
    assert_eq!(match_seq(&ops, 0, &chars(""), 0, &groups), Some(0));
}

#[test]
fn match_seq_max_repeat_bounded() {
    let groups = HashMap::default();
    let ops = vec![OpCode::MaxRepeat {
        min: 2,
        max: 3,
        item: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("aaaa"), 0, &groups), Some(3));
}

#[test]
fn match_seq_max_repeat_min_unsatisfied() {
    let groups = HashMap::default();
    let ops = vec![OpCode::MaxRepeat {
        min: 3,
        max: 5,
        item: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("aa"), 0, &groups), None);
}

#[test]
fn match_seq_max_repeat_with_trailing() {
    let groups = HashMap::default();
    let ops = vec![
        OpCode::MaxRepeat {
            min: 1,
            max: 3,
            item: sub(vec![lit('a')]),
        },
        lit('b'),
    ];
    assert_eq!(match_seq(&ops, 0, &chars("aaab"), 0, &groups), Some(4));
}

#[test]
fn match_seq_min_repeat_lazy() {
    let groups = HashMap::default();
    let ops = vec![
        OpCode::MinRepeat {
            min: 0,
            max: u32::MAX,
            item: sub(vec![lit('a')]),
        },
        lit('b'),
    ];
    assert_eq!(match_seq(&ops, 0, &chars("aaab"), 0, &groups), Some(4));
}

#[test]
fn match_seq_min_repeat_bounded() {
    let groups = HashMap::default();
    let ops = vec![OpCode::MinRepeat {
        min: 1,
        max: 2,
        item: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("a"), 0, &groups), Some(1));
}

#[test]
fn match_seq_min_repeat_no_match() {
    let groups = HashMap::default();
    let ops = vec![
        OpCode::MinRepeat {
            min: 0,
            max: u32::MAX,
            item: sub(vec![lit('a')]),
        },
        lit('b'),
    ];
    assert_eq!(match_seq(&ops, 0, &chars("aaa"), 0, &groups), None);
}

#[test]
fn match_seq_min_repeat_min_unsatisfied() {
    let groups = HashMap::default();
    let ops = vec![OpCode::MinRepeat {
        min: 3,
        max: 5,
        item: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("aa"), 0, &groups), None);
}

#[test]
fn match_seq_min_repeat_max_exhausted() {
    let groups = HashMap::default();
    let ops = vec![
        OpCode::MinRepeat {
            min: 0,
            max: 2,
            item: sub(vec![lit('a')]),
        },
        lit('b'),
    ];
    assert_eq!(match_seq(&ops, 0, &chars("aaab"), 0, &groups), None);
}

#[test]
fn match_seq_possessive_repeat() {
    let groups = HashMap::default();
    let ops = vec![OpCode::PossessiveRepeat {
        min: 0,
        max: u32::MAX,
        item: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("aaa"), 0, &groups), Some(3));
}

#[test]
fn match_seq_possessive_repeat_bounded() {
    let groups = HashMap::default();
    let ops = vec![OpCode::PossessiveRepeat {
        min: 0,
        max: 2,
        item: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("aaa"), 0, &groups), Some(2));
}

#[test]
fn match_seq_possessive_repeat_min_unsatisfied() {
    let groups = HashMap::default();
    let ops = vec![OpCode::PossessiveRepeat {
        min: 3,
        max: 5,
        item: sub(vec![lit('a')]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars("a"), 0, &groups), None);
}

#[test]
fn match_seq_min_repeat_zero_width_item_at_min() {
    let groups = HashMap::default();
    let ops = vec![OpCode::MinRepeat {
        min: 1,
        max: u32::MAX,
        item: sub(vec![]),
    }];
    assert_eq!(match_seq(&ops, 0, &chars(""), 0, &groups), Some(0));
}

#[test]
fn match_seq_min_repeat_zero_width_item_after_min() {
    let groups = HashMap::default();
    let ops = vec![
        OpCode::MinRepeat {
            min: 0,
            max: u32::MAX,
            item: sub(vec![]),
        },
        lit('a'),
    ];
    assert_eq!(match_seq(&ops, 0, &chars(""), 0, &groups), None);
}

fn set_len(set: &InSet) -> u64 {
    match set {
        InSet::Listed(chars) => chars.len() as u64,
        InSet::Complement(complement) => complement.len,
    }
}

fn listed(set: &InSet) -> Vec<char> {
    match set {
        InSet::Listed(chars) => chars.to_vec(),
        InSet::Complement(complement) => (0..complement.len)
            .map(|idx| complement.get(idx).unwrap())
            .collect(),
    }
}

#[test]
fn build_in_set_ascii_only_drops_nonascii_positive_literal() {
    let items = vec![SetItem::Literal('a' as u32), SetItem::Literal(0xFF)];
    let out = build_in_set(&items, SRE_FLAG_ASCII, &None).unwrap();
    assert_eq!(out, InSet::Listed(vec!['a'].into()));
}

#[test]
fn build_in_set_alphabet_drops_disallowed_positive_literal() {
    let items = vec![SetItem::Literal('a' as u32), SetItem::Literal('b' as u32)];
    let alphabet = IntervalSet::new(vec![('a' as u32, 'a' as u32)]).unwrap();
    let out = build_in_set(&items, 0, &Some(alphabet)).unwrap();
    assert_eq!(out, InSet::Listed(vec!['a'].into()));
}

#[test]
fn build_in_set_negated_ascii_only_excludes_nonascii() {
    let items = vec![SetItem::Negate, SetItem::Literal('a' as u32)];
    let alphabet = IntervalSet::new(vec![(b' ' as u32, 0x100)]).unwrap();
    let out = build_in_set(&items, SRE_FLAG_ASCII, &Some(alphabet)).unwrap();
    assert!(listed(&out).iter().all(|c| (*c as u32) < 128 && *c != 'a'));
}

#[test]
fn build_in_set_negated_default_set_is_the_complement_of_its_literals() {
    let items = vec![
        SetItem::Negate,
        SetItem::Literal('k' as u32),
        SetItem::Range('0' as u32, '2' as u32),
        SetItem::Literal(0x1_F600),
    ];
    let out = build_in_set(&items, 0, &None).unwrap();
    let InSet::Complement(complement) = &out else {
        panic!("{out:?} is not a complement");
    };
    assert_eq!(
        complement.excluded,
        vec![
            ('0' as u32, '0' as u32),
            ('1' as u32, '1' as u32),
            ('2' as u32, '2' as u32),
            ('k' as u32, 'k' as u32),
            (0xD800, 0xDFFF),
        ]
    );
    assert_eq!(set_len(&out), DEFAULT_ANY_LEN - 4);
    assert_eq!(listed(&out), reference_in_set(&items, 0, &None));
    let ignorecase = build_in_set(&items, SRE_FLAG_IGNORECASE, &None).unwrap();
    assert_eq!(set_len(&ignorecase), DEFAULT_ANY_LEN - 5);
    assert_eq!(
        listed(&ignorecase),
        reference_in_set(&items, SRE_FLAG_IGNORECASE, &None)
    );
    assert!(!listed(&ignorecase).contains(&'K'));
    assert!(complement.get(u64::from(u32::MAX) + 1).is_err());
}

#[test]
fn generate_op_empty_in_set_marks_invalid() {
    let mut ntc = NativeTestCase::for_choices(&[ChoiceValue::Integer(BigInt::from(0))], None, None);
    let cache = Mutex::new(HashMap::default());
    let char_cache = Mutex::new(HashMap::default());
    let mut state = ignorecase_state(&cache, &char_cache);
    let alphabet = Some(IntervalSet::new(vec![('x' as u32, 'x' as u32)]).unwrap());
    let items = vec![SetItem::Literal('a' as u32), SetItem::Literal('b' as u32)];
    let mut out = Vec::new();
    let result = generate_op(
        &mut ntc,
        &OpCode::In(items),
        &mut state,
        &alphabet,
        &mut out,
    );
    assert!(result.is_err());
    assert_eq!(ntc.status(), Some(Status::Invalid));
}

#[test]
fn generate_op_ignorecase_literal_outside_alphabet_marks_invalid() {
    let mut ntc = NativeTestCase::for_choices(&[ChoiceValue::Integer(BigInt::from(0))], None, None);
    let cache = Mutex::new(HashMap::default());
    let char_cache = Mutex::new(HashMap::default());
    let mut state = ignorecase_state(&cache, &char_cache);
    let alphabet = Some(IntervalSet::new(vec![('A' as u32, 'A' as u32)]).unwrap());
    let mut out = Vec::new();
    let result = generate_op(&mut ntc, &lit('a'), &mut state, &alphabet, &mut out);
    assert!(result.is_err());
    assert_eq!(ntc.status(), Some(Status::Invalid));
}

#[test]
fn generate_op_negated_class_covering_the_bmp_marks_invalid() {
    let mut ntc = NativeTestCase::for_choices(&[ChoiceValue::Integer(BigInt::from(0))], None, None);
    let cache = Mutex::new(HashMap::default());
    let char_cache = Mutex::new(HashMap::default());
    let mut state = ignorecase_state(&cache, &char_cache);
    let items = vec![SetItem::Negate, SetItem::Range(0, 0xFFFF)];
    let mut out = Vec::new();
    let result = generate_op(&mut ntc, &OpCode::In(items), &mut state, &None, &mut out);
    assert!(result.is_err());
    assert_eq!(ntc.status(), Some(Status::Invalid));
}

#[test]
fn compile_rejects_unparseable_patterns() {
    let err = CompiledRegex::compile("(unclosed", None).unwrap_err();
    assert!(matches!(err, EngineError::InvalidArgument(_)));
    assert!(err.to_string().contains("invalid regex pattern"));
    assert!(CompiledRegex::compile("a+b?", None).is_ok());
}

fn ignorecase_state<'a>(
    cache: &'a Mutex<HashMap<InKey, InSet>>,
    char_cache: &'a Mutex<HashMap<CharKey, Arc<[char]>>>,
) -> GenState<'a> {
    GenState {
        groups: HashMap::default(),
        capture: false,
        flags: SRE_FLAG_IGNORECASE,
        fullmatch: false,
        base: 0,
        deferred: None,
        in_cache: cache,
        char_cache,
    }
}

#[test]
fn typical_width_predicts_the_mean_length_of_generated_strings() {
    use crate::native::rng::EngineRng;
    let draws = 2000;
    for pattern in [
        "[0-9]+",
        "a?b*?c",
        "(ab|c){2,5}",
        "x(y)\\1",
        "(?>ab)c",
        "(a)?(?(1)b|cc)",
        "^k{3}$",
        "(?!z)[^a].",
        "a*+b",
    ] {
        let re = CompiledRegex::compile(pattern, None).unwrap();
        let predicted = re.typical_width();
        let mut total = 0;
        for seed in 0..draws {
            let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
            total += generate_regex(&mut ntc, &re, true).unwrap().chars().count();
        }
        let mean = total as f64 / draws as f64;
        assert!(
            (mean - predicted).abs() <= 0.15 * predicted + 0.1,
            "{pattern}: predicted {predicted}, drew {mean} on average"
        );
    }
}

#[test]
fn typical_count_is_the_minimum_plus_a_cut_off_geometric_tail() {
    assert_eq!(typical_count(3, Some(3)), 3.0);
    assert!((typical_count(0, Some(1)) - 0.6).abs() < 1e-9);
    assert!((typical_count(1, None) - 6.0).abs() < 1e-9);
    assert!(typical_count(0, Some(2)) < typical_count(0, Some(1000)));
}

#[test]
fn generate_op_ignorecase_eszett_never_emits_truncated_uppercase() {
    use crate::native::rng::EngineRng;
    let cache = Mutex::new(HashMap::default());
    let char_cache = Mutex::new(HashMap::default());
    for seed in 0..50 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let mut state = ignorecase_state(&cache, &char_cache);
        let mut out = Vec::new();
        generate_op(&mut ntc, &lit('ß'), &mut state, &None, &mut out).unwrap();
        assert_eq!(
            out,
            chars("ß"),
            "seed {seed} emitted a non-matching case variant"
        );
    }
}

#[test]
fn generate_op_ignorecase_plain_letter_emits_both_cases() {
    use crate::native::rng::EngineRng;
    let mut seen = HashSet::default();
    let cache = Mutex::new(HashMap::default());
    let char_cache = Mutex::new(HashMap::default());
    for seed in 0..50 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let mut state = ignorecase_state(&cache, &char_cache);
        let mut out = Vec::new();
        generate_op(&mut ntc, &lit('a'), &mut state, &None, &mut out).unwrap();
        seen.insert(out.into_iter().collect::<String>());
    }
    assert!(seen.contains("a") && seen.contains("A"), "saw {seen:?}");
}

#[test]
fn generate_op_ignorecase_not_literal_blacklists_swapcase_fixpoint() {
    use crate::native::rng::EngineRng;
    let alphabet = Some(
        IntervalSet::new(vec![
            ('I' as u32, 'I' as u32),
            ('i' as u32, 'i' as u32),
            ('x' as u32, 'x' as u32),
            (0x130, 0x130),
            (0x307, 0x307),
        ])
        .unwrap(),
    );
    let cache = Mutex::new(HashMap::default());
    let char_cache = Mutex::new(HashMap::default());
    for seed in 0..100 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let mut state = ignorecase_state(&cache, &char_cache);
        let mut out = Vec::new();
        generate_op(
            &mut ntc,
            &OpCode::NotLiteral('İ' as u32),
            &mut state,
            &alphabet,
            &mut out,
        )
        .unwrap();
        assert_eq!(out, chars("x"), "seed {seed} emitted a case-equal char");
    }
}

#[test]
fn generate_regex_handles_huge_character_class_ranges() {
    use crate::native::rng::EngineRng;
    let mut ntc = NativeTestCase::new_random(EngineRng::seeded(0)).unwrap();
    let re = CompiledRegex::compile("[\\x20-\\U0010FFFF]", None).unwrap();
    let s = generate_regex(&mut ntc, &re, false).unwrap();
    assert!(!s.is_empty());
}

#[test]
fn generate_regex_word_boundaries_hold_in_the_final_string() {
    use crate::native::rng::EngineRng;
    let re = CompiledRegex::compile(r"\bfoo\b", None).unwrap();
    let mut produced = 0;
    for seed in 0..300 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let Ok(s) = generate_regex(&mut ntc, &re, false) else {
            continue;
        };
        produced += 1;
        let cs = chars(&s);
        let matched = (0..cs.len().saturating_sub(2)).any(|i| {
            cs[i..i + 3] == ['f', 'o', 'o']
                && is_word_boundary(&cs, i)
                && is_word_boundary(&cs, i + 3)
        });
        assert!(matched, "seed {seed}: {s:?} contains no \\bfoo\\b match");
    }
    assert!(produced > 0, "every draw was rejected");
}

#[test]
fn generate_regex_end_anchor_inside_branch_holds_in_the_final_string() {
    use crate::native::rng::EngineRng;
    let re = CompiledRegex::compile("foo$|bar", None).unwrap();
    let mut produced = 0;
    for seed in 0..300 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let Ok(s) = generate_regex(&mut ntc, &re, false) else {
            continue;
        };
        produced += 1;
        assert!(
            s.contains("bar") || s.ends_with("foo") || s.ends_with("foo\n"),
            "seed {seed}: {s:?} matches neither branch"
        );
    }
    assert!(produced > 0, "every draw was rejected");
}

#[test]
fn generate_regex_fullmatch_lookahead_does_not_emit_the_assertion_body() {
    use crate::native::rng::EngineRng;
    let re = CompiledRegex::compile("(?=a)ab", None).unwrap();
    let mut produced = 0;
    for seed in 0..100 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let Ok(s) = generate_regex(&mut ntc, &re, true) else {
            continue;
        };
        produced += 1;
        assert_eq!(s, "ab", "seed {seed}: {s:?} is not a fullmatch of (?=a)ab");
    }
    assert!(produced > 0, "every draw was rejected");
}

#[test]
fn generate_regex_unsatisfiable_possessive_pattern_never_yields_a_wrong_string() {
    use crate::native::rng::EngineRng;
    let re = CompiledRegex::compile("a*+a", None).unwrap();
    for seed in 0..100 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        for fullmatch in [false, true] {
            let mut ntc2 = NativeTestCase::new_random(EngineRng::seeded(seed + 1000)).unwrap();
            let ntc_ref = if fullmatch { &mut ntc2 } else { &mut ntc };
            if let Ok(s) = generate_regex(ntc_ref, &re, fullmatch) {
                panic!(
                    "seed {seed} fullmatch={fullmatch}: a*+a produced {s:?}, but no string matches it"
                );
            }
        }
    }
}

#[test]
fn generate_regex_ignorecase_negated_class_excludes_swapcase_fixpoint() {
    use crate::native::rng::EngineRng;
    let alphabet = Some(
        IntervalSet::new(vec![
            ('I' as u32, 'I' as u32),
            ('a' as u32, 'a' as u32),
            ('i' as u32, 'i' as u32),
            ('x' as u32, 'x' as u32),
            (0x130, 0x130),
            (0x307, 0x307),
        ])
        .unwrap(),
    );
    let re = CompiledRegex::compile("(?i)[^\u{130}a]", alphabet).unwrap();
    let mut produced = 0;
    for seed in 0..100 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let Ok(s) = generate_regex(&mut ntc, &re, true) else {
            continue;
        };
        produced += 1;
        assert_eq!(
            s, "x",
            "seed {seed}: {s:?} is case-equal to an excluded char"
        );
    }
    assert!(produced > 0, "every draw was rejected");
}

#[test]
fn match_seq_max_repeat_counts_zero_width_iterations_toward_min() {
    let groups = HashMap::default();
    let optional_a = sub(vec![OpCode::MaxRepeat {
        min: 0,
        max: 1,
        item: sub(vec![lit('a')]),
    }]);
    let pattern = [OpCode::MaxRepeat {
        min: 3,
        max: 3,
        item: optional_a,
    }];
    assert_eq!(match_seq(&pattern, 0, &chars(""), 0, &groups), Some(0));
    assert_eq!(match_seq(&pattern, 0, &chars("aa"), 0, &groups), Some(2));
}

#[test]
fn match_seq_min_repeat_counts_zero_width_iterations_toward_min() {
    let groups = HashMap::default();
    let optional_a = sub(vec![OpCode::MaxRepeat {
        min: 0,
        max: 1,
        item: sub(vec![lit('a')]),
    }]);
    let pattern = [OpCode::MinRepeat {
        min: 3,
        max: 3,
        item: optional_a,
    }];
    assert_eq!(match_seq(&pattern, 0, &chars(""), 0, &groups), Some(0));
}

#[test]
fn generate_regex_fullmatch_lookbehind_is_validated_against_the_final_string() {
    use crate::native::rng::EngineRng;
    let re = CompiledRegex::compile("(?<=a)b", None).unwrap();
    for seed in 0..20 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        assert!(
            generate_regex(&mut ntc, &re, true).is_err(),
            "a fixed-width lookbehind can never hold at the start of a fullmatch"
        );
    }

    let re = CompiledRegex::compile("(?<=a*)b", None).unwrap();
    let mut produced = 0;
    for seed in 0..20 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        if let Ok(s) = generate_regex(&mut ntc, &re, true) {
            produced += 1;
            assert_eq!(s, "b");
        }
    }
    assert!(produced > 0, "an empty-matching lookbehind body must pass");
}

#[test]
fn generate_regex_retries_a_failed_negative_lookbehind_within_the_test_case() {
    use crate::native::rng::EngineRng;
    let re = CompiledRegex::compile(r"[ab]*(?<!a)c", None).unwrap();
    let mut produced = 0;
    for seed in 0..200 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        if let Ok(s) = generate_regex(&mut ntc, &re, true) {
            produced += 1;
            assert!(!s.ends_with("ac"), "{s:?} violates the lookbehind");
        }
    }
    assert!(
        produced > 190,
        "a candidate ending in `a` must be redrawn, not fail the test case: {produced}/200"
    );

    let re = CompiledRegex::compile(r"a(?<!a)c", None).unwrap();
    for seed in 0..10 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        assert!(
            generate_regex(&mut ntc, &re, true).is_err(),
            "a negative lookbehind that always matches can never hold"
        );
        assert_eq!(ntc.status(), Some(Status::Invalid));
    }
}

#[test]
fn generate_regex_end_anchor_with_no_newline_in_alphabet_does_not_pad() {
    use crate::native::rng::EngineRng;
    let alphabet = Some(IntervalSet::new(vec![('a' as u32, 'b' as u32)]).unwrap());
    let re = CompiledRegex::compile("a$", alphabet).unwrap();
    let mut produced = 0;
    for seed in 0..20 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let Ok(s) = generate_regex(&mut ntc, &re, false) else {
            continue;
        };
        produced += 1;
        assert!(s.ends_with('a'), "no newline available to pad with: {s:?}");
    }
    assert!(produced > 0);
}

#[test]
fn generate_regex_multiline_caret_in_the_middle_generates_nothing() {
    use crate::native::rng::EngineRng;
    let re = CompiledRegex::compile("(?m)a^b", None).unwrap();
    for seed in 0..20 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        assert!(
            generate_regex(&mut ntc, &re, false).is_err(),
            "a mid-pattern ^ preceded by a literal can never match"
        );
    }
}

#[test]
fn match_seq_possessive_repeat_counts_zero_width_iterations_toward_min() {
    let groups = HashMap::default();
    let optional_a = sub(vec![OpCode::MaxRepeat {
        min: 0,
        max: 1,
        item: sub(vec![lit('a')]),
    }]);
    let pattern = [OpCode::PossessiveRepeat {
        min: 3,
        max: 3,
        item: optional_a,
    }];
    assert_eq!(match_seq(&pattern, 0, &chars(""), 0, &groups), Some(0));
    assert_eq!(match_seq(&pattern, 0, &chars("aa"), 0, &groups), Some(2));
}

#[test]
fn codepoint_to_char_reports_surrogates_as_internal_errors() {
    assert_eq!(codepoint_to_char('a' as u32).unwrap(), 'a');
    assert!(codepoint_to_char(0xD800).is_err());
}

fn reference_in_category(c: char, cat: ChCode) -> bool {
    let group = |g: &str| unicodedata::is_in_group(c as u32, g).unwrap();
    let space = matches!(
        c,
        ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\x1f' | '\u{85}'
    ) || group("Z");
    let word = c == '_' || group("L") || group("N");
    match cat {
        ChCode::Digit => group("Nd"),
        ChCode::NotDigit => !group("Nd"),
        ChCode::Space => space,
        ChCode::NotSpace => !space,
        ChCode::Word => word,
        ChCode::NotWord => !word,
    }
}

fn reference_in_set(items: &[SetItem], flags: u32, alphabet: &Option<IntervalSet>) -> Vec<char> {
    let negate = matches!(items.first(), Some(SetItem::Negate));
    let mut positive = Vec::new();
    let mut categories = Vec::new();
    for item in items {
        match item {
            SetItem::Negate => {}
            SetItem::Literal(cp) => {
                add_with_swapcase(&mut positive, char::from_u32(*cp).unwrap(), flags);
            }
            SetItem::Range(lo, hi) => {
                for c in (*lo..=*hi).filter_map(char::from_u32) {
                    add_with_swapcase(&mut positive, c, flags);
                }
            }
            SetItem::Category(cat) => categories.push(*cat),
        }
    }
    let ascii_only = flags & SRE_FLAG_ASCII != 0;
    let in_classes = |c: char| categories.iter().any(|cat| reference_in_category(c, *cat));
    if !negate {
        let mut out = Vec::new();
        let mut seen: HashSet<char> = HashSet::default();
        for c in positive {
            if !(ascii_only && (c as u32) >= 128) && alphabet_allows(alphabet, c) && seen.insert(c)
            {
                out.push(c);
            }
        }
        for c in gather_chars(alphabet, in_classes) {
            if !(ascii_only && (c as u32) >= 128) && seen.insert(c) {
                out.push(c);
            }
        }
        out
    } else {
        let mut positive_set: HashSet<char> = HashSet::default();
        for c in positive {
            positive_set.extend(swapcase_blacklist(c, flags));
        }
        gather_chars(alphabet, |c| {
            !(ascii_only && (c as u32) >= 128) && !positive_set.contains(&c) && !in_classes(c)
        })
    }
}

#[test]
fn category_in_agrees_with_in_category_from_the_category_alone_codepoint_up() {
    use ChCode::*;
    let cats = [Digit, NotDigit, Space, NotSpace, Word, NotWord];
    for c in (CATEGORY_ALONE_FROM..=0x10FFFF).filter_map(char::from_u32) {
        let category = unicodedata::general_category_char(c);
        for cat in cats {
            assert_eq!(
                category_in(category, cat),
                in_category(c, category, cat),
                "{c:?} {cat:?}"
            );
        }
    }
    let differs = |c: char, cat: ChCode| {
        let category = unicodedata::general_category_char(c);
        category_in(category, cat) != in_category(c, category, cat)
    };
    assert!(differs('\t', Space));
    assert!(differs('_', Word));
}

#[test]
fn build_in_set_category_classes_match_a_lookup_per_character() {
    use ChCode::*;
    let alphabets = [
        None,
        Some(IntervalSet::new(vec![(0x20, 0x7E), (0xA0, 0x3FF), (0x1_F600, 0x1_F64F)]).unwrap()),
    ];
    let item_sets: Vec<Vec<SetItem>> = [Digit, NotDigit, Space, NotSpace, Word, NotWord]
        .iter()
        .flat_map(|cat| {
            [
                vec![SetItem::Category(*cat)],
                vec![SetItem::Negate, SetItem::Category(*cat)],
            ]
        })
        .chain([
            vec![SetItem::Negate, SetItem::Literal('a' as u32)],
            vec![
                SetItem::Negate,
                SetItem::Range('a' as u32, 'z' as u32),
                SetItem::Literal(0x1_F600),
            ],
            vec![
                SetItem::Literal('0' as u32),
                SetItem::Range('x' as u32, 'z' as u32),
                SetItem::Category(Digit),
            ],
            vec![
                SetItem::Negate,
                SetItem::Literal('a' as u32),
                SetItem::Category(Space),
                SetItem::Category(Digit),
            ],
            vec![
                SetItem::Literal('a' as u32),
                SetItem::Category(Word),
                SetItem::Category(Space),
            ],
            vec![SetItem::Literal('α' as u32), SetItem::Category(Word)],
            vec![
                SetItem::Negate,
                SetItem::Literal('α' as u32),
                SetItem::Category(Space),
            ],
            vec![
                SetItem::Negate,
                SetItem::Literal('α' as u32),
                SetItem::Category(Word),
            ],
        ])
        .collect();
    for alphabet in &alphabets {
        for items in &item_sets {
            for flags in [0, SRE_FLAG_IGNORECASE, SRE_FLAG_ASCII] {
                assert_eq!(
                    listed(&build_in_set(items, flags, alphabet).unwrap()),
                    reference_in_set(items, flags, alphabet),
                    "{items:?} with flags {flags}"
                );
            }
        }
    }
    let digits_and_zero = [SetItem::Literal('0' as u32), SetItem::Category(Digit)];
    let out = listed(&build_in_set(&digits_and_zero, 0, &None).unwrap());
    assert_eq!(out[0], '0');
    assert_eq!(out.iter().filter(|c| **c == '0').count(), 1);
    assert!(out.contains(&'٣'));
}

#[test]
fn default_any_char_indexes_the_default_any_scan() {
    for allow_newline in [false, true] {
        let scan = gather_chars(&None, |c| allow_newline || c != '\n');
        let n = DEFAULT_ANY_LEN - u64::from(!allow_newline);
        assert_eq!(scan.len() as u64, n);
        for (idx, &c) in scan.iter().enumerate() {
            assert_eq!(default_any_char(idx as u64, allow_newline).unwrap(), c);
        }
    }
    assert!(default_any_char(u64::from(u32::MAX) + 1, true).is_err());
}

#[test]
fn a_repeated_class_draws_what_repeating_it_in_a_group_would() {
    use crate::native::rng::EngineRng;
    fn repeated(re: &CompiledRegex) -> &SubPattern {
        match &re.parsed.pattern.data[..] {
            [OpCode::MaxRepeat { item, .. }]
            | [OpCode::MinRepeat { item, .. }]
            | [OpCode::PossessiveRepeat { item, .. }] => item,
            other => panic!("not a single repeat: {other:?}"),
        }
    }
    let listed = |c: char| c == 'a' || c == 'b';
    let complemented = |c: char| c != 'a' && c != 'b';
    let mut empty = 0;
    let mut filled = 0;
    for (direct, grouped, allowed) in [
        ("[ab]*", "([ab])*", &listed as &dyn Fn(char) -> bool),
        ("[ab]+?", "([ab])+?", &listed),
        ("[ab]{3}", "([ab]){3}", &listed),
        ("[ab]*+", "([ab])*+", &listed),
        ("[^ab]*", "([^ab])*", &complemented),
        ("[^ab]{2,4}", "([^ab]){2,4}", &complemented),
    ] {
        let direct = CompiledRegex::compile(direct, None).unwrap();
        let grouped = CompiledRegex::compile(grouped, None).unwrap();
        assert!(matches!(repeated(&direct).data[..], [OpCode::In(_)]));
        assert!(matches!(
            repeated(&grouped).data[..],
            [OpCode::Subpattern { group: Some(_), .. }]
        ));
        for seed in 0..200 {
            let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
            let one = generate_regex(&mut ntc, &direct, true).unwrap();
            let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
            let other = generate_regex(&mut ntc, &grouped, true).unwrap();
            assert_eq!(one, other, "seed {seed}");
            assert!(one.chars().all(allowed), "{one:?}");
            if one.is_empty() {
                empty += 1;
            } else {
                filled += 1;
            }
        }
    }
    assert!(empty > 0 && filled > 0, "{empty} empty, {filled} filled");
}

#[test]
fn a_repeated_empty_class_marks_invalid() {
    let alphabet = Some(IntervalSet::new(vec![('x' as u32, 'x' as u32)]).unwrap());
    for (items, alphabet, complemented) in [
        (vec![SetItem::Literal('a' as u32)], &alphabet, false),
        (
            vec![SetItem::Negate, SetItem::Range(0, 0xFFFF)],
            &None,
            true,
        ),
    ] {
        let mut ntc =
            NativeTestCase::for_choices(&[ChoiceValue::Integer(BigInt::from(0))], None, None);
        let cache = Mutex::new(HashMap::default());
        let char_cache = Mutex::new(HashMap::default());
        let mut state = ignorecase_state(&cache, &char_cache);
        let op = OpCode::MaxRepeat {
            min: 1,
            max: 1,
            item: sub(vec![OpCode::In(items)]),
        };
        let mut out = Vec::new();
        let result = generate_op(&mut ntc, &op, &mut state, alphabet, &mut out);
        assert!(result.is_err());
        assert_eq!(ntc.status(), Some(Status::Invalid));
        assert!(out.is_empty());
        let cache = cache.lock();
        let set = cache.values().next().unwrap();
        assert_eq!(
            matches!(set, InSet::Complement(complement) if complement.len == 0),
            complemented
        );
        assert_eq!(
            matches!(set, InSet::Listed(chars) if chars.is_empty()),
            !complemented
        );
    }
}

#[test]
fn a_captured_group_is_kept_only_when_the_pattern_reads_it_back() {
    use crate::native::rng::EngineRng;
    let cache = Mutex::new(HashMap::default());
    let char_cache = Mutex::new(HashMap::default());
    let op = OpCode::Subpattern {
        group: Some(1),
        add_flags: 0,
        del_flags: 0,
        p: sub(vec![lit('a')]),
    };
    for capture in [false, true] {
        let mut ntc = NativeTestCase::for_choices(&[], None, None);
        let mut state = ignorecase_state(&cache, &char_cache);
        state.capture = capture;
        state.flags = 0;
        let mut out = Vec::new();
        generate_op(&mut ntc, &op, &mut state, &None, &mut out).unwrap();
        assert_eq!(out, chars("a"));
        assert_eq!(state.groups.get(&1), capture.then(|| chars("a")).as_ref());
    }
    let reading = CompiledRegex::compile("(a|b)\\1", None).unwrap();
    let plain = CompiledRegex::compile("(a|b)(?!c)", None).unwrap();
    assert!(reading.parsed.group_refs);
    assert!(!plain.parsed.group_refs);
    for seed in 0..50 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let s = generate_regex(&mut ntc, &reading, true).unwrap();
        assert!(s == "aa" || s == "bb", "{s:?}");
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let s = generate_regex(&mut ntc, &plain, true).unwrap();
        assert!(s == "a" || s == "b", "{s:?}");
    }
}
