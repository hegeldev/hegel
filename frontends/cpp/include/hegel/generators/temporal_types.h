#pragma once

#include <algorithm>
#include <array>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <limits>
#include <optional>
#include <ostream>
#include <ratio>
#include <string>
#include <string_view>
#include <tuple>
#include <type_traits>
#include <utility>
#include <vector>

/**
 * @brief Date and time generators, and the plain values they draw.
 *
 * Every value type here is an aggregate of public fields mirroring the engine's
 * value model one to one, with no arithmetic and no parsing: a test that
 * uses a date library wraps the value it draws with
 * @ref hegel::generators::Generator::map "map", and nothing here competes
 * with that library. The generators over these types are
 * @ref hegel::generators::temporal::dates "dates()",
 * @ref hegel::generators::temporal::times "times()",
 * @ref hegel::generators::temporal::datetimes "datetimes()",
 * @ref hegel::generators::temporal::instants "instants()",
 * @ref hegel::generators::temporal::offsets "offsets()",
 * @ref hegel::generators::temporal::zones "zones()",
 * @ref hegel::generators::temporal::zoned_datetimes "zoned_datetimes()",
 * @ref hegel::generators::temporal::durations "durations()" and
 * @ref hegel::generators::temporal::spans "spans()", and the parameter structs
 * and option enums they take.
 *
 * Each value type compares, orders and hashes, and to_string() and
 * `operator<<` give the text the engine defines for it, in the form of RFC
 * 9557: `2024-03-10T01:30:00-05:00[America/New_York]`, with `[gap]` or
 * `[fold=0]`/`[fold=1]` appended to a skipped or repeated wall time
 * (`2024-03-10T02:30:00-04:00[America/New_York][gap]`), `-00:00` for an
 * unknown offset and `Z[Etc/Unknown]` for the unknown zone. A failure
 * report shows a drawn value as its type and that text, such as
 * `hegel::generators::temporal::Date{"2024-03-10"}`.
 *
 * The functions resolve(), offset_at() and info() query the default zone
 * source directly, outside any test: the engine's bundled IANA database
 * unless set_default_zone_source() set another. A ZoneSource answers the
 * same queries for its own data, and tzdb_version() names the bundled
 * release.
 *
 * Every zone draw resolves wall times against a zone source. C++ has no
 * time zone database that every toolchain ships, so the default is the
 * engine's bundled one. To draw against the database the code under test
 * uses, build a ZoneSource from it (`hegel/extras/boost_date_time.h` does
 * this for Boost.DateTime, and `hegel/extras/chrono_tzdb.h` for a standard
 * library with `std::chrono::tzdb`) and pass it to zones() and
 * zoned_datetimes(), or make it the default with
 * set_default_zone_source(). A generator reads the default when it is
 * built, and a query when it is called.
 */
namespace hegel::generators::temporal {

    /// The lowest year of a Date.
    inline constexpr int32_t MIN_YEAR = -999'999;
    /// The highest year of a Date.
    inline constexpr int32_t MAX_YEAR = 999'999;
    /// The first second of MIN_YEAR, the lowest `second` of an Instant.
    inline constexpr int64_t MIN_INSTANT = -31'619'087'596'800;
    /// The last second of MAX_YEAR, the highest `second` of an Instant.
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
        // fit in int64_t. `nanos` is within one second of zero, of either
        // sign.
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
        int32_t year = 1970; ///< The year, negative before year 0.
        uint8_t month = 1;   ///< The month, 1 for January.
        uint8_t day = 1;     ///< The day of the month, from 1.

        /**
         * @brief The same date as a calendar date of another library, or
         * `std::nullopt` when the year lies outside the range of that
         * library's year.
         *
         * @code{.cpp}
         * auto ymd = date.to_chrono<std::chrono::year_month_day>();
         * @endcode
         *
         * @tparam YearMonthDay `std::chrono::year_month_day` (C++20), or a
         *   type with its interface, such as `date::year_month_day`.
         * @return The date, or `std::nullopt`.
         */
        template <typename YearMonthDay>
        std::optional<YearMonthDay> to_chrono() const {
            using Year = decltype(std::declval<const YearMonthDay&>().year());
            using Month = decltype(std::declval<const YearMonthDay&>().month());
            using Day = decltype(std::declval<const YearMonthDay&>().day());
            if (year < static_cast<int>((Year::min)()) ||
                year > static_cast<int>((Year::max)())) {
                return std::nullopt;
            }
            return YearMonthDay{Year{year}, Month{month}, Day{day}};
        }

        /**
         * @brief The Date of a calendar date of another library, or
         * `std::nullopt` when it is not a valid date (`!ymd.ok()`).
         *
         * @tparam YearMonthDay `std::chrono::year_month_day` (C++20), or a
         *   type with its interface, such as `date::year_month_day`.
         * @param ymd The date to convert.
         * @return The date, or `std::nullopt`.
         */
        template <typename YearMonthDay>
        static std::optional<Date> from_chrono(const YearMonthDay& ymd) {
            if (!ymd.ok()) {
                return std::nullopt;
            }
            return Date{
                static_cast<int32_t>(static_cast<int>(ymd.year())),
                static_cast<uint8_t>(static_cast<unsigned>(ymd.month())),
                static_cast<uint8_t>(static_cast<unsigned>(ymd.day()))};
        }
    };

    /**
     * @brief A time of day from `00:00:00` to `23:59:59.999999999`.
     *
     * `hour` is in `[0, 23]`, `minute` and `second` in `[0, 59]` (no leap
     * seconds) and `nanosecond` in `[0, 999999999]`. Prints as `02:30:00`,
     * with the fraction appended when it is not zero: `02:30:00.5`.
     */
    struct Time {
        uint8_t hour = 0;        ///< The hour of the day.
        uint8_t minute = 0;      ///< The minute of the hour.
        uint8_t second = 0;      ///< The second of the minute.
        uint32_t nanosecond = 0; ///< The nanosecond of the second.
    };

    /**
     * @brief A wall-clock date and time with no zone or offset, in the
     * range of Date and Time. Prints as `2024-03-10T02:30:00`.
     */
    struct DateTime {
        Date date; ///< The calendar date.
        Time time; ///< The time of day.
    };

    /**
     * @brief An instant.
     *
     * `second`s since the Unix epoch in `[MIN_INSTANT, MAX_INSTANT]` (the
     * civil range of Date, about ±3.2 × 10¹³ seconds) and a `nanosecond` in
     * `[0, 999999999]` counted forward from that second, so -1.5 s is
     * `{-2, 500000000}`. Prints as the UTC wall time with a `Z` suffix:
     * `2000-01-01T00:00:00Z`.
     */
    struct Instant {
        int64_t second = 0;      ///< Seconds since 1970-01-01T00:00:00Z.
        uint32_t nanosecond = 0; ///< Nanoseconds after `second`.

        /**
         * @brief The same instant as a `std::chrono::system_clock` time
         * point, or `std::nullopt` when it is not a whole number of the
         * duration's units or does not fit in its representation.
         *
         * The range of Instant is wider than that of most time points: a
         * 64-bit count of nanoseconds holds the years 1678 to 2262 only,
         * and one of microseconds about ±292,000 years. Bound the
         * generator, or name a coarser duration, when the conversion must
         * succeed.
         *
         * @code{.cpp}
         * auto at = instant.to_chrono<std::chrono::system_clock::duration>();
         * @endcode
         *
         * @tparam Duration A `std::chrono::duration` whose period is a
         *   standard unit, such as `std::chrono::seconds`.
         * @return The time point, or `std::nullopt`.
         */
        template <typename Duration>
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
         * @brief The Instant of a `std::chrono::system_clock` time point,
         * or `std::nullopt` when it lies outside `[MIN_INSTANT,
         * MAX_INSTANT]`.
         *
         * @param time_point The time point to convert.
         * @return The instant, or `std::nullopt`.
         */
        template <typename Rep, typename Period>
        static std::optional<Instant>
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
            return Instant{parts->first, static_cast<uint32_t>(parts->second)};
        }
    };

    /**
     * @brief A UTC offset: a known number of seconds east of UTC within
     * ±25:59:59 (MAX_OFFSET), or the unknown offset of RFC 3339's `-00:00`,
     * which the offset and zone draws yield only when asked to.
     *
     * Prints as `+05:30`, `-00:44:30` (the seconds appended when they are
     * not zero) or `-00:00`. Two unknown offsets are equal whatever their
     * `seconds`.
     */
    struct UtcOffset {
        int32_t seconds = 0; ///< Seconds east of UTC; 0 when not `known`.
        bool known = true;   ///< False for the unknown offset.
    };

    /// The unknown offset, RFC 3339's `-00:00`.
    inline constexpr UtcOffset UNKNOWN_OFFSET{0, false};

    /**
     * @brief A time zone of a zone source, or UNKNOWN_ZONE.
     *
     * `name` is the name as it was drawn or written, link names such as
     * `Asia/Calcutta` included, and `primary_name` the zone it resolves to
     * (`Asia/Kolkata`), or the name itself for a zone the source lacks and
     * only the caller's database knows. Prints as the name.
     */
    struct TimeZone {
        std::string name;         ///< The name as written.
        std::string primary_name; ///< The zone whose data applies.
    };

    /// How a wall-clock time maps to instants in its zone.
    enum class TransitionKind {
        Unique, ///< Exactly one instant.
        Gap,    ///< None: the clocks skipped it.
        Fold,   ///< Two: the clocks repeated it.
    };

    /**
     * @brief A wall-clock time in a time zone, resolved to an instant.
     *
     * Carries the TransitionKind of the wall time, the offsets in force before
     * and after the transition it falls on (both the resolved offset for a
     * unique time), which repeat of a fold it means (`fold`, the later when
     * set), the resolved `offset` (unknown in UNKNOWN_ZONE and in a zone
     * only the caller's database knows) and the `instant`: the unique
     * instant, the one `fold` picks, or for a gap the wall time read with
     * `offset_before`. Prints as
     * `2024-03-10T01:30:00-05:00[America/New_York]`, with `[gap]`
     * (`2024-03-10T02:30:00-04:00[America/New_York][gap]`) or `[fold=0]` /
     * `[fold=1]` appended when relevant.
     */
    struct ZonedDateTime {
        TimeZone zone;  ///< The zone the wall time is read in.
        DateTime civil; ///< The wall-clock time.
        /// How the wall time maps to instants.
        TransitionKind kind = TransitionKind::Unique;
        UtcOffset offset_before; ///< The offset before the transition.
        UtcOffset offset_after;  ///< The offset after the transition.
        bool fold = false;       ///< Which repeat of a fold is meant.
        UtcOffset offset;        ///< The offset in force at `instant`.
        Instant instant;         ///< The resolved instant.
    };

    /**
     * @brief A signed duration: `seconds` plus `nanoseconds` of the same
     * sign, the latter within ±999999999, so -1.5 s is `{-1, -500000000}`.
     *
     * Prints in the ISO 8601 duration form `PT1H30M5.5S`, with a leading `-`
     * when negative and `PT0S` for zero.
     */
    struct SignedDuration {
        int64_t seconds = 0;     ///< Whole seconds.
        int32_t nanoseconds = 0; ///< Nanoseconds, of the sign of `seconds`.

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
     * `1y 2mo 3w 4d 5h 6m 7s 8ms 9µs 10ns`, and `0s` for zero. A span with
     * no positive unit has one leading `-` for all its units (`-3d 4h` is
     * -3 days and -4 hours), and a span with units of both signs has a sign
     * on each unit (`-3d +4h`).
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

    /// @cond INTERNAL
    namespace detail {

        // The fields a value type compares, orders and hashes by, in order
        // of significance.
        constexpr auto key(const Date& v) {
            return std::tie(v.year, v.month, v.day);
        }
        constexpr auto key(const Time& v) {
            return std::tie(v.hour, v.minute, v.second, v.nanosecond);
        }
        constexpr auto key(const DateTime& v) {
            return std::tuple_cat(key(v.date), key(v.time));
        }
        constexpr auto key(const Instant& v) {
            return std::tie(v.second, v.nanosecond);
        }
        constexpr std::tuple<bool, int32_t> key(const UtcOffset& v) {
            return {v.known, v.known ? v.seconds : 0};
        }
        constexpr auto key(const TimeZone& v) {
            return std::tie(v.name, v.primary_name);
        }
        constexpr auto key(const ZonedDateTime& v) {
            return std::tuple_cat(key(v.instant), key(v.zone), key(v.civil),
                                  std::tie(v.kind, v.fold),
                                  key(v.offset_before), key(v.offset_after),
                                  key(v.offset));
        }
        constexpr auto key(const SignedDuration& v) {
            return std::tie(v.seconds, v.nanoseconds);
        }
        constexpr auto key(const Span& v) {
            return std::tie(v.years, v.months, v.weeks, v.days, v.hours,
                            v.minutes, v.seconds, v.milliseconds,
                            v.microseconds, v.nanoseconds);
        }

        template <typename T, typename = void>
        struct is_value : std::false_type {};
        template <typename T>
        struct is_value<
            T, std::void_t<decltype(detail::key(std::declval<const T&>()))>>
            : std::true_type {};

        // `R` when `T` is one of the nine value types.
        template <typename T, typename R>
        using if_value = std::enable_if_t<is_value<T>::value, R>;

        template <typename F>
        void hash_field(std::size_t& seed, const F& field) {
            seed ^= std::hash<F>{}(field) +
                    static_cast<std::size_t>(0x9e3779b97f4a7c15ULL) +
                    (seed << 6) + (seed >> 2);
        }

        template <typename T> struct value_hash {
            std::size_t operator()(const T& value) const noexcept {
                std::size_t seed = 0;
                std::apply(
                    [&seed](const auto&... field) {
                        (hash_field(seed, field), ...);
                    },
                    key(value));
                return seed;
            }
        };

    } // namespace detail
    /// @endcond

    /**
     * @name Comparison
     *
     * Date, Time, DateTime, Instant, UtcOffset, TimeZone, ZonedDateTime,
     * SignedDuration and Span each compare for equality and have a total
     * order, so they work as keys of `std::set` and `std::map` and in
     * `sets()`; each also has a `std::hash`, for the unordered containers.
     *
     * Date, Time, DateTime and Instant order from earlier to later and
     * SignedDuration from shorter to longer. UtcOffset orders the unknown
     * offset before every known one, then from west to east. TimeZone
     * orders by `name`, then `primary_name`. ZonedDateTime orders by
     * `instant`, then by its other fields. Span orders by its units,
     * largest first, which is not an order by length: a span has no
     * length without a date to start from.
     */
    /// @{

    /// Compares two values of one value type for equality.
    /// @tparam T A value type of this namespace.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` are equal.
    template <typename T>
    constexpr detail::if_value<T, bool> operator==(const T& a, const T& b) {
        return detail::key(a) == detail::key(b);
    }
    /// Compares two values of one value type for inequality.
    /// @tparam T A value type of this namespace.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` and `b` differ.
    template <typename T>
    constexpr detail::if_value<T, bool> operator!=(const T& a, const T& b) {
        return !(a == b);
    }
    /// Compares two values of one value type for ordering.
    /// @tparam T A value type of this namespace.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is before `b`.
    template <typename T>
    constexpr detail::if_value<T, bool> operator<(const T& a, const T& b) {
        return detail::key(a) < detail::key(b);
    }
    /// Compares two values of one value type for ordering.
    /// @tparam T A value type of this namespace.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is after `b`.
    template <typename T>
    constexpr detail::if_value<T, bool> operator>(const T& a, const T& b) {
        return b < a;
    }
    /// Compares two values of one value type for ordering.
    /// @tparam T A value type of this namespace.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not after `b`.
    template <typename T>
    constexpr detail::if_value<T, bool> operator<=(const T& a, const T& b) {
        return !(b < a);
    }
    /// Compares two values of one value type for ordering.
    /// @tparam T A value type of this namespace.
    /// @param a The left operand.
    /// @param b The right operand.
    /// @return `true` when `a` is not before `b`.
    template <typename T>
    constexpr detail::if_value<T, bool> operator>=(const T& a, const T& b) {
        return !(a < b);
    }
    /// @}

    /**
     * @name Text
     *
     * The text of a value is the text the engine defines for it. No value
     * is refused: a value outside the range of its type prints its fields
     * as they are.
     */
    /// @{

    /// The text of a date: `2024-03-10` or, outside years 0000 to 9999,
    /// `+010000-01-01`.
    /// @param date The value to print.
    /// @return The text.
    std::string to_string(const Date& date);
    /// The text of a time of day: `02:30:00` or `02:30:00.5`.
    /// @param time The value to print.
    /// @return The text.
    std::string to_string(const Time& time);
    /// The text of a datetime: `2024-03-10T02:30:00`.
    /// @param datetime The value to print.
    /// @return The text.
    std::string to_string(const DateTime& datetime);
    /// The text of an instant: `2000-01-01T00:00:00Z`.
    /// @param instant The value to print.
    /// @return The text.
    std::string to_string(const Instant& instant);
    /// The text of an offset: `+05:30`, `-00:44:30` or `-00:00`.
    /// @param offset The value to print.
    /// @return The text.
    std::string to_string(const UtcOffset& offset);
    /// The text of a time zone: its name.
    /// @param zone The value to print.
    /// @return The text.
    std::string to_string(const TimeZone& zone);
    /// The text of a zoned datetime:
    /// `2024-03-10T02:30:00-04:00[America/New_York][gap]`.
    /// @param zoned The value to print.
    /// @return The text.
    /// @throws std::invalid_argument for a zone name that is not UTF-8.
    std::string to_string(const ZonedDateTime& zoned);
    /// The text of a duration: `PT1H30M5.5S`, `-PT1.5S` or `PT0S`.
    /// @param duration The value to print.
    /// @return The text.
    std::string to_string(const SignedDuration& duration);
    /// The text of a span: `1y 2mo 3d`, `-1y 2mo`, `+1y -2mo` or `0s`.
    /// @param span The value to print.
    /// @return The text.
    std::string to_string(const Span& span);

    /// Prints a value of a value type as to_string() gives it.
    /// @tparam T A value type of this namespace.
    /// @param os The stream to print to.
    /// @param value The value to print.
    /// @return `os`.
    template <typename T>
    detail::if_value<T, std::ostream&> operator<<(std::ostream& os,
                                                  const T& value) {
        return os << to_string(value);
    }
    /// @}

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

    /// What a zone source knows about a time zone name; see info().
    struct ZoneInfo {
        /// The zone the name resolves to: the name itself unless it is a
        /// link.
        std::string primary_name;
        /// Whether the name is a link to `primary_name`.
        bool is_link = false;
        /// The name's position on the shrink ladder; `UTC` is 0.
        uint16_t rank = 0;
        /// The zone's features, in ALL_TAGS order.
        std::vector<Tag> tags;

        /// Whether the zone has a feature.
        /// @param tag The feature to look for.
        /// @return `true` when `tags` holds `tag`.
        bool has(Tag tag) const {
            return std::find(tags.begin(), tags.end(), tag) != tags.end();
        }
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
        bool is_dst = false;
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
     * @brief What the default zone source knows about `name`, matched
     * exactly as the source spells it, or `std::nullopt` for a name it
     * lacks.
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
     * @throws std::invalid_argument for a zone the default source lacks, a
     *   `civil` out of range, or one whose instant in the zone lies outside
     *   the range of Instant, which a wall time within an offset's width
     *   of the civil range's ends can do.
     */
    ZonedDateTime resolve(std::string_view name, const DateTime& civil);

    /**
     * @brief The offset the zone `name` has in force at `instant`.
     *
     * @param name The zone name.
     * @param instant The instant to look up.
     * @return The offset in force.
     * @throws std::invalid_argument for a zone the default source lacks or
     *   an `instant` out of range.
     */
    ZoneOffset offset_at(std::string_view name, const Instant& instant);

} // namespace hegel::generators::temporal

/// @cond INTERNAL
namespace std {

    template <>
    struct hash<hegel::generators::temporal::Date>
        : hegel::generators::temporal::detail::value_hash<
              hegel::generators::temporal::Date> {};
    template <>
    struct hash<hegel::generators::temporal::Time>
        : hegel::generators::temporal::detail::value_hash<
              hegel::generators::temporal::Time> {};
    template <>
    struct hash<hegel::generators::temporal::DateTime>
        : hegel::generators::temporal::detail::value_hash<
              hegel::generators::temporal::DateTime> {};
    template <>
    struct hash<hegel::generators::temporal::Instant>
        : hegel::generators::temporal::detail::value_hash<
              hegel::generators::temporal::Instant> {};
    template <>
    struct hash<hegel::generators::temporal::UtcOffset>
        : hegel::generators::temporal::detail::value_hash<
              hegel::generators::temporal::UtcOffset> {};
    template <>
    struct hash<hegel::generators::temporal::TimeZone>
        : hegel::generators::temporal::detail::value_hash<
              hegel::generators::temporal::TimeZone> {};
    template <>
    struct hash<hegel::generators::temporal::ZonedDateTime>
        : hegel::generators::temporal::detail::value_hash<
              hegel::generators::temporal::ZonedDateTime> {};
    template <>
    struct hash<hegel::generators::temporal::SignedDuration>
        : hegel::generators::temporal::detail::value_hash<
              hegel::generators::temporal::SignedDuration> {};
    template <>
    struct hash<hegel::generators::temporal::Span>
        : hegel::generators::temporal::detail::value_hash<
              hegel::generators::temporal::Span> {};

} // namespace std
/// @endcond
