//! Tests for `src/native/lark/generate.rs`: that every string drawn from
//! a grammar is in it, that the size and depth caps hold, how `%ignore`
//! terminals are placed, and the lexer-fit checks that decide it.

use super::*;
use crate::native::core::state::codepoints_to_string;
use crate::native::core::{ChoiceKind, ChoiceValue, GenerationParameters, NativeTestCase};
use crate::native::draws::TextAlphabet;
use crate::native::draws::source::{RngDraws, SimplestDraws};
use crate::native::intervalsets::IntervalSet;
use crate::native::lark::analysis::Bound;
use crate::native::lark::builder::{ImportSource, NoImports};
use crate::native::lark::parse::Parser;
use crate::native::rng::EngineRng;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;
use core::ops::{Deref, DerefMut};
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

/// [`draw`] from an RNG alone, seeded as the test case's would be, with
/// the swarm parameters the test case draws from it first.
fn draw_from_rng(spec: &GrammarSpec, seed: u64) -> (Result<String, EngineError>, Option<Status>) {
    let mut rng = EngineRng::seeded(seed);
    let params = GenerationParameters::draw(&mut rng).unwrap();
    let mut draws = RngDraws::new(&mut rng, params.integer);
    let result = draw_grammar(&mut draws, spec).map(|cps| codepoints_to_string(&cps));
    (result, draws.status())
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

/// A token with its text and what the lexers find in it, to lay out
/// with others in a string of their own.
#[derive(Clone)]
struct Tok {
    token: Token,
    text: Vec<char>,
    scans: TokenScans,
}

impl Tok {
    fn reading(&self) -> &Reading {
        &self.scans.reading
    }

    fn lexable(&self) -> bool {
        matches!(self.reading(), Reading::Itself | Reading::Ignorables)
    }
}

impl Deref for Tok {
    type Target = Token;

    fn deref(&self) -> &Token {
        &self.token
    }
}

impl DerefMut for Tok {
    fn deref_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}

fn tok(spec: &GrammarSpec, name: &str, text: &str) -> Tok {
    let id = tid(spec, name);
    let text: Vec<char> = text.chars().collect();
    let scans = spec.scan_token(id, &text, None);
    let token = Token {
        id,
        start: 0,
        end: text.len(),
        droppable: false,
        scans: 0,
    };
    Tok { token, text, scans }
}

/// `tok` with its text read as `reading`, whatever the lexer says.
fn reread(tok: &mut Tok, reading: Reading) {
    tok.scans.reading = reading;
}

/// `toks` one after another: the text, the tokens placed in it and
/// their findings, which the tokens index.
fn lay_out(toks: &[Tok]) -> (Vec<char>, Vec<Token>, Vec<TokenScans>) {
    let mut text = Vec::new();
    let mut tokens = Vec::new();
    let mut findings = Vec::new();
    for tok in toks {
        let start = text.len();
        text.extend_from_slice(&tok.text);
        tokens.push(Token {
            start,
            end: text.len(),
            scans: findings.len(),
            ..tok.token.clone()
        });
        findings.push(tok.scans.clone());
    }
    (text, tokens, findings)
}

fn fit(spec: &GrammarSpec, toks: &[Tok]) -> Fit {
    let (text, tokens, findings) = lay_out(toks);
    spec.fit(&text, &tokens, &findings)
}

fn basic_fit(spec: &GrammarSpec, toks: &[Tok]) -> Fit {
    let (text, tokens, findings) = lay_out(toks);
    spec.basic_fit(&text, &tokens, &findings)
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
        let mut generation = Generation::new(&spec, 0.0);
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
        let mut generation = Generation::new(&spec, 0.0);
        assert!(generation.run(&mut ntc, 500).is_ok(), "seed {seed}");
        assert!(
            (250..=1500).contains(&generation.text.len()),
            "seed {seed}: {} characters for a budget of 500",
            generation.text.len()
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
    assert!(matches!(result, Err(EngineError::InvalidArgument(_))));
    assert_eq!(status, None);
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
fn unseparable_merging_tokens_leave_no_string_to_generate() {
    let spec = grammar("start: NAME NAME\nNAME: /[a-z]+/\n%ignore COMMENT\nCOMMENT: /#[a-z]*/\n");
    let (result, status) = draw(&spec, 0);
    assert!(matches!(result, Err(EngineError::InvalidArgument(_))));
    assert_eq!(status, None);
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
    assert_eq!(*ws.reading(), Reading::Opaque);
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
    let mut generation = Generation::new(&spec, 0.0);
    (
        generation.text,
        generation.tokens,
        generation.readings.findings,
    ) = lay_out(&[
        tok(&spec, "A", "a"),
        tok(&spec, "B", "b"),
        tok(&spec, "C", "c"),
    ]);
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
    assert_eq!(*tok(&spec, "A", "ab").reading(), Reading::Opaque);
    let spec = grammar(
        "start: KW NAME\nKW: \"if\"\nNAME: /[a-z]+/\n%ignore WS\n%ignore NL\nWS: / +/\nNL: /\\n/\n",
    );
    assert_eq!(
        *tok(&spec, "NAME", "if").reading(),
        Reading::Tokens(vec![tid(&spec, "KW")])
    );
    assert_eq!(*tok(&spec, "NAME", "iffy").reading(), Reading::Itself);
    assert_eq!(*tok(&spec, "WS", "  ").reading(), Reading::Itself);
    let ws = tid(&spec, "WS");
    assert_eq!(*tok(&spec, "WS", " \n").reading(), Reading::Ignorables);
    assert!(tok(&spec, "WS", " \n").lexable());
    assert_eq!(
        *tok(&spec, "WS", " x").reading(),
        Reading::Tokens(vec![ws, tid(&spec, "NAME")])
    );
    assert_eq!(*tok(&spec, "NAME", "9").reading(), Reading::Opaque);
    assert_eq!(*tok(&spec, "WS", "").reading(), Reading::Ignorables);
    assert_eq!(*tok(&spec, "NAME", "").reading(), Reading::Tokens(vec![]));
}

#[test]
fn a_reading_is_how_the_basic_lexer_reads_the_text_afresh() {
    let mut read = 0;
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
            let mut generation = Generation::new(&spec, 0.25);
            if generation.run(&mut ntc, 200).is_err() {
                continue;
            }
            for token in &generation.tokens {
                let text = &generation.text[token.start..token.end];
                let mut fresh = Vec::new();
                let lexed = spec.lex_all(text, &[], &mut fresh);
                let ids: Vec<TermId> = fresh.iter().map(|&(g, _, _)| g).collect();
                read += 1;
                match token.reading(&generation.readings.findings) {
                    Reading::Opaque => assert!(lexed.is_err(), "{name} {seed} {text:?}"),
                    Reading::Itself => {
                        assert!(lexed.is_ok(), "{name} {seed} {text:?}");
                        assert_eq!(ids.len(), 1, "{name} {seed} {text:?}");
                        assert!(spec.same_type(token.id, ids[0]), "{name} {seed} {text:?}");
                    }
                    Reading::Ignorables => {
                        assert!(lexed.is_ok(), "{name} {seed} {text:?}");
                        assert!(spec.is_ignore(token.id), "{name} {seed} {text:?}");
                        assert!(
                            ids.iter().all(|&g| spec.is_ignore(g)),
                            "{name} {seed} {text:?}"
                        );
                    }
                    Reading::Tokens(tokens) => {
                        assert!(lexed.is_ok(), "{name} {seed} {text:?}");
                        assert_eq!(*tokens, ids, "{name} {seed} {text:?}");
                    }
                }
            }
        }
    }
    assert!(read > 1000, "{read}");
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
    let (text, tokens, findings) = lay_out(&[c, tok(&spec, "C", "x")]);
    assert!(!spec.scans(&text, &tokens, &findings));
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
        *a.reading(),
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
        *i.reading(),
        Reading::Tokens(vec![tid(&spec, "X"), tid(&spec, "X")])
    );
    assert_eq!(
        basic_fit(&spec, &[a.clone(), i.clone(), tok(&spec, "A", "a")]),
        Fit::Retyped
    );
    let mut b = tok(&spec, "B", "xxb");
    reread(&mut b, Reading::Itself);
    assert_eq!(basic_fit(&spec, &[a.clone(), i.clone(), b]), Fit::Wrong);
}

#[test]
fn basic_fit_trusts_what_it_cannot_lex() {
    let spec = grammar("start: A B\nA: /(a*)*b/\nB: /c/\n");
    let a = tok(&spec, "A", &"a".repeat(40));
    assert_eq!(*a.reading(), Reading::Opaque);
    let short = tok(&spec, "A", "ab");
    let b = tok(&spec, "B", "c");
    assert_eq!(basic_fit(&spec, &[short.clone(), b.clone()]), Fit::Exact);
    let mut long = a;
    reread(&mut long, Reading::Itself);
    assert_eq!(basic_fit(&spec, &[long, b]), Fit::Exact);
}

#[test]
fn a_regex_may_run_on_past_the_next_token_to_a_later_one() {
    let spec = grammar("start: A B C\nA: /a(\\\\x|.)*?x/\nB: /y/\nC: /x/\n");
    let a = tok(&spec, "A", "a\\x");
    let b = tok(&spec, "B", "y");
    let c = tok(&spec, "C", "x");
    assert_eq!(*a.reading(), Reading::Itself);
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
    assert!(matches!(result, Err(EngineError::InvalidArgument(_))));
    assert_eq!(status, None);
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

fn fixture_grammar(name: &str, start: &str) -> Grammar {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/lark")
        .join(format!("{name}.lark"));
    let text = std::fs::read_to_string(&path).unwrap();
    let built = load_grammar(&text, Some(path.to_str().unwrap()), &FixtureImports).unwrap();
    compile(&built, &[start]).unwrap()
}

fn fixture_spec(
    name: &str,
    start: &str,
    max_size: u64,
    explicit: Vec<(String, Arc<StringSpec>)>,
) -> GrammarSpec {
    GrammarSpec::new(
        fixture_grammar(name, start),
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
            let mut generation = Generation::new(&spec, 0.25);
            if generation.run(&mut ntc, 200).is_err() {
                continue;
            }
            let tokens = &generation.tokens;
            let findings = &generation.readings.findings;
            for width in 2..=3 {
                for window in tokens.windows(width) {
                    let text = &generation.text[window[0].start..window[width - 1].end];
                    if spec.settled(text, window, findings) {
                        settled += 1;
                        assert!(spec.scans(text, window, findings), "{name} {seed} {text:?}");
                        assert_eq!(
                            spec.basic_fit(text, window, findings),
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
                let text = &generation.text;
                assert!(spec.scans(text, tokens, findings), "{name} {seed} {text:?}");
                assert!(
                    tokens
                        .iter()
                        .all(|t| spec.single(t, findings) && t.whole(findings))
                );
            }
        }
    }
    assert!(
        settled > 2 * unsettled,
        "{settled} settled, {unsettled} not"
    );
    assert!(whole_strings > 0);
}

#[test]
fn the_basic_scan_follows_the_own_one_unless_an_earlier_terminal_was_left_open() {
    let spec = grammar("start: A B\nA: /ab+/\nB: /a/\n");
    assert!(tok(&spec, "A", "abb").scans.basic_follows_own);
    assert!(!tok(&spec, "B", "a").scans.basic_follows_own);
}

#[test]
fn settling_a_basic_scan_that_follows_the_own_one_never_changes_the_answer() {
    let mut followed = 0;
    let mut checked = 0;
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
            let mut generation = Generation::new(&spec, 0.25);
            if generation.run(&mut ntc, 200).is_err() {
                continue;
            }
            let tokens = &generation.tokens;
            let findings = &generation.readings.findings;
            for token in tokens {
                if findings[token.scans].basic_follows_own {
                    followed += 1;
                } else {
                    checked += 1;
                }
            }
            let checked_findings: Vec<TokenScans> = findings
                .iter()
                .map(|scans| TokenScans {
                    basic_follows_own: false,
                    ..scans.clone()
                })
                .collect();
            for width in 2..=3 {
                for window in tokens.windows(width) {
                    let text = &generation.text[window[0].start..window[width - 1].end];
                    assert_eq!(
                        spec.settled(text, window, findings),
                        spec.settled(text, window, &checked_findings),
                        "{name} {seed} {text:?}"
                    );
                }
            }
        }
    }
    assert!(followed > checked, "{followed} followed, {checked} checked");
    assert!(checked > 0);
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
fn draws_work_in_the_buffers_the_spec_has_to_spare() {
    let spec = grammar("start: (\"ab\" | \"cd\")+\n");
    assert!(spec.buffers.lock().is_none());
    draw(&spec, 0).0.unwrap();
    let used = {
        let buffers = spec.buffers.lock();
        let kept = buffers.as_ref().unwrap();
        assert!(kept.text.is_empty() && kept.tokens.is_empty());
        assert!(kept.text.capacity() > 0 && kept.text.capacity() < 1 << 16);
        kept.text.capacity()
    };
    let roomy = Buffers {
        text: Vec::with_capacity(1 << 16),
        ..Buffers::new()
    };
    let displaced = roomy.offer(&mut spec.buffers.lock()).unwrap();
    assert_eq!(displaced.text.capacity(), used);
    draw(&spec, 1).0.unwrap();
    assert!(spec.buffers.lock().as_ref().unwrap().text.capacity() >= 1 << 16);

    let unkept = Buffers::new().offer(&mut spec.buffers.lock()).unwrap();
    assert_eq!(unkept.text.capacity(), 0);
    assert!(spec.buffers.lock().as_ref().unwrap().text.capacity() >= 1 << 16);

    let mut ntc = NativeTestCase::for_choices(&[], None, None);
    assert!(generate_grammar(&mut ntc, &spec).is_err());
    assert!(spec.buffers.lock().is_some());
}

#[test]
fn engine_errors_before_the_walk_propagate() {
    let spec = grammar("start: \"a\" | \"b\"\n");
    let mut ntc = NativeTestCase::for_choices(&[], None, None);
    assert!(matches!(
        generate_grammar(&mut ntc, &spec),
        Err(EngineError::Overrun)
    ));
    assert!(ntc.nodes.is_empty());
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

#[test]
fn chains_skip_ignorables_the_previous_one_would_swallow() {
    let spec = small(
        "start: NUM+\nNUM: /[0-9]/\nWS: / +/\nCOMMENT: /#[#a-z0-9 ]*/\n%ignore WS\n%ignore COMMENT\n",
        200,
    );
    for s in samples(&spec, 100) {
        let body = s.split('#').next().unwrap();
        assert!(
            body.chars().all(|c| c.is_ascii_digit() || c == ' '),
            "{s:?}"
        );
    }
}

#[test]
fn chain_pairs_that_keep_failing_are_given_up_on() {
    let spec = small(
        "start: NUM NUM NUM NUM NUM NUM NUM NUM\nNUM: /[0-9]/\nCOMMENT: /#[^\\n]*/\n\
         SEP: /\\n[^\\n]*|x/\n%ignore COMMENT\n%ignore SEP\n",
        4000,
    );
    let comment = tid(&spec, "COMMENT");
    let sep = tid(&spec, "SEP");
    let mut given_up = 0;
    for seed in 0..64 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let mut generation = Generation::new(&spec, 0.5);
        if generation.run(&mut ntc, 2000).is_err() {
            continue;
        }
        for (pair, &(failed, succeeded)) in &generation.chains {
            assert!(failed <= IGNORE_CHAIN_PATIENCE, "seed {seed} {pair:?}");
            assert_eq!(succeeded, 0, "seed {seed} {pair:?}");
        }
        if generation.chains.get(&(comment, sep)) == Some(&(IGNORE_CHAIN_PATIENCE, 0)) {
            given_up += 1;
        }
    }
    assert!(given_up > 8, "{given_up}");
    let mut generation = Generation::new(&spec, 0.5);
    generation.chains.insert((comment, sep), (7, 1));
    assert!(generation.hopeful(comment, sep));
    generation.chains.insert((comment, sep), (8, 1));
    assert!(!generation.hopeful(comment, sep));
}

#[test]
fn a_generation_remembers_the_lexing_of_its_token_texts_until_a_draw_finds_too_many() {
    let spec = grammar("start: NAME+\nNAME: /[a-z]+/\n");
    let name = tid(&spec, "NAME");
    let text = |n: usize| -> Vec<char> {
        format!("x{n}")
            .chars()
            .map(|c| c.to_digit(10).map_or(c, |d| (b'a' + d as u8) as char))
            .collect()
    };
    let mut generation = Generation::new(&spec, 0.0);
    let room = |readings: &ReadingsCache| {
        (
            readings.table.capacity(),
            readings.texts.capacity(),
            readings.findings.capacity(),
        )
    };
    let fresh = room(&generation.readings);
    assert!(fresh.0 >= READINGS_CAPACITY, "{fresh:?}");
    assert_eq!(fresh.1, READINGS_TEXT_CAPACITY);
    assert_eq!(fresh.2, READINGS_CAPACITY);
    let first = generation.readings.read_token(&spec, name, &text(0), None);
    assert_eq!(generation.readings.findings[first].reading, Reading::Itself);
    assert_eq!(generation.readings.len, 1);
    assert_eq!(
        generation.readings.read_token(&spec, name, &text(0), None),
        first
    );
    assert_eq!(generation.readings.len, 1);
    assert_eq!(generation.readings.findings.len(), 1);
    for n in 1..READINGS_CAPACITY - 1 {
        generation.readings.read_token(&spec, name, &text(n), None);
    }
    assert_eq!(generation.readings.len, READINGS_CAPACITY - 1);
    assert_eq!(room(&generation.readings), fresh);
    generation.recycle();
    let mut generation = Generation::new(&spec, 0.0);
    assert_eq!(generation.readings.len, READINGS_CAPACITY - 1);
    assert_eq!(
        generation.readings.read_token(&spec, name, &text(0), None),
        first
    );
    for n in READINGS_CAPACITY - 1..=READINGS_CAPACITY {
        generation.readings.read_token(&spec, name, &text(n), None);
    }
    assert_eq!(generation.readings.len, READINGS_CAPACITY + 1);
    assert_eq!(
        generation.readings.read_token(&spec, name, &text(0), None),
        first
    );
    generation.recycle();
    let mut generation = Generation::new(&spec, 0.0);
    assert_eq!(generation.readings.len, 0);
    assert!(generation.readings.findings.is_empty());
    assert!(generation.readings.texts.is_empty());
    assert_eq!(generation.readings.table.len(), 0);
    assert!(room(&generation.readings) >= fresh);
    let after = generation.readings.read_token(&spec, name, &text(0), None);
    assert_eq!(after, 0);
    assert_eq!(generation.readings.findings[after].reading, Reading::Itself);
    assert_eq!(generation.readings.len, 1);
    assert_eq!(generation.readings.texts, text(0));
}

#[test]
fn a_text_remembered_again_takes_the_place_of_what_was_held() {
    let spec = grammar("start: NAME+\nNAME: /[a-z]+/\n");
    let name = tid(&spec, "NAME");
    let text: Vec<char> = "abc".chars().collect();
    let mut readings = ReadingsCache::new();
    let first = readings.read_token(&spec, name, &text, None);
    let hash = text_hash(name, &text);
    let again = readings.remember(name, hash, &text, spec.scan_token(name, &text, None));
    assert_ne!(again, first);
    assert_eq!(readings.find(name, hash, &text), Some(again));
    assert_eq!(readings.len, 1);
    assert_eq!(readings.findings.len(), 2);
    assert_eq!(readings.texts, text);
    let shorter = &text[..2];
    assert!(
        readings
            .find(name, text_hash(name, shorter), shorter)
            .is_none()
    );
}

#[test]
fn a_text_is_remembered_apart_for_each_terminal_drawn_as() {
    let spec = grammar("start: NAME | ABC\nNAME: /[a-z]+/\nABC: \"abc\"\n");
    let name = tid(&spec, "NAME");
    let abc = tid(&spec, "ABC");
    let text: Vec<char> = "abc".chars().collect();
    let mut readings = ReadingsCache::new();
    let as_name = readings.read_token(&spec, name, &text, None);
    let as_abc = readings.read_token(&spec, abc, &text, None);
    assert_ne!(as_name, as_abc);
    assert_eq!(readings.len, 2);
    assert_eq!(readings.findings.len(), 2);
    assert_eq!(readings.texts.len(), 2 * text.len());
    assert_eq!(readings.recall(name, &text), Ok(as_name));
    assert_eq!(readings.recall(abc, &text), Ok(as_abc));
    assert_eq!(readings.read_token(&spec, name, &text, None), as_name);
    assert_eq!(readings.read_token(&spec, abc, &text, None), as_abc);
    assert_eq!(readings.len, 2);
    let hash = text_hash(name, &text);
    assert_eq!(readings.find(abc, hash, &text), None);
    assert_eq!(readings.find(name, text_hash(abc, &text), &text), None);
}

#[test]
fn a_text_recalled_is_what_was_learnt_of_it() {
    let spec = grammar("start: NAME+\nNAME: /[a-z]+/\n");
    let name = tid(&spec, "NAME");
    let text: Vec<char> = "abc".chars().collect();
    let mut readings = ReadingsCache::new();
    let Err(hash) = readings.recall(name, &text) else {
        panic!("a text not drawn yet was recalled");
    };
    assert_eq!(hash, text_hash(name, &text));
    let learnt = readings.learn(&spec, name, hash, &text, None);
    assert_eq!(readings.findings[learnt].reading, Reading::Itself);
    assert_eq!(readings.recall(name, &text), Ok(learnt));
    assert_eq!(readings.read_token(&spec, name, &text, None), learnt);
    assert_eq!(readings.len, 1);
    assert_eq!(readings.findings.len(), 1);
}

#[test]
fn a_remembered_text_the_pattern_does_not_take_whole_is_cut_like_a_fresh_one() {
    let text = "start: A\nA: /a|ab/\n";
    let fresh = grammar(text);
    let seeded = grammar(text);
    let a = tid(&seeded, "A");
    let ab: Vec<char> = "ab".chars().collect();
    let mut generation = Generation::new(&seeded, 0.0);
    let remembered = generation.readings.read_token(&seeded, a, &ab, None);
    assert_eq!(
        taken(&generation.readings.findings[remembered].own, ab.len()),
        1
    );
    generation.recycle();
    let strings = samples(&seeded, 50);
    assert_eq!(strings, samples(&fresh, 50));
    assert!(strings.iter().all(|s| s == "a"), "{strings:?}");
    let generation = Generation::new(&seeded, 0.0);
    assert_eq!(generation.readings.recall(a, &ab), Ok(remembered));
    assert_eq!(generation.readings.len, 2);
    assert_eq!(Generation::new(&fresh, 0.0).readings.len, 1);
}

#[test]
fn a_text_hashes_the_same_however_it_was_built_and_unlike_its_prefixes() {
    let texts: Vec<Vec<char>> = (0..6).map(|n| "abcde"[..n].chars().collect()).collect();
    for text in &texts {
        let copy = text.to_vec();
        assert_eq!(text_hash(0, text), text_hash(0, &copy));
        assert_ne!(text_hash(0, text), text_hash(1, text));
    }
    for (i, text) in texts.iter().enumerate() {
        for shorter in &texts[..i] {
            assert_ne!(text_hash(0, text), text_hash(0, shorter));
        }
    }
}

#[test]
fn nth_set_bit_counts_from_the_lowest() {
    assert_eq!(nth_set_bit(0b1011, 0), Some(0));
    assert_eq!(nth_set_bit(0b1011, 1), Some(1));
    assert_eq!(nth_set_bit(0b1011, 2), Some(3));
    assert_eq!(nth_set_bit(0b1011, 3), None);
    assert_eq!(nth_set_bit(0, 0), None);
    assert_eq!(nth_set_bit(1 << 63, 0), Some(63));
    assert_eq!(nth_set_bit(u64::MAX, 63), Some(63));
    assert_eq!(nth_set_bit(u64::MAX, 64), None);
    assert_eq!(nth_set_bit(u64::MAX, u64::MAX), None);
}

fn alt(min_size: u64, typical_size: u64, max_size: Bound, min_depth: u32) -> Alt {
    Alt {
        min_size,
        typical_size,
        max_size,
        min_depth,
        items: vec![],
    }
}

#[test]
fn the_classes_of_alternatives_are_worked_out_once_for_every_budget() {
    let alts = [
        alt(1, 2, Some(3), 0),
        alt(2, 4, None, 3),
        alt(2, 5, Some(9), 1),
        alt(7, 7, None, 2),
    ];
    let classes = AltClasses::of(&alts).unwrap();
    assert_eq!(
        classes,
        AltClasses {
            min_depth: 3,
            by_prefix: vec![(0, u64::MAX), (0b0010, 4), (0b0010, 4), (0b1010, 4)],
        }
    );
    for budget in 0..9 {
        for depth in 0..4 {
            let found = feasible_classes(&alts, budget, depth, 5);
            let at = classes.at(&alts, budget, depth, 5);
            if depth <= 2 {
                assert_eq!(at, Some(found), "{budget} {depth}");
            } else {
                assert_eq!(at, None, "{budget} {depth}");
            }
        }
    }
    assert_eq!(classes.at(&alts, 7, 2, 5), Some((0b1111, 0b1010, 4)));
    assert_eq!(classes.at(&alts, 6, 2, 5), Some((0b0111, 0b0010, 4)));
    assert_eq!(classes.at(&alts, 1, 2, 5), Some((0b0001, 0, u64::MAX)));
    assert_eq!(classes.at(&alts, 0, 2, 5), Some((0, 0, u64::MAX)));
    assert_eq!(classes.at(&alts, 7, 0, 2), None);
    let unsorted = [alt(2, 2, None, 0), alt(1, 1, None, 0)];
    assert_eq!(AltClasses::of(&unsorted), None);
    assert_eq!(
        AltClasses::of(&[]).unwrap(),
        AltClasses {
            min_depth: 0,
            by_prefix: vec![],
        }
    );
    assert_eq!(
        AltClasses::of(&[]).unwrap().at(&[], 3, 0, 1),
        Some((0, 0, u64::MAX))
    );
    let many: Vec<Alt> = (0..MASKED_ALTS as u64)
        .map(|i| alt(i, i, None, 0))
        .collect();
    let classes = AltClasses::of(&many).unwrap();
    assert_eq!(classes.by_prefix.len(), MASKED_ALTS);
    assert_eq!(classes.at(&many, 63, 0, 1), Some((u64::MAX, !1, 1)));
    assert_eq!(
        classes.at(&many, 62, 0, 1),
        Some((u64::MAX >> 1, (u64::MAX >> 1) & !1, 1))
    );
    let too_many: Vec<Alt> = (0..=MASKED_ALTS as u64)
        .map(|i| alt(i, i, None, 0))
        .collect();
    assert_eq!(AltClasses::of(&too_many), None);
}

#[test]
fn every_group_has_its_classes_by_its_number() {
    let spec = grammar(
        "start: (\"a\" | \"b\" (\"c\" | \"d\")+)* other\nother: \"e\" | (\"f\" | \"g\" \"h\")\n",
    );
    assert_eq!(spec.analysis.groups, 3);
    assert_eq!(spec.group_classes.len(), 3);
    let mut seen = 0;
    for rule in &spec.analysis.rules {
        for alt in &rule.alternatives {
            for node in &alt.items {
                if let NodeKind::Repeat { inner, .. } = &node.kind {
                    let NodeKind::Group(group, alts) = &inner.kind else {
                        panic!("{:?}", inner.kind);
                    };
                    assert_eq!(spec.group_classes[*group], AltClasses::of(alts));
                    seen += 1;
                }
                if let NodeKind::Group(group, alts) = &node.kind {
                    assert_eq!(spec.group_classes[*group], AltClasses::of(alts));
                    seen += 1;
                }
            }
        }
    }
    assert_eq!(seen, 2);
    assert!(spec.group_classes.iter().all(|classes| classes.is_some()));
    let strings = samples(&spec, 50);
    assert!(strings.iter().any(|s| s.contains('c') || s.contains('d')));
    assert!(strings.iter().any(|s| s.contains('g')));
}

#[test]
fn choosing_an_alternative_by_mask_agrees_with_scanning() {
    let spec = spec_with(
        "start: \"a\"\n",
        GrammarOptions {
            max_depth: 6,
            ..GrammarOptions::default()
        },
    )
    .unwrap();
    let mut rng = EngineRng::seeded(0);
    let (mut chosen, mut infeasible, mut beyond_mask, mut grew) = (0, 0, 0, 0);
    let (mut all, mut some, mut too_deep) = (0, 0, 0);
    for case in 0..600u64 {
        let n = 1 + usize::from(rng.next_byte()) % 70;
        let growth = rng.next_byte() % 3;
        let mut alts: Vec<Alt> = (0..n)
            .map(|_| {
                let min_size = u64::from(rng.next_byte() % 8);
                let typical_size = min_size + u64::from(rng.next_byte() % 4);
                let grows = match growth {
                    0 => false,
                    1 => true,
                    _ => rng.next_byte() % 2 == 0,
                };
                let max_size = (!grows).then_some(typical_size + 3);
                alt(
                    min_size,
                    typical_size,
                    max_size,
                    u32::from(rng.next_byte() % 4),
                )
            })
            .collect();
        alts.sort_by_key(|alt| alt.min_size);
        let budget = u64::from(rng.next_byte() % 12);
        let depth = u32::from(rng.next_byte() % 6);
        let mut by_mask = NativeTestCase::new_random(EngineRng::seeded(case)).unwrap();
        let mut by_classes = NativeTestCase::new_random(EngineRng::seeded(case)).unwrap();
        let mut by_scan = NativeTestCase::new_random(EngineRng::seeded(case)).unwrap();
        let mut generation = Generation::new(&spec, 0.0);
        let position = |alt: &Alt| alts.iter().position(|a| core::ptr::eq(a, alt)).unwrap();
        let masked = generation
            .choose_alt(&mut by_mask, &alts, None, budget, depth)
            .map(position);
        let classes = AltClasses::of(&alts);
        match classes
            .as_ref()
            .map(|classes| classes.at(&alts, budget, depth, spec.max_depth))
        {
            Some(Some((feasible, _, _))) if feasible.count_ones() as usize == n => all += 1,
            Some(Some(_)) => some += 1,
            Some(None) => too_deep += 1,
            None => {}
        }
        let classed = generation
            .choose_alt(&mut by_classes, &alts, classes.as_ref(), budget, depth)
            .map(position);
        let scanned = generation
            .choose_alt_scanning(&mut by_scan, &alts, budget, depth)
            .map(position);
        match (masked, classed, scanned) {
            (Ok(a), Ok(b), Ok(c)) => {
                assert_eq!((a, b), (c, c));
                chosen += 1;
                beyond_mask += usize::from(n > MASKED_ALTS);
                grew += usize::from(alts[a].growing());
            }
            (Err(_), Err(_), Err(_)) => infeasible += 1,
            (masked, classed, scanned) => {
                panic!("{masked:?} by mask, {classed:?} by classes but {scanned:?} by scanning")
            }
        }
        assert_eq!(by_mask.nodes, by_scan.nodes);
        assert_eq!(by_classes.nodes, by_scan.nodes);
    }
    assert!(
        chosen > 500 && infeasible >= 10 && beyond_mask > 30 && grew > 200,
        "{chosen} {infeasible} {beyond_mask} {grew}"
    );
    assert!(
        all > 50 && some > 50 && too_deep > 50,
        "{all} {some} {too_deep}"
    );
}

#[test]
fn rules_with_more_alternatives_than_a_mask_holds() {
    let keywords: Vec<String> = (0..70).map(|i| format!("\"k{i}\"")).collect();
    let spec = grammar(&format!("start: {}\n", keywords.join(" | ")));
    let drawn: Vec<usize> = samples(&spec, 40)
        .iter()
        .map(|s| s.strip_prefix('k').unwrap().parse().unwrap())
        .collect();
    assert!(drawn.iter().all(|&i| i < 70));
    assert!(drawn.iter().any(|&i| i >= MASKED_ALTS));
}

#[test]
fn an_ignorable_skipped_at_a_character_would_not_have_matched_there() {
    let mut skipped = 0;
    let mut tried = 0;
    for (name, start) in [
        ("json", "value"),
        ("features", "start"),
        ("verilog", "start"),
        ("template_lark", "start"),
    ] {
        let spec = fixture_spec(name, start, 400, vec![]);
        for ch in 0u8..128 {
            let ch = char::from(ch);
            let starters = spec.ignore_starters.at(ch);
            for (n, &ignore) in spec.ignore.iter().enumerate() {
                if Starters::has(starters, n) {
                    tried += 1;
                    continue;
                }
                skipped += 1;
                assert_eq!(
                    spec.lexer.match_terminal(ignore, &[ch], 0),
                    Lexed::NoMatch,
                    "{name} {ch:?} {n}"
                );
            }
        }
        assert_eq!(spec.ignore_starters.at('\u{3b1}'), u64::MAX);
    }
    assert!(skipped > 1000);
    assert!(tried > 10);
}

#[test]
fn drawing_from_an_rng_gives_the_strings_a_fresh_test_case_draws() {
    let text = (
        "TEXT".to_string(),
        Arc::new(StringSpec::text(&TextAlphabet::default(), 1, 5).unwrap()),
    );
    let specs = [
        grammar("start: \"a\" \"b\"* | \"c\"+"),
        grammar(
            "start: item (\",\" item)*\nitem: NAME | \"[\" start \"]\"\nNAME: /[a-z]+/\n\
             COMMENT: /#[^\\n]*/\n%import common.WS\n%ignore WS\n%ignore COMMENT",
        ),
        spec_with(
            "start: (TEXT \";\")+\n%declare TEXT",
            GrammarOptions {
                explicit: vec![text],
                ..GrammarOptions::default()
            },
        )
        .unwrap(),
        fixture_spec("json", "value", 2000, vec![]),
        fixture_spec("calc", "start", 2000, vec![]),
    ];
    for (i, spec) in specs.iter().enumerate() {
        for seed in 0..10 {
            let (from_test_case, status) = draw(spec, seed);
            let (from_rng, rng_status) = draw_from_rng(spec, seed);
            assert_eq!(
                from_test_case.as_ref().ok(),
                from_rng.as_ref().ok(),
                "grammar {i} seed {seed}"
            );
            assert_eq!(from_test_case.is_err(), from_rng.is_err());
            assert_eq!(status, rng_status, "grammar {i} seed {seed}");
        }
    }
}

/// The string the simplest source draws from a grammar, which parses
/// and concludes nothing; with how long the analysis says the grammar's
/// smallest string is.
fn simplest_string(grammar: Grammar, options: GrammarOptions) -> (String, u64) {
    let lexer = Lexer::new(&grammar.terminals);
    let parser = Parser::new(&grammar, grammar.start[0]);
    let spec = GrammarSpec::new(grammar, options).unwrap();
    let mut draws = SimplestDraws::default();
    let s = codepoints_to_string(&smallest_string(&mut draws, &spec).unwrap());
    assert_eq!(draws.status(), None);
    assert_eq!(spec.constraint().simplest(), None);
    assert_eq!(codepoints_to_string(spec.ensure_simplest().unwrap()), s);
    assert_eq!(
        spec.kind.simplest().unwrap(),
        spec.constraint().simplest().unwrap()
    );
    let chars: Vec<char> = s.chars().collect();
    assert!(parser.accepts(&lexer, &chars), "{s:?}");
    (s, spec.min_size())
}

fn simple_grammar(text: &str) -> Grammar {
    let built = load_grammar(text, None, &NoImports).unwrap();
    compile(&built, &["start"]).unwrap()
}

fn fixture_options(start: &str) -> GrammarOptions {
    GrammarOptions {
        start: start.to_string(),
        ..GrammarOptions::default()
    }
}

/// The simplest source aims for the smallest string and takes the
/// cheapest alternatives and the fewest repetitions at every step, so
/// the string it draws is a smallest one wherever each terminal's
/// simplest match is its shortest.
#[test]
fn the_simplest_source_draws_the_smallest_string_of_a_grammar() {
    let (s, min) = simplest_string(
        simple_grammar("start: \"a\" \"b\"* | \"c\"+"),
        GrammarOptions::default(),
    );
    assert!(s == "a" || s == "c", "{s:?}");
    assert_eq!(min, 1);
    let (s, min) = simplest_string(
        simple_grammar(
            "start: item (\",\" item)*\nitem: NAME | \"[\" start \"]\"\nNAME: /[a-z]+/\n\
             %import common.WS\n%ignore WS",
        ),
        GrammarOptions::default(),
    );
    assert_eq!((s.as_str(), min), ("a", 1));
    let (s, min) = simplest_string(
        simple_grammar("start: \"x\" \"y\"? (\"p\" | \"qq\") \"z\" ~ 2..4"),
        GrammarOptions::default(),
    );
    assert_eq!((s.as_str(), min), ("xpzz", 4));
    let (s, min) = simplest_string(fixture_grammar("turtle", "start"), fixture_options("start"));
    assert_eq!(s.chars().count() as u64, min, "{s:?}");
}

/// A terminal's simplest match takes the first alternative of its
/// pattern, which need not be the shortest: `SIGNED_NUMBER` lists
/// `FLOAT` before `INT`, so the simplest number is `0e0` where the
/// smallest is one digit.
#[test]
fn a_terminal_whose_first_alternative_is_not_its_shortest_draws_longer_than_the_minimum() {
    for (name, start) in [("json", "value"), ("calc", "start")] {
        let (s, min) = simplest_string(fixture_grammar(name, start), fixture_options(start));
        assert_eq!((s.as_str(), min), ("0e0", 1), "{name}");
    }
}

#[test]
fn a_grammar_string_is_one_choice_under_the_grammar_constraint() {
    let spec =
        grammar("start: item (\",\" item)*\nitem: NAME | \"[\" start \"]\"\nNAME: /[a-z]+/\n");
    for seed in 0..5 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        let s = generate_grammar(&mut ntc, &spec).unwrap();
        assert_eq!(ntc.nodes.len(), 1);
        let node = &ntc.nodes[0];
        assert_eq!(node.value(), ChoiceValue::String(cps(&s)));
        let ChoiceKind::String(kind) = node.kind() else {
            panic!("{node:?}");
        };
        assert_eq!(kind, *spec.kind);
        assert_eq!(kind.grammar.as_deref(), Some(spec.constraint().as_ref()));
        assert!(kind.validate(&cps(&s)));
        assert_eq!(ntc.spans.len(), 1);
        assert_eq!(ntc.spans[0].label, LABEL_GRAMMAR);
        let mut replay = NativeTestCase::for_choices(&[ChoiceValue::String(cps(&s))], None, None);
        assert_eq!(generate_grammar(&mut replay, &spec).unwrap(), s);
        assert_eq!(replay.nodes[0].value(), ChoiceValue::String(cps(&s)));
    }
}

#[test]
fn a_replayed_string_the_grammar_rejects_is_punned_to_the_simplest() {
    let spec = grammar("start: \"a\" \"b\"*\n");
    let mut ntc = NativeTestCase::for_choices(&[ChoiceValue::String(cps("abbb"))], None, None);
    assert_eq!(generate_grammar(&mut ntc, &spec).unwrap(), "abbb");
    let mut ntc = NativeTestCase::for_choices(&[ChoiceValue::String(cps("ba"))], None, None);
    assert_eq!(generate_grammar(&mut ntc, &spec).unwrap(), "a");
    assert_eq!(ntc.nodes[0].value(), ChoiceValue::String(cps("a")));
    let mut ntc = NativeTestCase::for_choices(&[ChoiceValue::Boolean(true)], None, None);
    assert_eq!(generate_grammar(&mut ntc, &spec).unwrap(), "a");
    let mut ntc = NativeTestCase::for_simplest(10).unwrap();
    assert_eq!(generate_grammar(&mut ntc, &spec).unwrap(), "a");
}

#[test]
fn a_failed_draw_concludes_the_test_case_invalid_through_the_rng_source() {
    let spec = grammar("start: NAME OP\nNAME: /[a-z]+/\nOP: /[a-z~]/\n");
    let (mut valid, mut invalid) = (0, 0);
    for seed in 0..40 {
        let (result, status) = draw(&spec, seed);
        match result {
            Ok(s) => {
                assert!(s.ends_with('~'), "{s:?}");
                assert_eq!(status, None);
                valid += 1;
            }
            Err(EngineError::InvalidTestCase) => {
                assert_eq!(status, Some(Status::Invalid));
                invalid += 1;
            }
            Err(e) => panic!("{e:?}"),
        }
    }
    assert!(valid > 0 && invalid > 0, "{valid} valid, {invalid} invalid");
}

#[test]
fn the_simplest_string_falls_back_on_a_seeded_draw_when_the_simplest_walk_cannot_lex() {
    let spec = grammar("start: a | b\na: NAME NAME\nb: \"xy\"\nNAME: /[a-z]+/\n");
    assert!(smallest_string(&mut SimplestDraws::default(), &spec).is_err());
    assert_eq!(codepoints_to_string(spec.ensure_simplest().unwrap()), "xy");
    assert_eq!(codepoints_to_string(&spec.kind.simplest().unwrap()), "xy");
}

#[test]
fn the_simplest_string_falls_back_on_a_full_draw_when_no_smallest_string_lexes() {
    let spec = grammar("start: a | b\na: NAME NAME\nb: NAME \"+\" NAME\nNAME: /[a-z]+/\n");
    assert!(smallest_string(&mut SimplestDraws::default(), &spec).is_err());
    assert_eq!(spec.seeded_string(true), None);
    let simplest = codepoints_to_string(spec.ensure_simplest().unwrap());
    assert!(simplest.contains('+'), "{simplest:?}");
    assert_eq!(spec.seeded_string(false), Some(cps(&simplest)));
}

#[test]
fn a_grammar_no_draw_can_generate_from_reports_so_at_the_first_draw() {
    let spec = grammar("start: NAME NAME\nNAME: /[a-z]+/\n");
    let mut ntc = NativeTestCase::new_random(EngineRng::seeded(0)).unwrap();
    let Err(EngineError::InvalidArgument(message)) = generate_grammar(&mut ntc, &spec) else {
        panic!("expected an invalid argument");
    };
    assert!(
        message.contains("no string of 'start' could be generated"),
        "{message}"
    );
    assert!(ntc.nodes.is_empty());
    assert_eq!(ntc.status(), None);
    assert!(spec.constraint().simplest().is_none());
}

fn cps(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}

/// A source that fails every draw, standing in for a test case that
/// has run out of data.
struct Failing;

impl Draws for Failing {
    fn draw_integer_uniform(&mut self, _: u64, _: u64) -> Result<u64, EngineError> {
        Err(EngineError::Overrun)
    }

    fn draw_integer_i64(&mut self, _: i64, _: i64) -> Result<i64, EngineError> {
        Err(EngineError::Overrun)
    }

    fn weighted(&mut self, _: f64, _: Option<bool>) -> Result<bool, EngineError> {
        Err(EngineError::Overrun)
    }

    fn weighted_against(&mut self, _: f64, _: u8, _: Option<bool>) -> Result<bool, EngineError> {
        Err(EngineError::Overrun)
    }

    fn weighted_precise(&mut self, _: f64, _: Option<bool>) -> Result<bool, EngineError> {
        Err(EngineError::Overrun)
    }

    fn draw_string(
        &mut self,
        _: Arc<IntervalSet>,
        _: usize,
        _: usize,
    ) -> Result<String, EngineError> {
        Err(EngineError::Overrun)
    }

    fn generate_string(&mut self, _: &StringSpec) -> Result<String, EngineError> {
        Err(EngineError::Overrun)
    }

    fn start_span(&mut self, _: u64) {}

    fn stop_span(&mut self, _: bool) {}

    fn conclude(&mut self, _: Status) {}
}

#[test]
fn an_error_from_the_source_ends_the_walk_with_that_error() {
    let spec = grammar("start: \"a\" | \"b\"\n");
    let result = smallest_string(&mut Failing, &spec);
    assert!(matches!(result, Err(EngineError::Overrun)), "{result:?}");
}
