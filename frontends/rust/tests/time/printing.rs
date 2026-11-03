use crate::common::utils::printed_draw_lines;
use hegel::extras::time as time_gs;
use hegel::generators as gs;

#[test]
fn every_time_generator_prints_its_drawn_value() {
    printed_draw_lines(time_gs::dates());
    printed_draw_lines(time_gs::times());
    printed_draw_lines(time_gs::primitive_datetimes());
    printed_draw_lines(time_gs::utc_offsets());
    printed_draw_lines(time_gs::offset_datetimes());
    printed_draw_lines(time_gs::durations());
    printed_draw_lines(time_gs::weekdays());
    printed_draw_lines(time_gs::months());
}

#[test]
fn every_time_default_generator_is_drawable() {
    printed_draw_lines(gs::default::<time::UtcDateTime>());
}
