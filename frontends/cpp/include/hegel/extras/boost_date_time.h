#pragma once

/**
 * @file
 * @brief Zone sources from Boost.DateTime time zones.
 *
 * Header-only, and the only part of Hegel that uses Boost: include it in a
 * project that already uses Boost.DateTime (1.73 or later, where the
 * library is header-only) and link Hegel and Boost as usual. A project
 * that builds Hegel as part of its own build (`FetchContent` or
 * `add_subdirectory`) can set `HEGEL_BOOST_DATE_TIME=ON` to get the CMake
 * target `hegel::boost_date_time`, which links Hegel and the Boost headers
 * (an installed Boost when CMake finds one, otherwise a Boost release
 * fetched at configure time). An installed Hegel has the header and no
 * such target.
 *
 * A Boost time zone has one rule for all time: its current standard
 * offset and, when it has daylight-saving time, the dates that time starts
 * and ends on each year. It has no history. zone_data() turns it into a
 * zone with that rule and no transitions, which is what Boost itself
 * computes, so wall times resolve the same way for Hegel and for Boost.
 *
 * Build the source once, outside the test body:
 *
 * @code{.cpp}
 * #include <hegel/extras/boost_date_time.h>
 *
 * boost::local_time::tz_database db;
 * db.load_from_file("date_time_zonespec.csv");
 * auto when = gs::temporal::zoned_datetimes(
 *     {.source = hegel::extras::boost_date_time::zone_source(db)});
 * hegel::test([&](hegel::TestCase& tc) {
 *     auto drawn = tc.draw(when);
 *     boost::local_time::local_date_time local(
 *         boost::posix_time::from_time_t(drawn.instant.second),
 *         db.time_zone_from_region(drawn.zone.name));
 * });
 * @endcode
 */

#include <boost/date_time/local_time/local_time.hpp>
#include <boost/date_time/local_time/tz_database.hpp>

#include <cstddef>
#include <cstdint>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include "hegel/generators/temporal_types.h"
#include "hegel/generators/zone_source.h"

/**
 * @brief Zone sources from Boost.DateTime; see
 * `hegel/extras/boost_date_time.h`.
 */
namespace hegel::extras::boost_date_time {

    /// @cond INTERNAL
    namespace detail {

        inline int32_t seconds_of(const ::boost::posix_time::time_duration& d) {
            return static_cast<int32_t>(d.total_seconds());
        }

        inline std::vector<std::string> split(const std::string& text,
                                              char separator) {
            std::vector<std::string> parts;
            size_t start = 0;
            size_t end = text.find(separator);
            while (end != std::string::npos) {
                parts.push_back(text.substr(start, end - start));
                start = end + 1;
                end = text.find(separator, start);
            }
            parts.push_back(text.substr(start));
            return parts;
        }

        inline int32_t rule_time(const std::string& text) {
            std::vector<std::string> fields = split(text, ':');
            int32_t seconds = 0;
            int32_t unit = 3600;
            for (const std::string& field : fields) {
                seconds += std::stoi(field) * unit;
                unit /= 60;
            }
            return seconds;
        }

        // A rule of a Boost POSIX string, as `Mm.w.d/time` for a weekday of
        // a month or `n/time` for a fixed month and day, which Boost writes
        // as its zero-based day of the leap year 2004.
        inline generators::temporal::RuleDate
        rule_date(const std::string& text) {
            std::vector<std::string> rule_and_time = split(text, '/');
            int32_t time = rule_time(rule_and_time.at(1));
            const std::string& rule = rule_and_time[0];
            if (rule.at(0) == 'M') {
                std::vector<std::string> fields = split(rule.substr(1), '.');
                return generators::temporal::RuleDate::month_week_day(
                    static_cast<generators::temporal::Month>(
                        std::stoi(fields.at(0)) - 1),
                    static_cast<uint8_t>(std::stoi(fields.at(1))),
                    static_cast<generators::temporal::Weekday>(
                        (std::stoi(fields.at(2)) + 6) % 7),
                    time);
            }
            int leap_day = std::stoi(rule);
            constexpr int february_29 = 59;
            if (leap_day == february_29) {
                throw std::invalid_argument(
                    "a Boost time zone whose daylight-saving time starts or "
                    "ends on February 29 has no date in common years");
            }
            return generators::temporal::RuleDate::julian(
                static_cast<uint16_t>(leap_day < february_29 ? leap_day + 1
                                                             : leap_day),
                time);
        }

    } // namespace detail
    /// @endcond

    /**
     * @brief The data of a Boost time zone: its rule for all time, with no
     * transitions.
     *
     * The standard type has the zone's base offset and standard
     * abbreviation; the daylight-saving type, when the zone has one, the
     * base offset plus its adjustment, and its daylight-saving
     * abbreviation.
     *
     * @param zone A Boost time zone, such as a
     *   `boost::local_time::posix_time_zone` or one from a
     *   `boost::local_time::tz_database`.
     * @return The zone's data, for ZoneSource::add_zone().
     * @throws std::invalid_argument for a null `zone`, or one whose
     *   daylight-saving time starts or ends on February 29.
     */
    inline generators::temporal::ZoneData
    zone_data(const ::boost::local_time::time_zone_ptr& zone) {
        if (!zone) {
            throw std::invalid_argument("zone_data: the time zone is null");
        }
        generators::temporal::ZoneType standard{
            detail::seconds_of(zone->base_utc_offset()), false,
            zone->std_zone_abbrev()};
        generators::temporal::ZoneRule rule{standard, std::nullopt};
        if (zone->has_dst()) {
            std::vector<std::string> parts =
                detail::split(zone->to_posix_string(), ',');
            rule.daylight = generators::temporal::DaylightRule{
                generators::temporal::ZoneType{
                    detail::seconds_of(zone->base_utc_offset() +
                                       zone->dst_offset()),
                    true, zone->dst_zone_abbrev()},
                detail::rule_date(parts.at(1)), detail::rule_date(parts.at(2))};
        }
        return generators::temporal::ZoneData{standard, {}, std::move(rule)};
    }

    /**
     * @brief A zone source holding every region of a Boost time zone
     * database, each under the name the database gives it, such as
     * `America/New_York`.
     *
     * @param db The database.
     * @return A new source with one zone per region.
     * @throws std::invalid_argument as zone_data() does, or for a region name
     *   the engine rejects.
     */
    inline generators::temporal::ZoneSource
    zone_source(const ::boost::local_time::tz_database& db) {
        generators::temporal::ZoneSource source;
        for (const std::string& region : db.region_list()) {
            source.add_zone(region,
                            zone_data(db.time_zone_from_region(region)));
        }
        return source;
    }

} // namespace hegel::extras::boost_date_time
