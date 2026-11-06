// Date and time generators: the hegel::generators::temporal values, their
// printing and chrono conversions, the database queries, and construction /
// parameter validation of every generator in generators/temporal.h.

#include <gtest/gtest.h>

#include "common/utils.h"

#include <chrono>
#include <cstdint>
#include <limits>
#include <optional>
#include <ratio>
#include <regex>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

#include <hegel/hegel.h>
#include <hegel/repr.h>

namespace gs = hegel::generators;

using hegel::tests::common::assert_all_examples;
using hegel::tests::common::find_any;
using hegel::tests::common::minimal;

namespace {

    template <typename T> std::string show(const T& value) {
        std::ostringstream os;
        os << value;
        return os.str();
    }

    gs::temporal::DateTime datetime(int32_t year, uint8_t month, uint8_t day,
                                    uint8_t hour, uint8_t minute,
                                    uint8_t second, uint32_t nanosecond = 0) {
        return gs::temporal::DateTime{{year, month, day},
                                      {hour, minute, second, nanosecond}};
    }

} // namespace

TEST(TemporalValues, DatesAndTimesPrintInRfc9557Form) {
    EXPECT_EQ(show(gs::temporal::Date{2024, 3, 10}), "2024-03-10");
    EXPECT_EQ(show(gs::temporal::Date{1, 1, 1}), "0001-01-01");
    EXPECT_EQ(show(gs::temporal::Date{10000, 1, 1}), "+010000-01-01");
    EXPECT_EQ(show(gs::temporal::Date{-1, 12, 31}), "-000001-12-31");
    EXPECT_EQ(show(gs::temporal::Time{2, 30, 0, 0}), "02:30:00");
    EXPECT_EQ(show(gs::temporal::Time{2, 30, 0, 500'000'000}), "02:30:00.5");
    EXPECT_EQ(show(gs::temporal::Time{2, 30, 0, 1}), "02:30:00.000000001");
    EXPECT_EQ(show(datetime(2024, 3, 10, 2, 30, 0)), "2024-03-10T02:30:00");
}

TEST(TemporalValues, InstantsPrintAsUtc) {
    EXPECT_EQ(show(gs::temporal::Instant{946'684'800, 0}),
              "2000-01-01T00:00:00Z");
    EXPECT_EQ(show(gs::temporal::Instant{0, 0}), "1970-01-01T00:00:00Z");
    EXPECT_EQ(show(gs::temporal::Instant{-1, 0}), "1969-12-31T23:59:59Z");
    EXPECT_EQ(show(gs::temporal::Instant{-2, 500'000'000}),
              "1969-12-31T23:59:58.5Z");
    EXPECT_EQ(show(gs::temporal::Instant{gs::temporal::MIN_INSTANT, 0}),
              "-999999-01-01T00:00:00Z");
    EXPECT_EQ(show(gs::temporal::Instant{gs::temporal::MAX_INSTANT, 0}),
              "+999999-12-31T23:59:59Z");
}

TEST(TemporalValues, OffsetsPrintWithSecondsOnlyWhenNeeded) {
    EXPECT_EQ(show(gs::temporal::UtcOffset{0}), "+00:00");
    EXPECT_EQ(show(gs::temporal::UtcOffset{19'800}), "+05:30");
    EXPECT_EQ(show(gs::temporal::UtcOffset{-18'000}), "-05:00");
    EXPECT_EQ(show(gs::temporal::UtcOffset{-2'670}), "-00:44:30");
    EXPECT_EQ(show(gs::temporal::UNKNOWN_OFFSET), "-00:00");
    EXPECT_EQ(gs::temporal::UNKNOWN_OFFSET,
              (gs::temporal::UtcOffset{123, false}));
    EXPECT_NE(gs::temporal::UNKNOWN_OFFSET, gs::temporal::UtcOffset{0});
}

TEST(TemporalValues, DurationsPrintInIso8601Form) {
    EXPECT_EQ(show(gs::temporal::SignedDuration{0, 0}), "PT0S");
    EXPECT_EQ(show(gs::temporal::SignedDuration{5405, 500'000'000}),
              "PT1H30M5.5S");
    EXPECT_EQ(show(gs::temporal::SignedDuration{3600, 0}), "PT1H");
    EXPECT_EQ(show(gs::temporal::SignedDuration{-1, -500'000'000}), "-PT1.5S");
    EXPECT_EQ(show(gs::temporal::SignedDuration{0, -1}), "-PT0.000000001S");
    EXPECT_EQ(show(gs::temporal::SignedDuration{
                  std::numeric_limits<int64_t>::min(), 0}),
              "-PT2562047788015215H30M8S");
}

TEST(TemporalValues, SpansPrintInFriendlyForm) {
    EXPECT_EQ(show(gs::temporal::Span{}), "0s");
    gs::temporal::Span every{1, 2, 3, 4, 5, 6, 7, 8, 9, 10};
    EXPECT_EQ(show(every), "1y 2mo 3w 4d 5h 6m 7s 8ms 9µs 10ns");
    gs::temporal::Span negative;
    negative.years = -1;
    negative.months = -2;
    EXPECT_EQ(show(negative), "-1y 2mo");
    gs::temporal::Span mixed;
    mixed.years = 1;
    mixed.months = -2;
    EXPECT_EQ(show(mixed), "1y -2mo");
    gs::temporal::Span lowest;
    lowest.nanoseconds = std::numeric_limits<int64_t>::min();
    EXPECT_EQ(show(lowest), "-9223372036854775808ns");
    for (size_t i = 0; i < gs::temporal::ALL_SPAN_UNITS.size(); ++i) {
        EXPECT_EQ(every.get(gs::temporal::ALL_SPAN_UNITS[i]),
                  static_cast<int64_t>(i + 1));
    }
    EXPECT_NE(every, mixed);
}

TEST(TemporalValues, ZonedDatetimesPrintAsTheEngineDoes) {
    gs::temporal::ZonedDateTime gap = gs::temporal::resolve(
        "America/New_York", datetime(2024, 3, 10, 2, 30, 0));
    EXPECT_EQ(gap.kind, gs::temporal::TransitionKind::Gap);
    EXPECT_EQ(gap.offset_before, gs::temporal::UtcOffset{-18'000});
    EXPECT_EQ(gap.offset_after, gs::temporal::UtcOffset{-14'400});
    EXPECT_EQ(gap.offset, gap.offset_after);
    EXPECT_EQ(show(gap), "2024-03-10T02:30:00-04:00[America/New_York][gap]");
    EXPECT_EQ(show(gap.instant), "2024-03-10T07:30:00Z");

    gs::temporal::ZonedDateTime fold = gs::temporal::resolve(
        "America/New_York", datetime(2024, 11, 3, 1, 30, 0));
    EXPECT_EQ(fold.kind, gs::temporal::TransitionKind::Fold);
    EXPECT_FALSE(fold.fold);
    EXPECT_EQ(show(fold),
              "2024-11-03T01:30:00-04:00[America/New_York][fold=0]");
    gs::temporal::ZonedDateTime later = fold;
    later.fold = true;
    later.offset = fold.offset_after;
    EXPECT_EQ(show(later),
              "2024-11-03T01:30:00-05:00[America/New_York][fold=1]");
    EXPECT_NE(later, fold);

    gs::temporal::ZonedDateTime unique = gs::temporal::resolve(
        "America/New_York", datetime(2024, 6, 1, 12, 0, 0));
    EXPECT_EQ(unique.kind, gs::temporal::TransitionKind::Unique);
    EXPECT_EQ(unique.offset, gs::temporal::UtcOffset{-14'400});
    EXPECT_EQ(show(unique), "2024-06-01T12:00:00-04:00[America/New_York]");

    gs::temporal::ZonedDateTime unknown = gs::temporal::resolve(
        gs::temporal::UNKNOWN_ZONE, datetime(2024, 6, 1, 12, 0, 0));
    EXPECT_EQ(unknown.offset, gs::temporal::UNKNOWN_OFFSET);
    EXPECT_EQ(show(unknown), "2024-06-01T12:00:00Z[Etc/Unknown]");
    EXPECT_EQ(show(unknown.instant), "2024-06-01T12:00:00Z");

    gs::temporal::ZonedDateTime opaque = unknown;
    opaque.zone = gs::temporal::TimeZone{"Mars/Olympus", "Mars/Olympus"};
    EXPECT_EQ(show(opaque), "2024-06-01T12:00:00-00:00[Mars/Olympus]");

    gs::temporal::ZonedDateTime calcutta =
        gs::temporal::resolve("Asia/Calcutta", datetime(2024, 6, 1, 12, 0, 0));
    EXPECT_EQ(calcutta.zone.primary_name, "Asia/Kolkata");
    EXPECT_EQ(show(calcutta.zone), "Asia/Calcutta");
    EXPECT_EQ(show(calcutta), "2024-06-01T12:00:00+05:30[Asia/Calcutta]");
}

TEST(TemporalValues, ResolveRejectsBadInput) {
    EXPECT_THROW(
        gs::temporal::resolve("Mars/Olympus", datetime(2024, 6, 1, 12, 0, 0)),
        std::invalid_argument);
    EXPECT_THROW(gs::temporal::resolve("UTC", datetime(2024, 2, 30, 0, 0, 0)),
                 std::invalid_argument);
    gs::temporal::DateTime last =
        datetime(gs::temporal::MAX_YEAR, 12, 31, 23, 59, 59);
    EXPECT_THROW(gs::temporal::resolve("America/New_York", last),
                 std::invalid_argument);
    EXPECT_EQ(gs::temporal::resolve("UTC", last).instant,
              (gs::temporal::Instant{gs::temporal::MAX_INSTANT, 0}));
}

TEST(TemporalValues, OffsetAtReportsTheOffsetInForce) {
    gs::temporal::ZoneOffset summer = gs::temporal::offset_at(
        "Europe/London", gs::temporal::Instant{1'717'243'200, 0});
    EXPECT_EQ(summer.offset, gs::temporal::UtcOffset{3600});
    EXPECT_TRUE(summer.is_dst);
    EXPECT_EQ(summer.abbreviation, "BST");
    gs::temporal::ZoneOffset winter = gs::temporal::offset_at(
        "Europe/London", gs::temporal::Instant{1'704'067'200, 0});
    EXPECT_EQ(winter.offset, gs::temporal::UtcOffset{0});
    EXPECT_FALSE(winter.is_dst);
    EXPECT_EQ(winter.abbreviation, "GMT");
    EXPECT_NE(summer, winter);
    gs::temporal::ZoneOffset unknown = gs::temporal::offset_at(
        gs::temporal::UNKNOWN_ZONE, gs::temporal::Instant{0, 0});
    EXPECT_EQ(unknown.offset, gs::temporal::UNKNOWN_OFFSET);
    EXPECT_EQ(unknown.abbreviation, "-00");
    EXPECT_THROW(
        gs::temporal::offset_at("Mars/Olympus", gs::temporal::Instant{0, 0}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::offset_at("UTC", gs::temporal::Instant{0, 1'000'000'000}),
        std::invalid_argument);
}

TEST(TemporalValues, InfoDescribesNames) {
    std::optional<gs::temporal::ZoneInfo> utc = gs::temporal::info("UTC");
    ASSERT_TRUE(utc.has_value());
    EXPECT_EQ(utc->primary_name, "UTC");
    EXPECT_FALSE(utc->is_link);
    EXPECT_EQ(utc->rank, 0);
    EXPECT_NE(
        std::find(utc->tags.begin(), utc->tags.end(), gs::temporal::Tag::Fixed),
        utc->tags.end());
    std::optional<gs::temporal::ZoneInfo> calcutta =
        gs::temporal::info("Asia/Calcutta");
    ASSERT_TRUE(calcutta.has_value());
    EXPECT_EQ(calcutta->primary_name, "Asia/Kolkata");
    EXPECT_TRUE(calcutta->is_link);
    EXPECT_NE(std::find(calcutta->tags.begin(), calcutta->tags.end(),
                        gs::temporal::Tag::Link),
              calcutta->tags.end());
    EXPECT_NE(std::find(calcutta->tags.begin(), calcutta->tags.end(),
                        gs::temporal::Tag::Fractional),
              calcutta->tags.end());
    EXPECT_NE(*utc, *calcutta);
    std::optional<gs::temporal::ZoneInfo> unknown =
        gs::temporal::info(gs::temporal::UNKNOWN_ZONE);
    ASSERT_TRUE(unknown.has_value());
    EXPECT_EQ(unknown->primary_name, gs::temporal::UNKNOWN_ZONE);
    EXPECT_EQ(unknown->tags,
              std::vector<gs::temporal::Tag>{gs::temporal::Tag::Undefined});
    EXPECT_EQ(unknown->rank, std::numeric_limits<uint16_t>::max());
    EXPECT_FALSE(gs::temporal::info("Mars/Olympus").has_value());
    EXPECT_FALSE(gs::temporal::info(std::string("UTC\0", 4)).has_value());
}

TEST(TemporalValues, TzdbVersionIsAnIanaRelease) {
    std::string version = gs::temporal::tzdb_version();
    ASSERT_EQ(version.size(), 5u) << version;
    for (size_t i = 0; i < 4; ++i) {
        EXPECT_TRUE(std::isdigit(static_cast<unsigned char>(version[i])))
            << version;
    }
    EXPECT_TRUE(std::islower(static_cast<unsigned char>(version[4])))
        << version;
}

TEST(TemporalValues, ComparisonsFollowTheFields) {
    EXPECT_LT((gs::temporal::Date{2024, 3, 10}),
              (gs::temporal::Date{2024, 3, 11}));
    EXPECT_LT((gs::temporal::Date{2024, 2, 28}),
              (gs::temporal::Date{2024, 3, 1}));
    EXPECT_LT((gs::temporal::Date{2023, 12, 31}),
              (gs::temporal::Date{2024, 1, 1}));
    EXPECT_GT((gs::temporal::Date{2024, 3, 11}),
              (gs::temporal::Date{2024, 3, 10}));
    EXPECT_LE((gs::temporal::Date{2024, 3, 10}),
              (gs::temporal::Date{2024, 3, 10}));
    EXPECT_GE((gs::temporal::Date{2024, 3, 10}),
              (gs::temporal::Date{2024, 3, 10}));
    EXPECT_NE((gs::temporal::Date{2024, 3, 10}),
              (gs::temporal::Date{2024, 3, 11}));

    EXPECT_LT((gs::temporal::Time{1, 0, 0, 0}),
              (gs::temporal::Time{2, 0, 0, 0}));
    EXPECT_LT((gs::temporal::Time{1, 0, 0, 0}),
              (gs::temporal::Time{1, 1, 0, 0}));
    EXPECT_LT((gs::temporal::Time{1, 0, 0, 0}),
              (gs::temporal::Time{1, 0, 1, 0}));
    EXPECT_LT((gs::temporal::Time{1, 0, 0, 0}),
              (gs::temporal::Time{1, 0, 0, 1}));
    EXPECT_GT((gs::temporal::Time{2, 0, 0, 0}),
              (gs::temporal::Time{1, 0, 0, 0}));
    EXPECT_LE((gs::temporal::Time{1, 0, 0, 0}),
              (gs::temporal::Time{1, 0, 0, 0}));
    EXPECT_GE((gs::temporal::Time{1, 0, 0, 0}),
              (gs::temporal::Time{1, 0, 0, 0}));
    EXPECT_NE((gs::temporal::Time{1, 0, 0, 0}),
              (gs::temporal::Time{1, 0, 0, 1}));

    gs::temporal::DateTime a = datetime(2024, 1, 1, 0, 0, 0);
    gs::temporal::DateTime b = datetime(2024, 1, 1, 0, 0, 1);
    gs::temporal::DateTime c = datetime(2024, 1, 2, 0, 0, 0);
    EXPECT_LT(a, b);
    EXPECT_LT(b, c);
    EXPECT_GT(c, a);
    EXPECT_LE(a, a);
    EXPECT_GE(a, a);
    EXPECT_NE(a, b);

    EXPECT_LT((gs::temporal::Instant{0, 0}), (gs::temporal::Instant{0, 1}));
    EXPECT_LT((gs::temporal::Instant{-1, 0}), (gs::temporal::Instant{0, 0}));
    EXPECT_GT((gs::temporal::Instant{0, 1}), (gs::temporal::Instant{0, 0}));
    EXPECT_LE((gs::temporal::Instant{0, 0}), (gs::temporal::Instant{0, 0}));
    EXPECT_GE((gs::temporal::Instant{0, 0}), (gs::temporal::Instant{0, 0}));
    EXPECT_NE((gs::temporal::Instant{0, 0}), (gs::temporal::Instant{0, 1}));

    EXPECT_LT((gs::temporal::SignedDuration{0, 0}),
              (gs::temporal::SignedDuration{0, 1}));
    EXPECT_LT((gs::temporal::SignedDuration{-1, 0}),
              (gs::temporal::SignedDuration{0, 0}));
    EXPECT_GT((gs::temporal::SignedDuration{0, 1}),
              (gs::temporal::SignedDuration{0, 0}));
    EXPECT_LE((gs::temporal::SignedDuration{0, 0}),
              (gs::temporal::SignedDuration{0, 0}));
    EXPECT_GE((gs::temporal::SignedDuration{0, 0}),
              (gs::temporal::SignedDuration{0, 0}));
    EXPECT_NE((gs::temporal::SignedDuration{0, 0}),
              (gs::temporal::SignedDuration{0, 1}));

    EXPECT_NE((gs::temporal::TimeZone{"UTC", "UTC"}),
              (gs::temporal::TimeZone{"Etc/UTC", "UTC"}));
}

TEST(TemporalChrono, DurationsConvertToAndFromChrono) {
    using namespace std::chrono;
    gs::temporal::SignedDuration d{5405, 500'000'000};
    EXPECT_EQ(d.to_chrono(), nanoseconds{5'405'500'000'000});
    EXPECT_EQ(d.to_chrono<milliseconds>(), milliseconds{5'405'500});
    EXPECT_EQ(d.to_chrono<seconds>(), std::nullopt);
    EXPECT_EQ((gs::temporal::SignedDuration{5405, 0}).to_chrono<seconds>(),
              seconds{5405});
    EXPECT_EQ((gs::temporal::SignedDuration{5400, 0}).to_chrono<minutes>(),
              minutes{90});
    EXPECT_EQ((gs::temporal::SignedDuration{5400, 0}).to_chrono<hours>(),
              std::nullopt);
    EXPECT_EQ((gs::temporal::SignedDuration{-7200, 0}).to_chrono<hours>(),
              hours{-2});
    EXPECT_EQ((gs::temporal::SignedDuration{-1, -500'000'000}).to_chrono(),
              nanoseconds{-1'500'000'000});
    EXPECT_EQ((gs::temporal::SignedDuration{-1, -500'000'000})
                  .to_chrono<milliseconds>(),
              milliseconds{-1500});
    EXPECT_EQ(
        (gs::temporal::SignedDuration{std::numeric_limits<int64_t>::max(), 0})
            .to_chrono(),
        std::nullopt);
    EXPECT_EQ(
        (gs::temporal::SignedDuration{std::numeric_limits<int64_t>::min(), 0})
            .to_chrono(),
        std::nullopt);
    EXPECT_EQ(
        (gs::temporal::SignedDuration{9'223'372'036, 854'775'807}).to_chrono(),
        nanoseconds{std::numeric_limits<int64_t>::max()});
    EXPECT_EQ(
        (gs::temporal::SignedDuration{9'223'372'036, 854'775'808}).to_chrono(),
        std::nullopt);
    EXPECT_EQ((gs::temporal::SignedDuration{-9'223'372'036, -854'775'808})
                  .to_chrono(),
              nanoseconds{std::numeric_limits<int64_t>::min()});
    EXPECT_EQ((gs::temporal::SignedDuration{-9'223'372'036, -854'775'809})
                  .to_chrono(),
              std::nullopt);
    EXPECT_EQ(
        (gs::temporal::SignedDuration{std::numeric_limits<int64_t>::max(), 0})
            .to_chrono<seconds>(),
        seconds{std::numeric_limits<int64_t>::max()});
    EXPECT_EQ(((gs::temporal::SignedDuration{1, 0})
                   .to_chrono<duration<int8_t, std::nano>>()),
              std::nullopt);
    using unsigned_seconds = duration<uint64_t>;
    EXPECT_EQ(
        (gs::temporal::SignedDuration{-1, 0}).to_chrono<unsigned_seconds>(),
        std::nullopt);
    EXPECT_EQ(
        (gs::temporal::SignedDuration{1, 0}).to_chrono<unsigned_seconds>(),
        unsigned_seconds{1});

    EXPECT_EQ(gs::temporal::SignedDuration::from_chrono(
                  nanoseconds{5'405'500'000'000}),
              d);
    EXPECT_EQ(gs::temporal::SignedDuration::from_chrono(milliseconds{-1500}),
              (gs::temporal::SignedDuration{-1, -500'000'000}));
    EXPECT_EQ(gs::temporal::SignedDuration::from_chrono(hours{-2}),
              (gs::temporal::SignedDuration{-7200, 0}));
    EXPECT_EQ(gs::temporal::SignedDuration::from_chrono(hours::max()),
              std::nullopt);
    EXPECT_EQ(gs::temporal::SignedDuration::from_chrono(hours::min()),
              std::nullopt);
    EXPECT_EQ(gs::temporal::SignedDuration::from_chrono(unsigned_seconds{5}),
              (gs::temporal::SignedDuration{5, 0}));
    EXPECT_EQ(
        gs::temporal::SignedDuration::from_chrono(unsigned_seconds::max()),
        std::nullopt);
}

TEST(TemporalChrono, InstantsConvertToAndFromChrono) {
    using namespace std::chrono;
    using sys_ns = time_point<system_clock, nanoseconds>;
    gs::temporal::Instant epoch{0, 0};
    EXPECT_EQ(epoch.to_chrono(), sys_ns{nanoseconds{0}});
    gs::temporal::Instant before{-2, 500'000'000};
    EXPECT_EQ(before.to_chrono(), sys_ns{nanoseconds{-1'500'000'000}});
    EXPECT_EQ(before.to_chrono<milliseconds>(),
              (time_point<system_clock, milliseconds>{milliseconds{-1500}}));
    EXPECT_EQ(before.to_chrono<seconds>(), std::nullopt);
    EXPECT_EQ((gs::temporal::Instant{gs::temporal::MAX_INSTANT, 0}).to_chrono(),
              std::nullopt);
    EXPECT_EQ(
        gs::temporal::Instant::from_chrono(sys_ns{nanoseconds{-1'500'000'000}}),
        before);
    EXPECT_EQ(gs::temporal::Instant::from_chrono(
                  time_point<system_clock, seconds>{seconds{-2}}),
              (gs::temporal::Instant{-2, 0}));
    EXPECT_EQ(gs::temporal::Instant::from_chrono(
                  time_point<system_clock, hours>{hours{1}}),
              (gs::temporal::Instant{3600, 0}));
    EXPECT_EQ(
        gs::temporal::Instant::from_chrono(time_point<system_clock, seconds>{
            seconds{gs::temporal::MAX_INSTANT + 1}}),
        std::nullopt);
    EXPECT_EQ(gs::temporal::Instant::from_chrono(
                  time_point<system_clock, hours>{hours::max()}),
              std::nullopt);
}

#if HEGEL_HAS_CHRONO_CALENDAR
TEST(TemporalChrono, DatesConvertToAndFromYearMonthDay) {
    using namespace std::chrono;
    std::optional<year_month_day> ymd =
        gs::temporal::Date{2024, 3, 10}.to_chrono();
    ASSERT_TRUE(ymd.has_value());
    EXPECT_EQ(*ymd, (year_month_day{year{2024}, month{3}, day{10}}));
    EXPECT_EQ((gs::temporal::Date{100'000, 1, 1}).to_chrono(), std::nullopt);
    EXPECT_EQ((gs::temporal::Date{-100'000, 1, 1}).to_chrono(), std::nullopt);
    EXPECT_EQ(gs::temporal::Date::from_chrono(*ymd),
              (gs::temporal::Date{2024, 3, 10}));
    EXPECT_EQ(
        gs::temporal::Date::from_chrono(year_month_day{sys_days{days{19'792}}}),
        (gs::temporal::Date{2024, 3, 10}));
    EXPECT_EQ(gs::temporal::Date::from_chrono(
                  year_month_day{year{2023}, month{2}, day{29}}),
              std::nullopt);
}
#endif

TEST(TemporalRepr, ValuesRenderInTheEngineForm) {
    using hegel::internal::cpp_type_name;
    using hegel::internal::repr;
    EXPECT_EQ(repr(gs::temporal::Date{2024, 3, 10}), "2024-03-10");
    EXPECT_EQ(repr(gs::temporal::Time{2, 30, 0, 0}), "02:30:00");
    EXPECT_EQ(repr(datetime(2024, 3, 10, 2, 30, 0)), "2024-03-10T02:30:00");
    EXPECT_EQ(repr(gs::temporal::Instant{0, 0}), "1970-01-01T00:00:00Z");
    EXPECT_EQ(repr(gs::temporal::UtcOffset{19'800}), "+05:30");
    EXPECT_EQ(repr(gs::temporal::TimeZone{"UTC", "UTC"}), "UTC");
    EXPECT_EQ(repr(gs::temporal::resolve("UTC", datetime(2000, 1, 1, 0, 0, 0))),
              "2000-01-01T00:00:00+00:00[UTC]");
    EXPECT_EQ(repr(gs::temporal::SignedDuration{90, 0}), "PT1M30S");
    EXPECT_EQ(repr(gs::temporal::Span{}), "0s");
    EXPECT_EQ(repr(std::optional<gs::temporal::Date>{
                  gs::temporal::Date{2024, 3, 10}}),
              "std::optional<hegel::generators::temporal::Date>{2024-03-10}");
    EXPECT_EQ(
        repr(std::vector<gs::temporal::Date>{gs::temporal::Date{2024, 3, 10}}),
        "std::vector<hegel::generators::temporal::Date>{2024-03-10}");

    EXPECT_EQ(cpp_type_name<std::chrono::nanoseconds>(),
              "std::chrono::nanoseconds");
    EXPECT_EQ(cpp_type_name<std::chrono::microseconds>(),
              "std::chrono::microseconds");
    EXPECT_EQ(cpp_type_name<std::chrono::milliseconds>(),
              "std::chrono::milliseconds");
    EXPECT_EQ(cpp_type_name<std::chrono::seconds>(), "std::chrono::seconds");
    EXPECT_EQ(cpp_type_name<std::chrono::minutes>(), "std::chrono::minutes");
    EXPECT_EQ(cpp_type_name<std::chrono::hours>(), "std::chrono::hours");
    EXPECT_EQ((cpp_type_name<std::chrono::duration<int, std::ratio<1, 100>>>()),
              "std::chrono::duration<int, std::ratio<1, 100>>");
    EXPECT_EQ(repr(std::chrono::milliseconds{1500}),
              "std::chrono::milliseconds{" +
                  repr(std::chrono::milliseconds::rep{1500}) + "}");
    EXPECT_EQ((repr(std::chrono::duration<int, std::ratio<1, 100>>{7})),
              "std::chrono::duration<int, std::ratio<1, 100>>{7}");
}

TEST(TemporalGenerators, DrawAll) {
    hegel::test(
        [](hegel::TestCase& tc) {
            (void)tc.draw(gs::temporal::dates());
            (void)tc.draw(gs::temporal::dates(
                {.min_value = gs::temporal::Date{2024, 1, 1},
                 .max_value = gs::temporal::Date{2024, 12, 31},
                 .weekdays = {{gs::temporal::Weekday::Monday}},
                 .months = {{gs::temporal::Month::March}}}));
            (void)tc.draw(gs::temporal::times());
            (void)tc.draw(gs::temporal::times(
                {.min_value = gs::temporal::Time{1, 0, 0, 0},
                 .max_value = gs::temporal::Time{2, 0, 0, 0},
                 .precision = gs::temporal::Precision::Minutes}));
            (void)tc.draw(gs::temporal::datetimes());
            (void)tc.draw(gs::temporal::datetimes(
                {.precision = gs::temporal::Precision::Hours}));
            (void)tc.draw(gs::temporal::instants());
            (void)tc.draw(gs::temporal::instants(
                {.min_value = gs::temporal::Instant{0, 0},
                 .precision = gs::temporal::Precision::Seconds}));
            (void)tc.draw(gs::temporal::offsets());
            (void)tc.draw(gs::temporal::offsets(
                {.min_value = gs::temporal::UtcOffset{-3600},
                 .max_value = gs::temporal::UtcOffset{3600},
                 .granularity = gs::temporal::Granularity::QuarterHours,
                 .unknown = true}));
            (void)tc.draw(gs::temporal::zones());
            (void)tc.draw(
                gs::temporal::zones({.links = false, .legacy = true}));
            (void)tc.draw(gs::temporal::zones(
                {.names = std::vector<std::string>{"Mars/Olympus"},
                 .opaque = true}));
            (void)tc.draw(gs::temporal::zoned_datetimes());
            (void)tc.draw(gs::temporal::zoned_datetimes(
                {.zones = gs::temporal::zones(
                     {.names = std::vector<std::string>{"Europe/London"}}),
                 .min_instant = gs::temporal::Instant{0, 0},
                 .max_instant = gs::temporal::Instant{86'400, 0},
                 .precision = gs::temporal::Precision::Seconds,
                 .allow_imaginary = false,
                 .ambiguous = gs::temporal::Ambiguous::Later}));
            (void)tc.draw(gs::temporal::durations());
            (void)tc.draw(
                gs::temporal::durations<gs::temporal::SignedDuration>());
            (void)tc.draw(gs::temporal::durations(
                {.min_value = gs::temporal::SignedDuration{0, 0},
                 .precision = gs::temporal::Precision::Milliseconds}));
            (void)tc.draw(gs::temporal::spans());
            (void)tc.draw(gs::temporal::spans(
                {.days = {{0, 30}},
                 .sign = gs::temporal::SpanSign::NonNegative}));
            (void)tc.draw(gs::temporal::durations<std::chrono::nanoseconds>());
            (void)tc.draw(gs::temporal::durations<std::chrono::seconds>(
                {.min_value = std::chrono::seconds{-10},
                 .max_value = std::chrono::seconds{10}}));
        },
        hegel::Settings{.test_cases = 20,
                        .database = hegel::Database::disabled()});
}

TEST(TemporalGenerators, DatesValidateTheirParameters) {
    EXPECT_THROW(
        gs::temporal::dates({.min_value = gs::temporal::Date{2024, 2, 30}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::dates({.max_value = gs::temporal::Date{2024, 13, 1}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::dates({.max_value = gs::temporal::Date{2024, 1, 0}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::dates(
            {.min_value =
                 gs::temporal::Date{gs::temporal::MAX_YEAR + 1, 1, 1}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::dates({.min_value = gs::temporal::Date{2024, 1, 2},
                             .max_value = gs::temporal::Date{2024, 1, 1}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::dates({.weekdays = std::vector<gs::temporal::Weekday>{}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::dates({.months = std::vector<gs::temporal::Month>{}}),
        std::invalid_argument);
    EXPECT_NO_THROW(
        gs::temporal::dates({.min_value = gs::temporal::Date{2024, 2, 29}}));
    EXPECT_NO_THROW(
        gs::temporal::dates({.min_value = gs::temporal::Date{2000, 2, 29}}));
    EXPECT_THROW(
        gs::temporal::dates({.min_value = gs::temporal::Date{1900, 2, 29}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::dates({.min_value = gs::temporal::Date{2024, 4, 31}}),
        std::invalid_argument);
    EXPECT_NO_THROW(
        gs::temporal::dates({.min_value = gs::temporal::Date{2024, 1, 31}}));
    EXPECT_NO_THROW(gs::temporal::dates(
        {.weekdays = {
             {gs::temporal::Weekday::Monday, gs::temporal::Weekday::Tuesday,
              gs::temporal::Weekday::Wednesday, gs::temporal::Weekday::Thursday,
              gs::temporal::Weekday::Friday, gs::temporal::Weekday::Saturday,
              gs::temporal::Weekday::Sunday}}}));
    auto no_allowed_day =
        gs::temporal::dates({.min_value = gs::temporal::Date{2024, 1, 1},
                             .max_value = gs::temporal::Date{2024, 1, 31},
                             .months = {{gs::temporal::Month::March}}});
    EXPECT_THROW(
        find_any<gs::temporal::Date>(
            no_allowed_day, [](const gs::temporal::Date&) { return true; }),
        std::invalid_argument);
}

TEST(TemporalGenerators, TimesValidateTheirParameters) {
    EXPECT_THROW(
        gs::temporal::times({.min_value = gs::temporal::Time{24, 0, 0, 0}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::times({.max_value = gs::temporal::Time{0, 60, 0, 0}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::times({.max_value = gs::temporal::Time{0, 0, 60, 0}}),
        std::invalid_argument);
    EXPECT_THROW(gs::temporal::times(
                     {.max_value = gs::temporal::Time{0, 0, 0, 1'000'000'000}}),
                 std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::times({.min_value = gs::temporal::Time{1, 0, 0, 0},
                             .max_value = gs::temporal::Time{0, 0, 0, 0}}),
        std::invalid_argument);
    auto no_whole_minute =
        gs::temporal::times({.min_value = gs::temporal::Time{1, 0, 1, 0},
                             .max_value = gs::temporal::Time{1, 0, 59, 0},
                             .precision = gs::temporal::Precision::Minutes});
    EXPECT_THROW(
        find_any<gs::temporal::Time>(
            no_whole_minute, [](const gs::temporal::Time&) { return true; }),
        std::invalid_argument);
}

TEST(TemporalGenerators, DatetimesValidateTheirParameters) {
    EXPECT_THROW(
        gs::temporal::datetimes({.min_value = datetime(2024, 2, 30, 0, 0, 0)}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::datetimes({.max_value = datetime(2024, 1, 1, 25, 0, 0)}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::datetimes({.min_value = datetime(2024, 1, 2, 0, 0, 0),
                                 .max_value = datetime(2024, 1, 1, 0, 0, 0)}),
        std::invalid_argument);
}

TEST(TemporalGenerators, InstantsValidateTheirParameters) {
    EXPECT_THROW(
        gs::temporal::instants(
            {.min_value =
                 gs::temporal::Instant{gs::temporal::MIN_INSTANT - 1, 0}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::instants(
            {.max_value =
                 gs::temporal::Instant{gs::temporal::MAX_INSTANT + 1, 0}}),
        std::invalid_argument);
    EXPECT_THROW(gs::temporal::instants(
                     {.max_value = gs::temporal::Instant{0, 1'000'000'000}}),
                 std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::instants({.min_value = gs::temporal::Instant{1, 0},
                                .max_value = gs::temporal::Instant{0, 0}}),
        std::invalid_argument);
}

TEST(TemporalGenerators, OffsetsValidateTheirParameters) {
    EXPECT_THROW(
        gs::temporal::offsets({.min_value = gs::temporal::UNKNOWN_OFFSET}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::offsets({.max_value = gs::temporal::UNKNOWN_OFFSET}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::offsets(
            {.min_value =
                 gs::temporal::UtcOffset{-gs::temporal::MAX_OFFSET - 1}}),
        std::invalid_argument);
    EXPECT_THROW(gs::temporal::offsets({.max_value =
                                            gs::temporal::UtcOffset{
                                                gs::temporal::MAX_OFFSET + 1}}),
                 std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::offsets({.min_value = gs::temporal::UtcOffset{1},
                               .max_value = gs::temporal::UtcOffset{0}}),
        std::invalid_argument);
    auto no_whole_hour = gs::temporal::offsets(
        {.min_value = gs::temporal::UtcOffset{1},
         .max_value = gs::temporal::UtcOffset{3599},
         .granularity = gs::temporal::Granularity::Hours});
    EXPECT_THROW(
        find_any<gs::temporal::UtcOffset>(
            no_whole_hour, [](const gs::temporal::UtcOffset&) { return true; }),
        std::invalid_argument);
}

TEST(TemporalGenerators, ZonesValidateTheirParameters) {
    EXPECT_THROW(gs::temporal::zones({.names = std::vector<std::string>{}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::temporal::zones(
                     {.names = std::vector<std::string>{"Mars/Olympus"}}),
                 std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::zones({.names = std::vector<std::string>{"utc"}}),
        std::invalid_argument);
    EXPECT_NO_THROW(gs::temporal::zones(
        {.names = std::vector<std::string>{
             "UTC", std::string(gs::temporal::UNKNOWN_ZONE)}}));
}

TEST(TemporalGenerators, ZonedDatetimesValidateTheirParameters) {
    EXPECT_THROW(gs::temporal::zoned_datetimes(
                     {.min_value = datetime(2024, 1, 1, 0, 0, 0),
                      .min_instant = gs::temporal::Instant{0, 0}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::temporal::zoned_datetimes(
                     {.max_value = datetime(2024, 1, 1, 0, 0, 0),
                      .max_instant = gs::temporal::Instant{0, 0}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::temporal::zoned_datetimes(
                     {.min_value = datetime(2024, 1, 2, 0, 0, 0),
                      .max_value = datetime(2024, 1, 1, 0, 0, 0)}),
                 std::invalid_argument);
    EXPECT_THROW(gs::temporal::zoned_datetimes(
                     {.min_value = datetime(2024, 2, 30, 0, 0, 0)}),
                 std::invalid_argument);
    EXPECT_THROW(gs::temporal::zoned_datetimes(
                     {.min_instant = gs::temporal::Instant{1, 0},
                      .max_instant = gs::temporal::Instant{0, 0}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::temporal::zoned_datetimes(
                     {.min_instant = gs::temporal::Instant{0, 1'000'000'000}}),
                 std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::zoned_datetimes(
            {.zones = gs::temporal::zones(
                 {.names = std::vector<std::string>{"Mars/Olympus"}})}),
        std::invalid_argument);
    EXPECT_THROW(gs::temporal::zoned_datetimes(
                     {.min_value = datetime(2024, 1, 1, 0, 0, 1),
                      .max_value = datetime(2024, 1, 1, 0, 0, 59),
                      .precision = gs::temporal::Precision::Minutes}),
                 std::invalid_argument);
}

TEST(TemporalGenerators, DurationsValidateTheirParameters) {
    EXPECT_THROW(gs::temporal::durations(
                     {.min_value = gs::temporal::SignedDuration{1, -1}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::temporal::durations(
                     {.max_value = gs::temporal::SignedDuration{-1, 1}}),
                 std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::durations(
            {.max_value = gs::temporal::SignedDuration{0, 1'000'000'000}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::durations(
            {.min_value = gs::temporal::SignedDuration{0, -1'000'000'000}}),
        std::invalid_argument);
    EXPECT_THROW(gs::temporal::durations(
                     {.min_value = gs::temporal::SignedDuration{1, 0},
                      .max_value = gs::temporal::SignedDuration{0, 0}}),
                 std::invalid_argument);
    EXPECT_NO_THROW(gs::temporal::durations(
        {.min_value = gs::temporal::SignedDuration{0, -5},
         .max_value = gs::temporal::SignedDuration{0, 5}}));
}

TEST(TemporalGenerators, SpansValidateTheirParameters) {
    EXPECT_THROW(gs::temporal::spans({.days = {{30, 0}}}),
                 std::invalid_argument);
    EXPECT_THROW(
        gs::temporal::spans(
            {.days = {{-1, 30}}, .sign = gs::temporal::SpanSign::NonNegative}),
        std::invalid_argument);
    EXPECT_THROW(gs::temporal::spans({.days = {{1, 30}}, .hours = {{-30, -1}}}),
                 std::invalid_argument);
    EXPECT_NO_THROW(
        gs::temporal::spans({.days = {{1, 30}},
                             .hours = {{-30, -1}},
                             .sign = gs::temporal::SpanSign::Mixed}));
}

TEST(TemporalGenerators, ChronoDurationsValidateTheirParameters) {
    using namespace std::chrono;
    EXPECT_THROW(gs::temporal::durations<seconds>(
                     {.min_value = seconds{1}, .max_value = seconds{0}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::temporal::durations<hours>({.min_value = hours::min()}),
                 std::invalid_argument);
    EXPECT_THROW(gs::temporal::durations<hours>({.max_value = hours::max()}),
                 std::invalid_argument);
    EXPECT_NO_THROW(gs::temporal::durations<hours>());
    EXPECT_NO_THROW((gs::temporal::durations<duration<int32_t, std::milli>>()));
}

TEST(TemporalGenerators,
     ChronoDurationsDrawWholeUnitsWithinTheTypeAndTheEngine) {
    using namespace std::chrono;
    assert_all_examples<minutes>(gs::temporal::durations<minutes>(),
                                 [](const minutes&) { return true; });
    assert_all_examples<duration<int8_t, std::micro>>(
        gs::temporal::durations<duration<int8_t, std::micro>>(),
        [](const duration<int8_t, std::micro>&) { return true; });
    assert_all_examples<milliseconds>(
        gs::temporal::durations<milliseconds>(
            {.min_value = milliseconds{-5}, .max_value = milliseconds{5}}),
        [](const milliseconds& d) {
            return d >= milliseconds{-5} && d <= milliseconds{5};
        });
    find_any<nanoseconds>(
        gs::temporal::durations<nanoseconds>(),
        [](const nanoseconds& d) { return d.count() % 1000 != 0; });
    find_any<hours>(gs::temporal::durations<hours>(),
                    [](const hours& d) { return d < hours{0}; });
}

#if HEGEL_HAS_REFLECTION

struct Appointment {
    gs::temporal::ZonedDateTime when;
    std::chrono::seconds length;
    std::optional<gs::temporal::Date> reminder;
};

TEST(TemporalGenerators, DefaultGeneratorsCoverEveryValueType) {
    hegel::test(
        [](hegel::TestCase& tc) {
            (void)tc.draw(gs::default_generator<gs::temporal::Date>());
            (void)tc.draw(gs::default_generator<gs::temporal::Time>());
            (void)tc.draw(gs::default_generator<gs::temporal::DateTime>());
            (void)tc.draw(gs::default_generator<gs::temporal::Instant>());
            (void)tc.draw(gs::default_generator<gs::temporal::UtcOffset>());
            (void)tc.draw(gs::default_generator<gs::temporal::TimeZone>());
            (void)tc.draw(gs::default_generator<gs::temporal::ZonedDateTime>());
            (void)tc.draw(
                gs::default_generator<gs::temporal::SignedDuration>());
            (void)tc.draw(gs::default_generator<gs::temporal::Span>());
            (void)tc.draw(gs::default_generator<std::chrono::milliseconds>());
            Appointment a = tc.draw(gs::default_generator<Appointment>());
            EXPECT_TRUE(gs::temporal::info(a.when.zone.name).has_value());
        },
        hegel::Settings{.test_cases = 20,
                        .database = hegel::Database::disabled()});
}

#endif

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"

TEST(DeprecatedStringGenerators, DatesAreIsoDatesFromYearOneTo9999) {
    std::regex date(R"(\d{4}-\d{2}-\d{2})");
    assert_all_examples<std::string>(gs::dates(), [&](const std::string& s) {
        return std::regex_match(s, date) && s >= "0001-01-01" &&
               s <= "9999-12-31";
    });
    EXPECT_EQ(minimal<std::string>(
                  gs::dates(), [](const std::string& s) { return s < "0002"; }),
              "0001-12-31");
    EXPECT_EQ(
        minimal<std::string>(gs::dates(),
                             [](const std::string& s) { return s >= "9999"; }),
        "9999-01-01");
}

TEST(DeprecatedStringGenerators, TimesHaveMicrosecondsWhenNonzero) {
    std::regex time(R"(\d{2}:\d{2}:\d{2}(\.\d{6})?)");
    assert_all_examples<std::string>(gs::times(), [&](const std::string& s) {
        return std::regex_match(s, time);
    });
    EXPECT_EQ(
        find_any<std::string>(
            gs::times(), [](const std::string& s) { return s.size() == 8; })
            .size(),
        8U);
    EXPECT_EQ(
        find_any<std::string>(
            gs::times(), [](const std::string& s) { return s.size() == 15; })
            .size(),
        15U);
}

TEST(DeprecatedStringGenerators, DatetimesJoinADateAndATime) {
    std::regex datetime(R"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{6})?)");
    assert_all_examples<std::string>(
        gs::datetimes(),
        [&](const std::string& s) { return std::regex_match(s, datetime); });
}

#pragma GCC diagnostic pop
