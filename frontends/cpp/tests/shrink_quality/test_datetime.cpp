#include <gtest/gtest.h>

#include "common/utils.h"

#include <chrono>
#include <cstdint>
#include <sstream>
#include <string>

using hegel::tests::common::minimal;
namespace gs = hegel::generators;
namespace dt = hegel::datetime;

namespace {

    template <typename T> std::string show(const T& value) {
        std::ostringstream os;
        os << value;
        return os.str();
    }

    dt::DateTime datetime(int32_t year, uint8_t month, uint8_t day,
                          uint8_t hour, uint8_t minute, uint8_t second) {
        return dt::DateTime{{year, month, day}, {hour, minute, second, 0}};
    }

} // namespace

TEST(ShrinkDatetime, DatesShrinkTowardTheMillennium) {
    EXPECT_EQ(
        minimal<dt::Date>(gs::dates(), [](const dt::Date&) { return true; }),
        (dt::Date{2000, 1, 1}));
    EXPECT_EQ(minimal<dt::Date>(gs::dates({.min_value = dt::Date{2024, 1, 1}}),
                                [](const dt::Date&) { return true; }),
              (dt::Date{2024, 1, 1}));
    EXPECT_EQ(
        minimal<dt::Date>(gs::dates({.max_value = dt::Date{1999, 12, 31}}),
                          [](const dt::Date&) { return true; }),
        (dt::Date{1999, 12, 31}));
    EXPECT_EQ(
        minimal<dt::Date>(gs::dates({.weekdays = {{dt::Weekday::Monday}}}),
                          [](const dt::Date&) { return true; }),
        (dt::Date{2000, 1, 3}));
}

TEST(ShrinkDatetime, TimesShrinkTowardMidnight) {
    EXPECT_EQ(
        minimal<dt::Time>(gs::times(), [](const dt::Time&) { return true; }),
        (dt::Time{0, 0, 0, 0}));
    EXPECT_EQ(
        minimal<dt::Time>(gs::times({.precision = dt::Precision::Seconds}),
                          [](const dt::Time& t) { return t.hour >= 1; }),
        (dt::Time{1, 0, 0, 0}));
}

TEST(ShrinkDatetime, DatetimesShrinkTowardTheMillennium) {
    EXPECT_EQ(minimal<dt::DateTime>(gs::datetimes(),
                                    [](const dt::DateTime&) { return true; }),
              datetime(2000, 1, 1, 0, 0, 0));
}

TEST(ShrinkDatetime, TimestampsShrinkTowardTheMillennium) {
    EXPECT_EQ(minimal<dt::Timestamp>(gs::timestamps(),
                                     [](const dt::Timestamp&) { return true; }),
              (dt::Timestamp{946'684'800, 0}));
    EXPECT_EQ(minimal<dt::Timestamp>(
                  gs::timestamps({.max_value = dt::Timestamp{0, 0}}),
                  [](const dt::Timestamp&) { return true; }),
              (dt::Timestamp{0, 0}));
}

TEST(ShrinkDatetime, UtcOffsetsShrinkToZeroAndToKnown) {
    EXPECT_EQ(minimal<dt::UtcOffset>(gs::utc_offsets(),
                                     [](const dt::UtcOffset&) { return true; }),
              dt::UtcOffset{0});
    EXPECT_EQ(minimal<dt::UtcOffset>(gs::utc_offsets({.unknown = true}),
                                     [](const dt::UtcOffset&) { return true; }),
              dt::UtcOffset{0});
    EXPECT_EQ(minimal<dt::UtcOffset>(gs::utc_offsets({.unknown = true}),
                                     [](const dt::UtcOffset& o) {
                                         return !o.known || o.seconds >= 3600 ||
                                                o.seconds <= -3600;
                                     }),
              dt::UtcOffset{3600});
    EXPECT_EQ(
        minimal<dt::UtcOffset>(gs::utc_offsets({.unknown = true}),
                               [](const dt::UtcOffset& o) { return !o.known; }),
        dt::UNKNOWN_OFFSET);
}

TEST(ShrinkDatetime, TimezonesShrinkToUtc) {
    EXPECT_EQ(minimal<dt::TimeZone>(gs::timezones(),
                                    [](const dt::TimeZone&) { return true; })
                  .name,
              "UTC");
    EXPECT_EQ(minimal<dt::TimeZone>(gs::timezones({.legacy = true}),
                                    [](const dt::TimeZone&) { return true; })
                  .name,
              "UTC");
}

TEST(ShrinkDatetime, ZonedDatetimesShrinkToUtcAtTheMillennium) {
    EXPECT_EQ(show(minimal<dt::ZonedDateTime>(
                  gs::zoned_datetimes(),
                  [](const dt::ZonedDateTime&) { return true; })),
              "2000-01-01T00:00:00+00:00[UTC]");
    EXPECT_EQ(
        show(minimal<dt::ZonedDateTime>(
            gs::zoned_datetimes({.min_instant = dt::Timestamp{0, 0},
                                 .max_instant = dt::Timestamp{86'400, 0}}),
            [](const dt::ZonedDateTime&) { return true; })),
        "1970-01-02T00:00:00+00:00[UTC]");
}

TEST(ShrinkDatetime, ZonedDatetimesShrinkPastAFoldAtTheLowerBound) {
    gs::TimezonesParams new_york{
        .names = std::vector<std::string>{"America/New_York"}};
    dt::DateTime in_fold = datetime(2024, 11, 3, 1, 30, 0);
    EXPECT_EQ(
        show(minimal<dt::ZonedDateTime>(
            gs::zoned_datetimes({.timezones = new_york, .min_value = in_fold}),
            [](const dt::ZonedDateTime&) { return true; })),
        "2024-11-03T02:00:00-05:00[America/New_York]");
    EXPECT_EQ(
        show(minimal<dt::ZonedDateTime>(
            gs::zoned_datetimes({.timezones = new_york, .min_value = in_fold}),
            [](const dt::ZonedDateTime& z) {
                return z.kind == dt::Kind::Fold;
            })),
        "2024-11-03T01:30:00-04:00[America/New_York][fold=0]");
    EXPECT_EQ(show(minimal<dt::ZonedDateTime>(
                  gs::zoned_datetimes({.timezones = new_york,
                                       .min_value = in_fold,
                                       .ambiguous = dt::Ambiguous::Earlier}),
                  [](const dt::ZonedDateTime&) { return true; })),
              "2024-11-03T01:30:00-04:00[America/New_York][fold=0]");
}

TEST(ShrinkDatetime, SignedDurationsShrinkToZero) {
    EXPECT_EQ(minimal<dt::SignedDuration>(
                  gs::signed_durations(),
                  [](const dt::SignedDuration&) { return true; }),
              (dt::SignedDuration{0, 0}));
    EXPECT_EQ(minimal<dt::SignedDuration>(
                  gs::signed_durations(),
                  [](const dt::SignedDuration& d) { return d.seconds < 0; }),
              (dt::SignedDuration{-1, 0}));
}

TEST(ShrinkDatetime, SpansShrinkToZero) {
    EXPECT_EQ(
        minimal<dt::Span>(gs::spans(), [](const dt::Span&) { return true; }),
        dt::Span{});
    dt::Span one_day;
    one_day.days = 1;
    EXPECT_EQ(minimal<dt::Span>(gs::spans({.days = {{1, 30}}}),
                                [](const dt::Span&) { return true; }),
              one_day);
}

TEST(ShrinkDatetime, DurationsShrinkToZero) {
    using std::chrono::milliseconds;
    EXPECT_EQ(minimal<milliseconds>(gs::durations<milliseconds>(),
                                    [](const milliseconds&) { return true; }),
              milliseconds{0});
    EXPECT_EQ(minimal<milliseconds>(gs::durations<milliseconds>(),
                                    [](const milliseconds& d) {
                                        return d >= milliseconds{1500};
                                    }),
              milliseconds{1500});
}
