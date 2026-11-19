#include <gtest/gtest.h>

#include "common/utils.h"

#include <algorithm>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

using hegel::tests::common::assert_all_examples;
using hegel::tests::common::assert_no_examples;
using hegel::tests::common::find_any;
namespace gs = hegel::generators;

namespace {

    gs::temporal::DateTime datetime(int32_t year, uint8_t month, uint8_t day,
                                    uint8_t hour, uint8_t minute,
                                    uint8_t second) {
        return gs::temporal::DateTime{{year, month, day},
                                      {hour, minute, second, 0}};
    }

    // Tomohiko Sakamoto's algorithm: 0 is Sunday.
    int weekday_of(const gs::temporal::Date& d) {
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

    bool has_tag(const std::string& name, gs::temporal::Tag tag) {
        std::optional<gs::temporal::ZoneInfo> info = gs::temporal::info(name);
        return info.has_value() &&
               std::find(info->tags.begin(), info->tags.end(), tag) !=
                   info->tags.end();
    }

    // Whether the engine's resolver agrees with a zoned draw.
    bool agrees_with_resolve(const gs::temporal::ZonedDateTime& z) {
        gs::temporal::ZonedDateTime r =
            gs::temporal::resolve(z.zone.name, z.civil);
        gs::temporal::Instant expected = r.instant;
        gs::temporal::UtcOffset expected_offset = r.offset;
        if (z.kind == gs::temporal::TransitionKind::Fold && z.fold) {
            expected.second += r.offset_before.seconds - r.offset_after.seconds;
            expected_offset = r.offset_after;
        }
        return r.zone == z.zone && r.kind == z.kind &&
               r.offset_before == z.offset_before &&
               r.offset_after == z.offset_after && expected == z.instant &&
               expected_offset == z.offset &&
               (z.kind == gs::temporal::TransitionKind::Fold || !z.fold);
    }

    // Wall-clock bounds a few years inside the civil range, so that every
    // drawn wall time resolves to an instant in range.
    const gs::temporal::DateTime resolvable_min =
        datetime(-999'990, 1, 1, 0, 0, 0);
    const gs::temporal::DateTime resolvable_max =
        datetime(999'990, 12, 31, 23, 59, 59);

    gs::temporal::ZonedDatetimesParams new_york_2024() {
        return gs::temporal::ZonedDatetimesParams{
            .zones = gs::temporal::zones(
                {.names = std::vector<std::string>{"America/New_York"}}),
            .min_value = datetime(2024, 1, 1, 0, 0, 0),
            .max_value = datetime(2024, 12, 31, 23, 59, 59)};
    }

    gs::temporal::TimeZone zone(const std::string& name) {
        return gs::temporal::TimeZone{name,
                                      gs::temporal::info(name)->primary_name};
    }

    gs::temporal::ZonedDatetimesParams
    new_york_2024_from(gs::Generator<gs::temporal::TimeZone> zones) {
        gs::temporal::ZonedDatetimesParams params = new_york_2024();
        params.zones = std::move(zones);
        return params;
    }

} // namespace

TEST(FindTemporal, DatesRespectBounds) {
    gs::temporal::Date min{2024, 1, 1};
    gs::temporal::Date max{2024, 12, 31};
    assert_all_examples<gs::temporal::Date>(
        gs::temporal::dates({.min_value = min, .max_value = max}),
        [=](const gs::temporal::Date& d) { return d >= min && d <= max; });
    find_any<gs::temporal::Date>(gs::temporal::dates(),
                                 [](const gs::temporal::Date& d) {
                                     return d.year > 9999 || d.year < 0;
                                 });
    find_any<gs::temporal::Date>(
        gs::temporal::dates({.min_value = min, .max_value = max}),
        [](const gs::temporal::Date& d) {
            return d.month == 2 && d.day == 29;
        });
}

TEST(FindTemporal, DatesRespectWeekdayAndMonthMasks) {
    assert_all_examples<gs::temporal::Date>(
        gs::temporal::dates({.min_value = gs::temporal::Date{-9999, 1, 1},
                             .max_value = gs::temporal::Date{9999, 12, 31},
                             .weekdays = {{gs::temporal::Weekday::Saturday,
                                           gs::temporal::Weekday::Sunday}}}),
        [](const gs::temporal::Date& d) {
            int w = weekday_of(d);
            return w == 6 || w == 0;
        });
    assert_all_examples<gs::temporal::Date>(
        gs::temporal::dates({.months = {{gs::temporal::Month::February,
                                         gs::temporal::Month::December}}}),
        [](const gs::temporal::Date& d) {
            return d.month == 2 || d.month == 12;
        });
    assert_all_examples<std::vector<gs::temporal::Date>>(
        gs::vectors(gs::temporal::dates(
                        {.max_value = gs::temporal::Date{1999, 12, 31}}),
                    {.max_size = 5}),
        [](const std::vector<gs::temporal::Date>& v) {
            return std::all_of(v.begin(), v.end(),
                               [](const gs::temporal::Date& d) {
                                   return d <= gs::temporal::Date{1999, 12, 31};
                               });
        });
}

TEST(FindTemporal, TimesRespectBoundsAndPrecision) {
    gs::temporal::Time min{6, 0, 0, 0};
    gs::temporal::Time max{18, 0, 0, 0};
    assert_all_examples<gs::temporal::Time>(
        gs::temporal::times({.min_value = min, .max_value = max}),
        [=](const gs::temporal::Time& t) { return t >= min && t <= max; });
    assert_all_examples<gs::temporal::Time>(
        gs::temporal::times({.precision = gs::temporal::Precision::Seconds}),
        [](const gs::temporal::Time& t) { return t.nanosecond == 0; });
    assert_all_examples<gs::temporal::Time>(
        gs::temporal::times(
            {.precision = gs::temporal::Precision::Milliseconds}),
        [](const gs::temporal::Time& t) {
            return t.nanosecond % 1'000'000 == 0;
        });
    assert_all_examples<gs::temporal::Time>(
        gs::temporal::times({.precision = gs::temporal::Precision::Hours}),
        [](const gs::temporal::Time& t) {
            return t.minute == 0 && t.second == 0 && t.nanosecond == 0;
        });
    find_any<gs::temporal::Time>(
        gs::temporal::times(),
        [](const gs::temporal::Time& t) { return t.nanosecond % 1000 != 0; });
}

TEST(FindTemporal, DatetimesRespectBoundsAndPrecision) {
    gs::temporal::DateTime min = datetime(2024, 1, 1, 0, 0, 0);
    gs::temporal::DateTime max = datetime(2024, 12, 31, 23, 59, 59);
    assert_all_examples<gs::temporal::DateTime>(
        gs::temporal::datetimes({.min_value = min, .max_value = max}),
        [=](const gs::temporal::DateTime& d) { return d >= min && d <= max; });
    assert_all_examples<gs::temporal::DateTime>(
        gs::temporal::datetimes(
            {.precision = gs::temporal::Precision::Minutes}),
        [](const gs::temporal::DateTime& d) {
            return d.time.second == 0 && d.time.nanosecond == 0;
        });
}

TEST(FindTemporal, InstantsRespectBoundsAndPrecision) {
    gs::temporal::Instant min{0, 0};
    gs::temporal::Instant max{86'400, 0};
    assert_all_examples<gs::temporal::Instant>(
        gs::temporal::instants({.min_value = min, .max_value = max}),
        [=](const gs::temporal::Instant& t) { return t >= min && t <= max; });
    assert_all_examples<gs::temporal::Instant>(
        gs::temporal::instants({.precision = gs::temporal::Precision::Seconds}),
        [](const gs::temporal::Instant& t) { return t.nanosecond == 0; });
    assert_all_examples<gs::temporal::Instant>(
        gs::temporal::instants(
            {.precision = gs::temporal::Precision::Microseconds}),
        [](const gs::temporal::Instant& t) {
            return t.nanosecond % 1000 == 0;
        });
    find_any<gs::temporal::Instant>(
        gs::temporal::instants(),
        [](const gs::temporal::Instant& t) {
            return t.second == 0 && t.nanosecond == 0;
        },
        10'000);
}

TEST(FindTemporal, OffsetsRespectGranularityAndReachTheUnknown) {
    assert_all_examples<gs::temporal::UtcOffset>(
        gs::temporal::offsets(), [](const gs::temporal::UtcOffset& o) {
            return o.known && o.seconds >= -gs::temporal::MAX_OFFSET &&
                   o.seconds <= gs::temporal::MAX_OFFSET;
        });
    assert_all_examples<gs::temporal::UtcOffset>(
        gs::temporal::offsets(
            {.granularity = gs::temporal::Granularity::Hours}),
        [](const gs::temporal::UtcOffset& o) { return o.seconds % 3600 == 0; });
    assert_all_examples<gs::temporal::UtcOffset>(
        gs::temporal::offsets(
            {.granularity = gs::temporal::Granularity::QuarterHours}),
        [](const gs::temporal::UtcOffset& o) { return o.seconds % 900 == 0; });
    assert_all_examples<gs::temporal::UtcOffset>(
        gs::temporal::offsets(
            {.granularity = gs::temporal::Granularity::Minutes}),
        [](const gs::temporal::UtcOffset& o) { return o.seconds % 60 == 0; });
    assert_all_examples<gs::temporal::UtcOffset>(
        gs::temporal::offsets({.min_value = gs::temporal::UtcOffset{0},
                               .max_value = gs::temporal::UtcOffset{3600}}),
        [](const gs::temporal::UtcOffset& o) {
            return o.seconds >= 0 && o.seconds <= 3600;
        });
    find_any<gs::temporal::UtcOffset>(
        gs::temporal::offsets({.allow_unknown = true}),
        [](const gs::temporal::UtcOffset& o) { return !o.known; });
    find_any<gs::temporal::UtcOffset>(
        gs::temporal::offsets(),
        [](const gs::temporal::UtcOffset& o) { return o.seconds % 3600 != 0; });
}

TEST(FindTemporal, ZonesRespectTheSpec) {
    assert_all_examples<gs::temporal::TimeZone>(
        gs::temporal::zones(), [](const gs::temporal::TimeZone& zone) {
            std::optional<gs::temporal::ZoneInfo> info =
                gs::temporal::info(zone.name);
            return info.has_value() &&
                   info->primary_name == zone.primary_name &&
                   !has_tag(zone.name, gs::temporal::Tag::Legacy) &&
                   zone.name != gs::temporal::UNKNOWN_ZONE;
        });
    std::vector<std::string> names = {"Europe/London", "Asia/Calcutta"};
    assert_all_examples<gs::temporal::TimeZone>(
        gs::temporal::zones({.names = names}),
        [names](const gs::temporal::TimeZone& zone) {
            return std::find(names.begin(), names.end(), zone.name) !=
                   names.end();
        });
    find_any<gs::temporal::TimeZone>(gs::temporal::zones({.names = names}),
                                     [](const gs::temporal::TimeZone& zone) {
                                         return zone.primary_name ==
                                                "Asia/Kolkata";
                                     });
    assert_no_examples<gs::temporal::TimeZone>(
        gs::temporal::zones({.allow_links = false}),
        [](const gs::temporal::TimeZone& zone) {
            return has_tag(zone.name, gs::temporal::Tag::Link);
        });
    find_any<gs::temporal::TimeZone>(
        gs::temporal::zones(), [](const gs::temporal::TimeZone& zone) {
            return has_tag(zone.name, gs::temporal::Tag::Link);
        });
    find_any<gs::temporal::TimeZone>(
        gs::temporal::zones({.allow_legacy = true}),
        [](const gs::temporal::TimeZone& zone) {
            return has_tag(zone.name, gs::temporal::Tag::Legacy);
        });
    find_any<gs::temporal::TimeZone>(
        gs::temporal::zones({.allow_unknown = true}),
        [](const gs::temporal::TimeZone& zone) {
            return zone.name == gs::temporal::UNKNOWN_ZONE;
        });
    assert_all_examples<gs::temporal::TimeZone>(
        gs::temporal::zones({.names = std::vector<std::string>{"Mars/Olympus"},
                             .allow_opaque = true}),
        [](const gs::temporal::TimeZone& zone) {
            return zone.name == "Mars/Olympus" &&
                   zone.primary_name == "Mars/Olympus";
        });
}

TEST(FindTemporal, ZonedDatetimesAgreeWithResolve) {
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(
            {.min_value = resolvable_min, .max_value = resolvable_max}),
        agrees_with_resolve);
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(
            {.zones = gs::temporal::zones(
                 {.names = std::vector<std::string>{"Europe/London"}}),
             .min_value = resolvable_min,
             .max_value = resolvable_max}),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.zone.name == "Europe/London" && agrees_with_resolve(z);
        });
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(
            {.zones = gs::temporal::zones(
                 {.names = std::vector<std::string>{std::string(
                      gs::temporal::UNKNOWN_ZONE)}})}),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.zone.name == gs::temporal::UNKNOWN_ZONE &&
                   z.offset == gs::temporal::UNKNOWN_OFFSET &&
                   z.kind == gs::temporal::TransitionKind::Unique &&
                   gs::temporal::resolve(gs::temporal::UNKNOWN_ZONE, z.civil)
                           .instant == z.instant;
        });
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(
            {.zones = gs::temporal::zones(
                 {.names = std::vector<std::string>{"Mars/Olympus"},
                  .allow_opaque = true})}),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.zone.name == "Mars/Olympus" &&
                   z.offset == gs::temporal::UNKNOWN_OFFSET &&
                   z.kind == gs::temporal::TransitionKind::Unique;
        });
}

TEST(FindTemporal, ZonedDatetimesReachTheGapAndBothFolds) {
    gs::temporal::ZonedDateTime gap = find_any<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(new_york_2024()),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.kind == gs::temporal::TransitionKind::Gap;
        },
        2000);
    EXPECT_EQ(gap.civil.date, (gs::temporal::Date{2024, 3, 10}));
    EXPECT_EQ(gap.civil.time.hour, 2);
    EXPECT_EQ(gap.offset_before, gs::temporal::UtcOffset{-18'000});
    EXPECT_EQ(gap.offset_after, gs::temporal::UtcOffset{-14'400});
    EXPECT_TRUE(agrees_with_resolve(gap));

    gs::temporal::ZonedDateTime earlier = find_any<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(new_york_2024()),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.kind == gs::temporal::TransitionKind::Fold && !z.fold;
        },
        2000);
    EXPECT_EQ(earlier.civil.date, (gs::temporal::Date{2024, 11, 3}));
    EXPECT_EQ(earlier.civil.time.hour, 1);
    EXPECT_EQ(earlier.offset, gs::temporal::UtcOffset{-14'400});
    EXPECT_TRUE(agrees_with_resolve(earlier));

    gs::temporal::ZonedDateTime later = find_any<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(new_york_2024()),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.kind == gs::temporal::TransitionKind::Fold && z.fold;
        },
        2000);
    EXPECT_EQ(later.civil.date, (gs::temporal::Date{2024, 11, 3}));
    EXPECT_EQ(later.civil.time.hour, 1);
    EXPECT_EQ(later.offset, gs::temporal::UtcOffset{-18'000});
    EXPECT_TRUE(agrees_with_resolve(later));
}

TEST(FindTemporal, ZonedDatetimesRespectBounds) {
    gs::temporal::DateTime min = datetime(2024, 1, 1, 0, 0, 0);
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes({.min_value = min}),
        [=](const gs::temporal::ZonedDateTime& z) { return z.civil >= min; });
    gs::temporal::DateTime max = datetime(1999, 12, 31, 0, 0, 0);
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes({.max_value = max}),
        [=](const gs::temporal::ZonedDateTime& z) { return z.civil <= max; });
    gs::temporal::Instant epoch{0, 0};
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes({.min_instant = epoch}),
        [=](const gs::temporal::ZonedDateTime& z) {
            return z.instant >= epoch;
        });
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes({.max_instant = epoch}),
        [=](const gs::temporal::ZonedDateTime& z) {
            return z.instant <= epoch;
        });
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(
            {.min_instant = epoch,
             .max_instant = gs::temporal::Instant{86'400, 0}}),
        [=](const gs::temporal::ZonedDateTime& z) {
            return z.instant >= epoch && z.instant.second <= 86'400 &&
                   agrees_with_resolve(z);
        });
}

TEST(FindTemporal, ZonedDatetimesRespectPrecisionAndPolicies) {
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(
            {.precision = gs::temporal::Precision::Minutes}),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.civil.time.second == 0 && z.civil.time.nanosecond == 0;
        });
    gs::temporal::ZonedDatetimesParams avoid = new_york_2024();
    avoid.allow_imaginary = false;
    assert_no_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(avoid),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.kind == gs::temporal::TransitionKind::Gap;
        },
        2000);
    gs::temporal::ZonedDatetimesParams earlier = new_york_2024();
    earlier.ambiguous = gs::temporal::Ambiguous::Earlier;
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(earlier),
        [](const gs::temporal::ZonedDateTime& z) {
            return !z.fold && agrees_with_resolve(z);
        },
        2000);
    gs::temporal::ZonedDatetimesParams later = new_york_2024();
    later.ambiguous = gs::temporal::Ambiguous::Later;
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(later),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.fold == (z.kind == gs::temporal::TransitionKind::Fold) &&
                   agrees_with_resolve(z);
        },
        2000);
    find_any<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(later),
        [](const gs::temporal::ZonedDateTime& z) { return z.fold; }, 2000);
    assert_all_examples<std::vector<gs::temporal::ZonedDateTime>>(
        gs::vectors(
            gs::temporal::zoned_datetimes(
                {.min_value = resolvable_min, .max_value = resolvable_max}),
            {.max_size = 3}),
        [](const std::vector<gs::temporal::ZonedDateTime>& v) {
            return std::all_of(v.begin(), v.end(), agrees_with_resolve);
        });
}

TEST(FindTemporal, ZonedDatetimesDrawFromAnyZoneGenerator) {
    std::vector<gs::temporal::TimeZone> zones = {
        zone("Europe/London"), zone("Asia/Calcutta"), zone("US/Eastern")};
    gs::Generator<gs::temporal::ZonedDateTime> drawn =
        gs::temporal::zoned_datetimes({.zones = gs::sampled_from(zones),
                                       .min_value = resolvable_min,
                                       .max_value = resolvable_max});
    assert_all_examples<gs::temporal::ZonedDateTime>(
        drawn, [&](const gs::temporal::ZonedDateTime& z) {
            return std::find(zones.begin(), zones.end(), z.zone) !=
                       zones.end() &&
                   agrees_with_resolve(z);
        });
    for (const gs::temporal::TimeZone& expected : zones) {
        find_any<gs::temporal::ZonedDateTime>(
            drawn, [&](const gs::temporal::ZonedDateTime& z) {
                return z.zone == expected;
            });
    }
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(
            {.zones = gs::temporal::zones().map(
                 [](gs::temporal::TimeZone z) { return z; }),
             .min_value = resolvable_min,
             .max_value = resolvable_max}),
        agrees_with_resolve);
}

TEST(FindTemporal, ZonedDatetimesFromAGeneratorReachTheGapAndBothFolds) {
    gs::temporal::ZonedDatetimesParams params =
        new_york_2024_from(gs::just(zone("America/New_York")));
    gs::temporal::ZonedDateTime gap = find_any<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(params),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.kind == gs::temporal::TransitionKind::Gap;
        },
        2000);
    EXPECT_EQ(gap.civil.date, (gs::temporal::Date{2024, 3, 10}));
    EXPECT_TRUE(agrees_with_resolve(gap));
    gs::temporal::ZonedDateTime earlier = find_any<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(params),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.kind == gs::temporal::TransitionKind::Fold && !z.fold;
        },
        2000);
    EXPECT_EQ(earlier.offset, gs::temporal::UtcOffset{-14'400});
    gs::temporal::ZonedDateTime later = find_any<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(params),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.kind == gs::temporal::TransitionKind::Fold && z.fold;
        },
        2000);
    EXPECT_EQ(later.offset, gs::temporal::UtcOffset{-18'000});
}

TEST(FindTemporal, ZonedDatetimesFromAGeneratorRespectBoundsAndPolicies) {
    gs::Generator<gs::temporal::TimeZone> new_york =
        gs::just(zone("America/New_York"));
    gs::temporal::Instant epoch{0, 0};
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(
            {.zones = new_york,
             .min_instant = epoch,
             .max_instant = gs::temporal::Instant{86'400, 0}}),
        [=](const gs::temporal::ZonedDateTime& z) {
            return z.instant >= epoch && z.instant.second <= 86'400 &&
                   agrees_with_resolve(z);
        });
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(
            {.zones = new_york, .precision = gs::temporal::Precision::Minutes}),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.civil.time.second == 0 && z.civil.time.nanosecond == 0;
        });
    gs::temporal::ZonedDatetimesParams avoid = new_york_2024_from(new_york);
    avoid.allow_imaginary = false;
    assert_no_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(avoid),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.kind == gs::temporal::TransitionKind::Gap;
        },
        2000);
    gs::temporal::ZonedDatetimesParams earlier = new_york_2024_from(new_york);
    earlier.ambiguous = gs::temporal::Ambiguous::Earlier;
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(earlier),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.civil >= datetime(2024, 1, 1, 0, 0, 0) &&
                   z.civil <= datetime(2024, 12, 31, 23, 59, 59) && !z.fold;
        },
        2000);
}

TEST(FindTemporal, ZonedDatetimesFromAGeneratorRefuseZonesTheSourceLacks) {
    gs::temporal::TimeZone mars{"Mars/Olympus", "Mars/Olympus"};
    try {
        find_any<gs::temporal::ZonedDateTime>(
            gs::temporal::zoned_datetimes({.zones = gs::just(mars)}),
            [](const gs::temporal::ZonedDateTime&) { return true; });
        ADD_FAILURE() << "a zone the source lacks was drawn";
    } catch (const std::invalid_argument& e) {
        EXPECT_NE(std::string(e.what()).find("\"Mars/Olympus\""),
                  std::string::npos)
            << e.what();
    }
    assert_all_examples<gs::temporal::ZonedDateTime>(
        gs::temporal::zoned_datetimes(
            {.zones = gs::just(zone(std::string(gs::temporal::UNKNOWN_ZONE)))}),
        [](const gs::temporal::ZonedDateTime& z) {
            return z.zone.name == gs::temporal::UNKNOWN_ZONE &&
                   z.offset == gs::temporal::UNKNOWN_OFFSET &&
                   z.kind == gs::temporal::TransitionKind::Unique;
        });
}

TEST(FindTemporal, DurationsRespectBoundsAndPrecision) {
    gs::temporal::SignedDuration zero{0, 0};
    assert_all_examples<gs::temporal::SignedDuration>(
        gs::temporal::durations({.min_value = zero}),
        [=](const gs::temporal::SignedDuration& d) { return d >= zero; });
    gs::temporal::SignedDuration max{60, 0};
    assert_all_examples<gs::temporal::SignedDuration>(
        gs::temporal::durations({.min_value = zero, .max_value = max}),
        [=](const gs::temporal::SignedDuration& d) {
            return d >= zero && d <= max;
        });
    assert_all_examples<gs::temporal::SignedDuration>(
        gs::temporal::durations(
            {.precision = gs::temporal::Precision::Seconds}),
        [](const gs::temporal::SignedDuration& d) {
            return d.nanoseconds == 0;
        });
    assert_all_examples<gs::temporal::SignedDuration>(
        gs::temporal::durations(), [](const gs::temporal::SignedDuration& d) {
            return d.nanoseconds > -1'000'000'000 &&
                   d.nanoseconds < 1'000'000'000 &&
                   (d.seconds == 0 || d.nanoseconds == 0 ||
                    (d.seconds < 0) == (d.nanoseconds < 0));
        });
    find_any<gs::temporal::SignedDuration>(
        gs::temporal::durations(),
        [](const gs::temporal::SignedDuration& d) { return d.seconds < 0; });
    find_any<gs::temporal::SignedDuration>(
        gs::temporal::durations(),
        [](const gs::temporal::SignedDuration& d) {
            return d.seconds == 86'400;
        },
        10'000);
}

namespace {
    bool uniform_sign(const gs::temporal::Span& s) {
        bool all_non_negative = true;
        bool all_non_positive = true;
        for (gs::temporal::SpanUnit unit : gs::temporal::ALL_SPAN_UNITS) {
            all_non_negative = all_non_negative && s.get(unit) >= 0;
            all_non_positive = all_non_positive && s.get(unit) <= 0;
        }
        return all_non_negative || all_non_positive;
    }
} // namespace

TEST(FindTemporal, SpansRespectUnitBoundsAndSign) {
    assert_all_examples<gs::temporal::Span>(gs::temporal::spans(),
                                            uniform_sign);
    find_any<gs::temporal::Span>(
        gs::temporal::spans(),
        [](const gs::temporal::Span& s) { return s.years < 0; });
    find_any<gs::temporal::Span>(
        gs::temporal::spans(),
        [](const gs::temporal::Span& s) { return s.nanoseconds > 0; });
    assert_all_examples<gs::temporal::Span>(
        gs::temporal::spans({.days = {{0, 30}}, .hours = {{-23, 23}}}),
        [](const gs::temporal::Span& s) {
            bool others_zero = true;
            for (gs::temporal::SpanUnit unit : gs::temporal::ALL_SPAN_UNITS) {
                if (unit != gs::temporal::SpanUnit::Days &&
                    unit != gs::temporal::SpanUnit::Hours) {
                    others_zero = others_zero && s.get(unit) == 0;
                }
            }
            return s.days >= 0 && s.days <= 30 && s.hours >= -23 &&
                   s.hours <= 23 && others_zero;
        });
    assert_all_examples<gs::temporal::Span>(
        gs::temporal::spans({.sign = gs::temporal::SpanSign::NonNegative}),
        [](const gs::temporal::Span& s) {
            for (gs::temporal::SpanUnit unit : gs::temporal::ALL_SPAN_UNITS) {
                if (s.get(unit) < 0) {
                    return false;
                }
            }
            return true;
        });
    find_any<gs::temporal::Span>(
        gs::temporal::spans({.sign = gs::temporal::SpanSign::Mixed}),
        [](const gs::temporal::Span& s) { return !uniform_sign(s); });
    assert_all_examples<gs::temporal::Span>(
        gs::temporal::spans(
            {.days = {{-30, 30}}, .sign = gs::temporal::SpanSign::Uniform}),
        uniform_sign);
    assert_all_examples<std::vector<gs::temporal::Span>>(
        gs::vectors(gs::temporal::spans({.days = {{0, 30}}}), {.max_size = 5}),
        [](const std::vector<gs::temporal::Span>& v) {
            return std::all_of(v.begin(), v.end(),
                               [](const gs::temporal::Span& s) {
                                   return s.days >= 0 && s.days <= 30;
                               });
        });
}
