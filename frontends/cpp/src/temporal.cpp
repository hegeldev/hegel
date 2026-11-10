#include <hegel/generators/temporal_types.h>

#include <engine.h>
#include <hegel.h>
#include <temporal_ffi.h>

#include <cstdint>
#include <limits>
#include <optional>
#include <string>
#include <utility>

namespace hegel::generators::temporal {

    namespace detail {

        std::optional<int64_t> units_from_parts(int64_t seconds, int64_t nanos,
                                                int64_t unit_nanos) {
            if (unit_nanos >= nanos_per_second) {
                int64_t seconds_per_unit = unit_nanos / nanos_per_second;
                if (nanos != 0 || seconds % seconds_per_unit != 0) {
                    return std::nullopt;
                }
                return seconds / seconds_per_unit;
            }
            int64_t units_per_second = nanos_per_second / unit_nanos;
            // An instant before the epoch counts its nanoseconds forward
            // from a negative second. Counted back from the next second,
            // the two parts have one sign, and neither the product nor the
            // sum below leaves the range unless the result does.
            if (seconds < 0 && nanos > 0) {
                seconds += 1;
                nanos -= nanos_per_second;
            }
            if (nanos % unit_nanos != 0 ||
                seconds >
                    std::numeric_limits<int64_t>::max() / units_per_second ||
                seconds <
                    std::numeric_limits<int64_t>::min() / units_per_second) {
                return std::nullopt;
            }
            int64_t whole = seconds * units_per_second;
            int64_t fraction = nanos / unit_nanos;
            if ((fraction > 0 &&
                 whole > std::numeric_limits<int64_t>::max() - fraction) ||
                (fraction < 0 &&
                 whole < std::numeric_limits<int64_t>::min() - fraction)) {
                return std::nullopt;
            }
            return whole + fraction;
        }

        std::optional<std::pair<int64_t, int64_t>>
        parts_from_units(int64_t count, int64_t unit_nanos, bool floor) {
            if (unit_nanos >= nanos_per_second) {
                int64_t seconds_per_unit = unit_nanos / nanos_per_second;
                if (count > std::numeric_limits<int64_t>::max() /
                                seconds_per_unit ||
                    count < std::numeric_limits<int64_t>::min() /
                                seconds_per_unit) {
                    return std::nullopt;
                }
                return std::pair<int64_t, int64_t>{count * seconds_per_unit, 0};
            }
            int64_t units_per_second = nanos_per_second / unit_nanos;
            int64_t seconds = count / units_per_second;
            int64_t remainder = count % units_per_second;
            if (floor && remainder < 0) {
                seconds -= 1;
                remainder += units_per_second;
            }
            return std::pair<int64_t, int64_t>{seconds, remainder * unit_nanos};
        }

    } // namespace detail

    std::string to_string(const Date& date) {
        return impl::format_date(impl::date_to_c(date));
    }

    std::string to_string(const Time& time) {
        return impl::format_time(impl::time_to_c(time));
    }

    std::string to_string(const DateTime& datetime) {
        return impl::format_datetime(impl::datetime_to_c(datetime));
    }

    std::string to_string(const Instant& instant) {
        return impl::format_timestamp(impl::timestamp_to_c(instant));
    }

    std::string to_string(const UtcOffset& offset) {
        return impl::format_offset(impl::offset_to_c(offset));
    }

    std::string to_string(const TimeZone& zone) { return zone.name; }

    std::string to_string(const ZonedDateTime& zoned) {
        hegel_zoned_datetime_t c{};
        c.civil = impl::datetime_to_c(zoned.civil);
        hegel_offset_t offset = impl::offset_to_c(zoned.offset);
        c.offset_seconds = offset.seconds;
        c.offset_known = offset.known;
        c.fold = zoned.fold;
        switch (zoned.kind) {
        case TransitionKind::Gap:
            c.kind = HEGEL_TIMEZONE_KIND_GAP;
            break;
        case TransitionKind::Fold:
            c.kind = HEGEL_TIMEZONE_KIND_FOLD;
            break;
        default:
            c.kind = HEGEL_TIMEZONE_KIND_UNIQUE;
            break;
        }
        return impl::format_zoned_datetime(c, zoned.zone.name);
    }

    std::string to_string(const SignedDuration& duration) {
        return impl::format_duration(impl::duration_to_c(duration));
    }

    std::string to_string(const Span& span) {
        return impl::format_span(hegel_span_t{
            span.years, span.months, span.weeks, span.days, span.hours,
            span.minutes, span.seconds, span.milliseconds, span.microseconds,
            span.nanoseconds});
    }

    int64_t Span::get(SpanUnit unit) const {
        switch (unit) {
        case SpanUnit::Years:
            return years;
        case SpanUnit::Months:
            return months;
        case SpanUnit::Weeks:
            return weeks;
        case SpanUnit::Days:
            return days;
        case SpanUnit::Hours:
            return hours;
        case SpanUnit::Minutes:
            return minutes;
        case SpanUnit::Seconds:
            return seconds;
        case SpanUnit::Milliseconds:
            return milliseconds;
        case SpanUnit::Microseconds:
            return microseconds;
        default:
            return nanoseconds;
        }
    }

    std::string tzdb_version() { return impl::tzdb_version(); }

} // namespace hegel::generators::temporal

namespace hegel::impl {

    hegel_date_t date_to_c(const generators::temporal::Date& date) {
        return hegel_date_t{date.year, date.month, date.day};
    }

    generators::temporal::Date date_from_c(const hegel_date_t& date) {
        return generators::temporal::Date{date.year, date.month, date.day};
    }

    hegel_time_t time_to_c(const generators::temporal::Time& time) {
        return hegel_time_t{time.hour, time.minute, time.second,
                            time.nanosecond};
    }

    generators::temporal::Time time_from_c(const hegel_time_t& time) {
        return generators::temporal::Time{time.hour, time.minute, time.second,
                                          time.nanosecond};
    }

    hegel_datetime_t
    datetime_to_c(const generators::temporal::DateTime& datetime) {
        return hegel_datetime_t{date_to_c(datetime.date),
                                time_to_c(datetime.time)};
    }

    generators::temporal::DateTime
    datetime_from_c(const hegel_datetime_t& datetime) {
        return generators::temporal::DateTime{date_from_c(datetime.date),
                                              time_from_c(datetime.time)};
    }

    hegel_timestamp_t
    timestamp_to_c(const generators::temporal::Instant& timestamp) {
        return hegel_timestamp_t{timestamp.second, timestamp.nanosecond};
    }

    generators::temporal::Instant
    timestamp_from_c(const hegel_timestamp_t& timestamp) {
        return generators::temporal::Instant{timestamp.second,
                                             timestamp.nanosecond};
    }

    hegel_duration_t
    duration_to_c(const generators::temporal::SignedDuration& duration) {
        return hegel_duration_t{duration.seconds, duration.nanoseconds};
    }

    generators::temporal::SignedDuration
    duration_from_c(const hegel_duration_t& duration) {
        return generators::temporal::SignedDuration{duration.seconds,
                                                    duration.nanoseconds};
    }

    hegel_offset_t offset_to_c(const generators::temporal::UtcOffset& offset) {
        return hegel_offset_t{offset.known ? offset.seconds : 0, offset.known};
    }

    generators::temporal::UtcOffset offset_from_c(int32_t seconds, bool known) {
        return known ? generators::temporal::UtcOffset{seconds, true}
                     : generators::temporal::UNKNOWN_OFFSET;
    }

    generators::temporal::TransitionKind
    kind_from_c(hegel_timezone_kind_t kind) {
        switch (kind) {
        case HEGEL_TIMEZONE_KIND_GAP:
            return generators::temporal::TransitionKind::Gap;
        case HEGEL_TIMEZONE_KIND_FOLD:
            return generators::temporal::TransitionKind::Fold;
        default:
            return generators::temporal::TransitionKind::Unique;
        }
    }

} // namespace hegel::impl
