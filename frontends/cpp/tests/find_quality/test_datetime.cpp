#include <gtest/gtest.h>

#include "common/utils.h"

#include <algorithm>
#include <cstdint>
#include <string>
#include <vector>

using hegel::tests::common::assert_all_examples;
using hegel::tests::common::assert_no_examples;
using hegel::tests::common::find_any;
namespace gs = hegel::generators;
namespace dt = hegel::datetime;

namespace {

    dt::DateTime datetime(int32_t year, uint8_t month, uint8_t day,
                          uint8_t hour, uint8_t minute, uint8_t second) {
        return dt::DateTime{{year, month, day}, {hour, minute, second, 0}};
    }

    // Tomohiko Sakamoto's algorithm: 0 is Sunday.
    int weekday_of(const dt::Date& d) {
        int64_t y = d.year - (d.month <= 2 ? 1 : 0);
        int64_t era = (y >= 0 ? y : y - 399) / 400;
        int64_t yoe = y - era * 400;
        int64_t doy =
            (153 * (d.month + (d.month > 2 ? -3 : 9)) + 2) / 5 + d.day - 1;
        int64_t doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        int64_t days = era * 146'097 + doe - 719'468;
        int64_t w = (days + 4) % 7;
        return static_cast<int>(w < 0 ? w + 7 : w);
    }

    bool has_tag(const std::string& name, dt::Tag tag) {
        std::optional<dt::ZoneInfo> info = dt::info(name);
        return info.has_value() &&
               std::find(info->tags.begin(), info->tags.end(), tag) !=
                   info->tags.end();
    }

    // Whether the engine's resolver agrees with a zoned draw.
    bool agrees_with_resolve(const dt::ZonedDateTime& z) {
        dt::ZonedDateTime r = dt::resolve(z.zone.name, z.civil);
        dt::Timestamp expected = r.timestamp;
        dt::UtcOffset expected_offset = r.offset;
        if (z.kind == dt::Kind::Fold && z.fold) {
            expected.second += r.offset_before - r.offset_after;
            expected_offset = dt::UtcOffset{r.offset_after};
        }
        return r.zone == z.zone && r.kind == z.kind &&
               r.offset_before == z.offset_before &&
               r.offset_after == z.offset_after && expected == z.timestamp &&
               expected_offset == z.offset &&
               (z.kind == dt::Kind::Fold || !z.fold);
    }

    // Wall-clock bounds a few years inside the civil range, so that every
    // drawn wall time resolves to an instant in range.
    const dt::DateTime resolvable_min = datetime(-999'990, 1, 1, 0, 0, 0);
    const dt::DateTime resolvable_max = datetime(999'990, 12, 31, 23, 59, 59);

    gs::ZonedDatetimesParams new_york_2024() {
        return gs::ZonedDatetimesParams{
            .timezones = {.names =
                              std::vector<std::string>{"America/New_York"}},
            .min_value = datetime(2024, 1, 1, 0, 0, 0),
            .max_value = datetime(2024, 12, 31, 23, 59, 59)};
    }

} // namespace

TEST(FindDatetime, DatesRespectBounds) {
    dt::Date min{2024, 1, 1};
    dt::Date max{2024, 12, 31};
    assert_all_examples<dt::Date>(
        gs::dates({.min_value = min, .max_value = max}),
        [=](const dt::Date& d) { return d >= min && d <= max; });
    find_any<dt::Date>(gs::dates(), [](const dt::Date& d) {
        return d.year > 9999 || d.year < 0;
    });
    find_any<dt::Date>(
        gs::dates({.min_value = min, .max_value = max}),
        [](const dt::Date& d) { return d.month == 2 && d.day == 29; });
}

TEST(FindDatetime, DatesRespectWeekdayAndMonthMasks) {
    assert_all_examples<dt::Date>(
        gs::dates({.min_value = dt::Date{-9999, 1, 1},
                   .max_value = dt::Date{9999, 12, 31},
                   .weekdays = {{dt::Weekday::Saturday, dt::Weekday::Sunday}}}),
        [](const dt::Date& d) {
            int w = weekday_of(d);
            return w == 6 || w == 0;
        });
    assert_all_examples<dt::Date>(
        gs::dates({.months = {{dt::Month::February, dt::Month::December}}}),
        [](const dt::Date& d) { return d.month == 2 || d.month == 12; });
    assert_all_examples<std::vector<dt::Date>>(
        gs::vectors(gs::dates({.max_value = dt::Date{1999, 12, 31}}),
                    {.max_size = 5}),
        [](const std::vector<dt::Date>& v) {
            return std::all_of(v.begin(), v.end(), [](const dt::Date& d) {
                return d <= dt::Date{1999, 12, 31};
            });
        });
}

TEST(FindDatetime, TimesRespectBoundsAndPrecision) {
    dt::Time min{6, 0, 0, 0};
    dt::Time max{18, 0, 0, 0};
    assert_all_examples<dt::Time>(
        gs::times({.min_value = min, .max_value = max}),
        [=](const dt::Time& t) { return t >= min && t <= max; });
    assert_all_examples<dt::Time>(
        gs::times({.precision = dt::Precision::Seconds}),
        [](const dt::Time& t) { return t.nanosecond == 0; });
    assert_all_examples<dt::Time>(
        gs::times({.precision = dt::Precision::Milliseconds}),
        [](const dt::Time& t) { return t.nanosecond % 1'000'000 == 0; });
    assert_all_examples<dt::Time>(
        gs::times({.precision = dt::Precision::Hours}), [](const dt::Time& t) {
            return t.minute == 0 && t.second == 0 && t.nanosecond == 0;
        });
    find_any<dt::Time>(gs::times(), [](const dt::Time& t) {
        return t.nanosecond % 1000 != 0;
    });
}

TEST(FindDatetime, DatetimesRespectBoundsAndPrecision) {
    dt::DateTime min = datetime(2024, 1, 1, 0, 0, 0);
    dt::DateTime max = datetime(2024, 12, 31, 23, 59, 59);
    assert_all_examples<dt::DateTime>(
        gs::datetimes({.min_value = min, .max_value = max}),
        [=](const dt::DateTime& d) { return d >= min && d <= max; });
    assert_all_examples<dt::DateTime>(
        gs::datetimes({.precision = dt::Precision::Minutes}),
        [](const dt::DateTime& d) {
            return d.time.second == 0 && d.time.nanosecond == 0;
        });
}

TEST(FindDatetime, TimestampsRespectBoundsAndPrecision) {
    dt::Timestamp min{0, 0};
    dt::Timestamp max{86'400, 0};
    assert_all_examples<dt::Timestamp>(
        gs::timestamps({.min_value = min, .max_value = max}),
        [=](const dt::Timestamp& t) { return t >= min && t <= max; });
    assert_all_examples<dt::Timestamp>(
        gs::timestamps({.precision = dt::Precision::Seconds}),
        [](const dt::Timestamp& t) { return t.nanosecond == 0; });
    assert_all_examples<dt::Timestamp>(
        gs::timestamps({.precision = dt::Precision::Microseconds}),
        [](const dt::Timestamp& t) { return t.nanosecond % 1000 == 0; });
    find_any<dt::Timestamp>(gs::timestamps(), [](const dt::Timestamp& t) {
        return t.second == 0 && t.nanosecond == 0;
    });
}

TEST(FindDatetime, UtcOffsetsRespectGranularityAndReachTheUnknown) {
    assert_all_examples<dt::UtcOffset>(
        gs::utc_offsets(), [](const dt::UtcOffset& o) {
            return o.known && o.seconds >= -dt::MAX_OFFSET &&
                   o.seconds <= dt::MAX_OFFSET;
        });
    assert_all_examples<dt::UtcOffset>(
        gs::utc_offsets({.granularity = dt::Granularity::Hours}),
        [](const dt::UtcOffset& o) { return o.seconds % 3600 == 0; });
    assert_all_examples<dt::UtcOffset>(
        gs::utc_offsets({.granularity = dt::Granularity::QuarterHours}),
        [](const dt::UtcOffset& o) { return o.seconds % 900 == 0; });
    assert_all_examples<dt::UtcOffset>(
        gs::utc_offsets({.granularity = dt::Granularity::Minutes}),
        [](const dt::UtcOffset& o) { return o.seconds % 60 == 0; });
    assert_all_examples<dt::UtcOffset>(
        gs::utc_offsets(
            {.min_value = dt::UtcOffset{0}, .max_value = dt::UtcOffset{3600}}),
        [](const dt::UtcOffset& o) {
            return o.seconds >= 0 && o.seconds <= 3600;
        });
    find_any<dt::UtcOffset>(gs::utc_offsets({.unknown = true}),
                            [](const dt::UtcOffset& o) { return !o.known; });
    find_any<dt::UtcOffset>(gs::utc_offsets(), [](const dt::UtcOffset& o) {
        return o.seconds % 3600 != 0;
    });
}

TEST(FindDatetime, TimezonesRespectTheSpec) {
    assert_all_examples<dt::TimeZone>(
        gs::timezones(), [](const dt::TimeZone& zone) {
            std::optional<dt::ZoneInfo> info = dt::info(zone.name);
            return info.has_value() &&
                   info->primary_name == zone.primary_name &&
                   !has_tag(zone.name, dt::Tag::Legacy) &&
                   zone.name != dt::UNKNOWN_ZONE;
        });
    std::vector<std::string> names = {"Europe/London", "Asia/Calcutta"};
    assert_all_examples<dt::TimeZone>(
        gs::timezones({.names = names}), [names](const dt::TimeZone& zone) {
            return std::find(names.begin(), names.end(), zone.name) !=
                   names.end();
        });
    find_any<dt::TimeZone>(gs::timezones({.names = names}),
                           [](const dt::TimeZone& zone) {
                               return zone.primary_name == "Asia/Kolkata";
                           });
    assert_no_examples<dt::TimeZone>(
        gs::timezones({.links = false}), [](const dt::TimeZone& zone) {
            return has_tag(zone.name, dt::Tag::Link);
        });
    find_any<dt::TimeZone>(gs::timezones(), [](const dt::TimeZone& zone) {
        return has_tag(zone.name, dt::Tag::Link);
    });
    find_any<dt::TimeZone>(gs::timezones({.legacy = true}),
                           [](const dt::TimeZone& zone) {
                               return has_tag(zone.name, dt::Tag::Legacy);
                           });
    find_any<dt::TimeZone>(
        gs::timezones({.unknown = true}),
        [](const dt::TimeZone& zone) { return zone.name == dt::UNKNOWN_ZONE; });
    assert_all_examples<dt::TimeZone>(
        gs::timezones({.names = std::vector<std::string>{"Mars/Olympus"},
                       .opaque = true}),
        [](const dt::TimeZone& zone) {
            return zone.name == "Mars/Olympus" &&
                   zone.primary_name == "Mars/Olympus";
        });
}

TEST(FindDatetime, ZonedDatetimesAgreeWithResolve) {
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes(
            {.min_value = resolvable_min, .max_value = resolvable_max}),
        agrees_with_resolve);
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes(
            {.timezones = {.names = std::vector<std::string>{"Europe/London"}},
             .min_value = resolvable_min,
             .max_value = resolvable_max}),
        [](const dt::ZonedDateTime& z) {
            return z.zone.name == "Europe/London" && agrees_with_resolve(z);
        });
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes(
            {.timezones = {.names = std::vector<std::string>{std::string(
                               dt::UNKNOWN_ZONE)}}}),
        [](const dt::ZonedDateTime& z) {
            return z.zone.name == dt::UNKNOWN_ZONE &&
                   z.offset == dt::UNKNOWN_OFFSET &&
                   z.kind == dt::Kind::Unique &&
                   dt::resolve(dt::UNKNOWN_ZONE, z.civil).timestamp ==
                       z.timestamp;
        });
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes(
            {.timezones = {.names = std::vector<std::string>{"Mars/Olympus"},
                           .opaque = true}}),
        [](const dt::ZonedDateTime& z) {
            return z.zone.name == "Mars/Olympus" &&
                   z.offset == dt::UNKNOWN_OFFSET && z.kind == dt::Kind::Unique;
        });
}

TEST(FindDatetime, ZonedDatetimesReachTheGapAndBothFolds) {
    dt::ZonedDateTime gap = find_any<dt::ZonedDateTime>(
        gs::zoned_datetimes(new_york_2024()),
        [](const dt::ZonedDateTime& z) { return z.kind == dt::Kind::Gap; },
        2000);
    EXPECT_EQ(gap.civil.date, (dt::Date{2024, 3, 10}));
    EXPECT_EQ(gap.civil.time.hour, 2);
    EXPECT_EQ(gap.offset_before, -18'000);
    EXPECT_EQ(gap.offset_after, -14'400);
    EXPECT_TRUE(agrees_with_resolve(gap));

    dt::ZonedDateTime earlier = find_any<dt::ZonedDateTime>(
        gs::zoned_datetimes(new_york_2024()),
        [](const dt::ZonedDateTime& z) {
            return z.kind == dt::Kind::Fold && !z.fold;
        },
        2000);
    EXPECT_EQ(earlier.civil.date, (dt::Date{2024, 11, 3}));
    EXPECT_EQ(earlier.civil.time.hour, 1);
    EXPECT_EQ(earlier.offset, dt::UtcOffset{-14'400});
    EXPECT_TRUE(agrees_with_resolve(earlier));

    dt::ZonedDateTime later = find_any<dt::ZonedDateTime>(
        gs::zoned_datetimes(new_york_2024()),
        [](const dt::ZonedDateTime& z) {
            return z.kind == dt::Kind::Fold && z.fold;
        },
        2000);
    EXPECT_EQ(later.civil.date, (dt::Date{2024, 11, 3}));
    EXPECT_EQ(later.civil.time.hour, 1);
    EXPECT_EQ(later.offset, dt::UtcOffset{-18'000});
    EXPECT_TRUE(agrees_with_resolve(later));
}

TEST(FindDatetime, ZonedDatetimesRespectBounds) {
    dt::DateTime min = datetime(2024, 1, 1, 0, 0, 0);
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes({.min_value = min}),
        [=](const dt::ZonedDateTime& z) { return z.civil >= min; });
    dt::DateTime max = datetime(1999, 12, 31, 0, 0, 0);
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes({.max_value = max}),
        [=](const dt::ZonedDateTime& z) { return z.civil <= max; });
    dt::Timestamp epoch{0, 0};
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes({.min_instant = epoch}),
        [=](const dt::ZonedDateTime& z) { return z.timestamp >= epoch; });
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes({.max_instant = epoch}),
        [=](const dt::ZonedDateTime& z) { return z.timestamp <= epoch; });
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes(
            {.min_instant = epoch, .max_instant = dt::Timestamp{86'400, 0}}),
        [=](const dt::ZonedDateTime& z) {
            return z.timestamp >= epoch && z.timestamp.second <= 86'400 &&
                   agrees_with_resolve(z);
        });
}

TEST(FindDatetime, ZonedDatetimesRespectPrecisionAndPolicies) {
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes({.precision = dt::Precision::Minutes}),
        [](const dt::ZonedDateTime& z) {
            return z.civil.time.second == 0 && z.civil.time.nanosecond == 0;
        });
    gs::ZonedDatetimesParams avoid = new_york_2024();
    avoid.allow_imaginary = false;
    assert_no_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes(avoid),
        [](const dt::ZonedDateTime& z) { return z.kind == dt::Kind::Gap; },
        2000);
    gs::ZonedDatetimesParams earlier = new_york_2024();
    earlier.ambiguous = dt::Ambiguous::Earlier;
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes(earlier),
        [](const dt::ZonedDateTime& z) {
            return !z.fold && agrees_with_resolve(z);
        },
        2000);
    gs::ZonedDatetimesParams later = new_york_2024();
    later.ambiguous = dt::Ambiguous::Later;
    assert_all_examples<dt::ZonedDateTime>(
        gs::zoned_datetimes(later),
        [](const dt::ZonedDateTime& z) {
            return z.fold == (z.kind == dt::Kind::Fold) &&
                   agrees_with_resolve(z);
        },
        2000);
    find_any<dt::ZonedDateTime>(
        gs::zoned_datetimes(later),
        [](const dt::ZonedDateTime& z) { return z.fold; }, 2000);
    assert_all_examples<std::vector<dt::ZonedDateTime>>(
        gs::vectors(gs::zoned_datetimes({.min_value = resolvable_min,
                                         .max_value = resolvable_max}),
                    {.max_size = 3}),
        [](const std::vector<dt::ZonedDateTime>& v) {
            return std::all_of(v.begin(), v.end(), agrees_with_resolve);
        });
}

TEST(FindDatetime, SignedDurationsRespectBoundsAndPrecision) {
    dt::SignedDuration zero{0, 0};
    assert_all_examples<dt::SignedDuration>(
        gs::signed_durations({.min_value = zero}),
        [=](const dt::SignedDuration& d) { return d >= zero; });
    dt::SignedDuration max{60, 0};
    assert_all_examples<dt::SignedDuration>(
        gs::signed_durations({.min_value = zero, .max_value = max}),
        [=](const dt::SignedDuration& d) { return d >= zero && d <= max; });
    assert_all_examples<dt::SignedDuration>(
        gs::signed_durations({.precision = dt::Precision::Seconds}),
        [](const dt::SignedDuration& d) { return d.nanoseconds == 0; });
    assert_all_examples<dt::SignedDuration>(
        gs::signed_durations(), [](const dt::SignedDuration& d) {
            return d.nanoseconds > -1'000'000'000 &&
                   d.nanoseconds < 1'000'000'000 &&
                   (d.seconds == 0 || d.nanoseconds == 0 ||
                    (d.seconds < 0) == (d.nanoseconds < 0));
        });
    find_any<dt::SignedDuration>(
        gs::signed_durations(),
        [](const dt::SignedDuration& d) { return d.seconds < 0; });
    find_any<dt::SignedDuration>(
        gs::signed_durations(),
        [](const dt::SignedDuration& d) { return d.seconds == 86'400; });
}

namespace {
    bool uniform_sign(const dt::Span& s) {
        bool all_non_negative = true;
        bool all_non_positive = true;
        for (dt::SpanUnit unit : dt::ALL_SPAN_UNITS) {
            all_non_negative = all_non_negative && s.get(unit) >= 0;
            all_non_positive = all_non_positive && s.get(unit) <= 0;
        }
        return all_non_negative || all_non_positive;
    }
} // namespace

TEST(FindDatetime, SpansRespectUnitBoundsAndSign) {
    assert_all_examples<dt::Span>(gs::spans(), uniform_sign);
    find_any<dt::Span>(gs::spans(),
                       [](const dt::Span& s) { return s.years < 0; });
    find_any<dt::Span>(gs::spans(),
                       [](const dt::Span& s) { return s.nanoseconds > 0; });
    assert_all_examples<dt::Span>(
        gs::spans({.days = {{0, 30}}, .hours = {{-23, 23}}}),
        [](const dt::Span& s) {
            bool others_zero = true;
            for (dt::SpanUnit unit : dt::ALL_SPAN_UNITS) {
                if (unit != dt::SpanUnit::Days && unit != dt::SpanUnit::Hours) {
                    others_zero = others_zero && s.get(unit) == 0;
                }
            }
            return s.days >= 0 && s.days <= 30 && s.hours >= -23 &&
                   s.hours <= 23 && others_zero;
        });
    assert_all_examples<dt::Span>(
        gs::spans({.sign = dt::SpanSign::NonNegative}), [](const dt::Span& s) {
            for (dt::SpanUnit unit : dt::ALL_SPAN_UNITS) {
                if (s.get(unit) < 0) {
                    return false;
                }
            }
            return true;
        });
    find_any<dt::Span>(gs::spans({.sign = dt::SpanSign::Mixed}),
                       [](const dt::Span& s) { return !uniform_sign(s); });
    assert_all_examples<dt::Span>(
        gs::spans({.days = {{-30, 30}}, .sign = dt::SpanSign::Uniform}),
        uniform_sign);
    assert_all_examples<std::vector<dt::Span>>(
        gs::vectors(gs::spans({.days = {{0, 30}}}), {.max_size = 5}),
        [](const std::vector<dt::Span>& v) {
            return std::all_of(v.begin(), v.end(), [](const dt::Span& s) {
                return s.days >= 0 && s.days <= 30;
            });
        });
}
