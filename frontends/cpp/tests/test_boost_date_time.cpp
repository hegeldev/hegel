// The Boost.DateTime adapter: Boost time zones as zone sources, checked
// against what Boost itself computes.

#include <gtest/gtest.h>

#include "common/utils.h"

#include <cstdint>
#include <sstream>
#include <stdexcept>
#include <string>
#include <variant>
#include <vector>

#include <hegel/extras/boost_date_time.h>
#include <hegel/hegel.h>

namespace gs = hegel::generators;
namespace temporal = hegel::generators::temporal;
namespace adapter = hegel::extras::boost_date_time;
namespace lt = boost::local_time;
namespace pt = boost::posix_time;

using hegel::tests::common::assert_all_examples;

namespace {

    const char* const zonespec =
        R"("America/New_York","EST","EST","EDT","EDT","-05:00:00","+01:00:00","2;0;3","+02:00:00","1;0;11","+02:00:00"
"Australia/Sydney","AEST","AEST","AEDT","AEDT","+10:00:00","+01:00:00","1;0;10","+02:00:00","1;0;4","+03:00:00"
"Europe/London","GMT","GMT","BST","BST","+00:00:00","+01:00:00","-1;0;3","+01:00:00","-1;0;10","+02:00:00"
"Asia/Kolkata","IST","IST","","","+05:30:00","+00:00:00","","","","+00:00:00"
"America/St_Johns","NST","NST","NDT","NDT","-03:30:00","+01:00:00","2;0;3","+02:00:00","1;0;11","+02:00:00"
)";

    lt::tz_database database() {
        lt::tz_database db;
        std::istringstream in(zonespec);
        db.load_from_stream(in);
        return db;
    }

    lt::time_zone_ptr posix(const std::string& spec) {
        return lt::time_zone_ptr(new lt::posix_time_zone(spec));
    }

    temporal::ZoneRule rule_of(const lt::time_zone_ptr& zone) {
        return std::get<temporal::ZoneRule>(adapter::zone_data(zone).after);
    }

    pt::ptime utc(int64_t instant) {
        return pt::ptime(boost::gregorian::date(1970, 1, 1)) +
               pt::seconds(static_cast<long>(instant));
    }

    void expect_matches_boost(const temporal::ZoneSource& source,
                              const std::string& name,
                              const lt::time_zone_ptr& zone) {
        auto instant = temporal::instants(
            {.min_value = temporal::Instant{-2'208'988'800, 0},
             .max_value = temporal::Instant{4'102'444'800, 0},
             .precision = temporal::Precision::Seconds});
        hegel::test(
            [&](hegel::TestCase& tc) {
                int64_t second = tc.draw(instant).second;
                lt::local_date_time local(utc(second), zone);
                int64_t boost_offset =
                    (local.local_time() - local.utc_time()).total_seconds();
                auto ours =
                    source.offset_at(name, temporal::Instant{second, 0});
                EXPECT_EQ(ours.offset.seconds, boost_offset) << name;
                EXPECT_EQ(ours.is_dst, local.is_dst()) << name;
            },
            hegel::Settings{.test_cases = 300,
                            .database = hegel::Database::disabled()});
    }

} // namespace

TEST(BoostDateTime, ADatabaseBecomesASourceUnderItsRegionNames) {
    lt::tz_database db = database();
    temporal::ZoneSource source = adapter::zone_source(db);
    for (const std::string& region : db.region_list()) {
        ASSERT_TRUE(source.info(region).has_value()) << region;
        expect_matches_boost(source, region, db.time_zone_from_region(region));
    }
    assert_all_examples<temporal::TimeZone>(
        temporal::zones({.source = source}),
        [&](const temporal::TimeZone& zone) {
            return db.time_zone_from_region(zone.name) != nullptr;
        });
}

TEST(BoostDateTime, ZonesHaveTheirRuleAndNoHistory) {
    temporal::ZoneData data =
        adapter::zone_data(posix("EST-05EDT,M3.2.0/2,M11.1.0/2"));
    EXPECT_TRUE(data.transitions.empty());
    EXPECT_EQ(data.initial.offset_seconds, -5 * 3600);
    EXPECT_EQ(data.initial.abbreviation, "EST");
    auto rule = std::get<temporal::ZoneRule>(data.after);
    ASSERT_TRUE(rule.daylight.has_value());
    EXPECT_EQ(rule.daylight->type.offset_seconds, -4 * 3600);
    EXPECT_TRUE(rule.daylight->type.is_dst);
    EXPECT_EQ(rule.daylight->type.abbreviation, "EDT");
    EXPECT_EQ(rule.daylight->start.kind,
              temporal::RuleDate::Kind::MonthWeekDay);
    EXPECT_EQ(rule.daylight->start.month, temporal::Month::March);
    EXPECT_EQ(rule.daylight->start.week, 2);
    EXPECT_EQ(rule.daylight->start.weekday, temporal::Weekday::Sunday);
    EXPECT_EQ(rule.daylight->start.time, 7200);
    EXPECT_EQ(rule.daylight->end.month, temporal::Month::November);

    auto fixed = rule_of(posix("IST+05:30"));
    EXPECT_EQ(fixed.standard.offset_seconds, 5 * 3600 + 1800);
    EXPECT_FALSE(fixed.daylight.has_value());
}

TEST(BoostDateTime, FixedDayRulesBecomeJulianDays) {
    auto julian = rule_of(posix("XST+03XDT,J60/1:30:15,J300/2"));
    EXPECT_EQ(julian.daylight->start.kind, temporal::RuleDate::Kind::Julian);
    EXPECT_EQ(julian.daylight->start.day, 60);
    EXPECT_EQ(julian.daylight->start.time, 3600 + 1800 + 15);
    EXPECT_EQ(julian.daylight->end.day, 300);

    auto january = rule_of(posix("XST+03XDT,10/2,100/2"));
    EXPECT_EQ(january.daylight->start.day, 11);
    EXPECT_EQ(january.daylight->end.day, 100);

    EXPECT_THROW(adapter::zone_data(posix("XST+03XDT,59/2,100/2")),
                 std::invalid_argument);
    EXPECT_THROW(adapter::zone_data(lt::time_zone_ptr()),
                 std::invalid_argument);

    temporal::ZoneSource source;
    source.add_zone("Etc/TestJulian",
                    adapter::zone_data(posix("XST+03XDT,J60/1:30:15,J300/2")));
    expect_matches_boost(source, "Etc/TestJulian",
                         posix("XST+03XDT,J60/1:30:15,J300/2"));
}

TEST(BoostDateTime, NegativeTransitionTimesAreRead) {
    lt::time_zone_ptr zone(new lt::custom_time_zone(
        lt::time_zone_names("Test Standard", "TST", "Test Daylight", "TDT"),
        pt::hours(2),
        lt::dst_adjustment_offsets(pt::hours(1), pt::time_duration(-1, -30, 0),
                                   pt::hours(26)),
        lt::dst_calc_rule_ptr(new lt::nth_kday_dst_rule(
            lt::nth_kday_dst_rule::start_rule(
                boost::gregorian::nth_kday_of_month::second,
                boost::gregorian::Sunday, boost::gregorian::Mar),
            lt::nth_kday_dst_rule::end_rule(
                boost::gregorian::nth_kday_of_month::first,
                boost::gregorian::Sunday, boost::gregorian::Oct)))));
    auto rule = rule_of(zone);
    EXPECT_EQ(rule.daylight->start.time, -5400);
    EXPECT_EQ(rule.daylight->end.time, 26 * 3600);
}
