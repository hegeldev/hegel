//! Tests for `src/native/lark/generate.rs`: that every string drawn from
//! a grammar is in it, that the size and depth caps hold, how `%ignore`
//! terminals are placed, and the lexer-fit checks that decide it.

use super::*;
use crate::native::core::ChoiceValue;
use crate::native::lark::builder::{ImportSource, NoImports};
use crate::native::rng::EngineRng;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;
use std::path::Path;

fn spec_with(text: &str, options: GrammarOptions) -> GrammarResult<GrammarSpec> {
    from_lark(text, &NoImports, options)
}

fn grammar(text: &str) -> GrammarSpec {
    spec_with(text, GrammarOptions::default()).unwrap()
}

fn small(text: &str, max_size: u64) -> GrammarSpec {
    spec_with(
        text,
        GrammarOptions {
            max_size,
            ..GrammarOptions::default()
        },
    )
    .unwrap()
}

fn explicit(name: &str, pattern: &str) -> (String, Arc<StringSpec>) {
    (
        name.to_string(),
        Arc::new(StringSpec::regex(pattern, true, None).unwrap()),
    )
}

fn draw(spec: &GrammarSpec, seed: u64) -> (Result<String, EngineError>, Option<Status>) {
    let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
    let result = generate_grammar(&mut ntc, spec);
    (result, ntc.status())
}

fn samples(spec: &GrammarSpec, n: u64) -> Vec<String> {
    (0..n).map(|seed| draw(spec, seed).0.unwrap()).collect()
}

fn tid(spec: &GrammarSpec, name: &str) -> TermId {
    spec.terminal_names()
        .iter()
        .position(|n| n == name)
        .unwrap()
}

fn tok(spec: &GrammarSpec, name: &str, text: &str) -> Token {
    let id = tid(spec, name);
    let text: Vec<char> = text.chars().collect();
    let scans = spec.scan_token(id, &text, None);
    Token {
        id,
        reading: spec.reading(id, &text, &scans.basic),
        text,
        droppable: false,
        scans,
    }
}

fn fit(spec: &GrammarSpec, tokens: &[Token]) -> Fit {
    let refs: Vec<&Token> = tokens.iter().collect();
    spec.fit(&refs)
}

fn basic_fit(spec: &GrammarSpec, tokens: &[Token]) -> Fit {
    let refs: Vec<&Token> = tokens.iter().collect();
    let text: Vec<char> = tokens.iter().flat_map(|t| t.text.iter().copied()).collect();
    spec.basic_fit(&text, &refs)
}

const PARENS: &str = "start: \"()\" | \"(\" start \")\" | start start\n";

fn balanced(s: &str) -> bool {
    let mut depth = 0i64;
    for c in s.chars() {
        depth += if c == '(' { 1 } else { -1 };
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

#[test]
fn explicit_generator_for_an_unused_terminal_is_an_error() {
    let err = spec_with(
        "start: \"a\"\n",
        GrammarOptions {
            explicit: vec![explicit("NOPE", "x")],
            ..GrammarOptions::default()
        },
    )
    .err()
    .unwrap();
    assert_eq!(
        err.message,
        "explicit generator for terminal 'NOPE', which the grammar does not use"
    );
}

#[test]
fn declared_terminal_without_a_generator_is_an_error() {
    let err = spec_with("start: X\n%declare X\n", GrammarOptions::default())
        .err()
        .unwrap();
    assert_eq!(
        err.message,
        "Terminal 'X' is declared but has no definition: supply a generator for it with \
         explicit"
    );
}

#[test]
fn alphabet_must_be_a_text_generator() {
    let err = spec_with(
        "start: A\nA: /a+/\n",
        GrammarOptions {
            alphabet: Some(Arc::new(StringSpec::regex("a", true, None).unwrap())),
            ..GrammarOptions::default()
        },
    )
    .err()
    .unwrap();
    assert_eq!(
        err.message,
        "Cannot generate terminal A from a+: regex alphabet must be a text string generator"
    );
}

#[test]
fn caps_below_the_smallest_string_are_errors() {
    let err = spec_with(
        "start: \"abc\"\n",
        GrammarOptions {
            max_size: 2,
            ..GrammarOptions::default()
        },
    )
    .err()
    .unwrap();
    assert_eq!(
        err.message,
        "max_size is 2 but the smallest string 'start' can produce has 3 characters"
    );
    let err = spec_with(
        "start: a\na: b\nb: \"x\"\n",
        GrammarOptions {
            max_depth: 2,
            ..GrammarOptions::default()
        },
    )
    .err()
    .unwrap();
    assert_eq!(
        err.message,
        "max_depth is 2 but the shallowest string 'start' can produce nests 3 rules deep"
    );
}

#[test]
fn budgets_count_terminals_at_their_typical_length() {
    let spec = small("start: NAME+\nNAME: /[a-z]+/ \" \"\n", 1 << 20);
    assert_eq!(spec.analysis.rules[spec.start].typical_size, 7);
    for seed in 0..32 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let mut generation = Generation {
            spec: &spec,
            tokens: Vec::new(),
            consumed: 0,
            ignore_rate: 0.0,
            unsettled: false,
        };
        assert!(generation.run(&mut ntc, 70).is_ok());
        assert!(
            generation.tokens.len() <= 10,
            "seed {seed}: {}",
            generation.tokens.len()
        );
    }
}

#[test]
fn rules_whose_alternatives_all_grow_still_keep_to_their_budget() {
    let spec = fixture_spec("calc", "start", 4000, vec![]);
    for seed in 0..32 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let mut generation = Generation {
            spec: &spec,
            tokens: Vec::new(),
            consumed: 0,
            ignore_rate: 0.0,
            unsettled: false,
        };
        assert!(generation.run(&mut ntc, 500).is_ok(), "seed {seed}");
        assert!(
            (250..=1500).contains(&generation.consumed),
            "seed {seed}: {} characters for a budget of 500",
            generation.consumed
        );
    }
}

#[test]
fn grammar_errors_propagate() {
    assert!(spec_with("start: (\n", GrammarOptions::default()).is_err());
    assert!(spec_with("start: start \"x\"\n", GrammarOptions::default()).is_err());
    let err = spec_with(
        "start: \"a\"\n",
        GrammarOptions {
            start: "nope".to_string(),
            ..GrammarOptions::default()
        },
    )
    .err()
    .unwrap();
    assert!(err.message.contains("nope"), "{}", err.message);
}

#[test]
fn generated_strings_are_in_the_grammar_and_within_the_cap() {
    let spec = small(PARENS, 64);
    assert_eq!(spec.min_size(), 2);
    let strings = samples(&spec, 200);
    for s in &strings {
        assert!(balanced(s), "{s:?}");
        assert!(s.len() <= 64, "{s:?}");
    }
    assert!(strings.iter().any(|s| s.len() > 40));
    assert!(strings.iter().any(|s| s.len() == 2));
    assert!(strings.iter().any(|s| s.contains("()()")));
    assert!(strings.iter().any(|s| s.contains("(())")));
}

#[test]
fn depth_cap_bounds_the_nesting() {
    let spec = spec_with(
        "start: \"x\" | \"(\" start \")\"\n",
        GrammarOptions {
            max_depth: 4,
            ..GrammarOptions::default()
        },
    )
    .unwrap();
    let strings = samples(&spec, 100);
    for s in &strings {
        let opens = s.chars().filter(|&c| c == '(').count();
        assert!(opens <= 3, "{s:?}");
    }
    assert!(strings.iter().any(|s| s.starts_with("(((")));
}

#[test]
fn repetitions_cover_their_whole_range() {
    let spec = small("start: \"a\"~1..3 \"b\"* \"c\"+ [\"d\"] (\"e\"?)*\n", 40);
    let strings = samples(&spec, 200);
    for s in &strings {
        let count = |c| s.chars().filter(|&x| x == c).count();
        assert!((1..=3).contains(&count('a')), "{s:?}");
        assert!(count('c') >= 1, "{s:?}");
        assert!(count('d') <= 1, "{s:?}");
        assert!(s.len() <= 40, "{s:?}");
    }
    assert!(strings.iter().any(|s| s.contains("aaa")));
    assert!(strings.iter().any(|s| s.contains("bbbb")));
    assert!(strings.iter().any(|s| s.contains('d')));
    assert!(strings.iter().any(|s| !s.contains('b')));
    assert!(strings.iter().any(|s| s.contains('e')));
}

#[test]
fn growing_only_and_closing_only_rules() {
    let spec = small("start: \"a\"*\n", 20);
    let strings = samples(&spec, 50);
    assert!(
        strings
            .iter()
            .all(|s| s.chars().all(|c| c == 'a') && s.len() <= 20)
    );
    assert!(strings.iter().any(|s| s.len() > 10));
    let spec = grammar("start: \"a\" | \"bb\"\n");
    let strings = samples(&spec, 50);
    assert!(strings.iter().all(|s| s == "a" || s == "bb"));
    assert!(strings.iter().any(|s| s == "bb"));
}

#[test]
fn unproductive_parts_never_appear() {
    let spec = small(
        "start: useless* \"b\" (loop | \"c\")\nuseless: useless \"y\"\nloop: \"x\" loop\n",
        10,
    );
    assert!(samples(&spec, 20).iter().all(|s| s == "bc"));
}

#[test]
fn bounded_items_leave_their_leftover_to_growing_ones() {
    let spec = small("start: \"a\"? \"b\"*\n", 30);
    let strings = samples(&spec, 200);
    assert!(strings.iter().any(|s| s.len() > 15));
    let spec = small("start: \"a\"? \"b\"\n", 30);
    let strings = samples(&spec, 20);
    assert!(strings.iter().all(|s| s == "b" || s == "ab"));
}

#[test]
fn oversize_strings_are_regenerated_then_rejected() {
    let spec = spec_with(
        "start: X\n%declare X\n",
        GrammarOptions {
            explicit: vec![explicit("X", "a{1,3}")],
            max_size: 2,
            ..GrammarOptions::default()
        },
    )
    .unwrap();
    let strings = samples(&spec, 50);
    assert!(strings.iter().all(|s| s == "a" || s == "aa"));
    assert!(strings.iter().any(|s| s == "aa"));
    let spec = spec_with(
        "start: X\n%declare X\n",
        GrammarOptions {
            explicit: vec![explicit("X", "aaaaa")],
            max_size: 1,
            ..GrammarOptions::default()
        },
    )
    .unwrap();
    let (result, status) = draw(&spec, 0);
    assert!(matches!(result, Err(EngineError::InvalidTestCase)));
    assert_eq!(status, Some(Status::Invalid));
}

#[test]
fn terminals_are_cut_to_what_their_pattern_reads_from_the_front() {
    let spec = grammar("start: A\nA: /a|ab/\n");
    assert!(samples(&spec, 30).iter().all(|s| s == "a"));
    let a = tid(&spec, "A");
    assert_eq!(spec.whole(a, &['x']).0, 1);
}

#[test]
fn keyword_only_terminals_are_accepted_after_the_retries() {
    let spec = grammar("start: NAME \"if\" \"iff\"\nNAME: /if|iff/\n%ignore \" \"\n");
    let strings = samples(&spec, 20);
    assert!(
        strings
            .iter()
            .all(|s| ["ifififf", "iffififf"].contains(&s.replace(' ', "").as_str())),
        "{strings:?}"
    );
}

#[test]
fn separators_are_forced_between_tokens_that_would_merge() {
    let spec = small(
        "start: NAME NAME\nNAME: /[a-z]+/\n%ignore WS\nWS: /[ \\t]+/\n",
        40,
    );
    let strings = samples(&spec, 100);
    for s in &strings {
        assert_eq!(s.split_whitespace().count(), 2, "{s:?}");
    }
}

#[test]
fn separators_the_basic_lexer_alone_needs_are_optional() {
    let spec = grammar("start: A B\nA: /a/\nB: /[ab]/\n%ignore COMMENT\nCOMMENT: /#[^\\n]*/\n");
    let strings = samples(&spec, 50);
    assert!(strings.iter().any(|s| s == "ab"), "{strings:?}");
    for s in &strings {
        assert!(s.starts_with("ab") || s.starts_with("aa"), "{s:?}");
        assert!(s.len() == 2 || s.chars().nth(2) == Some('#'), "{s:?}");
    }
}

#[test]
fn unseparable_merging_tokens_make_the_test_case_invalid() {
    let spec = grammar("start: NAME NAME\nNAME: /[a-z]+/\n%ignore COMMENT\nCOMMENT: /#[a-z]*/\n");
    let (result, status) = draw(&spec, 0);
    assert!(matches!(result, Err(EngineError::InvalidTestCase)));
    assert_eq!(status, Some(Status::Invalid));
}

#[test]
fn comments_are_followed_by_the_newline_that_ends_them() {
    let spec = small(
        "start: NAME NAME\nNAME: /[a-z]+/\n%ignore COMMENT\n%ignore NL\nCOMMENT: /#[a-z]*/\nNL: /\\n/\n",
        60,
    );
    let strings = samples(&spec, 100);
    let mut with_comment = 0;
    for s in &strings {
        let stripped: Vec<&str> = s
            .split('\n')
            .map(|line| line.split('#').next().unwrap())
            .filter(|w| !w.is_empty())
            .collect();
        assert_eq!(stripped.len(), 2, "{s:?}");
        with_comment += usize::from(s.contains('#'));
    }
    assert!(with_comment > 0);
}

#[test]
fn comments_without_a_newline_are_dropped_for_whitespace() {
    let spec = small(
        "start: NAME NAME\nNAME: /[a-z]+/\n%ignore COMMENT\n%ignore WS\nCOMMENT: /#[a-z ]*/\nWS: / +/\n",
        60,
    );
    let strings = samples(&spec, 100);
    for s in &strings {
        let code = s.split('#').next().unwrap();
        assert_eq!(
            code.split(' ').filter(|w| !w.is_empty()).count(),
            2,
            "{s:?}"
        );
    }
    assert!(strings.iter().any(|s| s.contains('#')), "{strings:?}");
}

#[test]
fn declared_ignorables_are_trusted() {
    let spec = spec_with(
        "start: NAME NAME\nNAME: /[a-z]+/\n%declare WS\n%ignore WS\n",
        GrammarOptions {
            explicit: vec![explicit("WS", " ")],
            max_size: 30,
            ..GrammarOptions::default()
        },
    )
    .unwrap();
    let strings = samples(&spec, 30);
    assert!(
        strings
            .iter()
            .all(|s| s.split(' ').filter(|w| !w.is_empty()).count() == 2),
        "{strings:?}"
    );
    let ws = tok(&spec, "WS", " ");
    assert_eq!(ws.reading, Reading::Opaque);
    let a = tok(&spec, "NAME", "ab");
    let b = tok(&spec, "NAME", "cd");
    assert_eq!(fit(&spec, &[a, ws, b]), Fit::Exact);
}

#[test]
fn optional_tokens_may_be_swallowed_by_their_neighbours() {
    let spec = small("start: A B? C\nA: /a/\nB: /b/\nC: /b?c/\n", 10);
    let strings = samples(&spec, 50);
    assert!(
        strings
            .iter()
            .all(|s| ["ac", "abc", "abbc"].contains(&s.as_str())),
        "{strings:?}"
    );
    assert!(strings.iter().any(|s| s == "abc"));
    let mut generation = Generation {
        spec: &spec,
        tokens: vec![
            tok(&spec, "A", "a"),
            tok(&spec, "B", "b"),
            tok(&spec, "C", "c"),
        ],
        consumed: 0,
        ignore_rate: 0.0,
        unsettled: false,
    };
    generation.mark_droppable(0);
    assert!(generation.tokens.iter().all(|t| !t.droppable));
    generation.mark_droppable(3);
    assert!(generation.tokens.iter().all(|t| !t.droppable));
    generation.mark_droppable(2);
    assert!(generation.tokens[2].droppable);
    assert!(!generation.tokens[1].droppable);
}

#[test]
fn readings_of_terminal_texts() {
    let spec = grammar("start: A B\nA: /ab/\nB.2: /a/\n");
    assert_eq!(tok(&spec, "A", "ab").reading, Reading::Opaque);
    let spec = grammar(
        "start: KW NAME\nKW: \"if\"\nNAME: /[a-z]+/\n%ignore WS\n%ignore NL\nWS: / +/\nNL: /\\n/\n",
    );
    assert_eq!(
        tok(&spec, "NAME", "if").reading,
        Reading::Tokens(vec![tid(&spec, "KW")])
    );
    assert_eq!(tok(&spec, "NAME", "iffy").reading, Reading::Itself);
    assert_eq!(tok(&spec, "WS", "  ").reading, Reading::Itself);
    let ws = tid(&spec, "WS");
    assert_eq!(tok(&spec, "WS", " \n").reading, Reading::Ignorables);
    assert!(tok(&spec, "WS", " \n").lexable());
    assert_eq!(
        tok(&spec, "WS", " x").reading,
        Reading::Tokens(vec![ws, tid(&spec, "NAME")])
    );
}

#[test]
fn scans_follows_the_dynamic_lexer() {
    let spec = grammar("start: A B C\nA: /ab?/\nB: /b/\nC: /c/\n%ignore WS\nWS: / +b?/\n");
    let a = tok(&spec, "A", "a");
    let b = tok(&spec, "B", "b");
    let c = tok(&spec, "C", "c");
    let ws = tok(&spec, "WS", " ");
    assert_eq!(fit(&spec, &[a.clone(), b.clone()]), Fit::Wrong);
    assert_eq!(fit(&spec, &[a.clone(), ws.clone(), b.clone()]), Fit::Wrong);
    assert_eq!(
        fit(&spec, &[a.clone(), ws.clone(), c.clone()]),
        Fit::Settled
    );
    assert_eq!(fit(&spec, &[b.clone(), c.clone()]), Fit::Settled);
    assert_eq!(fit(&spec, &[a.clone(), ws.clone()]), Fit::Settled);
    assert_eq!(fit(&spec, &[ws.clone(), ws.clone(), c.clone()]), Fit::Exact);
    assert_eq!(fit(&spec, &[a.clone(), a.clone()]), Fit::Settled);
    let mut dropped = b.clone();
    dropped.droppable = true;
    assert_eq!(fit(&spec, &[a.clone(), dropped.clone()]), Fit::Retyped);
    assert_eq!(
        fit(&spec, &[a.clone(), dropped.clone(), c.clone()]),
        Fit::Retyped
    );
    assert_eq!(
        fit(
            &spec,
            &[
                a.clone(),
                ws.clone(),
                dropped.clone(),
                ws.clone(),
                c.clone()
            ]
        ),
        Fit::Retyped
    );
    assert_eq!(
        fit(
            &spec,
            &[
                c.clone(),
                ws.clone(),
                dropped.clone(),
                ws.clone(),
                c.clone()
            ]
        ),
        Fit::Retyped
    );
    let text: Vec<char> = "cx".chars().collect();
    assert!(!spec.scans(&text, &[&c, &tok(&spec, "C", "x")]));
}

#[test]
fn scans_gives_involved_patterns_the_benefit_of_the_doubt() {
    let spec = grammar("start: A B\nA: /(a*)*b/\nB: /c/\n%ignore WS\nWS: /( *)*\\t/\n");
    let a = tok(&spec, "A", &"a".repeat(40));
    let b = tok(&spec, "B", "c");
    let ws = tok(&spec, "WS", &" ".repeat(40));
    assert_eq!(fit(&spec, &[a.clone(), b.clone()]), Fit::Exact);
    assert_eq!(fit(&spec, &[ws.clone(), b.clone()]), Fit::Exact);
    assert_eq!(fit(&spec, &[b.clone(), ws.clone()]), Fit::Exact);
}

#[test]
fn basic_fit_distinguishes_retyped_from_wrong() {
    let spec = grammar(
        "start: Z A B C Y I2 J\nZ: /z/\nA: /a/\nB: /b/\nC.3: /(?<=z)a/\nY: /y/\nI2: /xy/\nJ: / +a/\n%ignore I1\n%ignore I2\nI1: / x?/\n",
    );
    let z = tok(&spec, "Z", "z");
    let a = tok(&spec, "A", "a");
    let b = tok(&spec, "B", "b");
    let i1 = tok(&spec, "I1", " ");
    let i2 = tok(&spec, "I2", "xy");
    assert_eq!(basic_fit(&spec, &[a.clone(), b.clone()]), Fit::Exact);
    assert_eq!(basic_fit(&spec, &[z.clone(), a.clone()]), Fit::Retyped);
    assert_eq!(
        basic_fit(&spec, &[a.clone(), i1.clone(), i2.clone(), b.clone()]),
        Fit::Retyped
    );
    assert_eq!(
        basic_fit(&spec, &[z.clone(), i1.clone(), a.clone()]),
        Fit::Wrong
    );
}

#[test]
fn basic_fit_rejects_merged_and_split_tokens() {
    let spec = grammar("start: A B | C\nA: /a/\nB: /b/\nC: /ab/\n");
    assert_eq!(
        basic_fit(&spec, &[tok(&spec, "A", "a"), tok(&spec, "B", "b")]),
        Fit::Wrong
    );
    let spec = grammar("start: A B | C\nA: /a/\nB: /xb/\nC: /ax/\n");
    assert_eq!(
        basic_fit(&spec, &[tok(&spec, "A", "a"), tok(&spec, "B", "xb")]),
        Fit::Wrong
    );
    let spec = grammar(
        "start: P Q A B Z PQ\nP.3: /p/\nQ.3: /q/\nA: /pq/\nB.3: /q+b/\nZ: /z/\nPQ.3: /(?<=z)pq/\n",
    );
    let a = tok(&spec, "A", "pq");
    assert_eq!(
        a.reading,
        Reading::Tokens(vec![tid(&spec, "P"), tid(&spec, "Q")])
    );
    assert_eq!(
        basic_fit(&spec, &[a.clone(), tok(&spec, "B", "qb")]),
        Fit::Wrong
    );
    assert_eq!(
        basic_fit(&spec, &[tok(&spec, "Z", "z"), a.clone()]),
        Fit::Wrong
    );
    assert_eq!(
        basic_fit(&spec, &[a.clone(), tok(&spec, "Z", "z")]),
        Fit::Exact
    );
}

#[test]
fn basic_fit_reads_ignorable_stretches_loosely() {
    let spec = grammar("start: A B X\nA: /a/\nB.3: /(?<=ax)x+b/\nX.2: /x/\n%ignore I\nI: /x+/\n");
    let a = tok(&spec, "A", "a");
    let i = tok(&spec, "I", "xx");
    assert_eq!(
        i.reading,
        Reading::Tokens(vec![tid(&spec, "X"), tid(&spec, "X")])
    );
    assert_eq!(
        basic_fit(&spec, &[a.clone(), i.clone(), tok(&spec, "A", "a")]),
        Fit::Retyped
    );
    let b = Token {
        reading: Reading::Itself,
        ..tok(&spec, "B", "xxb")
    };
    assert_eq!(basic_fit(&spec, &[a.clone(), i.clone(), b]), Fit::Wrong);
}

#[test]
fn basic_fit_trusts_what_it_cannot_lex() {
    let spec = grammar("start: A B\nA: /(a*)*b/\nB: /c/\n");
    let a = tok(&spec, "A", &"a".repeat(40));
    assert_eq!(a.reading, Reading::Opaque);
    let short = tok(&spec, "A", "ab");
    let b = tok(&spec, "B", "c");
    assert_eq!(basic_fit(&spec, &[short.clone(), b.clone()]), Fit::Exact);
    let long = Token {
        reading: Reading::Itself,
        ..a
    };
    assert_eq!(basic_fit(&spec, &[long, b]), Fit::Exact);
}

#[test]
fn a_regex_may_run_on_past_the_next_token_to_a_later_one() {
    let spec = grammar("start: A B C\nA: /a(\\\\x|.)*?x/\nB: /y/\nC: /x/\n");
    let a = tok(&spec, "A", "a\\x");
    let b = tok(&spec, "B", "y");
    let c = tok(&spec, "C", "x");
    assert_eq!(a.reading, Reading::Itself);
    assert_eq!(fit(&spec, &[a.clone(), b.clone()]), Fit::Exact);
    assert_eq!(fit(&spec, &[b.clone(), c.clone()]), Fit::Settled);
    assert_eq!(fit(&spec, &[a.clone(), b.clone(), c.clone()]), Fit::Wrong);
    let longer = tok(&spec, "A", "a\\xx");
    assert_eq!(fit(&spec, &[longer, b, c]), Fit::Settled);
}

#[test]
fn strings_that_do_not_lex_back_whole_are_regenerated() {
    let text = "start: A B C\nA: /a(\\\\x|.)*?x/\nB: /y/\nC: /x/\n";
    let spec = spec_with(
        text,
        GrammarOptions {
            explicit: vec![explicit("A", "a\\\\x|a\\\\xx")],
            ..GrammarOptions::default()
        },
    )
    .unwrap();
    let strings = samples(&spec, 30);
    assert!(strings.iter().all(|s| s == "a\\xxyx"), "{strings:?}");
    let spec = spec_with(
        text,
        GrammarOptions {
            explicit: vec![explicit("A", "a\\\\x")],
            ..GrammarOptions::default()
        },
    )
    .unwrap();
    let (result, status) = draw(&spec, 0);
    assert!(matches!(result, Err(EngineError::InvalidTestCase)));
    assert_eq!(status, Some(Status::Invalid));
}

struct FixtureImports;

impl ImportSource for FixtureImports {
    fn load(&self, base_path: Option<&str>, grammar_path: &str) -> Option<(String, String)> {
        let base = base_path?;
        let path = Path::new(base).join(grammar_path);
        let text = std::fs::read_to_string(&path).ok()?;
        Some((path.to_str().unwrap().to_string(), text))
    }
}

fn fixture_spec(
    name: &str,
    start: &str,
    max_size: u64,
    explicit: Vec<(String, Arc<StringSpec>)>,
) -> GrammarSpec {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/lark")
        .join(format!("{name}.lark"));
    let text = std::fs::read_to_string(&path).unwrap();
    let built = load_grammar(&text, Some(path.to_str().unwrap()), &FixtureImports).unwrap();
    let grammar = compile(&built, &[start]).unwrap();
    GrammarSpec::new(
        grammar,
        GrammarOptions {
            start: start.to_string(),
            max_size,
            explicit,
            ..GrammarOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn fixture_grammars_generate() {
    for (name, start) in [("json", "value"), ("calc", "start"), ("features", "start")] {
        let spec = fixture_spec(name, start, 2000, vec![]);
        let strings = samples(&spec, 20);
        assert!(strings.iter().all(|s| s.chars().count() <= 2000), "{name}");
        assert!(strings.iter().any(|s| s.chars().count() > 500), "{name}");
    }
}

#[test]
fn cached_scans_extend_as_fresh_matches_do() {
    let mut extended = 0;
    for (name, start) in [
        ("json", "value"),
        ("calc", "start"),
        ("features", "start"),
        ("verilog", "start"),
        ("template_lark", "start"),
        ("turtle", "start"),
    ] {
        let spec = fixture_spec(name, start, 48, vec![]);
        let lexer = &spec.lexer;
        let terminals: Vec<TermId> = (0..spec.terminal_names().len())
            .filter(|&id| spec.is_pattern(id))
            .collect();
        for seed in 0..6 {
            let Ok(sample) = draw(&spec, seed).0 else {
                continue;
            };
            let text: Vec<char> = sample.chars().collect();
            for a in 0..text.len() {
                for b in a + 1..=text.len() {
                    let part = &text[a..b];
                    let scan = lexer.next_token_scan(part, 0);
                    if let Some(lexed) = lexer.extend(&scan, &text, a, b - a, true) {
                        extended += 1;
                        assert_eq!(lexed, lexer.next_token(&text, a), "{name} {a}..{b}");
                    }
                    for &id in &terminals {
                        let scan = lexer.match_terminal_scan(id, part, 0);
                        if let Some(lexed) = lexer.extend(&scan, &text, a, b - a, false) {
                            extended += 1;
                            assert_eq!(
                                lexed,
                                lexer.match_terminal(id, &text, a),
                                "{name} {} {a}..{b}",
                                spec.terminal_names()[id]
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(extended > 10_000);
}

#[test]
fn settled_stretches_read_back_exactly_and_settled_strings_lex_as_wholes() {
    let mut settled = 0;
    let mut unsettled = 0;
    let mut whole_strings = 0;
    for (name, start) in [
        ("json", "value"),
        ("calc", "start"),
        ("features", "start"),
        ("verilog", "start"),
        ("template_lark", "start"),
        ("turtle", "start"),
    ] {
        let spec = fixture_spec(name, start, 400, vec![]);
        for seed in 0..8 {
            let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
            let mut generation = Generation {
                spec: &spec,
                tokens: Vec::new(),
                consumed: 0,
                ignore_rate: 0.25,
                unsettled: false,
            };
            if generation.run(&mut ntc, 200).is_err() {
                continue;
            }
            let tokens: Vec<&Token> = generation.tokens.iter().collect();
            for width in 2..=3 {
                for window in tokens.windows(width) {
                    let text = text_of(window);
                    if spec.settled(&text, window) {
                        settled += 1;
                        assert!(spec.scans(&text, window), "{name} {seed} {text:?}");
                        assert_eq!(
                            spec.basic_fit(&text, window),
                            Fit::Exact,
                            "{name} {seed} {text:?}"
                        );
                    } else {
                        unsettled += 1;
                    }
                }
            }
            if !generation.unsettled {
                whole_strings += 1;
                let text = text_of(&tokens);
                assert!(spec.scans(&text, &tokens), "{name} {seed} {text:?}");
                assert!(tokens.iter().all(|t| spec.single(t) && t.whole()));
            }
        }
    }
    assert!(
        settled > 2 * unsettled,
        "{settled} settled, {unsettled} not"
    );
    assert!(whole_strings > 0);
}

/// Writes the corpus `scripts/check-lark.py` parses with Lark: for each
/// fixture grammar named in `HEGEL_LARK_CORPUS_GRAMMARS`, one JSON string
/// per line of `HEGEL_LARK_CORPUS_N` generated strings, in
/// `HEGEL_LARK_CORPUS_DIR/<grammar>.jsonl`. The start rule is the first
/// one the grammar's `.expected` file names.
#[test]
#[ignore]
fn write_corpus() {
    let dir = std::env::var("HEGEL_LARK_CORPUS_DIR").unwrap();
    let grammars = std::env::var("HEGEL_LARK_CORPUS_GRAMMARS").unwrap();
    let n: u64 = std::env::var("HEGEL_LARK_CORPUS_N")
        .map(|s| s.parse().unwrap())
        .unwrap_or(100);
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lark");
    for name in grammars.split(',').filter(|name| !name.is_empty()) {
        let expected = std::fs::read_to_string(fixtures.join(format!("{name}.expected"))).unwrap();
        let start = expected
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("start "))
            .and_then(|starts| starts.split_whitespace().next())
            .unwrap();
        let spec = fixture_spec(name, start, DEFAULT_MAX_SIZE, vec![]);
        let mut lines = String::new();
        let mut sizes = vec![];
        let mut invalid = 0;
        let started = std::time::Instant::now();
        for seed in 0..n {
            let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
            let outcome = generate_grammar(&mut ntc, &spec);
            ntc.freeze();
            match outcome {
                Ok(s) => {
                    sizes.push(s.chars().count());
                    lines.push_str(&serde_json::to_string(&s).unwrap());
                    lines.push('\n');
                }
                Err(_) => invalid += 1,
            }
        }
        std::fs::write(Path::new(&dir).join(format!("{name}.jsonl")), lines).unwrap();
        sizes.sort_unstable();
        std::eprintln!(
            "{name}: {} strings, {invalid} invalid, median {} chars, max {} chars, {:?} per string",
            sizes.len(),
            sizes.get(sizes.len() / 2).copied().unwrap_or(0),
            sizes.last().copied().unwrap_or(0),
            started.elapsed() / n as u32
        );
    }
}

#[test]
fn engine_errors_inside_the_walk_propagate() {
    let spec = grammar("start: \"a\" | \"b\"\n");
    let mut ntc = NativeTestCase::for_choices(&[ChoiceValue::Boolean(false)], None, None);
    assert!(matches!(
        generate_grammar(&mut ntc, &spec),
        Err(EngineError::Overrun)
    ));
}

#[test]
fn ignore_cost_compares_insertions_with_tokens() {
    let spec = grammar("start: /[ab]/ /cd/\n");
    assert_eq!(spec.ignore_cost, 0.0);
    let spec = grammar("start: /[ab]/ /cd/\nWS: / {9}/\n%ignore WS\n");
    assert_eq!(spec.ignore_cost, 6.0);
    let spec = grammar("start: WS\nWS: / {3}/\n%ignore WS\n");
    assert_eq!(spec.ignore_cost, 1.0);
}

#[test]
fn token_budgets_leave_room_for_ignore_insertions() {
    assert_eq!(token_budget(350, 1, 0.0, 9.0), 350);
    assert_eq!(token_budget(350, 1, 1.0, 9.0), 35);
    assert_eq!(token_budget(350, 1, 0.25, 4.0), 175);
    assert_eq!(token_budget(3, 2, 1.0, 9.0), 2);
}
