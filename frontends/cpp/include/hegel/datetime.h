#pragma once

#include <array>
#include <chrono>
#include <cstdint>
#include <limits>
#include <optional>
#include <ostream>
#include <ratio>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

/// @cond INTERNAL
// HEGEL_HAS_CHRONO_CALENDAR: whether std::chrono::year_month_day is
// available. libstdc++ reports it through __cpp_lib_chrono; libc++ has had
// the calendar types since version 7 but kept the macro at its C++17 value.
#if defined(__cpp_lib_chrono) && __cpp_lib_chrono >= 201907L
#define HEGEL_HAS_CHRONO_CALENDAR 1
#elif defined(_LIBCPP_VERSION) && __cplusplus >= 202002L
#define HEGEL_HAS_CHRONO_CALENDAR 1
#else
#define HEGEL_HAS_CHRONO_CALENDAR 0
#endif
/// @endcond

/**
 * @brief Plain date and time values, as the engine draws them.
 *
 * Every type here is an aggregate of public fields mirroring the engine's
 * value model one to one, with no arithmetic and no parsing: a test that
 * uses a date library wraps the value it draws with
 * @ref hegel::generators::Generator::map "map", and nothing here competes
 * with that library. The generators over these types are
 * @ref hegel::generators::dates "dates()",
 * @ref hegel::generators::times "times()",
 * @ref hegel::generators::datetimes "datetimes()",
 * @ref hegel::generators::timestamps "timestamps()",
 * @ref hegel::generators::utc_offsets "utc_offsets()",
 * @ref hegel::generators::timezones "timezones()",
 * @ref hegel::generators::zoned_datetimes "zoned_datetimes()",
 * @ref hegel::generators::signed_durations "signed_durations()" and
 * @ref hegel::generators::spans "spans()"; the option enums of their
 * parameters live here too.
 *
 * Each `operator<<` prints the RFC 9557 form the engine prints a
 * counterexample in, so a failing value reads the same from every language
 * binding: `2024-03-10T01:30:00-05:00[America/New_York]`, with `[gap]` or
 * `[fold=0]`/`[fold=1]` appended to a skipped or repeated wall time
 * (`2024-03-10T02:30:00-04:00[America/New_York][gap]`), `-00:00` for an
 * unknown offset and `Z[Etc/Unknown]` for the unknown zone.
 *
 * The functions tzdb_version(), resolve(), offset_at() and info() query the
 * engine's bundled time zone database directly, outside any test.
 */
namespace hegel::datetime {

    /// The lowest year of a Date.
    inline constexpr int32_t MIN_YEAR = -999'999;
    /// The highest year of a Date.
    inline constexpr int32_t MAX_YEAR = 999'999;
    /// The first second of MIN_YEAR, the lowest `second` of a Timestamp.
    inline constexpr int64_t MIN_INSTANT = -31'619'087'596'800;
    /// The last second of MAX_YEAR, the highest `second` of a Timestamp.
    inline constexpr int64_t MAX_INSTANT = 31'494'784'780'799;
    /// The largest magnitude of a known UtcOffset in seconds, 25:59:59.
    inline constexpr int32_t MAX_OFFSET = 93'599;
    /// The name of the unknown time zone, whose instants are known and whose
    /// offset is not.
    inline constexpr std::string_view UNKNOWN_ZONE = "Etc/Unknown";

    /// @cond INTERNAL
    namespace detail {

        inline constexpr int64_t nanos_per_second = 1'000'000'000;

        template <typename Rep> bool fits(int64_t value) {
            static_assert(std::is_integral_v<Rep>,
                          "a chrono conversion needs an integral Rep");
            if constexpr (std::is_signed_v<Rep>) {
                return value >= std::numeric_limits<Rep>::min() &&
                       value <= std::numeric_limits<Rep>::max();
            } else {
                return value >= 0 && static_cast<uint64_t>(value) <=
                                         std::numeric_limits<Rep>::max();
            }
        }

        // The length of a chrono period in nanoseconds. The period must be
        // a whole number of nanoseconds that divides a second or is a
        // multiple of one, which every standard unit is.
        template <typename Period> struct period_nanos {
            using ratio = std::ratio_divide<Period, std::nano>;
            static_assert(ratio::den == 1,
                          "the period must be a whole number of nanoseconds");
            static constexpr int64_t value = ratio::num;
            static_assert(value >= nanos_per_second
                              ? value % nanos_per_second == 0
                              : nanos_per_second % value == 0,
                          "the period must divide a second or be a multiple "
                          "of one");
        };

        // The count of `unit_nanos`-nanosecond units in `seconds` plus
        // `nanos`, or nullopt when that is not a whole number or does not
        // fit in int64_t.
        std::optional<int64_t> units_from_parts(int64_t seconds, int64_t nanos,
                                                int64_t unit_nanos);

        // The seconds and nanoseconds of `count` units of `unit_nanos`
        // nanoseconds, the nanoseconds of the sign of `count` or, with
        // `floor`, non-negative; nullopt when the seconds do not fit in
        // int64_t.
        std::optional<std::pair<int64_t, int64_t>>
        parts_from_units(int64_t count, int64_t unit_nanos, bool floor);

        template <typename Duration>
        std::optional<Duration> duration_from_parts(int64_t seconds,
                                                    int64_t nanos) {
            std::optional<int64_t> count = units_from_parts(
                seconds, nanos, period_nanos<typename Duration::period>::value);
            if (!count || !fits<typename Duration::rep>(*count)) {
                return std::nullopt;
            }
            return Duration(static_cast<typename Duration::rep>(*count));
        }

        template <typename Rep, typename Period>
        std::optional<std::pair<int64_t, int64_t>>
        parts_from_duration(std::chrono::duration<Rep, Period> d, bool floor) {
            static_assert(std::is_integral_v<Rep>,
                          "a chrono conversion needs an integral Rep");
            if constexpr (std::is_unsigned_v<Rep>) {
                if (static_cast<uint64_t>(d.count()) >
                    static_cast<uint64_t>(
                        std::numeric_limits<int64_t>::max())) {
                    return std::nullopt;
                }
            }
            return parts_from_units(static_cast<int64_t>(d.count()),
                                    period_nanos<Period>::value, floor);
        }

    } // namespace detail
    /// @endcond

    /**
     * @brief A proleptic Gregorian calendar date.
     *
     * `year` is in `[MIN_YEAR, MAX_YEAR]` (±999,999), `month` in `[1, 12]`
     * and `day` in `[1, days in the month]`. Prints as `2024-03-10`, with a
     * six-digit signed year outside `0000` to `9999`.
     */
    struct Date {
        int32_t year;  ///< The year, negative before year 0.
        uint8_t month; ///< The month, 1 for January.
        uint8_t day;   ///< The day of the month, from 1.

#if HEGEL_HAS_CHRONO_CALENDAR
        /**
         * @brief The same date as a `std::chrono::year_month_day`, or
         * `std::nullopt` when the year lies outside `std::chrono::year`.
         *
         * @return The date, or `std::nullopt`.
         */
        std::optional<std::chrono::year_month_day> to_chrono() const {
            if (year < static_cast<int>(std::chrono::year::min()) ||
                year > static_cast<int>(std::chrono::year::max())) {
                return std::nullopt;
            }
            return std::chrono::year_month_day{std::chrono::year{year},
                                               std::chrono::month{month},
                                               std::chrono::day{day}};
        }

        /**
         * @brief The Date of a `std::chrono::year_month_day`, or
         * `std::nullopt` when it is not a valid date (`!ymd.ok()`).
         *
         * @param ymd The date to convert.
         * @return The date, or `std::nullopt`.
         */
        static std::optional<Date>
        from_chrono(const std::chrono::year_month_day& ymd) {
            if (!ymd.ok()) {
                return std::nullopt;
            }
            return Date{
                static_cast<int32_t>(static_cast<int>(ymd.year())),
                static_cast<uint8_t>(static_cast<unsigned>(ymd.month())),
                static_cast<uint8_t>(static_cast<unsigned>(ymd.day()))};
        }
#endif
    };

    /// @name Date comparison
    /// @{

    /// Compares two dates for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const Date& a, const Date& b) {
        return a.year == b.year && a.month == b.month && a.day == b.day;
    }
    /// Compares two dates for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const Date& a, const Date& b) { return !(a == b); }
    /// Compares two dates for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is before `b`.
    inline bool operator<(const Date& a, const Date& b) {
        if (a.year != b.year) {
            return a.year < b.year;
        }
        if (a.month != b.month) {
            return a.month < b.month;
        }
        return a.day < b.day;
    }
    /// Compares two dates for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is after `b`.
    inline bool operator>(const Date& a, const Date& b) { return b < a; }
    /// Compares two dates for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not after `b`.
    inline bool operator<=(const Date& a, const Date& b) { return !(b < a); }
    /// Compares two dates for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not before `b`.
    inline bool operator>=(const Date& a, const Date& b) { return !(a < b); }
    /// @}

    /// Prints as `2024-03-10` or, outside years 0000 to 9999, `+010000-01-01`.
    /// @param os The stream to print to.
    /// @param date The value to print.
    /// @return `os`.
    std::ostream& operator<<(std::ostream& os, const Date& date);

    /**
     * @brief A time of day from `00:00:00` to `23:59:59.999999999`.
     *
     * `hour` is in `[0, 23]`, `minute` and `second` in `[0, 59]` (no leap
     * seconds) and `nanosecond` in `[0, 999999999]`. Prints as `02:30:00`,
     * with the fraction appended when it is not zero: `02:30:00.5`.
     */
    struct Time {
        uint8_t hour;        ///< The hour of the day.
        uint8_t minute;      ///< The minute of the hour.
        uint8_t second;      ///< The second of the minute.
        uint32_t nanosecond; ///< The nanosecond of the second.
    };

    /// @name Time comparison
    /// @{

    /// Compares two times for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const Time& a, const Time& b) {
        return a.hour == b.hour && a.minute == b.minute &&
               a.second == b.second && a.nanosecond == b.nanosecond;
    }
    /// Compares two times for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const Time& a, const Time& b) { return !(a == b); }
    /// Compares two times for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is before `b`.
    inline bool operator<(const Time& a, const Time& b) {
        if (a.hour != b.hour) {
            return a.hour < b.hour;
        }
        if (a.minute != b.minute) {
            return a.minute < b.minute;
        }
        if (a.second != b.second) {
            return a.second < b.second;
        }
        return a.nanosecond < b.nanosecond;
    }
    /// Compares two times for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is after `b`.
    inline bool operator>(const Time& a, const Time& b) { return b < a; }
    /// Compares two times for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not after `b`.
    inline bool operator<=(const Time& a, const Time& b) { return !(b < a); }
    /// Compares two times for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not before `b`.
    inline bool operator>=(const Time& a, const Time& b) { return !(a < b); }
    /// @}

    /// Prints as `02:30:00` or `02:30:00.5`.
    /// @param os The stream to print to.
    /// @param time The value to print.
    /// @return `os`.
    std::ostream& operator<<(std::ostream& os, const Time& time);

    /**
     * @brief A wall-clock date and time with no zone or offset, in the
     * range of Date and Time. Prints as `2024-03-10T02:30:00`.
     */
    struct DateTime {
        Date date; ///< The calendar date.
        Time time; ///< The time of day.
    };

    /// @name DateTime comparison
    /// @{

    /// Compares two datetimes for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const DateTime& a, const DateTime& b) {
        return a.date == b.date && a.time == b.time;
    }
    /// Compares two datetimes for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const DateTime& a, const DateTime& b) {
        return !(a == b);
    }
    /// Compares two datetimes for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is before `b`.
    inline bool operator<(const DateTime& a, const DateTime& b) {
        if (a.date != b.date) {
            return a.date < b.date;
        }
        return a.time < b.time;
    }
    /// Compares two datetimes for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is after `b`.
    inline bool operator>(const DateTime& a, const DateTime& b) {
        return b < a;
    }
    /// Compares two datetimes for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not after `b`.
    inline bool operator<=(const DateTime& a, const DateTime& b) {
        return !(b < a);
    }
    /// Compares two datetimes for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not before `b`.
    inline bool operator>=(const DateTime& a, const DateTime& b) {
        return !(a < b);
    }
    /// @}

    /// Prints as `2024-03-10T02:30:00`.
    /// @param os The stream to print to.
    /// @param datetime The value to print.
    /// @return `os`.
    std::ostream& operator<<(std::ostream& os, const DateTime& datetime);

    /**
     * @brief An instant.
     *
     * `second`s since the Unix epoch in `[MIN_INSTANT, MAX_INSTANT]` (the
     * civil range of Date, about ±3.2 × 10¹⁶ seconds) and a `nanosecond` in
     * `[0, 999999999]` counted forward from that second, so -1.5 s is
     * `{-2, 500000000}`. Prints as the UTC wall time with a `Z` suffix:
     * `2000-01-01T00:00:00Z`.
     */
    struct Timestamp {
        int64_t second;      ///< Seconds since 1970-01-01T00:00:00Z.
        uint32_t nanosecond; ///< Nanoseconds after `second`.

        /**
         * @brief The same instant as a `std::chrono::system_clock` time
         * point, or `std::nullopt` when it is not a whole number of the
         * duration's units or does not fit in its representation.
         *
         * @tparam Duration A `std::chrono::duration` whose period is a
         *   standard unit; `std::chrono::nanoseconds` by default.
         * @return The time point, or `std::nullopt`.
         */
        template <typename Duration = std::chrono::nanoseconds>
        std::optional<
            std::chrono::time_point<std::chrono::system_clock, Duration>>
        to_chrono() const {
            std::optional<Duration> since_epoch =
                detail::duration_from_parts<Duration>(second, nanosecond);
            if (!since_epoch) {
                return std::nullopt;
            }
            return std::chrono::time_point<std::chrono::system_clock, Duration>(
                *since_epoch);
        }

        /**
         * @brief The Timestamp of a `std::chrono::system_clock` time point,
         * or `std::nullopt` when it lies outside `[MIN_INSTANT,
         * MAX_INSTANT]`.
         *
         * @param time_point The time point to convert.
         * @return The timestamp, or `std::nullopt`.
         */
        template <typename Rep, typename Period>
        static std::optional<Timestamp>
        from_chrono(std::chrono::time_point<std::chrono::system_clock,
                                            std::chrono::duration<Rep, Period>>
                        time_point) {
            std::optional<std::pair<int64_t, int64_t>> parts =
                detail::parts_from_duration(time_point.time_since_epoch(),
                                            /*floor=*/true);
            if (!parts || parts->first < MIN_INSTANT ||
                parts->first > MAX_INSTANT) {
                return std::nullopt;
            }
            return Timestamp{parts->first,
                             static_cast<uint32_t>(parts->second)};
        }
    };

    /// @name Timestamp comparison
    /// @{

    /// Compares two timestamps for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const Timestamp& a, const Timestamp& b) {
        return a.second == b.second && a.nanosecond == b.nanosecond;
    }
    /// Compares two timestamps for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const Timestamp& a, const Timestamp& b) {
        return !(a == b);
    }
    /// Compares two timestamps for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is before `b`.
    inline bool operator<(const Timestamp& a, const Timestamp& b) {
        if (a.second != b.second) {
            return a.second < b.second;
        }
        return a.nanosecond < b.nanosecond;
    }
    /// Compares two timestamps for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is after `b`.
    inline bool operator>(const Timestamp& a, const Timestamp& b) {
        return b < a;
    }
    /// Compares two timestamps for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not after `b`.
    inline bool operator<=(const Timestamp& a, const Timestamp& b) {
        return !(b < a);
    }
    /// Compares two timestamps for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not before `b`.
    inline bool operator>=(const Timestamp& a, const Timestamp& b) {
        return !(a < b);
    }
    /// @}

    /// Prints as `2000-01-01T00:00:00Z`.
    /// @param os The stream to print to.
    /// @param timestamp The value to print.
    /// @return `os`.
    std::ostream& operator<<(std::ostream& os, const Timestamp& timestamp);

    /**
     * @brief A UTC offset: a known number of seconds east of UTC within
     * ±25:59:59 (MAX_OFFSET), or the unknown offset of RFC 3339's `-00:00`,
     * which the offset and zone draws yield only when asked to.
     *
     * Prints as `+05:30`, `-00:44:30` (the seconds appended when they are
     * not zero) or `-00:00`.
     */
    struct UtcOffset {
        int32_t seconds = 0; ///< Seconds east of UTC; 0 when not `known`.
        bool known = true;   ///< False for the unknown offset.
    };

    /// The unknown offset, RFC 3339's `-00:00`.
    inline constexpr UtcOffset UNKNOWN_OFFSET{0, false};

    /// @name UtcOffset comparison
    /// @{

    /// Compares two offsets for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const UtcOffset& a, const UtcOffset& b) {
        return a.known == b.known && (!a.known || a.seconds == b.seconds);
    }
    /// Compares two offsets for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const UtcOffset& a, const UtcOffset& b) {
        return !(a == b);
    }
    /// @}

    /// Prints as `+05:30`, `-00:44:30` or `-00:00`.
    /// @param os The stream to print to.
    /// @param offset The value to print.
    /// @return `os`.
    std::ostream& operator<<(std::ostream& os, const UtcOffset& offset);

    /**
     * @brief A time zone of the bundled IANA database, or UNKNOWN_ZONE.
     *
     * `name` is the name as it was drawn or written, link names such as
     * `Asia/Calcutta` included, and `primary_name` the zone it resolves to
     * (`Asia/Kolkata`), or the name itself for a zone the engine's database
     * lacks and only the caller's knows. Prints as the name.
     */
    struct TimeZone {
        std::string name;         ///< The name as written.
        std::string primary_name; ///< The zone whose data applies.
    };

    /// @name TimeZone comparison
    /// @{

    /// Compares two zones for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const TimeZone& a, const TimeZone& b) {
        return a.name == b.name && a.primary_name == b.primary_name;
    }
    /// Compares two zones for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const TimeZone& a, const TimeZone& b) {
        return !(a == b);
    }
    /// @}

    /// Prints as the name.
    /// @param os The stream to print to.
    /// @param zone The value to print.
    /// @return `os`.
    std::ostream& operator<<(std::ostream& os, const TimeZone& zone);

    /// How a wall-clock time maps to instants in its zone.
    enum class Kind {
        Unique, ///< Exactly one instant.
        Gap,    ///< None: the clocks skipped it.
        Fold,   ///< Two: the clocks repeated it.
    };

    /**
     * @brief A wall-clock time in a time zone, resolved to an instant.
     *
     * Carries the Kind of the wall time, the offsets in force before and
     * after the transition it falls on (both the resolved offset for a
     * unique time), which repeat of a fold it means (`fold`, the later when
     * set), the resolved `offset` (unknown in UNKNOWN_ZONE and in a zone
     * only the caller's database knows) and the `timestamp`: the unique
     * instant, the one `fold` picks, or for a gap the wall time read with
     * `offset_before`. Prints as
     * `2024-03-10T01:30:00-05:00[America/New_York]`, with `[gap]`
     * (`2024-03-10T02:30:00-04:00[America/New_York][gap]`) or `[fold=0]` /
     * `[fold=1]` appended when relevant.
     */
    struct ZonedDateTime {
        TimeZone zone;         ///< The zone the wall time is read in.
        DateTime civil;        ///< The wall-clock time.
        Kind kind;             ///< How the wall time maps to instants.
        int32_t offset_before; ///< The offset before the transition.
        int32_t offset_after;  ///< The offset after the transition.
        bool fold;             ///< Which repeat of a fold is meant.
        UtcOffset offset;      ///< The offset in force at `timestamp`.
        Timestamp timestamp;   ///< The resolved instant.
    };

    /// @name ZonedDateTime comparison
    /// @{

    /// Compares two zoned datetimes for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const ZonedDateTime& a, const ZonedDateTime& b) {
        return a.zone == b.zone && a.civil == b.civil && a.kind == b.kind &&
               a.offset_before == b.offset_before &&
               a.offset_after == b.offset_after && a.fold == b.fold &&
               a.offset == b.offset && a.timestamp == b.timestamp;
    }
    /// Compares two zoned datetimes for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const ZonedDateTime& a, const ZonedDateTime& b) {
        return !(a == b);
    }
    /// @}

    /// Prints as `2024-03-10T02:30:00-04:00[America/New_York][gap]`.
    /// @param os The stream to print to.
    /// @param zoned The value to print.
    /// @return `os`.
    std::ostream& operator<<(std::ostream& os, const ZonedDateTime& zoned);

    /**
     * @brief A signed duration: `seconds` plus `nanoseconds` of the same
     * sign, the latter within ±999999999, so -1.5 s is `{-1, -500000000}`.
     *
     * Prints in the ISO 8601 duration form `PT1H30M5.5S`, with a leading `-`
     * when negative and `PT0S` for zero.
     */
    struct SignedDuration {
        int64_t seconds;     ///< Whole seconds.
        int32_t nanoseconds; ///< Nanoseconds, of the sign of `seconds`.

        /**
         * @brief The same duration as a `std::chrono::duration`, or
         * `std::nullopt` when it is not a whole number of the duration's
         * units or does not fit in its representation.
         *
         * @tparam Duration A `std::chrono::duration` whose period is a
         *   standard unit; `std::chrono::nanoseconds` by default.
         * @return The duration, or `std::nullopt`.
         */
        template <typename Duration = std::chrono::nanoseconds>
        std::optional<Duration> to_chrono() const {
            return detail::duration_from_parts<Duration>(seconds, nanoseconds);
        }

        /**
         * @brief The SignedDuration of a `std::chrono::duration`, or
         * `std::nullopt` when its seconds do not fit in `int64_t`.
         *
         * @param duration The duration to convert.
         * @return The duration, or `std::nullopt`.
         */
        template <typename Rep, typename Period>
        static std::optional<SignedDuration>
        from_chrono(std::chrono::duration<Rep, Period> duration) {
            std::optional<std::pair<int64_t, int64_t>> parts =
                detail::parts_from_duration(duration, /*floor=*/false);
            if (!parts) {
                return std::nullopt;
            }
            return SignedDuration{parts->first,
                                  static_cast<int32_t>(parts->second)};
        }
    };

    /// @name SignedDuration comparison
    /// @{

    /// Compares two durations for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const SignedDuration& a, const SignedDuration& b) {
        return a.seconds == b.seconds && a.nanoseconds == b.nanoseconds;
    }
    /// Compares two durations for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const SignedDuration& a, const SignedDuration& b) {
        return !(a == b);
    }
    /// Compares two durations for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is shorter than `b`.
    inline bool operator<(const SignedDuration& a, const SignedDuration& b) {
        if (a.seconds != b.seconds) {
            return a.seconds < b.seconds;
        }
        return a.nanoseconds < b.nanoseconds;
    }
    /// Compares two durations for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is longer than `b`.
    inline bool operator>(const SignedDuration& a, const SignedDuration& b) {
        return b < a;
    }
    /// Compares two durations for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not longer than `b`.
    inline bool operator<=(const SignedDuration& a, const SignedDuration& b) {
        return !(b < a);
    }
    /// Compares two durations for ordering.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not shorter than `b`.
    inline bool operator>=(const SignedDuration& a, const SignedDuration& b) {
        return !(a < b);
    }
    /// @}

    /// Prints as `PT1H30M5.5S`, `-PT1.5S` or `PT0S`.
    /// @param os The stream to print to.
    /// @param duration The value to print.
    /// @return `os`.
    std::ostream& operator<<(std::ostream& os, const SignedDuration& duration);

    /// A unit of a Span, largest first.
    enum class SpanUnit {
        Years,        ///< Calendar years.
        Months,       ///< Calendar months.
        Weeks,        ///< Weeks.
        Days,         ///< Days.
        Hours,        ///< Hours.
        Minutes,      ///< Minutes.
        Seconds,      ///< Seconds.
        Milliseconds, ///< Milliseconds.
        Microseconds, ///< Microseconds.
        Nanoseconds,  ///< Nanoseconds.
    };

    /// Every SpanUnit, largest first.
    inline constexpr std::array<SpanUnit, 10> ALL_SPAN_UNITS = {
        SpanUnit::Years,       SpanUnit::Months,       SpanUnit::Weeks,
        SpanUnit::Days,        SpanUnit::Hours,        SpanUnit::Minutes,
        SpanUnit::Seconds,     SpanUnit::Milliseconds, SpanUnit::Microseconds,
        SpanUnit::Nanoseconds,
    };

    /**
     * @brief A calendar span: a signed count of each unit from years to
     * nanoseconds, never normalised (90 minutes is not an hour and a half,
     * and a month is not a number of days).
     *
     * Every unit shares one sign unless the span was drawn with
     * SpanSign::Mixed. Prints each nonzero unit in the friendly form
     * `1y 2mo 3w 4d 5h 6m 7s 8ms 9µs 10ns`, with one leading `-` when every
     * unit is negative or zero, a `-` on each negative unit when the signs
     * are mixed, and `0s` for zero.
     */
    struct Span {
        int64_t years = 0;        ///< Calendar years.
        int64_t months = 0;       ///< Calendar months.
        int64_t weeks = 0;        ///< Weeks.
        int64_t days = 0;         ///< Days.
        int64_t hours = 0;        ///< Hours.
        int64_t minutes = 0;      ///< Minutes.
        int64_t seconds = 0;      ///< Seconds.
        int64_t milliseconds = 0; ///< Milliseconds.
        int64_t microseconds = 0; ///< Microseconds.
        int64_t nanoseconds = 0;  ///< Nanoseconds.

        /// The count of one unit in this span.
        /// @param unit The unit to read.
        /// @return The count of `unit`.
        int64_t get(SpanUnit unit) const;
    };

    /// @name Span comparison
    /// @{

    /// Compares two spans for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const Span& a, const Span& b) {
        return a.years == b.years && a.months == b.months &&
               a.weeks == b.weeks && a.days == b.days && a.hours == b.hours &&
               a.minutes == b.minutes && a.seconds == b.seconds &&
               a.milliseconds == b.milliseconds &&
               a.microseconds == b.microseconds &&
               a.nanoseconds == b.nanoseconds;
    }
    /// Compares two spans for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const Span& a, const Span& b) { return !(a == b); }
    /// @}

    /// Prints as `1y 2mo 3d`, `-1y 2mo`, `1y -2mo` or `0s`.
    /// @param os The stream to print to.
    /// @param span The value to print.
    /// @return `os`.
    std::ostream& operator<<(std::ostream& os, const Span& span);

    /// The unit the time part of a drawn value is counted in: the drawn
    /// value is a whole number of these units and shrinks in them.
    enum class Precision {
        Nanoseconds,  ///< Whole nanoseconds.
        Microseconds, ///< Whole microseconds.
        Milliseconds, ///< Whole milliseconds.
        Seconds,      ///< Whole seconds.
        Minutes,      ///< Whole minutes.
        Hours,        ///< Whole hours.
    };

    /// The values a UTC offset draw restricts itself to.
    enum class Granularity {
        Hours,        ///< Whole hours.
        QuarterHours, ///< Whole quarter hours.
        Minutes,      ///< Whole minutes.
        Seconds,      ///< Whole seconds.
    };

    /// A day of the week, for restricting a date draw.
    enum class Weekday {
        Monday,    ///< Monday.
        Tuesday,   ///< Tuesday.
        Wednesday, ///< Wednesday.
        Thursday,  ///< Thursday.
        Friday,    ///< Friday.
        Saturday,  ///< Saturday.
        Sunday,    ///< Sunday.
    };

    /// A month of the year, for restricting a date draw.
    enum class Month {
        January,   ///< January.
        February,  ///< February.
        March,     ///< March.
        April,     ///< April.
        May,       ///< May.
        June,      ///< June.
        July,      ///< July.
        August,    ///< August.
        September, ///< September.
        October,   ///< October.
        November,  ///< November.
        December,  ///< December.
    };

    /// Which instant a zoned datetime draw means by a wall time the clocks
    /// repeated.
    enum class Ambiguous {
        Both,    ///< Either, decided by a drawn fold that shrinks to the
                 ///< earlier.
        Earlier, ///< The earlier instant, with no fold drawn.
        Later,   ///< The later instant, with no fold drawn.
    };

    /// How the signs of a span's units are drawn.
    enum class SpanSign {
        Uniform,     ///< One sign for every unit.
        Mixed,       ///< Each unit signed on its own.
        NonNegative, ///< Magnitudes only; every enabled unit's lower bound
                     ///< must be at least 0.
    };

    /**
     * @brief A feature of a time zone, derived from its offsets and name.
     *
     * "After 1970" means at any instant from 1970-01-01T00:00:00Z on, the
     * rule in force after the last explicit transition included.
     */
    enum class Tag {
        Fixed,         ///< No transition after 1970.
        WholeHour,     ///< Every offset after 1970 is a whole number of hours.
        Fractional,    ///< Some offset after 1970 is a whole number of minutes
                       ///< but not of hours (`+05:30`, `+05:45`, `-03:30`).
        Seconds,       ///< Some offset in the zone's history is not a whole
                       ///< number of minutes (local mean time, Monrovia's
                       ///< `-00:44:30` until 1972).
        Dst,           ///< The current rule has a DST offset exactly one hour
                       ///< from the standard one.
        DstOther,      ///< The current rule has a DST offset of another size
                       ///< (Lord Howe's 30 minutes, Troll's two hours).
        Multi,         ///< Some calendar year has more than two offset changes.
        BaseChange,    ///< The lowest offset in force during some year after
                       ///< 1970 differs from the previous year's.
        Midnight,      ///< A transition after 1970 whose wall time before or
                       ///< after it is midnight.
        SpansMidnight, ///< A transition after 1970 whose wall times before
                       ///< and after it fall on different dates.
        WholeDay,      ///< A transition that skips or repeats a whole day
                       ///< (Pacific/Apia 2011).
        SameOffset,    ///< A transition after 1970 that changes the
                       ///< abbreviation but not the offset.
        Undefined,     ///< The zone uses the `-00` undefined-offset
                       ///< abbreviation.
        Link,          ///< The name is a link to another zone.
        Legacy,        ///< The name is not `Area/Location` for a current area
                       ///< (`EST5EDT`, `US/Eastern`).
        InvertedSign,  ///< The name is one of the `Etc/GMT+5` family, whose
                       ///< sign is POSIX's: `Etc/GMT+5` is five hours behind
                       ///< UTC.
    };

    /// Every Tag, in the order of the engine's bit set.
    inline constexpr std::array<Tag, 16> ALL_TAGS = {
        Tag::Fixed,     Tag::WholeHour,     Tag::Fractional, Tag::Seconds,
        Tag::Dst,       Tag::DstOther,      Tag::Multi,      Tag::BaseChange,
        Tag::Midnight,  Tag::SpansMidnight, Tag::WholeDay,   Tag::SameOffset,
        Tag::Undefined, Tag::Link,          Tag::Legacy,     Tag::InvertedSign,
    };

    /// What the bundled database knows about a time zone name; see info().
    struct ZoneInfo {
        /// The zone the name resolves to: the name itself unless it is a
        /// link.
        std::string primary_name;
        /// Whether the name is a link to `primary_name`.
        bool is_link;
        /// The name's position on the shrink ladder; `UTC` is 0.
        uint16_t rank;
        /// The zone's features, in ALL_TAGS order.
        std::vector<Tag> tags;
    };

    /// @name ZoneInfo comparison
    /// @{

    /// Compares two zone descriptions for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const ZoneInfo& a, const ZoneInfo& b) {
        return a.primary_name == b.primary_name && a.is_link == b.is_link &&
               a.rank == b.rank && a.tags == b.tags;
    }
    /// Compares two zone descriptions for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const ZoneInfo& a, const ZoneInfo& b) {
        return !(a == b);
    }
    /// @}

    /// The offset a time zone has in force at an instant; see offset_at().
    struct ZoneOffset {
        UtcOffset offset; ///< The offset in force.
        /// The database's daylight-saving flag for the offset, informational
        /// only.
        bool is_dst;
        /// The abbreviation in force, such as `EST`, or `-00` when the
        /// offset is unknown.
        std::string abbreviation;
    };

    /// @name ZoneOffset comparison
    /// @{

    /// Compares two zone offsets for equality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    inline bool operator==(const ZoneOffset& a, const ZoneOffset& b) {
        return a.offset == b.offset && a.is_dst == b.is_dst &&
               a.abbreviation == b.abbreviation;
    }
    /// Compares two zone offsets for inequality.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    inline bool operator!=(const ZoneOffset& a, const ZoneOffset& b) {
        return !(a == b);
    }
    /// @}

    /**
     * @brief The IANA release of the time zone database bundled in the
     * engine, such as `"2026e"`.
     *
     * A test that resolves zones with its own database can compare the two
     * and warn when they differ: the engine decides what to generate, the
     * other database what the value means.
     *
     * @return The release name.
     */
    std::string tzdb_version();

    /**
     * @brief What the bundled database knows about `name`, matched exactly
     * as the database spells it, or `std::nullopt` for a name it lacks.
     *
     * UNKNOWN_ZONE is known, with itself as primary name, Tag::Undefined and
     * the highest rank.
     *
     * @param name The zone name to look up.
     * @return The description, or `std::nullopt`.
     */
    std::optional<ZoneInfo> info(std::string_view name);

    /**
     * @brief Classify the wall-clock time `civil` in the zone `name` and
     * resolve it to an instant, meaning the earlier instant of a repeated
     * wall time (`fold` false); the later is the wall time read with
     * `offset_after`.
     *
     * @param name The zone name.
     * @param civil The wall-clock time to resolve.
     * @return The resolved zoned datetime.
     * @throws std::invalid_argument for a zone the bundled database lacks, a
     *   `civil` out of range, or one whose instant in the zone lies outside
     *   the range of Timestamp, which a wall time within an offset's width
     *   of the civil range's ends can do.
     */
    ZonedDateTime resolve(std::string_view name, const DateTime& civil);

    /**
     * @brief The offset the zone `name` has in force at `timestamp`.
     *
     * @param name The zone name.
     * @param timestamp The instant to look up.
     * @return The offset in force.
     * @throws std::invalid_argument for a zone the bundled database lacks or
     *   a `timestamp` out of range.
     */
    ZoneOffset offset_at(std::string_view name, const Timestamp& timestamp);

} // namespace hegel::datetime
