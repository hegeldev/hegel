#pragma once

/**
 * @file
 * @brief Zone sources from the C++20 time zone database, `std::chrono::tzdb`.
 *
 * Header-only. Include it only with a standard library that implements
 * `std::chrono::get_tzdb()` and `std::chrono::time_zone::get_info()`, such
 * as libstdc++ from GCC 13 on; the rest of Hegel does not need them.
 *
 * The standard library does not expose a zone's rules, only lookups, so
 * each zone is an offset oracle: the engine asks `get_info()` for each
 * transition over a range of years and, after the last one, infers the
 * yearly rule, as the IANA data the library reads applies its rule for
 * ever.
 *
 * @code{.cpp}
 * #include <hegel/extras/chrono_tzdb.h>
 *
 * auto source = hegel::extras::chrono_tzdb::zone_source();
 * auto when = tc.draw(gs::temporal::zoned_datetimes(
 *     {.zones = gs::temporal::zones({.source = source})}));
 * auto zoned = std::chrono::zoned_time(
 *     when.zone.name, std::chrono::sys_seconds(
 *                         std::chrono::seconds(when.instant.second)));
 * @endcode
 */

#include <chrono>
#include <cstdint>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

#include "hegel/generators/zone_source.h"

/**
 * @brief Zone sources from `std::chrono::tzdb`; see
 * `hegel/extras/chrono_tzdb.h`.
 */
namespace hegel::extras::chrono_tzdb {

    /**
     * @brief An offset oracle over a standard library time zone.
     *
     * @param zone The zone, which must outlive the call it is probed in.
     * @param first_year The first year probed.
     * @param last_year The last year probed; after it the yearly rule of the
     *   last two years holds.
     * @return The oracle, for ZoneSource::add_oracle().
     */
    inline generators::temporal::ZoneOracle
    oracle(const std::chrono::time_zone& zone, int32_t first_year = 1800,
           int32_t last_year = 2100) {
        const std::chrono::time_zone* tz = &zone;
        generators::temporal::ZoneOracle result;
        result.offset_at = [tz](int64_t instant) {
            std::chrono::sys_info info = tz->get_info(
                std::chrono::sys_seconds(std::chrono::seconds(instant)));
            return generators::temporal::ZoneType{
                static_cast<int32_t>(info.offset.count()),
                info.save != std::chrono::minutes(0), info.abbrev};
        };
        result.next_transition = [tz](int64_t after) -> std::optional<int64_t> {
            std::chrono::sys_info info = tz->get_info(
                std::chrono::sys_seconds(std::chrono::seconds(after)));
            return info.end.time_since_epoch().count();
        };
        result.first_year = first_year;
        result.last_year = last_year;
        result.after = generators::temporal::AfterProbe::Infer;
        return result;
    }

    /**
     * @brief A zone source over the standard library's time zone database,
     * `std::chrono::get_tzdb()`.
     *
     * Every zone and link name of the database is declared, its links
     * added, and its release recorded as the version; each zone is probed
     * through oracle() the first time a draw or query needs it.
     *
     * @return A new source.
     * @throws std::runtime_error when the standard library cannot load its
     *   database.
     */
    inline generators::temporal::ZoneSource zone_source() {
        const std::chrono::tzdb& db = std::chrono::get_tzdb();
        std::vector<std::string> names;
        names.reserve(db.zones.size());
        for (const std::chrono::time_zone& zone : db.zones) {
            names.emplace_back(zone.name());
        }
        generators::temporal::ZoneSource source;
        source.declare(names).set_version(db.version);
        for (const std::chrono::time_zone_link& link : db.links) {
            source.add_link(link.name(), link.target());
        }
        source.set_loader([&db](generators::temporal::ZoneSource& s,
                                const std::string& name) {
            const std::chrono::time_zone* zone = nullptr;
            try {
                zone = db.locate_zone(name);
            } catch (const std::runtime_error&) {
                return;
            }
            s.add_oracle(name, oracle(*zone));
        });
        return source;
    }

} // namespace hegel::extras::chrono_tzdb
