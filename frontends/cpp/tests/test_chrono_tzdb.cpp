// The std::chrono::tzdb adapter: the standard library's time zones as zone
// sources, checked against what the standard library computes.

#include <gtest/gtest.h>

#include "common/utils.h"

#include <chrono>
#include <cstdint>
#include <stdexcept>
#include <string>

#include <hegel/extras/chrono_tzdb.h>
#include <hegel/hegel.h>

namespace gs = hegel::generators;
namespace temporal = hegel::generators::temporal;
namespace adapter = hegel::extras::chrono_tzdb;

using hegel::tests::common::assert_all_examples;

TEST(ChronoTzdb, ZonesMatchTheStandardLibrary) {
    temporal::ZoneSource source = adapter::zone_source();
    EXPECT_EQ(source.version(), std::chrono::get_tzdb().version);
    auto zone = temporal::zones(
        {.names =
             std::vector<std::string>{"America/New_York", "Europe/London",
                                      "Australia/Lord_Howe", "Asia/Kolkata",
                                      "Asia/Calcutta", "America/Sao_Paulo"},
         .source = source});
    auto instant =
        temporal::instants({.min_value = temporal::Instant{-5'364'662'400, 0},
                            .max_value = temporal::Instant{7'258'118'400, 0},
                            .precision = temporal::Precision::Seconds});
    hegel::test(
        [&](hegel::TestCase& tc) {
            std::string name = tc.draw(zone).name;
            int64_t second = tc.draw(instant).second;
            std::chrono::sys_info info =
                std::chrono::locate_zone(name)->get_info(
                    std::chrono::sys_seconds(std::chrono::seconds(second)));
            auto ours = source.offset_at(name, temporal::Instant{second, 0});
            EXPECT_EQ(ours.offset.seconds, info.offset.count()) << name;
            EXPECT_EQ(ours.abbreviation, info.abbrev) << name;
        },
        hegel::Settings{.test_cases = 300,
                        .database = hegel::Database::disabled()});
}

TEST(ChronoTzdb, LinksAndUnknownNames) {
    temporal::ZoneSource source = adapter::zone_source();
    auto calcutta = source.info("Asia/Calcutta");
    ASSERT_TRUE(calcutta.has_value());
    EXPECT_TRUE(calcutta->is_link);
    EXPECT_FALSE(source.info("Mars/Olympus").has_value());
    EXPECT_THROW(source.offset_at("Mars/Olympus", temporal::Instant{0, 0}),
                 std::invalid_argument);
    EXPECT_EQ(source.offset_at("Asia/Kolkata", temporal::Instant{0, 0})
                  .offset.seconds,
              5 * 3600 + 1800);
    assert_all_examples<temporal::ZonedDateTime>(
        temporal::zoned_datetimes(
            {.zones = temporal::zones({.source = source})}),
        [](const temporal::ZonedDateTime& z) {
            return std::chrono::locate_zone(z.zone.name) != nullptr;
        },
        30);
}

TEST(ChronoTzdb, AnOracleTakesItsProbeRange) {
    temporal::ZoneSource source;
    source.add_oracle(
        "America/New_York",
        adapter::oracle(*std::chrono::locate_zone("America/New_York"), 2000,
                        2010));
    EXPECT_EQ(
        source
            .offset_at("America/New_York", temporal::Instant{1'720'000'000, 0})
            .offset.seconds,
        -4 * 3600);
    EXPECT_TRUE(
        source
            .offset_at("America/New_York", temporal::Instant{1'720'000'000, 0})
            .is_dst);
}
