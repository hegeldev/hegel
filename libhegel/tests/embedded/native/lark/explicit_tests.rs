use super::*;
use crate::native::lark::builder::NoImports;
use crate::native::lark::generate::{GrammarOptions, from_lark};
use crate::native::rng::EngineRng;
use alloc::string::ToString;
use alloc::vec;

fn spec(text: &str, explicit: &[&str]) -> Arc<GrammarSpec> {
    capped_spec(text, explicit, GrammarOptions::default().max_size)
}

fn capped_spec(text: &str, explicit: &[&str], max_size: u64) -> Arc<GrammarSpec> {
    Arc::new(
        from_lark(
            text,
            &NoImports,
            GrammarOptions {
                explicit: explicit.iter().map(|n| n.to_string()).collect(),
                max_size,
                ..GrammarOptions::default()
            },
        )
        .unwrap(),
    )
}

fn complete(draw: &mut ExplicitDraw, ntc: &mut NativeTestCase, value: &str) -> Option<String> {
    loop {
        match draw.next(ntc) {
            Ok(Some(_)) => draw.provide(value).unwrap(),
            Ok(None) => return Some(draw.result().unwrap().to_string()),
            Err(EngineError::InvalidTestCase) => return None,
            Err(e) => panic!("unexpected error {e:?}"),
        }
    }
}

fn ntc(seed: u64) -> NativeTestCase {
    NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap()
}

#[test]
fn a_grammar_without_explicit_terminals_completes_at_once() {
    let mut draw = ExplicitDraw::new(spec(r#"start: "ab""#, &[]), 0);
    let mut ntc = ntc(1);
    assert_eq!(draw.result(), None);
    assert_eq!(draw.next(&mut ntc).unwrap(), None);
    assert_eq!(draw.result(), Some("ab"));
    assert_eq!(draw.next(&mut ntc).unwrap(), None);
    assert_eq!(draw.result(), Some("ab"));
}

#[test]
fn values_are_spliced_in_for_each_marker_in_order() {
    let mut draw = ExplicitDraw::new(
        spec(
            "start: \"(\" A \",\" B \",\" A \")\"\n%declare A B",
            &["A", "B"],
        ),
        0,
    );
    let mut ntc = ntc(2);
    let mut asked = vec![];
    while let Some(i) = draw.next(&mut ntc).unwrap() {
        asked.push(i);
        draw.provide(["x", "yy"][i]).unwrap();
    }
    assert_eq!(asked, vec![0, 1, 0]);
    assert_eq!(draw.result(), Some("(x,yy,x)"));
}

#[test]
fn a_value_must_be_provided_before_the_next_terminal() {
    let mut draw = ExplicitDraw::new(spec("start: A\n%declare A", &["A"]), 0);
    let mut ntc = ntc(3);
    assert!(draw.provide("early").is_err());
    assert_eq!(draw.next(&mut ntc).unwrap(), Some(0));
    let EngineError::InvalidArgument(message) = draw.next(&mut ntc).unwrap_err() else {
        panic!("expected an invalid argument");
    };
    assert!(message.contains("'A' is due"));
    assert_eq!(draw.result(), None);
    draw.provide("v").unwrap();
    assert_eq!(draw.next(&mut ntc).unwrap(), None);
    assert_eq!(draw.result(), Some("v"));
    assert!(draw.provide("late").is_err());
}

#[test]
fn values_too_large_for_the_cap_are_redrawn_then_rejected() {
    let mut draw = ExplicitDraw::new(spec("start: A+\n%declare A", &["A"]), 10);
    let mut ntc = ntc(4);
    let mut attempts = 0;
    let error = loop {
        match draw.next(&mut ntc) {
            Ok(Some(_)) => draw.provide("0123456789a").unwrap(),
            Ok(None) => panic!("an oversized value cannot fit"),
            Err(e) => break e,
        }
        attempts += 1;
    };
    assert!(matches!(error, EngineError::InvalidTestCase));
    assert!(attempts >= crate::HEGEL_FILTER_ATTEMPTS as usize);
    assert_eq!(ntc.status(), Some(Status::Invalid));
}

#[test]
fn a_draw_that_overflows_is_drawn_again_with_a_smaller_budget() {
    let grammar = spec("start: (\"a\" A)+\n%declare A", &["A"]);
    assert_eq!(ExplicitDraw::new(Arc::clone(&grammar), 0).max_size, 8192);
    let mut completed = 0;
    let mut redrawn = 0;
    for seed in 0..100 {
        let mut draw = ExplicitDraw::new(Arc::clone(&grammar), 6);
        let mut ntc = ntc(seed);
        loop {
            match draw.next(&mut ntc) {
                Ok(Some(_)) => draw.provide("bb").unwrap(),
                Ok(None) => {
                    assert!(draw.result().unwrap().chars().count() <= 6);
                    completed += 1;
                    break;
                }
                Err(EngineError::InvalidTestCase) => break,
                Err(e) => panic!("unexpected error {e:?}"),
            }
        }
        if draw.attempts > 0 {
            redrawn += 1;
        }
    }
    assert!(completed > 0);
    assert!(redrawn > 0);
}

#[test]
fn a_cap_below_the_smallest_string_is_read_as_it_without_explicit_terminals() {
    let mut draw = ExplicitDraw::new(spec(r#"start: "abc""#, &[]), 1);
    assert_eq!(draw.max_size, 3);
    assert_eq!(complete(&mut draw, &mut ntc(5), "").unwrap(), "abc");
}

#[test]
fn a_cap_below_the_smallest_string_is_read_as_it_with_explicit_terminals() {
    let mut draw = ExplicitDraw::new(spec("start: \"ab\" A\n%declare A", &["A"]), 1);
    assert_eq!(draw.max_size, 2);
    assert_eq!(complete(&mut draw, &mut ntc(6), "").unwrap(), "ab");
}

#[test]
fn a_cap_above_the_grammars_is_read_as_it_without_explicit_terminals() {
    let grammar = capped_spec(r#"start: "a"+"#, &[], 5);
    for seed in 0..50 {
        let mut draw = ExplicitDraw::new(Arc::clone(&grammar), 100);
        assert_eq!(draw.max_size, 5);
        if let Some(text) = complete(&mut draw, &mut ntc(seed), "") {
            assert!(text.chars().count() <= 5);
        }
    }
}

#[test]
fn a_cap_above_the_grammars_is_read_as_it_with_explicit_terminals() {
    let grammar = capped_spec("start: (\"a\" A)+\n%declare A", &["A"], 5);
    let mut completed = 0;
    for seed in 0..50 {
        let mut draw = ExplicitDraw::new(Arc::clone(&grammar), 100);
        assert_eq!(draw.max_size, 5);
        if let Some(text) = complete(&mut draw, &mut ntc(seed), "bb") {
            assert!(text.chars().count() <= 5);
            completed += 1;
        }
    }
    assert!(completed > 0);
}
