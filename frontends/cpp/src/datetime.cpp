#include <hegel/datetime.h>

#include <datetime_ffi.h>
#include <engine.h>
#include <hegel.h>

#include <cstdint>
#include <cstdio>
#include <limits>
#include <optional>
#include <ostream>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace hegel::datetime {

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

    namespace {

        constexpr int64_t seconds_per_day = 86'400;

        void write_year(std::ostream& os, int64_t year) {
            char buf[16];
            if (year >= 0 && year <= 9999) {
                std::snprintf(buf, sizeof(buf), "%04lld",
                              static_cast<long long>(year));
            } else {
                std::snprintf(buf, sizeof(buf), "%+07lld",
                              static_cast<long long>(year));
            }
            os << buf;
        }

        void write_two(std::ostream& os, unsigned value) {
            char buf[8];
            std::snprintf(buf, sizeof(buf), "%02u", value);
            os << buf;
        }

        void write_fraction(std::ostream& os, uint32_t nanosecond) {
            if (nanosecond == 0) {
                return;
            }
            char buf[16];
            std::snprintf(buf, sizeof(buf), "%09u", nanosecond);
            std::string digits(buf);
            while (digits.back() == '0') {
                digits.pop_back();
            }
            os << '.' << digits;
        }

        void write_date(std::ostream& os, int64_t year, unsigned month,
                        unsigned day) {
            write_year(os, year);
            os << '-';
            write_two(os, month);
            os << '-';
            write_two(os, day);
        }

        void write_time(std::ostream& os, const Time& time) {
            write_two(os, time.hour);
            os << ':';
            write_two(os, time.minute);
            os << ':';
            write_two(os, time.second);
            write_fraction(os, time.nanosecond);
        }

        // Howard Hinnant's civil_from_days.
        void civil_from_days(int64_t days, int64_t& year, unsigned& month,
                             unsigned& day) {
            int64_t z = days + 719'468;
            int64_t era = (z >= 0 ? z : z - 146'096) / 146'097;
            int64_t doe = z - era * 146'097;
            int64_t yoe =
                (doe - doe / 1460 + doe / 36'524 - doe / 146'096) / 365;
            int64_t doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            int64_t mp = (5 * doy + 2) / 153;
            day = static_cast<unsigned>(doy - (153 * mp + 2) / 5 + 1);
            month = static_cast<unsigned>(mp < 10 ? mp + 3 : mp - 9);
            year = yoe + era * 400 + (month <= 2 ? 1 : 0);
        }

    } // namespace

    std::ostream& operator<<(std::ostream& os, const Date& date) {
        write_date(os, date.year, date.month, date.day);
        return os;
    }

    std::ostream& operator<<(std::ostream& os, const Time& time) {
        write_time(os, time);
        return os;
    }

    std::ostream& operator<<(std::ostream& os, const DateTime& datetime) {
        return os << datetime.date << 'T' << datetime.time;
    }

    std::ostream& operator<<(std::ostream& os, const Timestamp& timestamp) {
        int64_t days = timestamp.second / seconds_per_day;
        int64_t second_of_day = timestamp.second % seconds_per_day;
        if (second_of_day < 0) {
            days -= 1;
            second_of_day += seconds_per_day;
        }
        int64_t year = 0;
        unsigned month = 0;
        unsigned day = 0;
        civil_from_days(days, year, month, day);
        write_date(os, year, month, day);
        os << 'T';
        write_time(os, Time{static_cast<uint8_t>(second_of_day / 3600),
                            static_cast<uint8_t>(second_of_day / 60 % 60),
                            static_cast<uint8_t>(second_of_day % 60),
                            timestamp.nanosecond});
        return os << 'Z';
    }

    std::ostream& operator<<(std::ostream& os, const UtcOffset& offset) {
        if (!offset.known) {
            return os << "-00:00";
        }
        os << (offset.seconds < 0 ? '-' : '+');
        auto magnitude = static_cast<unsigned>(
            offset.seconds < 0 ? -static_cast<int64_t>(offset.seconds)
                               : offset.seconds);
        write_two(os, magnitude / 3600);
        os << ':';
        write_two(os, magnitude / 60 % 60);
        if (magnitude % 60 != 0) {
            os << ':';
            write_two(os, magnitude % 60);
        }
        return os;
    }

    std::ostream& operator<<(std::ostream& os, const TimeZone& zone) {
        return os << zone.name;
    }

    std::ostream& operator<<(std::ostream& os, const ZonedDateTime& zoned) {
        os << zoned.civil;
        if (zoned.zone.name == UNKNOWN_ZONE) {
            os << 'Z';
        } else {
            os << zoned.offset;
        }
        os << '[' << zoned.zone << ']';
        switch (zoned.kind) {
        case Kind::Gap:
            os << "[gap]";
            break;
        case Kind::Fold:
            os << "[fold=" << (zoned.fold ? 1 : 0) << ']';
            break;
        default:
            break;
        }
        return os;
    }

    std::ostream& operator<<(std::ostream& os, const SignedDuration& duration) {
        if (duration.seconds < 0 || duration.nanoseconds < 0) {
            os << '-';
        }
        os << "PT";
        auto seconds = static_cast<uint64_t>(duration.seconds);
        if (duration.seconds < 0) {
            seconds = static_cast<uint64_t>(0) - seconds;
        }
        uint64_t hours = seconds / 3600;
        uint64_t minutes = seconds / 60 % 60;
        seconds %= 60;
        auto nanos = static_cast<uint32_t>(duration.nanoseconds < 0
                                               ? -duration.nanoseconds
                                               : duration.nanoseconds);
        if (hours != 0) {
            os << hours << 'H';
        }
        if (minutes != 0) {
            os << minutes << 'M';
        }
        if (seconds != 0 || nanos != 0 || (hours == 0 && minutes == 0)) {
            os << seconds;
            write_fraction(os, nanos);
            os << 'S';
        }
        return os;
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

    std::ostream& operator<<(std::ostream& os, const Span& span) {
        static constexpr const char* suffixes[] = {"y", "mo", "w",  "d",  "h",
                                                   "m", "s",  "ms", "µs", "ns"};
        bool any_negative = false;
        bool any_positive = false;
        for (SpanUnit unit : ALL_SPAN_UNITS) {
            int64_t value = span.get(unit);
            any_negative = any_negative || value < 0;
            any_positive = any_positive || value > 0;
        }
        bool uniform_negative = any_negative && !any_positive;
        if (uniform_negative) {
            os << '-';
        }
        bool written = false;
        for (size_t i = 0; i < ALL_SPAN_UNITS.size(); ++i) {
            int64_t value = span.get(ALL_SPAN_UNITS[i]);
            if (value == 0) {
                continue;
            }
            if (written) {
                os << ' ';
            }
            written = true;
            if (uniform_negative) {
                os << static_cast<uint64_t>(0) - static_cast<uint64_t>(value);
            } else {
                os << value;
            }
            os << suffixes[i];
        }
        if (!written) {
            os << "0s";
        }
        return os;
    }

    std::string tzdb_version() { return impl::tzdb_version(); }

    std::optional<ZoneInfo> info(std::string_view name) {
        hegel_timezone_info_t c = impl::timezone_info(name);
        if (!c.known) {
            return std::nullopt;
        }
        std::vector<Tag> tags;
        for (size_t bit = 0; bit < ALL_TAGS.size(); ++bit) {
            if ((c.tags & (1u << bit)) != 0) {
                tags.push_back(ALL_TAGS[bit]);
            }
        }
        return ZoneInfo{std::string(c.primary_name.data, c.primary_name.len),
                        c.is_link, c.rank, std::move(tags)};
    }

    ZonedDateTime resolve(std::string_view name, const DateTime& civil) {
        hegel_timezone_resolution_t r =
            impl::timezone_resolve(name, impl::datetime_to_c(civil));
        return ZonedDateTime{
            impl::zone_named(std::string(name)),
            civil,
            impl::kind_from_c(r.kind),
            r.offset_before,
            r.offset_after,
            false,
            impl::offset_from_c(r.offset_seconds, r.offset_known),
            impl::timestamp_from_c(r.timestamp)};
    }

    ZoneOffset offset_at(std::string_view name, const Timestamp& timestamp) {
        hegel_timezone_offset_t o =
            impl::timezone_offset_at(name, impl::timestamp_to_c(timestamp));
        return ZoneOffset{impl::offset_from_c(o.offset_seconds, o.offset_known),
                          o.is_dst,
                          std::string(o.abbreviation.data, o.abbreviation.len)};
    }

} // namespace hegel::datetime

namespace hegel::impl {

    hegel_date_t date_to_c(const datetime::Date& date) {
        return hegel_date_t{date.year, date.month, date.day};
    }

    datetime::Date date_from_c(const hegel_date_t& date) {
        return datetime::Date{date.year, date.month, date.day};
    }

    hegel_time_t time_to_c(const datetime::Time& time) {
        return hegel_time_t{time.hour, time.minute, time.second,
                            time.nanosecond};
    }

    datetime::Time time_from_c(const hegel_time_t& time) {
        return datetime::Time{time.hour, time.minute, time.second,
                              time.nanosecond};
    }

    hegel_datetime_t datetime_to_c(const datetime::DateTime& datetime) {
        return hegel_datetime_t{date_to_c(datetime.date),
                                time_to_c(datetime.time)};
    }

    datetime::DateTime datetime_from_c(const hegel_datetime_t& datetime) {
        return datetime::DateTime{date_from_c(datetime.date),
                                  time_from_c(datetime.time)};
    }

    hegel_timestamp_t timestamp_to_c(const datetime::Timestamp& timestamp) {
        return hegel_timestamp_t{timestamp.second, timestamp.nanosecond};
    }

    datetime::Timestamp timestamp_from_c(const hegel_timestamp_t& timestamp) {
        return datetime::Timestamp{timestamp.second, timestamp.nanosecond};
    }

    hegel_duration_t duration_to_c(const datetime::SignedDuration& duration) {
        return hegel_duration_t{duration.seconds, duration.nanoseconds};
    }

    datetime::SignedDuration duration_from_c(const hegel_duration_t& duration) {
        return datetime::SignedDuration{duration.seconds, duration.nanoseconds};
    }

    datetime::UtcOffset offset_from_c(int32_t seconds, bool known) {
        return known ? datetime::UtcOffset{seconds, true}
                     : datetime::UNKNOWN_OFFSET;
    }

    datetime::Kind kind_from_c(hegel_timezone_kind_t kind) {
        switch (kind) {
        case HEGEL_TIMEZONE_KIND_GAP:
            return datetime::Kind::Gap;
        case HEGEL_TIMEZONE_KIND_FOLD:
            return datetime::Kind::Fold;
        default:
            return datetime::Kind::Unique;
        }
    }

    datetime::TimeZone zone_named(std::string name) {
        std::string primary_name = name;
        if (std::optional<datetime::ZoneInfo> zone_info =
                datetime::info(name)) {
            primary_name = std::move(zone_info->primary_name);
        }
        return datetime::TimeZone{std::move(name), std::move(primary_name)};
    }

} // namespace hegel::impl
