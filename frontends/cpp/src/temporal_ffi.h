#pragma once

/**
 * @cond INTERNAL
 *
 * Conversions between the hegel::generators::temporal value types and the
 * libhegel C structs, shared by temporal.cpp and generators_temporal.cpp.
 */

#include <hegel/generators/temporal_types.h>
#include <hegel/generators/zone_source.h>

#include <hegel.h>

#include <string>

namespace hegel::impl {

    hegel_date_t date_to_c(const generators::temporal::Date& date);
    generators::temporal::Date date_from_c(const hegel_date_t& date);
    hegel_time_t time_to_c(const generators::temporal::Time& time);
    generators::temporal::Time time_from_c(const hegel_time_t& time);
    hegel_datetime_t
    datetime_to_c(const generators::temporal::DateTime& datetime);
    generators::temporal::DateTime
    datetime_from_c(const hegel_datetime_t& datetime);
    hegel_timestamp_t
    timestamp_to_c(const generators::temporal::Instant& timestamp);
    generators::temporal::Instant
    timestamp_from_c(const hegel_timestamp_t& timestamp);
    hegel_duration_t
    duration_to_c(const generators::temporal::SignedDuration& duration);
    generators::temporal::SignedDuration
    duration_from_c(const hegel_duration_t& duration);
    generators::temporal::UtcOffset offset_from_c(int32_t seconds, bool known);
    generators::temporal::TransitionKind
    kind_from_c(hegel_timezone_kind_t kind);
    /// The engine handle of a zone source.
    const hegel_zone_source_t*
    source_handle(const generators::temporal::ZoneSource& source);
    /// The zone `name` names, with the primary name `source` reports, or
    /// the name itself for a zone only the caller knows.
    generators::temporal::TimeZone
    zone_named(std::string name,
               const generators::temporal::ZoneSource& source);

} // namespace hegel::impl

/// @endcond
