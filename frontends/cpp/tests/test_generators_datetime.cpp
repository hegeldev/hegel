// Date and time generators: the hegel::datetime values, their printing and
// chrono conversions, the database queries, and construction / parameter
// validation of every generator in generators/datetime.h.

#include <gtest/gtest.h>

#include "common/utils.h"

#include <chrono>
#include <cstdint>
#include <limits>
#include <optional>
#include <ratio>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

#include <hegel/hegel.h>
#include <hegel/repr.h>

namespace gs = hegel::generators;
namespace dt = hegel::datetime;

using hegel::tests::common::assert_all_examples;
using hegel::tests::common::find_any;

namespace {

    template <typename T> std::string show(const T& value) {
        std::ostringstream os;
        os << value;
        return os.str();
    }

    dt::DateTime datetime(int32_t year, uint8_t month, uint8_t day,
                          uint8_t hour, uint8_t minute, uint8_t second,
                          uint32_t nanosecond = 0) {
        return dt::DateTime{{year, month, day},
                            {hour, minute, second, nanosecond}};
    }

} // namespace

TEST(DatetimeValues, DatesAndTimesPrintInRfc9557Form) {
    EXPECT_EQ(show(dt::Date{2024, 3, 10}), "2024-03-10");
    EXPECT_EQ(show(dt::Date{1, 1, 1}), "0001-01-01");
    EXPECT_EQ(show(dt::Date{10000, 1, 1}), "+010000-01-01");
    EXPECT_EQ(show(dt::Date{-1, 12, 31}), "-000001-12-31");
    EXPECT_EQ(show(dt::Time{2, 30, 0, 0}), "02:30:00");
    EXPECT_EQ(show(dt::Time{2, 30, 0, 500'000'000}), "02:30:00.5");
    EXPECT_EQ(show(dt::Time{2, 30, 0, 1}), "02:30:00.000000001");
    EXPECT_EQ(show(datetime(2024, 3, 10, 2, 30, 0)), "2024-03-10T02:30:00");
}

TEST(DatetimeValues, TimestampsPrintAsUtc) {
    EXPECT_EQ(show(dt::Timestamp{946'684'800, 0}), "2000-01-01T00:00:00Z");
    EXPECT_EQ(show(dt::Timestamp{0, 0}), "1970-01-01T00:00:00Z");
    EXPECT_EQ(show(dt::Timestamp{-1, 0}), "1969-12-31T23:59:59Z");
    EXPECT_EQ(show(dt::Timestamp{-2, 500'000'000}), "1969-12-31T23:59:58.5Z");
    EXPECT_EQ(show(dt::Timestamp{dt::MIN_INSTANT, 0}),
              "-999999-01-01T00:00:00Z");
    EXPECT_EQ(show(dt::Timestamp{dt::MAX_INSTANT, 0}),
              "+999999-12-31T23:59:59Z");
}

TEST(DatetimeValues, OffsetsPrintWithSecondsOnlyWhenNeeded) {
    EXPECT_EQ(show(dt::UtcOffset{0}), "+00:00");
    EXPECT_EQ(show(dt::UtcOffset{19'800}), "+05:30");
    EXPECT_EQ(show(dt::UtcOffset{-18'000}), "-05:00");
    EXPECT_EQ(show(dt::UtcOffset{-2'670}), "-00:44:30");
    EXPECT_EQ(show(dt::UNKNOWN_OFFSET), "-00:00");
    EXPECT_EQ(dt::UNKNOWN_OFFSET, (dt::UtcOffset{123, false}));
    EXPECT_NE(dt::UNKNOWN_OFFSET, dt::UtcOffset{0});
}

TEST(DatetimeValues, SignedDurationsPrintInIso8601Form) {
    EXPECT_EQ(show(dt::SignedDuration{0, 0}), "PT0S");
    EXPECT_EQ(show(dt::SignedDuration{5405, 500'000'000}), "PT1H30M5.5S");
    EXPECT_EQ(show(dt::SignedDuration{3600, 0}), "PT1H");
    EXPECT_EQ(show(dt::SignedDuration{-1, -500'000'000}), "-PT1.5S");
    EXPECT_EQ(show(dt::SignedDuration{0, -1}), "-PT0.000000001S");
    EXPECT_EQ(show(dt::SignedDuration{std::numeric_limits<int64_t>::min(), 0}),
              "-PT2562047788015215H30M8S");
}

TEST(DatetimeValues, SpansPrintInFriendlyForm) {
    EXPECT_EQ(show(dt::Span{}), "0s");
    dt::Span every{1, 2, 3, 4, 5, 6, 7, 8, 9, 10};
    EXPECT_EQ(show(every), "1y 2mo 3w 4d 5h 6m 7s 8ms 9µs 10ns");
    dt::Span negative;
    negative.years = -1;
    negative.months = -2;
    EXPECT_EQ(show(negative), "-1y 2mo");
    dt::Span mixed;
    mixed.years = 1;
    mixed.months = -2;
    EXPECT_EQ(show(mixed), "1y -2mo");
    dt::Span lowest;
    lowest.nanoseconds = std::numeric_limits<int64_t>::min();
    EXPECT_EQ(show(lowest), "-9223372036854775808ns");
    for (size_t i = 0; i < dt::ALL_SPAN_UNITS.size(); ++i) {
        EXPECT_EQ(every.get(dt::ALL_SPAN_UNITS[i]),
                  static_cast<int64_t>(i + 1));
    }
    EXPECT_NE(every, mixed);
}

TEST(DatetimeValues, ZonedDatetimesPrintAsTheEngineDoes) {
    dt::ZonedDateTime gap =
        dt::resolve("America/New_York", datetime(2024, 3, 10, 2, 30, 0));
    EXPECT_EQ(gap.kind, dt::Kind::Gap);
    EXPECT_EQ(gap.offset_before, -18'000);
    EXPECT_EQ(gap.offset_after, -14'400);
    EXPECT_EQ(gap.offset, dt::UtcOffset{gap.offset_after});
    EXPECT_EQ(show(gap), "2024-03-10T02:30:00-04:00[America/New_York][gap]");
    EXPECT_EQ(show(gap.timestamp), "2024-03-10T07:30:00Z");

    dt::ZonedDateTime fold =
        dt::resolve("America/New_York", datetime(2024, 11, 3, 1, 30, 0));
    EXPECT_EQ(fold.kind, dt::Kind::Fold);
    EXPECT_FALSE(fold.fold);
    EXPECT_EQ(show(fold),
              "2024-11-03T01:30:00-04:00[America/New_York][fold=0]");
    dt::ZonedDateTime later = fold;
    later.fold = true;
    later.offset = dt::UtcOffset{fold.offset_after};
    EXPECT_EQ(show(later),
              "2024-11-03T01:30:00-05:00[America/New_York][fold=1]");
    EXPECT_NE(later, fold);

    dt::ZonedDateTime unique =
        dt::resolve("America/New_York", datetime(2024, 6, 1, 12, 0, 0));
    EXPECT_EQ(unique.kind, dt::Kind::Unique);
    EXPECT_EQ(unique.offset, dt::UtcOffset{-14'400});
    EXPECT_EQ(show(unique), "2024-06-01T12:00:00-04:00[America/New_York]");

    dt::ZonedDateTime unknown =
        dt::resolve(dt::UNKNOWN_ZONE, datetime(2024, 6, 1, 12, 0, 0));
    EXPECT_EQ(unknown.offset, dt::UNKNOWN_OFFSET);
    EXPECT_EQ(show(unknown), "2024-06-01T12:00:00Z[Etc/Unknown]");
    EXPECT_EQ(show(unknown.timestamp), "2024-06-01T12:00:00Z");

    dt::ZonedDateTime opaque = unknown;
    opaque.zone = dt::TimeZone{"Mars/Olympus", "Mars/Olympus"};
    EXPECT_EQ(show(opaque), "2024-06-01T12:00:00-00:00[Mars/Olympus]");

    dt::ZonedDateTime calcutta =
        dt::resolve("Asia/Calcutta", datetime(2024, 6, 1, 12, 0, 0));
    EXPECT_EQ(calcutta.zone.primary_name, "Asia/Kolkata");
    EXPECT_EQ(show(calcutta.zone), "Asia/Calcutta");
    EXPECT_EQ(show(calcutta), "2024-06-01T12:00:00+05:30[Asia/Calcutta]");
}

TEST(DatetimeValues, ResolveRejectsBadInput) {
    EXPECT_THROW(dt::resolve("Mars/Olympus", datetime(2024, 6, 1, 12, 0, 0)),
                 std::invalid_argument);
    EXPECT_THROW(dt::resolve("UTC", datetime(2024, 2, 30, 0, 0, 0)),
                 std::invalid_argument);
    dt::DateTime last = datetime(dt::MAX_YEAR, 12, 31, 23, 59, 59);
    EXPECT_THROW(dt::resolve("America/New_York", last), std::invalid_argument);
    EXPECT_EQ(dt::resolve("UTC", last).timestamp,
              (dt::Timestamp{dt::MAX_INSTANT, 0}));
}

TEST(DatetimeValues, OffsetAtReportsTheOffsetInForce) {
    dt::ZoneOffset summer =
        dt::offset_at("Europe/London", dt::Timestamp{1'717'243'200, 0});
    EXPECT_EQ(summer.offset, dt::UtcOffset{3600});
    EXPECT_TRUE(summer.is_dst);
    EXPECT_EQ(summer.abbreviation, "BST");
    dt::ZoneOffset winter =
        dt::offset_at("Europe/London", dt::Timestamp{1'704'067'200, 0});
    EXPECT_EQ(winter.offset, dt::UtcOffset{0});
    EXPECT_FALSE(winter.is_dst);
    EXPECT_EQ(winter.abbreviation, "GMT");
    EXPECT_NE(summer, winter);
    dt::ZoneOffset unknown =
        dt::offset_at(dt::UNKNOWN_ZONE, dt::Timestamp{0, 0});
    EXPECT_EQ(unknown.offset, dt::UNKNOWN_OFFSET);
    EXPECT_EQ(unknown.abbreviation, "-00");
    EXPECT_THROW(dt::offset_at("Mars/Olympus", dt::Timestamp{0, 0}),
                 std::invalid_argument);
    EXPECT_THROW(dt::offset_at("UTC", dt::Timestamp{0, 1'000'000'000}),
                 std::invalid_argument);
}

TEST(DatetimeValues, InfoDescribesNames) {
    std::optional<dt::ZoneInfo> utc = dt::info("UTC");
    ASSERT_TRUE(utc.has_value());
    EXPECT_EQ(utc->primary_name, "UTC");
    EXPECT_FALSE(utc->is_link);
    EXPECT_EQ(utc->rank, 0);
    EXPECT_NE(std::find(utc->tags.begin(), utc->tags.end(), dt::Tag::Fixed),
              utc->tags.end());
    std::optional<dt::ZoneInfo> calcutta = dt::info("Asia/Calcutta");
    ASSERT_TRUE(calcutta.has_value());
    EXPECT_EQ(calcutta->primary_name, "Asia/Kolkata");
    EXPECT_TRUE(calcutta->is_link);
    EXPECT_NE(
        std::find(calcutta->tags.begin(), calcutta->tags.end(), dt::Tag::Link),
        calcutta->tags.end());
    EXPECT_NE(std::find(calcutta->tags.begin(), calcutta->tags.end(),
                        dt::Tag::Fractional),
              calcutta->tags.end());
    EXPECT_NE(*utc, *calcutta);
    std::optional<dt::ZoneInfo> unknown = dt::info(dt::UNKNOWN_ZONE);
    ASSERT_TRUE(unknown.has_value());
    EXPECT_EQ(unknown->primary_name, dt::UNKNOWN_ZONE);
    EXPECT_EQ(unknown->tags, std::vector<dt::Tag>{dt::Tag::Undefined});
    EXPECT_EQ(unknown->rank, std::numeric_limits<uint16_t>::max());
    EXPECT_FALSE(dt::info("Mars/Olympus").has_value());
    EXPECT_FALSE(dt::info(std::string("UTC\0", 4)).has_value());
}

TEST(DatetimeValues, TzdbVersionIsAnIanaRelease) {
    std::string version = dt::tzdb_version();
    ASSERT_EQ(version.size(), 5u) << version;
    for (size_t i = 0; i < 4; ++i) {
        EXPECT_TRUE(std::isdigit(static_cast<unsigned char>(version[i])))
            << version;
    }
    EXPECT_TRUE(std::islower(static_cast<unsigned char>(version[4])))
        << version;
}

TEST(DatetimeValues, ComparisonsFollowTheFields) {
    EXPECT_LT((dt::Date{2024, 3, 10}), (dt::Date{2024, 3, 11}));
    EXPECT_LT((dt::Date{2024, 2, 28}), (dt::Date{2024, 3, 1}));
    EXPECT_LT((dt::Date{2023, 12, 31}), (dt::Date{2024, 1, 1}));
    EXPECT_GT((dt::Date{2024, 3, 11}), (dt::Date{2024, 3, 10}));
    EXPECT_LE((dt::Date{2024, 3, 10}), (dt::Date{2024, 3, 10}));
    EXPECT_GE((dt::Date{2024, 3, 10}), (dt::Date{2024, 3, 10}));
    EXPECT_NE((dt::Date{2024, 3, 10}), (dt::Date{2024, 3, 11}));

    EXPECT_LT((dt::Time{1, 0, 0, 0}), (dt::Time{2, 0, 0, 0}));
    EXPECT_LT((dt::Time{1, 0, 0, 0}), (dt::Time{1, 1, 0, 0}));
    EXPECT_LT((dt::Time{1, 0, 0, 0}), (dt::Time{1, 0, 1, 0}));
    EXPECT_LT((dt::Time{1, 0, 0, 0}), (dt::Time{1, 0, 0, 1}));
    EXPECT_GT((dt::Time{2, 0, 0, 0}), (dt::Time{1, 0, 0, 0}));
    EXPECT_LE((dt::Time{1, 0, 0, 0}), (dt::Time{1, 0, 0, 0}));
    EXPECT_GE((dt::Time{1, 0, 0, 0}), (dt::Time{1, 0, 0, 0}));
    EXPECT_NE((dt::Time{1, 0, 0, 0}), (dt::Time{1, 0, 0, 1}));

    dt::DateTime a = datetime(2024, 1, 1, 0, 0, 0);
    dt::DateTime b = datetime(2024, 1, 1, 0, 0, 1);
    dt::DateTime c = datetime(2024, 1, 2, 0, 0, 0);
    EXPECT_LT(a, b);
    EXPECT_LT(b, c);
    EXPECT_GT(c, a);
    EXPECT_LE(a, a);
    EXPECT_GE(a, a);
    EXPECT_NE(a, b);

    EXPECT_LT((dt::Timestamp{0, 0}), (dt::Timestamp{0, 1}));
    EXPECT_LT((dt::Timestamp{-1, 0}), (dt::Timestamp{0, 0}));
    EXPECT_GT((dt::Timestamp{0, 1}), (dt::Timestamp{0, 0}));
    EXPECT_LE((dt::Timestamp{0, 0}), (dt::Timestamp{0, 0}));
    EXPECT_GE((dt::Timestamp{0, 0}), (dt::Timestamp{0, 0}));
    EXPECT_NE((dt::Timestamp{0, 0}), (dt::Timestamp{0, 1}));

    EXPECT_LT((dt::SignedDuration{0, 0}), (dt::SignedDuration{0, 1}));
    EXPECT_LT((dt::SignedDuration{-1, 0}), (dt::SignedDuration{0, 0}));
    EXPECT_GT((dt::SignedDuration{0, 1}), (dt::SignedDuration{0, 0}));
    EXPECT_LE((dt::SignedDuration{0, 0}), (dt::SignedDuration{0, 0}));
    EXPECT_GE((dt::SignedDuration{0, 0}), (dt::SignedDuration{0, 0}));
    EXPECT_NE((dt::SignedDuration{0, 0}), (dt::SignedDuration{0, 1}));

    EXPECT_NE((dt::TimeZone{"UTC", "UTC"}), (dt::TimeZone{"Etc/UTC", "UTC"}));
}

TEST(DatetimeChrono, SignedDurationsConvertToAndFromChrono) {
    using namespace std::chrono;
    dt::SignedDuration d{5405, 500'000'000};
    EXPECT_EQ(d.to_chrono(), nanoseconds{5'405'500'000'000});
    EXPECT_EQ(d.to_chrono<milliseconds>(), milliseconds{5'405'500});
    EXPECT_EQ(d.to_chrono<seconds>(), std::nullopt);
    EXPECT_EQ((dt::SignedDuration{5405, 0}).to_chrono<seconds>(),
              seconds{5405});
    EXPECT_EQ((dt::SignedDuration{5400, 0}).to_chrono<minutes>(), minutes{90});
    EXPECT_EQ((dt::SignedDuration{5400, 0}).to_chrono<hours>(), std::nullopt);
    EXPECT_EQ((dt::SignedDuration{-7200, 0}).to_chrono<hours>(), hours{-2});
    EXPECT_EQ((dt::SignedDuration{-1, -500'000'000}).to_chrono(),
              nanoseconds{-1'500'000'000});
    EXPECT_EQ((dt::SignedDuration{-1, -500'000'000}).to_chrono<milliseconds>(),
              milliseconds{-1500});
    EXPECT_EQ((dt::SignedDuration{std::numeric_limits<int64_t>::max(), 0})
                  .to_chrono(),
              std::nullopt);
    EXPECT_EQ((dt::SignedDuration{std::numeric_limits<int64_t>::min(), 0})
                  .to_chrono(),
              std::nullopt);
    EXPECT_EQ((dt::SignedDuration{9'223'372'036, 854'775'807}).to_chrono(),
              nanoseconds{std::numeric_limits<int64_t>::max()});
    EXPECT_EQ((dt::SignedDuration{9'223'372'036, 854'775'808}).to_chrono(),
              std::nullopt);
    EXPECT_EQ((dt::SignedDuration{-9'223'372'036, -854'775'808}).to_chrono(),
              nanoseconds{std::numeric_limits<int64_t>::min()});
    EXPECT_EQ((dt::SignedDuration{-9'223'372'036, -854'775'809}).to_chrono(),
              std::nullopt);
    EXPECT_EQ((dt::SignedDuration{std::numeric_limits<int64_t>::max(), 0})
                  .to_chrono<seconds>(),
              seconds{std::numeric_limits<int64_t>::max()});
    EXPECT_EQ(
        ((dt::SignedDuration{1, 0}).to_chrono<duration<int8_t, std::nano>>()),
        std::nullopt);
    using unsigned_seconds = duration<uint64_t>;
    EXPECT_EQ((dt::SignedDuration{-1, 0}).to_chrono<unsigned_seconds>(),
              std::nullopt);
    EXPECT_EQ((dt::SignedDuration{1, 0}).to_chrono<unsigned_seconds>(),
              unsigned_seconds{1});

    EXPECT_EQ(dt::SignedDuration::from_chrono(nanoseconds{5'405'500'000'000}),
              d);
    EXPECT_EQ(dt::SignedDuration::from_chrono(milliseconds{-1500}),
              (dt::SignedDuration{-1, -500'000'000}));
    EXPECT_EQ(dt::SignedDuration::from_chrono(hours{-2}),
              (dt::SignedDuration{-7200, 0}));
    EXPECT_EQ(dt::SignedDuration::from_chrono(hours::max()), std::nullopt);
    EXPECT_EQ(dt::SignedDuration::from_chrono(hours::min()), std::nullopt);
    EXPECT_EQ(dt::SignedDuration::from_chrono(unsigned_seconds{5}),
              (dt::SignedDuration{5, 0}));
    EXPECT_EQ(dt::SignedDuration::from_chrono(unsigned_seconds::max()),
              std::nullopt);
}

TEST(DatetimeChrono, TimestampsConvertToAndFromChrono) {
    using namespace std::chrono;
    using sys_ns = time_point<system_clock, nanoseconds>;
    dt::Timestamp epoch{0, 0};
    EXPECT_EQ(epoch.to_chrono(), sys_ns{nanoseconds{0}});
    dt::Timestamp before{-2, 500'000'000};
    EXPECT_EQ(before.to_chrono(), sys_ns{nanoseconds{-1'500'000'000}});
    EXPECT_EQ(before.to_chrono<milliseconds>(),
              (time_point<system_clock, milliseconds>{milliseconds{-1500}}));
    EXPECT_EQ(before.to_chrono<seconds>(), std::nullopt);
    EXPECT_EQ((dt::Timestamp{dt::MAX_INSTANT, 0}).to_chrono(), std::nullopt);
    EXPECT_EQ(dt::Timestamp::from_chrono(sys_ns{nanoseconds{-1'500'000'000}}),
              before);
    EXPECT_EQ(dt::Timestamp::from_chrono(
                  time_point<system_clock, seconds>{seconds{-2}}),
              (dt::Timestamp{-2, 0}));
    EXPECT_EQ(
        dt::Timestamp::from_chrono(time_point<system_clock, hours>{hours{1}}),
        (dt::Timestamp{3600, 0}));
    EXPECT_EQ(dt::Timestamp::from_chrono(time_point<system_clock, seconds>{
                  seconds{dt::MAX_INSTANT + 1}}),
              std::nullopt);
    EXPECT_EQ(dt::Timestamp::from_chrono(
                  time_point<system_clock, hours>{hours::max()}),
              std::nullopt);
}

#if HEGEL_HAS_CHRONO_CALENDAR
TEST(DatetimeChrono, DatesConvertToAndFromYearMonthDay) {
    using namespace std::chrono;
    std::optional<year_month_day> ymd = dt::Date{2024, 3, 10}.to_chrono();
    ASSERT_TRUE(ymd.has_value());
    EXPECT_EQ(*ymd, (year_month_day{year{2024}, month{3}, day{10}}));
    EXPECT_EQ((dt::Date{100'000, 1, 1}).to_chrono(), std::nullopt);
    EXPECT_EQ((dt::Date{-100'000, 1, 1}).to_chrono(), std::nullopt);
    EXPECT_EQ(dt::Date::from_chrono(*ymd), (dt::Date{2024, 3, 10}));
    EXPECT_EQ(dt::Date::from_chrono(year_month_day{sys_days{days{19'792}}}),
              (dt::Date{2024, 3, 10}));
    EXPECT_EQ(
        dt::Date::from_chrono(year_month_day{year{2023}, month{2}, day{29}}),
        std::nullopt);
}
#endif

TEST(DatetimeRepr, ValuesRenderInTheEngineForm) {
    using hegel::internal::cpp_type_name;
    using hegel::internal::repr;
    EXPECT_EQ(repr(dt::Date{2024, 3, 10}), "2024-03-10");
    EXPECT_EQ(repr(dt::Time{2, 30, 0, 0}), "02:30:00");
    EXPECT_EQ(repr(datetime(2024, 3, 10, 2, 30, 0)), "2024-03-10T02:30:00");
    EXPECT_EQ(repr(dt::Timestamp{0, 0}), "1970-01-01T00:00:00Z");
    EXPECT_EQ(repr(dt::UtcOffset{19'800}), "+05:30");
    EXPECT_EQ(repr(dt::TimeZone{"UTC", "UTC"}), "UTC");
    EXPECT_EQ(repr(dt::resolve("UTC", datetime(2000, 1, 1, 0, 0, 0))),
              "2000-01-01T00:00:00+00:00[UTC]");
    EXPECT_EQ(repr(dt::SignedDuration{90, 0}), "PT1M30S");
    EXPECT_EQ(repr(dt::Span{}), "0s");
    EXPECT_EQ(repr(std::optional<dt::Date>{dt::Date{2024, 3, 10}}),
              "std::optional<hegel::datetime::Date>{2024-03-10}");
    EXPECT_EQ(repr(std::vector<dt::Date>{dt::Date{2024, 3, 10}}),
              "std::vector<hegel::datetime::Date>{2024-03-10}");

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

TEST(DatetimeGenerators, DrawAll) {
    hegel::test(
        [](hegel::TestCase& tc) {
            (void)tc.draw(gs::dates());
            (void)tc.draw(gs::dates({.min_value = dt::Date{2024, 1, 1},
                                     .max_value = dt::Date{2024, 12, 31},
                                     .weekdays = {{dt::Weekday::Monday}},
                                     .months = {{dt::Month::March}}}));
            (void)tc.draw(gs::times());
            (void)tc.draw(gs::times({.min_value = dt::Time{1, 0, 0, 0},
                                     .max_value = dt::Time{2, 0, 0, 0},
                                     .precision = dt::Precision::Minutes}));
            (void)tc.draw(gs::datetimes());
            (void)tc.draw(gs::datetimes({.precision = dt::Precision::Hours}));
            (void)tc.draw(gs::timestamps());
            (void)tc.draw(
                gs::timestamps({.min_value = dt::Timestamp{0, 0},
                                .precision = dt::Precision::Seconds}));
            (void)tc.draw(gs::utc_offsets());
            (void)tc.draw(
                gs::utc_offsets({.min_value = dt::UtcOffset{-3600},
                                 .max_value = dt::UtcOffset{3600},
                                 .granularity = dt::Granularity::QuarterHours,
                                 .unknown = true}));
            (void)tc.draw(gs::timezones());
            (void)tc.draw(gs::timezones({.links = false, .legacy = true}));
            (void)tc.draw(gs::timezones(
                {.names = std::vector<std::string>{"Mars/Olympus"},
                 .opaque = true}));
            (void)tc.draw(gs::zoned_datetimes());
            (void)tc.draw(gs::zoned_datetimes(
                {.timezones = {.names =
                                   std::vector<std::string>{"Europe/London"}},
                 .min_instant = dt::Timestamp{0, 0},
                 .max_instant = dt::Timestamp{86'400, 0},
                 .precision = dt::Precision::Seconds,
                 .allow_imaginary = false,
                 .ambiguous = dt::Ambiguous::Later}));
            (void)tc.draw(gs::signed_durations());
            (void)tc.draw(gs::signed_durations(
                {.min_value = dt::SignedDuration{0, 0},
                 .precision = dt::Precision::Milliseconds}));
            (void)tc.draw(gs::spans());
            (void)tc.draw(gs::spans(
                {.days = {{0, 30}}, .sign = dt::SpanSign::NonNegative}));
            (void)tc.draw(gs::durations());
            (void)tc.draw(gs::durations<std::chrono::seconds>(
                {.min_value = std::chrono::seconds{-10},
                 .max_value = std::chrono::seconds{10}}));
        },
        hegel::Settings{.test_cases = 20,
                        .database = hegel::Database::disabled()});
}

TEST(DatetimeGenerators, DatesValidateTheirParameters) {
    EXPECT_THROW(gs::dates({.min_value = dt::Date{2024, 2, 30}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::dates({.max_value = dt::Date{2024, 13, 1}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::dates({.max_value = dt::Date{2024, 1, 0}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::dates({.min_value = dt::Date{dt::MAX_YEAR + 1, 1, 1}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::dates({.min_value = dt::Date{2024, 1, 2},
                            .max_value = dt::Date{2024, 1, 1}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::dates({.weekdays = std::vector<dt::Weekday>{}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::dates({.months = std::vector<dt::Month>{}}),
                 std::invalid_argument);
    EXPECT_NO_THROW(gs::dates({.min_value = dt::Date{2024, 2, 29}}));
    EXPECT_NO_THROW(gs::dates({.min_value = dt::Date{2000, 2, 29}}));
    EXPECT_THROW(gs::dates({.min_value = dt::Date{1900, 2, 29}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::dates({.min_value = dt::Date{2024, 4, 31}}),
                 std::invalid_argument);
    EXPECT_NO_THROW(gs::dates({.min_value = dt::Date{2024, 1, 31}}));
    EXPECT_NO_THROW(
        gs::dates({.weekdays = {{dt::Weekday::Monday, dt::Weekday::Tuesday,
                                 dt::Weekday::Wednesday, dt::Weekday::Thursday,
                                 dt::Weekday::Friday, dt::Weekday::Saturday,
                                 dt::Weekday::Sunday}}}));
    auto no_allowed_day = gs::dates({.min_value = dt::Date{2024, 1, 1},
                                     .max_value = dt::Date{2024, 1, 31},
                                     .months = {{dt::Month::March}}});
    EXPECT_THROW(find_any<dt::Date>(no_allowed_day,
                                    [](const dt::Date&) { return true; }),
                 std::invalid_argument);
}

TEST(DatetimeGenerators, TimesValidateTheirParameters) {
    EXPECT_THROW(gs::times({.min_value = dt::Time{24, 0, 0, 0}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::times({.max_value = dt::Time{0, 60, 0, 0}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::times({.max_value = dt::Time{0, 0, 60, 0}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::times({.max_value = dt::Time{0, 0, 0, 1'000'000'000}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::times({.min_value = dt::Time{1, 0, 0, 0},
                            .max_value = dt::Time{0, 0, 0, 0}}),
                 std::invalid_argument);
    auto no_whole_minute = gs::times({.min_value = dt::Time{1, 0, 1, 0},
                                      .max_value = dt::Time{1, 0, 59, 0},
                                      .precision = dt::Precision::Minutes});
    EXPECT_THROW(find_any<dt::Time>(no_whole_minute,
                                    [](const dt::Time&) { return true; }),
                 std::invalid_argument);
}

TEST(DatetimeGenerators, DatetimesValidateTheirParameters) {
    EXPECT_THROW(gs::datetimes({.min_value = datetime(2024, 2, 30, 0, 0, 0)}),
                 std::invalid_argument);
    EXPECT_THROW(gs::datetimes({.max_value = datetime(2024, 1, 1, 25, 0, 0)}),
                 std::invalid_argument);
    EXPECT_THROW(gs::datetimes({.min_value = datetime(2024, 1, 2, 0, 0, 0),
                                .max_value = datetime(2024, 1, 1, 0, 0, 0)}),
                 std::invalid_argument);
}

TEST(DatetimeGenerators, TimestampsValidateTheirParameters) {
    EXPECT_THROW(
        gs::timestamps({.min_value = dt::Timestamp{dt::MIN_INSTANT - 1, 0}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::timestamps({.max_value = dt::Timestamp{dt::MAX_INSTANT + 1, 0}}),
        std::invalid_argument);
    EXPECT_THROW(gs::timestamps({.max_value = dt::Timestamp{0, 1'000'000'000}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::timestamps({.min_value = dt::Timestamp{1, 0},
                                 .max_value = dt::Timestamp{0, 0}}),
                 std::invalid_argument);
}

TEST(DatetimeGenerators, UtcOffsetsValidateTheirParameters) {
    EXPECT_THROW(gs::utc_offsets({.min_value = dt::UNKNOWN_OFFSET}),
                 std::invalid_argument);
    EXPECT_THROW(gs::utc_offsets({.max_value = dt::UNKNOWN_OFFSET}),
                 std::invalid_argument);
    EXPECT_THROW(
        gs::utc_offsets({.min_value = dt::UtcOffset{-dt::MAX_OFFSET - 1}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::utc_offsets({.max_value = dt::UtcOffset{dt::MAX_OFFSET + 1}}),
        std::invalid_argument);
    EXPECT_THROW(gs::utc_offsets({.min_value = dt::UtcOffset{1},
                                  .max_value = dt::UtcOffset{0}}),
                 std::invalid_argument);
    auto no_whole_hour =
        gs::utc_offsets({.min_value = dt::UtcOffset{1},
                         .max_value = dt::UtcOffset{3599},
                         .granularity = dt::Granularity::Hours});
    EXPECT_THROW(find_any<dt::UtcOffset>(
                     no_whole_hour, [](const dt::UtcOffset&) { return true; }),
                 std::invalid_argument);
}

TEST(DatetimeGenerators, TimezonesValidateTheirParameters) {
    EXPECT_THROW(gs::timezones({.names = std::vector<std::string>{}}),
                 std::invalid_argument);
    EXPECT_THROW(
        gs::timezones({.names = std::vector<std::string>{"Mars/Olympus"}}),
        std::invalid_argument);
    EXPECT_THROW(gs::timezones({.names = std::vector<std::string>{"utc"}}),
                 std::invalid_argument);
    EXPECT_NO_THROW(gs::timezones({.names = std::vector<std::string>{
                                       "UTC", std::string(dt::UNKNOWN_ZONE)}}));
}

TEST(DatetimeGenerators, ZonedDatetimesValidateTheirParameters) {
    EXPECT_THROW(
        gs::zoned_datetimes({.min_value = datetime(2024, 1, 1, 0, 0, 0),
                             .min_instant = dt::Timestamp{0, 0}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::zoned_datetimes({.max_value = datetime(2024, 1, 1, 0, 0, 0),
                             .max_instant = dt::Timestamp{0, 0}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::zoned_datetimes({.min_value = datetime(2024, 1, 2, 0, 0, 0),
                             .max_value = datetime(2024, 1, 1, 0, 0, 0)}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::zoned_datetimes({.min_value = datetime(2024, 2, 30, 0, 0, 0)}),
        std::invalid_argument);
    EXPECT_THROW(gs::zoned_datetimes({.min_instant = dt::Timestamp{1, 0},
                                      .max_instant = dt::Timestamp{0, 0}}),
                 std::invalid_argument);
    EXPECT_THROW(
        gs::zoned_datetimes({.min_instant = dt::Timestamp{0, 1'000'000'000}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::zoned_datetimes(
            {.timezones = {.names = std::vector<std::string>{"Mars/Olympus"}}}),
        std::invalid_argument);
    EXPECT_THROW(
        gs::zoned_datetimes({.min_value = datetime(2024, 1, 1, 0, 0, 1),
                             .max_value = datetime(2024, 1, 1, 0, 0, 59),
                             .precision = dt::Precision::Minutes}),
        std::invalid_argument);
}

TEST(DatetimeGenerators, SignedDurationsValidateTheirParameters) {
    EXPECT_THROW(gs::signed_durations({.min_value = dt::SignedDuration{1, -1}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::signed_durations({.max_value = dt::SignedDuration{-1, 1}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::signed_durations(
                     {.max_value = dt::SignedDuration{0, 1'000'000'000}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::signed_durations(
                     {.min_value = dt::SignedDuration{0, -1'000'000'000}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::signed_durations({.min_value = dt::SignedDuration{1, 0},
                                       .max_value = dt::SignedDuration{0, 0}}),
                 std::invalid_argument);
    EXPECT_NO_THROW(
        gs::signed_durations({.min_value = dt::SignedDuration{0, -5},
                              .max_value = dt::SignedDuration{0, 5}}));
}

TEST(DatetimeGenerators, SpansValidateTheirParameters) {
    EXPECT_THROW(gs::spans({.days = {{30, 0}}}), std::invalid_argument);
    EXPECT_THROW(
        gs::spans({.days = {{-1, 30}}, .sign = dt::SpanSign::NonNegative}),
        std::invalid_argument);
    EXPECT_THROW(gs::spans({.days = {{1, 30}}, .hours = {{-30, -1}}}),
                 std::invalid_argument);
    EXPECT_NO_THROW(gs::spans({.days = {{1, 30}},
                               .hours = {{-30, -1}},
                               .sign = dt::SpanSign::Mixed}));
}

TEST(DatetimeGenerators, DurationsValidateTheirParameters) {
    using namespace std::chrono;
    EXPECT_THROW(gs::durations<seconds>(
                     {.min_value = seconds{1}, .max_value = seconds{0}}),
                 std::invalid_argument);
    EXPECT_THROW(gs::durations<hours>({.min_value = hours::min()}),
                 std::invalid_argument);
    EXPECT_THROW(gs::durations<hours>({.max_value = hours::max()}),
                 std::invalid_argument);
    EXPECT_NO_THROW(gs::durations<hours>());
    EXPECT_NO_THROW((gs::durations<duration<int32_t, std::milli>>()));
}

TEST(DatetimeGenerators, DurationsDrawWholeUnitsWithinTheTypeAndTheEngine) {
    using namespace std::chrono;
    assert_all_examples<minutes>(gs::durations<minutes>(),
                                 [](const minutes&) { return true; });
    assert_all_examples<duration<int8_t, std::micro>>(
        gs::durations<duration<int8_t, std::micro>>(),
        [](const duration<int8_t, std::micro>&) { return true; });
    assert_all_examples<milliseconds>(
        gs::durations<milliseconds>(
            {.min_value = milliseconds{-5}, .max_value = milliseconds{5}}),
        [](const milliseconds& d) {
            return d >= milliseconds{-5} && d <= milliseconds{5};
        });
    find_any<nanoseconds>(gs::durations(), [](const nanoseconds& d) {
        return d.count() % 1000 != 0;
    });
    find_any<hours>(gs::durations<hours>(),
                    [](const hours& d) { return d < hours{0}; });
}

#if HEGEL_HAS_REFLECTION

struct Appointment {
    dt::ZonedDateTime when;
    std::chrono::seconds length;
    std::optional<dt::Date> reminder;
};

TEST(DatetimeGenerators, DefaultGeneratorsCoverEveryValueType) {
    hegel::test(
        [](hegel::TestCase& tc) {
            (void)tc.draw(gs::default_generator<dt::Date>());
            (void)tc.draw(gs::default_generator<dt::Time>());
            (void)tc.draw(gs::default_generator<dt::DateTime>());
            (void)tc.draw(gs::default_generator<dt::Timestamp>());
            (void)tc.draw(gs::default_generator<dt::UtcOffset>());
            (void)tc.draw(gs::default_generator<dt::TimeZone>());
            (void)tc.draw(gs::default_generator<dt::ZonedDateTime>());
            (void)tc.draw(gs::default_generator<dt::SignedDuration>());
            (void)tc.draw(gs::default_generator<dt::Span>());
            (void)tc.draw(gs::default_generator<std::chrono::milliseconds>());
            Appointment a = tc.draw(gs::default_generator<Appointment>());
            EXPECT_TRUE(dt::info(a.when.zone.name).has_value());
        },
        hegel::Settings{.test_cases = 20,
                        .database = hegel::Database::disabled()});
}

#endif
