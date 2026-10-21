use crate::common::utils::printed_draw_lines;
use hegel::extras::jiff as jiff_gs;
use hegel::generators as gs;
use jiff::tz::{AmbiguousOffset, AmbiguousTimestamp, AmbiguousZoned, TimeZone};

#[test]
fn every_jiff_generator_prints_its_drawn_value() {
    printed_draw_lines(jiff_gs::dates());
    printed_draw_lines(jiff_gs::times());
    printed_draw_lines(jiff_gs::datetimes());
    printed_draw_lines(jiff_gs::timestamps());
    printed_draw_lines(jiff_gs::spans());
    printed_draw_lines(jiff_gs::signed_durations());
    printed_draw_lines(jiff_gs::offsets());
    printed_draw_lines(jiff_gs::timezones());
    printed_draw_lines(jiff_gs::zoneds());
    printed_draw_lines(jiff_gs::iso_week_dates());
    printed_draw_lines(jiff_gs::weekdays());
    printed_draw_lines(jiff_gs::eras());
}

#[test]
fn jiff_values_print_in_their_display_form() {
    let lines = printed_draw_lines(
        jiff_gs::zoneds().timezones(jiff_gs::timezones().names(["Europe/London"])),
    );
    assert!(
        lines.iter().any(|line| line.contains("[Europe/London]")),
        "{lines:?}"
    );
    let lines = printed_draw_lines(jiff_gs::timezones().names(["Asia/Calcutta"]));
    assert!(
        lines.iter().any(|line| line.contains("Asia/Calcutta")),
        "{lines:?}"
    );
}

#[test]
fn boxed_jiff_default_generators_are_drawable() {
    printed_draw_lines(gs::default::<TimeZone>());
    printed_draw_lines(gs::default::<AmbiguousOffset>());
    printed_draw_lines(gs::default::<AmbiguousZoned>());
    printed_draw_lines(gs::default::<AmbiguousTimestamp>());
}
