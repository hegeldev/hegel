#pragma once

/**
 * @cond INTERNAL
 *
 * Conversions between the hegel::datetime value types and the libhegel C
 * structs, shared by datetime.cpp and generators_datetime.cpp.
 */

#include <hegel/datetime.h>

#include <hegel.h>

#include <string>

namespace hegel::impl {

    hegel_date_t date_to_c(const datetime::Date& date);
    datetime::Date date_from_c(const hegel_date_t& date);
    hegel_time_t time_to_c(const datetime::Time& time);
    datetime::Time time_from_c(const hegel_time_t& time);
    hegel_datetime_t datetime_to_c(const datetime::DateTime& datetime);
    datetime::DateTime datetime_from_c(const hegel_datetime_t& datetime);
    hegel_timestamp_t timestamp_to_c(const datetime::Timestamp& timestamp);
    datetime::Timestamp timestamp_from_c(const hegel_timestamp_t& timestamp);
    hegel_duration_t duration_to_c(const datetime::SignedDuration& duration);
    datetime::SignedDuration duration_from_c(const hegel_duration_t& duration);
    datetime::UtcOffset offset_from_c(int32_t seconds, bool known);
    datetime::Kind kind_from_c(hegel_timezone_kind_t kind);
    /// The zone `name` names, with the primary name the bundled database
    /// reports, or the name itself for a zone only the caller knows.
    datetime::TimeZone zone_named(std::string name);

} // namespace hegel::impl

/// @endcond
