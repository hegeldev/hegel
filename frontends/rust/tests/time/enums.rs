use crate::common::utils::{assert_all_examples, check_can_generate_examples, find_any, minimal};
use hegel::extras::time as time_gs;
use hegel::generators as gs;
use time::{Month, Weekday};

#[test]
fn test_weekdays_default() {
    check_can_generate_examples(time_gs::weekdays());
    for n in 0..7 {
        let weekday = Weekday::Monday.nth_next(n);
        find_any(time_gs::weekdays(), move |w| *w == weekday);
    }
    assert_eq!(minimal(time_gs::weekdays(), |_| true), Weekday::Monday);
}

#[test]
fn test_weekdays_in_vec() {
    assert_all_examples(gs::vecs(time_gs::weekdays()).max_size(7), |v| {
        v.iter().all(|w| w.number_days_from_monday() < 7)
    });
}

#[test]
fn test_weekday_default_generator() {
    check_can_generate_examples(gs::default::<Weekday>());
}

#[test]
fn test_months_default() {
    check_can_generate_examples(time_gs::months());
    for n in 1..=12 {
        let month = Month::try_from(n).unwrap();
        find_any(time_gs::months(), move |m| *m == month);
    }
    assert_eq!(minimal(time_gs::months(), |_| true), Month::January);
}

#[test]
fn test_months_in_vec() {
    assert_all_examples(gs::vecs(time_gs::months()).max_size(7), |v| {
        v.iter().all(|m| (1..=12).contains(&(*m as u8)))
    });
}

#[test]
fn test_month_default_generator() {
    check_can_generate_examples(gs::default::<Month>());
}
