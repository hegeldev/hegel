//! Tests for `src/native/lark/lexsim.rs`: the simulated basic lexer's
//! terminal order and keyword re-typing, the dynamic lexer's per-terminal
//! matching, and the first-character analysis that speeds both up.

use super::*;
use crate::native::lark::builder::{NoImports, load_grammar};
use crate::native::lark::compile::{Grammar, compile};
use crate::native::re::parser::parse_pattern;
use alloc::vec;

fn compiled(text: &str) -> Grammar {
    let built = load_grammar(text, None, &NoImports).unwrap();
    compile(&built, &["start"]).unwrap()
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn lex<'g>(grammar: &'g Grammar, text: &str) -> Vec<(&'g str, usize)> {
    let lexer = Lexer::new(&grammar.terminals);
    let text = chars(text);
    let mut pos = 0;
    let mut out = vec![];
    while pos < text.len() {
        let Lexed::Token(id, end) = lexer.next_token(&text, pos) else {
            panic!("no token at {pos}");
        };
        if !grammar.ignore.contains(&id) {
            out.push((grammar.terminals[id].name.as_str(), end - pos));
        }
        pos = end;
    }
    out
}

fn firsts(pattern: &str) -> Option<FirstChars> {
    first_chars(&parse_pattern(pattern, 0).unwrap())
}

fn ranges(pattern: &str) -> Vec<(u32, u32)> {
    firsts(pattern).unwrap().0
}

#[test]
fn keyword_matched_by_a_regex_is_retyped() {
    let g = compiled("start: NAME \"if\" \"while\"\nNAME: /[a-z]+/\n%ignore \" \"\n");
    assert_eq!(
        lex(&g, "if while iffy"),
        vec![("IF", 2), ("WHILE", 5), ("NAME", 4)]
    );
}

#[test]
fn keyword_at_a_different_priority_is_not_retyped() {
    let g = compiled("start: NAME KW\nNAME: /[a-z]+/\nKW.2: \"if\"\n%ignore \" \"\n");
    assert_eq!(lex(&g, "iffy if"), vec![("KW", 2), ("NAME", 2), ("KW", 2)]);
}

#[test]
fn keyword_with_flags_the_regex_lacks_stays_in_the_order() {
    let g = compiled("start: NAME KW\nNAME: /[a-z]+/\nKW: \"if\"i\n%ignore \" \"\n");
    assert_eq!(lex(&g, "IF if"), vec![("KW", 2), ("KW", 2)]);
}

#[test]
fn longer_possible_match_comes_first() {
    let g = compiled("start: A B\nA: /x/\nB: /x[0-9]?/\n");
    assert_eq!(lex(&g, "xx1"), vec![("B", 1), ("B", 2)]);
}

#[test]
fn longer_pattern_source_breaks_width_ties_then_name() {
    let g = compiled("start: A B C\nA: /a|b/\nB: /[ab]/\nC: /b/\n");
    assert_eq!(lex(&g, "ab"), vec![("B", 1), ("B", 1)]);
    let g = compiled("start: B A\nA: /[ab]/\nB: /[ba]/\n");
    assert_eq!(lex(&g, "a"), vec![("A", 1)]);
}

#[test]
fn priority_beats_width() {
    let g = compiled("start: A B\nA.1: /a/\nB: /a+/\n");
    assert_eq!(lex(&g, "aa"), vec![("A", 1), ("A", 1)]);
}

#[test]
fn unlexable_text_is_no_match() {
    let g = compiled("start: A\nA: /a/\n");
    let lexer = Lexer::new(&g.terminals);
    assert_eq!(lexer.next_token(&chars("b"), 0), Lexed::NoMatch);
    assert_eq!(lexer.next_token(&chars("a"), 1), Lexed::NoMatch);
}

#[test]
fn declared_terminals_take_no_part_in_the_order() {
    let g = compiled("start: A B\nA: /a/\n%declare B\n");
    let lexer = Lexer::new(&g.terminals);
    let b = g.terminal_id("B").unwrap();
    assert_eq!(lexer.match_terminal(b, &chars("a"), 0), Lexed::Unknown);
    assert_eq!(lex(&g, "a"), vec![("A", 1)]);
}

#[test]
fn match_terminal_reports_its_own_pattern_only() {
    let g = compiled("start: A B\nA: /a+/\nB: /ab/\n");
    let lexer = Lexer::new(&g.terminals);
    let a = g.terminal_id("A").unwrap();
    let b = g.terminal_id("B").unwrap();
    assert_eq!(
        lexer.match_terminal(a, &chars("aab"), 0),
        Lexed::Token(a, 2)
    );
    assert_eq!(
        lexer.match_terminal(b, &chars("aab"), 1),
        Lexed::Token(b, 3)
    );
    assert_eq!(lexer.match_terminal(b, &chars("aab"), 0), Lexed::NoMatch);
}

#[test]
fn matcher_running_out_of_budget_is_unknown() {
    let g = compiled("start: A\nA: /(a*)*b/\n");
    let lexer = Lexer::new(&g.terminals);
    let text = chars(&"a".repeat(40));
    assert_eq!(lexer.match_terminal(0, &text, 0), Lexed::Unknown);
    assert_eq!(lexer.next_token(&text, 0), Lexed::Unknown);
}

#[test]
fn first_chars_of_literals_sets_and_branches() {
    assert_eq!(ranges("abc"), vec![(97, 97)]);
    assert_eq!(ranges("[a-cx]|b|z"), vec![(97, 99), (120, 120), (122, 122)]);
    assert_eq!(ranges("(?:q|r)s"), vec![(113, 114)]);
    assert_eq!(ranges("(?>t)u"), vec![(116, 116)]);
    assert_eq!(ranges("(?!x)y"), vec![(121, 121)]);
    assert_eq!(ranges("^\\Ab"), vec![(98, 98)]);
}

#[test]
fn first_chars_skip_nullable_prefixes() {
    assert_eq!(ranges("a*b"), vec![(97, 98)]);
    assert_eq!(ranges("a?b"), vec![(97, 98)]);
    assert_eq!(ranges("(a|)b"), vec![(97, 98)]);
    assert_eq!(ranges("a{2,}"), vec![(97, 97)]);
    assert_eq!(ranges("a*?b"), vec![(97, 98)]);
    assert_eq!(ranges("a*+b"), vec![(97, 98)]);
}

#[test]
fn first_chars_give_up_on_unknowable_starts() {
    assert_eq!(firsts("[^a]b"), None);
    assert_eq!(firsts("[^ab]c"), None);
    assert_eq!(firsts(".b"), None);
    assert_eq!(firsts("\\D"), None);
    assert_eq!(firsts("\\S"), None);
    assert_eq!(firsts("\\W"), None);
    assert_eq!(firsts("(a)\\1"), Some(FirstChars(vec![(97, 97)])));
    assert_eq!(firsts("(a)?\\1b"), None);
    assert_eq!(firsts("(?i:a)b"), None);
    assert_eq!(firsts("(?i)ab"), None);
    assert_eq!(firsts("a*"), None);
    assert_eq!(firsts("(?!)"), Some(FirstChars(vec![])));
}

#[test]
fn first_chars_of_category_classes_cover_the_class() {
    let digits = firsts("\\d").unwrap();
    assert!(digits.contains('7') && digits.contains('\u{663}') && !digits.contains('a'));
    let spaces = firsts("\\s*//").unwrap();
    assert!(spaces.contains(' ') && spaces.contains('\u{3000}') && spaces.contains('/'));
    assert!(spaces.contains('\u{1c}') && !spaces.contains('x'));
    let words = firsts("\\w+").unwrap();
    assert!(words.contains('_') && words.contains('\u{e9}') && words.contains('\u{663}'));
    assert!(!words.contains('-') && !words.contains(' '));
}

#[test]
fn first_chars_of_conditional_groups() {
    assert_eq!(ranges("(a)?(?(1)b|c)"), vec![(97, 99)]);
    assert_eq!(firsts("(a)?(?(1)b)"), None);
    assert_eq!(ranges("(a)(?(1)b|c)"), vec![(97, 97)]);
}

#[test]
fn first_chars_merge_adjacent_and_overlapping_ranges() {
    assert_eq!(ranges("[a-c]|[d-f]|[e-h]|z"), vec![(97, 104), (122, 122)]);
    let first = firsts("[a-c]|z").unwrap();
    assert!(first.contains('b'));
    assert!(first.contains('z'));
    assert!(!first.contains('d'));
    assert!(!first.contains('{'));
}

#[test]
fn first_chars_prune_terminals_in_the_order() {
    let g = compiled("start: A B\nA: /a+/\nB: /[^a]+/\n");
    assert_eq!(lex(&g, "aab"), vec![("A", 2), ("B", 1)]);
}

#[test]
fn extend_carries_what_the_end_of_the_text_left_open() {
    let grammar = compiled(
        "start: NAME KW C D E F\nNAME: /[a-z]+/\nKW: \"if\"\nC: \"ab\"\nD: /(?!x)[0-9]+/\nE: /a\\b/\nF: /(?<=x)(a?b)+/\n",
    );
    let lexer = Lexer::new(&grammar.terminals);
    let id = |name: &str| {
        grammar
            .terminals
            .iter()
            .position(|t| t.name == name)
            .unwrap()
    };
    let scan = lexer.next_token_scan(&chars("i"), 0);
    assert_eq!(scan.lexed, Lexed::Token(id("NAME"), 1));
    assert_eq!(
        lexer.extend(&scan, &chars("if "), 0, 1, true),
        Some(Lexed::Token(id("KW"), 2))
    );
    assert_eq!(
        lexer.extend(&scan, &chars("ifx"), 0, 1, true),
        Some(Lexed::Token(id("NAME"), 3))
    );
    assert_eq!(
        lexer.extend(&scan, &chars("xif"), 1, 1, false),
        Some(Lexed::Token(id("NAME"), 3))
    );
    let scan = lexer.match_terminal_scan(id("C"), &chars("a"), 0);
    assert_eq!(scan.lexed, Lexed::NoMatch);
    assert_eq!(
        lexer.extend(&scan, &chars("ab"), 0, 1, false),
        Some(Lexed::Token(id("C"), 2))
    );
    assert_eq!(
        lexer.extend(&scan, &chars("ax"), 0, 1, false),
        Some(Lexed::NoMatch)
    );
    let scan = lexer.match_terminal_scan(id("C"), &chars("x"), 0);
    assert_eq!(
        lexer.extend(&scan, &chars("xab"), 0, 1, false),
        Some(Lexed::NoMatch)
    );
    let scan = lexer.match_terminal_scan(id("D"), &chars("12"), 0);
    assert_eq!(scan.lexed, Lexed::Token(id("D"), 2));
    assert_eq!(
        lexer.extend(&scan, &chars("123-"), 0, 2, false),
        Some(Lexed::Token(id("D"), 3))
    );
    let scan = lexer.match_terminal_scan(id("E"), &chars("a"), 0);
    assert_eq!(scan.lexed, Lexed::Token(id("E"), 1));
    assert_eq!(lexer.extend(&scan, &chars("ab"), 0, 1, false), None);
    let scan = lexer.match_terminal_scan(id("F"), &chars("b"), 0);
    assert_eq!(scan.lexed, Lexed::NoMatch);
    assert_eq!(lexer.extend(&scan, &chars("bb"), 0, 1, false), None);
    assert!(
        lexer
            .extend(
                &lexer.match_terminal_scan(id("F"), &chars("xb"), 1),
                &chars("xbb"),
                1,
                1,
                false
            )
            .is_some()
    );
    let scan = lexer.match_terminal_scan(id("C"), &chars("ab"), 2);
    assert_eq!(scan.lexed, Lexed::NoMatch);
    assert_eq!(lexer.extend(&scan, &chars("abab"), 2, 0, false), None);
    assert_eq!(
        lexer.extend(
            &lexer.next_token_scan(&chars("a"), 1),
            &chars("ab"),
            1,
            0,
            false
        ),
        None
    );
}
