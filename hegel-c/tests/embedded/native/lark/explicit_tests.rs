//! Tests for `src/native/lark/explicit.rs`: the ends an explicit
//! terminal's matcher finds for each kind of generator, and that every
//! string the internet generators draw is matched whole.

use super::*;
use crate::native::core::GenerationParameters;
use crate::native::draws::TextAlphabet;
use crate::native::draws::source::{Draws, RngDraws};
use crate::native::lark::builder::{NoImports, load_grammar};
use crate::native::lark::compile::compile;
use crate::native::rng::EngineRng;
use alloc::string::ToString;
use alloc::vec;

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn ends_of(matcher: &ExplicitMatcher, text: &str, pos: usize) -> Vec<usize> {
    let mut out = Vec::new();
    matcher.ends(&mut None, &chars(text), pos, &mut out);
    out
}

fn alphabet(chars: &str) -> TextAlphabet {
    TextAlphabet {
        categories: Some(vec![]),
        include_characters: Some(chars.to_string()),
        ..TextAlphabet::default()
    }
}

fn text_matcher(chars: &str, min_size: usize, max_size: usize) -> ExplicitMatcher {
    ExplicitMatcher::new(&StringSpec::text(&alphabet(chars), min_size, max_size).unwrap()).unwrap()
}

fn regex_matcher(pattern: &str, fullmatch: bool) -> ExplicitMatcher {
    ExplicitMatcher::new(&StringSpec::regex(pattern, fullmatch, None).unwrap()).unwrap()
}

#[test]
fn a_text_terminal_ends_anywhere_in_a_run_of_its_alphabet_within_its_lengths() {
    let digits = text_matcher("0123456789", 1, 3);
    assert_eq!(ends_of(&digits, "12345x", 0), vec![1, 2, 3]);
    assert_eq!(ends_of(&digits, "12x", 0), vec![1, 2]);
    assert_eq!(ends_of(&digits, "x12", 0), Vec::<usize>::new());
    assert_eq!(ends_of(&digits, "x12", 1), vec![2, 3]);
    assert_eq!(ends_of(&digits, "", 0), Vec::<usize>::new());
    let optional = text_matcher("ab", 0, 10);
    assert_eq!(ends_of(&optional, "x", 0), vec![0]);
    assert_eq!(ends_of(&optional, "abx", 0), vec![0, 1, 2]);
    assert_eq!(ends_of(&optional, "ab", 2), vec![2]);
}

#[test]
fn a_regex_terminal_ends_wherever_the_pattern_matches_a_whole_prefix() {
    let m = regex_matcher("a|ab", true);
    assert_eq!(ends_of(&m, "abc", 0), vec![1, 2]);
    assert_eq!(ends_of(&m, "abc", 1), Vec::<usize>::new());
    let m = regex_matcher("[a-z]{2,3}", true);
    assert_eq!(ends_of(&m, "abcd", 0), vec![2, 3]);
    assert_eq!(ends_of(&m, "abcd", 2), vec![4]);
    assert_eq!(ends_of(&m, "abcd", 3), Vec::<usize>::new());
    let m = regex_matcher("(?:ab)*", true);
    assert_eq!(ends_of(&m, "ababx", 0), vec![0, 2, 4]);
}

#[test]
fn a_regex_terminal_the_simulation_declines_is_matched_by_backtracking() {
    let m = regex_matcher("(a|b)\\1", true);
    let Kind::Pattern { program, .. } = &m.kind else {
        panic!("a pattern");
    };
    assert!(program.is_none());
    assert_eq!(ends_of(&m, "aab", 0), vec![2]);
    assert_eq!(ends_of(&m, "abb", 0), Vec::<usize>::new());
    assert_eq!(ends_of(&m, "abb", 1), vec![3]);
}

#[test]
fn a_searching_regex_terminal_ends_anywhere_from_the_first_match_on() {
    let m = regex_matcher("b+", false);
    assert_eq!(ends_of(&m, "aabba", 0), vec![3, 4, 5]);
    assert_eq!(ends_of(&m, "aabba", 3), vec![4, 5]);
    assert_eq!(ends_of(&m, "aabba", 4), Vec::<usize>::new());
    assert_eq!(ends_of(&m, "", 0), Vec::<usize>::new());
    let m = regex_matcher("aaaa|b", false);
    assert_eq!(ends_of(&m, "aaab", 0), vec![4]);
    let m = regex_matcher("x*", false);
    assert_eq!(ends_of(&m, "ab", 0), vec![0, 1, 2]);
}

#[test]
fn a_searching_regex_terminal_finds_its_earliest_ends_once_for_the_text() {
    let m = regex_matcher(".*x", false);
    let mut searched = None;
    let text = chars("aaxaaaax");
    let mut out = Vec::new();
    m.ends(&mut searched, &text, 0, &mut out);
    assert_eq!(out, vec![3, 4, 5, 6, 7, 8]);
    assert_eq!(searched, Some(vec![3, 3, 3, 8, 8, 8, 8, 8, 9]));
    out.clear();
    m.ends(&mut searched, &text, 4, &mut out);
    assert_eq!(out, vec![8]);
    out.clear();
    m.ends(&mut searched, &text, 8, &mut out);
    assert_eq!(out, Vec::<usize>::new());
    let long: Vec<char> = core::iter::repeat_n('a', 4000).collect();
    let started = std::time::Instant::now();
    let mut searched = None;
    for pos in 0..=long.len() {
        out.clear();
        m.ends(&mut searched, &long, pos, &mut out);
        assert!(out.is_empty());
    }
    assert!(started.elapsed() < core::time::Duration::from_secs(5));
    let searched = searched.unwrap();
    assert_eq!(searched.len(), long.len() + 1);
    assert!(searched.iter().all(|&end| end == long.len() + 1));
}

#[test]
fn every_internet_string_drawn_is_matched_whole() {
    let specs = [
        StringSpec::email(),
        StringSpec::url(),
        StringSpec::domain(255).unwrap(),
    ];
    for spec in &specs {
        let matcher = ExplicitMatcher::new(spec).unwrap();
        let mut whole = 0;
        for seed in 0..200 {
            let mut rng = EngineRng::seeded(seed);
            let params = GenerationParameters::draw(&mut rng).unwrap();
            let mut draws = RngDraws::new(&mut rng, params.integer);
            let Ok(s) = draws.generate_string(spec) else {
                continue;
            };
            let text = chars(&s);
            let mut ends = Vec::new();
            matcher.ends(&mut None, &text, 0, &mut ends);
            assert!(ends.contains(&text.len()), "{s:?} {ends:?}");
            whole += 1;
        }
        assert!(whole > 150);
    }
}

#[test]
fn the_internet_patterns_reject_what_their_generators_never_draw() {
    let email = ExplicitMatcher::new(&StringSpec::email()).unwrap();
    assert_eq!(ends_of(&email, "a@b.com", 0), vec![6, 7]);
    assert_eq!(ends_of(&email, "@b.com", 0), Vec::<usize>::new());
    assert_eq!(ends_of(&email, "a@com", 0), Vec::<usize>::new());
    let url = ExplicitMatcher::new(&StringSpec::url()).unwrap();
    assert_eq!(ends_of(&url, "ftp://a.com/", 0), Vec::<usize>::new());
    assert_eq!(
        ends_of(&url, "http://a.com:80/p%2Fq#f?", 0),
        vec![16, 17, 20, 21, 22, 23, 24]
    );
    let domain = ExplicitMatcher::new(&StringSpec::domain(255).unwrap()).unwrap();
    assert_eq!(ends_of(&domain, "1a.com", 0), Vec::<usize>::new());
    assert_eq!(ends_of(&domain, "a.b.com", 0), vec![6, 7]);
    assert_eq!(ends_of(&domain, "a.b", 0), Vec::<usize>::new());
}

#[test]
fn grammar_terminals_match_explicit_terminals_by_their_generators_and_the_rest_by_pattern() {
    let built = load_grammar(
        "start: WORD \"!\" NUM\n%declare WORD NUM\n",
        None,
        &NoImports,
    )
    .unwrap();
    let grammar = compile(&built, &["start"]).unwrap();
    let word = grammar.terminal_id("WORD").unwrap();
    let num = grammar.terminal_id("NUM").unwrap();
    let bang = grammar
        .terminals
        .iter()
        .position(|t| t.name != "WORD" && t.name != "NUM")
        .unwrap();
    let explicit = [(
        word,
        Arc::new(StringSpec::regex("[a-z]{3}", true, None).unwrap()),
    )];
    let terminals = GrammarTerminals::new(&grammar, &explicit).unwrap();
    let text = chars("abc!12");
    let mut memo = terminals.begin(&text);
    let mut out = Vec::new();
    terminals.ends(&mut memo, word, &text, 0, &mut out);
    assert_eq!(out, vec![3]);
    out.clear();
    terminals.ends(&mut memo, bang, &text, 3, &mut out);
    assert_eq!(out, vec![4]);
    out.clear();
    terminals.ends(&mut memo, num, &text, 4, &mut out);
    assert_eq!(out, Vec::<usize>::new());
    let explicit = [(
        num,
        Arc::new(StringSpec::text(&alphabet("0123456789"), 1, 8).unwrap()),
    )];
    let terminals = GrammarTerminals::new(&grammar, &explicit).unwrap();
    let mut memo = terminals.begin(&text);
    terminals.ends(&mut memo, num, &text, 4, &mut out);
    assert_eq!(out, vec![5, 6]);
    assert!(matches!(
        terminals.lexer().match_terminal(bang, &text, 3),
        crate::native::lark::lexsim::Lexed::Token(id, 4) if id == bang
    ));
}
