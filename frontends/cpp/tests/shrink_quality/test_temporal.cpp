#include <gtest/gtest.h>

#include "common/utils.h"

#include <chrono>
#include <cstdint>
#include <sstream>
#include <string>
#include <vector>

using hegel::tests::common::minimal;
namespace gs = hegel::generators;

namespace {

    template <typename T> std::string show(const T& value) {
        std::ostringstream os;
        os << value;
        return os.str();
    }

    gs::temporal::DateTime datetime(int32_t year, uint8_t month, uint8_t day,
                                    uint8_t hour, uint8_t minute,
                                    uint8_t second) {
        return gs::temporal::DateTime{{year, month, day},
                                      {hour, minute, second, 0}};
    }

} // namespace

TEST(ShrinkTemporal, DatesShrinkTowardTheMillennium) {
    EXPECT_EQ(minimal<gs::temporal::Date>(
                  gs::temporal::dates(),
                  [](const gs::temporal::Date&) { return true; }),
              (gs::temporal::Date{2000, 1, 1}));
    EXPECT_EQ(
        minimal<gs::temporal::Date>(
            gs::temporal::dates({.min_value = gs::temporal::Date{2024, 1, 1}}),
            [](const gs::temporal::Date&) { return true; }),
        (gs::temporal::Date{2024, 1, 1}));
    EXPECT_EQ(minimal<gs::temporal::Date>(
                  gs::temporal::dates(
                      {.max_value = gs::temporal::Date{1999, 12, 31}}),
                  [](const gs::temporal::Date&) { return true; }),
              (gs::temporal::Date{1999, 12, 31}));
    EXPECT_EQ(minimal<gs::temporal::Date>(
                  gs::temporal::dates(
                      {.weekdays = {{gs::temporal::Weekday::Monday}}}),
                  [](const gs::temporal::Date&) { return true; }),
              (gs::temporal::Date{2000, 1, 3}));
}

TEST(ShrinkTemporal, TimesShrinkTowardMidnight) {
    EXPECT_EQ(minimal<gs::temporal::Time>(
                  gs::temporal::times(),
                  [](const gs::temporal::Time&) { return true; }),
              (gs::temporal::Time{0, 0, 0, 0}));
    EXPECT_EQ(minimal<gs::temporal::Time>(
                  gs::temporal::times(
                      {.precision = gs::temporal::Precision::Seconds}),
                  [](const gs::temporal::Time& t) { return t.hour >= 1; }),
              (gs::temporal::Time{1, 0, 0, 0}));
}

TEST(ShrinkTemporal, DatetimesShrinkTowardTheMillennium) {
    EXPECT_EQ(minimal<gs::temporal::DateTime>(
                  gs::temporal::datetimes(),
                  [](const gs::temporal::DateTime&) { return true; }),
              datetime(2000, 1, 1, 0, 0, 0));
}

TEST(ShrinkTemporal, InstantsShrinkTowardTheMillennium) {
    EXPECT_EQ(minimal<gs::temporal::Instant>(
                  gs::temporal::instants(),
                  [](const gs::temporal::Instant&) { return true; }),
              (gs::temporal::Instant{946'684'800, 0}));
    EXPECT_EQ(
        minimal<gs::temporal::Instant>(
            gs::temporal::instants({.max_value = gs::temporal::Instant{0, 0}}),
            [](const gs::temporal::Instant&) { return true; }),
        (gs::temporal::Instant{0, 0}));
}

TEST(ShrinkTemporal, OffsetsShrinkToZeroAndToKnown) {
    EXPECT_EQ(minimal<gs::temporal::UtcOffset>(
                  gs::temporal::offsets(),
                  [](const gs::temporal::UtcOffset&) { return true; }),
              gs::temporal::UtcOffset{0});
    EXPECT_EQ(minimal<gs::temporal::UtcOffset>(
                  gs::temporal::offsets({.unknown = true}),
                  [](const gs::temporal::UtcOffset&) { return true; }),
              gs::temporal::UtcOffset{0});
    EXPECT_EQ(minimal<gs::temporal::UtcOffset>(
                  gs::temporal::offsets({.unknown = true}),
                  [](const gs::temporal::UtcOffset& o) {
                      return !o.known || o.seconds >= 3600 ||
                             o.seconds <= -3600;
                  }),
              gs::temporal::UtcOffset{3600});
    EXPECT_EQ(minimal<gs::temporal::UtcOffset>(
                  gs::temporal::offsets({.unknown = true}),
                  [](const gs::temporal::UtcOffset& o) { return !o.known; }),
              gs::temporal::UNKNOWN_OFFSET);
}

TEST(ShrinkTemporal, ZonesShrinkToUtc) {
    EXPECT_EQ(minimal<gs::temporal::TimeZone>(
                  gs::temporal::zones(),
                  [](const gs::temporal::TimeZone&) { return true; })
                  .name,
              "UTC");
    EXPECT_EQ(minimal<gs::temporal::TimeZone>(
                  gs::temporal::zones({.legacy = true}),
                  [](const gs::temporal::TimeZone&) { return true; })
                  .name,
              "UTC");
}

TEST(ShrinkTemporal, ZonedDatetimesShrinkToUtcAtTheMillennium) {
    EXPECT_EQ(show(minimal<gs::temporal::ZonedDateTime>(
                  gs::temporal::zoned_datetimes(),
                  [](const gs::temporal::ZonedDateTime&) { return true; })),
              "2000-01-01T00:00:00+00:00[UTC]");
    EXPECT_EQ(show(minimal<gs::temporal::ZonedDateTime>(
                  gs::temporal::zoned_datetimes(
                      {.min_instant = gs::temporal::Instant{0, 0},
                       .max_instant = gs::temporal::Instant{86'400, 0}}),
                  [](const gs::temporal::ZonedDateTime&) { return true; })),
              "1970-01-02T00:00:00+00:00[UTC]");
}

TEST(ShrinkTemporal, ZonedDatetimesShrinkPastAFoldAtTheLowerBound) {
    gs::Generator<gs::temporal::TimeZone> new_york = gs::temporal::zones(
        {.names = std::vector<std::string>{"America/New_York"}});
    gs::temporal::DateTime in_fold = datetime(2024, 11, 3, 1, 30, 0);
    EXPECT_EQ(show(minimal<gs::temporal::ZonedDateTime>(
                  gs::temporal::zoned_datetimes(
                      {.zones = new_york, .min_value = in_fold}),
                  [](const gs::temporal::ZonedDateTime&) { return true; })),
              "2024-11-03T02:00:00-05:00[America/New_York]");
    EXPECT_EQ(show(minimal<gs::temporal::ZonedDateTime>(
                  gs::temporal::zoned_datetimes(
                      {.zones = new_york, .min_value = in_fold}),
                  [](const gs::temporal::ZonedDateTime& z) {
                      return z.kind == gs::temporal::TransitionKind::Fold;
                  })),
              "2024-11-03T01:30:00-04:00[America/New_York][fold=0]");
    EXPECT_EQ(show(minimal<gs::temporal::ZonedDateTime>(
                  gs::temporal::zoned_datetimes(
                      {.zones = new_york,
                       .min_value = in_fold,
                       .ambiguous = gs::temporal::Ambiguous::Earlier}),
                  [](const gs::temporal::ZonedDateTime&) { return true; })),
              "2024-11-03T01:30:00-04:00[America/New_York][fold=0]");
}

TEST(ShrinkTemporal, ZonedDatetimesFromAGeneratorShrinkAsTheBuiltInOneDoes) {
    gs::temporal::DateTime in_fold = datetime(2024, 11, 3, 1, 30, 0);
    gs::Generator<gs::temporal::TimeZone> new_york = gs::just(
        gs::temporal::TimeZone{"America/New_York", "America/New_York"});
    EXPECT_EQ(show(minimal<gs::temporal::ZonedDateTime>(
                  gs::temporal::zoned_datetimes(
                      {.zones = new_york, .min_value = in_fold}),
                  [](const gs::temporal::ZonedDateTime&) { return true; })),
              "2024-11-03T02:00:00-05:00[America/New_York]");
    EXPECT_EQ(
        show(minimal<gs::temporal::ZonedDateTime>(
            gs::temporal::zoned_datetimes(
                {.zones = gs::sampled_from(std::vector<gs::temporal::TimeZone>{
                     {"Europe/Paris", "Europe/Paris"},
                     {"America/New_York", "America/New_York"}})}),
            [](const gs::temporal::ZonedDateTime& z) {
                return z.zone.name == "America/New_York";
            })),
        "2000-01-01T00:00:00-05:00[America/New_York]");
}

TEST(ShrinkTemporal, DurationsShrinkToZero) {
    EXPECT_EQ(minimal<gs::temporal::SignedDuration>(
                  gs::temporal::durations(),
                  [](const gs::temporal::SignedDuration&) { return true; }),
              (gs::temporal::SignedDuration{0, 0}));
    EXPECT_EQ(minimal<gs::temporal::SignedDuration>(
                  gs::temporal::durations(),
                  [](const gs::temporal::SignedDuration& d) {
                      return d.seconds < 0;
                  }),
              (gs::temporal::SignedDuration{-1, 0}));
}

TEST(ShrinkTemporal, SpansShrinkToZero) {
    EXPECT_EQ(minimal<gs::temporal::Span>(
                  gs::temporal::spans(),
                  [](const gs::temporal::Span&) { return true; }),
              gs::temporal::Span{});
    gs::temporal::Span one_day;
    one_day.days = 1;
    EXPECT_EQ(minimal<gs::temporal::Span>(
                  gs::temporal::spans({.days = {{1, 30}}}),
                  [](const gs::temporal::Span&) { return true; }),
              one_day);
}

TEST(ShrinkTemporal, ChronoDurationsShrinkToZero) {
    using std::chrono::milliseconds;
    EXPECT_EQ(minimal<milliseconds>(gs::temporal::durations<milliseconds>(),
                                    [](const milliseconds&) { return true; }),
              milliseconds{0});
    EXPECT_EQ(minimal<milliseconds>(gs::temporal::durations<milliseconds>(),
                                    [](const milliseconds& d) {
                                        return d >= milliseconds{1500};
                                    }),
              milliseconds{1500});
}
