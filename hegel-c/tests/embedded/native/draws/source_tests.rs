//! Tests for `src/native/draws/source.rs`: that a test case answers the
//! `Draws` operations as its own methods do.

use super::*;
use crate::native::draws::{TextAlphabet, many_more};
use crate::native::rng::EngineRng;

#[test]
fn a_test_case_draws_through_the_trait_as_through_its_own_methods() {
    let mut a = NativeTestCase::new_random(EngineRng::seeded(7)).unwrap();
    let mut b = NativeTestCase::new_random(EngineRng::seeded(7)).unwrap();
    let mut state_a = ManyState::new(1, Some(5));
    let mut state_b = ManyState::new(1, Some(5));
    for _ in 0..50 {
        assert_eq!(
            Draws::draw_integer_uniform(&mut a, 3, 1000).unwrap(),
            b.draw_integer_uniform(3, 1000).unwrap()
        );
        assert_eq!(
            Draws::draw_integer_i64(&mut a, -20, 20).unwrap(),
            b.draw_integer_i64(-20, 20).unwrap()
        );
        assert_eq!(
            Draws::weighted(&mut a, 0.3, None).unwrap(),
            b.weighted(0.3, None).unwrap()
        );
        assert_eq!(
            Draws::weighted_against(&mut a, 0.3, 100, None).unwrap(),
            b.weighted_against(0.3, 100, None).unwrap()
        );
        assert_eq!(
            Draws::weighted_precise(&mut a, 0.3, None).unwrap(),
            b.weighted_precise(0.3, None).unwrap()
        );
        assert_eq!(
            a.many_more(&mut state_a).unwrap(),
            many_more(&mut b, &mut state_b).unwrap()
        );
    }
    assert_eq!(state_a.count, state_b.count);
    let spec = StringSpec::text(&TextAlphabet::default(), 1, 4).unwrap();
    assert_eq!(
        Draws::generate_string(&mut a, &spec).unwrap(),
        generate_string(&mut b, &spec).unwrap()
    );
    let depth = a.span_depth();
    Draws::start_span(&mut a, 11);
    assert_eq!(a.span_depth(), depth + 1);
    Draws::stop_span(&mut a, true);
    assert_eq!(a.span_depth(), depth);
    assert!(a.has_discards);
    Draws::conclude(&mut a, Status::Invalid);
    assert_eq!(a.status(), Some(Status::Invalid));
    Draws::conclude(&mut a, Status::Valid);
    assert_eq!(a.status(), Some(Status::Invalid));
}

#[test]
fn rejecting_below_the_minimum_concludes_the_test_case_invalid() {
    let mut ntc = NativeTestCase::new_random(EngineRng::seeded(0)).unwrap();
    let mut state = ManyState::new(2, Some(5));
    for _ in 0..3 {
        assert!(ntc.many_more(&mut state).unwrap());
        ntc.many_reject(&mut state).unwrap();
    }
    assert!(ntc.many_more(&mut state).unwrap());
    assert!(matches!(
        Draws::many_reject(&mut ntc, &mut state).unwrap_err(),
        EngineError::InvalidTestCase
    ));
    assert_eq!(ntc.status(), Some(Status::Invalid));
}

#[test]
fn rejecting_past_the_minimum_stops_the_collection() {
    let mut ntc = NativeTestCase::new_random(EngineRng::seeded(0)).unwrap();
    let mut state = ManyState::new(0, Some(50));
    let mut drawn = 0;
    while ntc.many_more(&mut state).unwrap() {
        drawn += 1;
        ntc.many_reject(&mut state).unwrap();
        assert!(drawn < 100);
    }
    assert!(state.force_stop);
    assert_eq!(state.count, 0);
}
