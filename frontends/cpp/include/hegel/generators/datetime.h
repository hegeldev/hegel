#pragma once

#include <chrono>
#include <cstdint>
#include <optional>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <vector>

#include "hegel/core.h"
#include "hegel/datetime.h"

namespace hegel::generators {

    // =============================================================================
    // Parameter structs
    // =============================================================================

    /**
     * @brief Parameters for dates() generator.
     */
    struct DatesParams {
        /// Minimum date (inclusive). Default: January 1 of
        /// hegel::datetime::MIN_YEAR.
        std::optional<datetime::Date> min_value;
        /// Maximum date (inclusive). Default: December 31 of
        /// hegel::datetime::MAX_YEAR.
        std::optional<datetime::Date> max_value;
        /// Draw only dates on these weekdays. A drawn day on another
        /// weekday is moved to the nearest allowed day inside the bounds
        /// rather than rejected. Default: every weekday.
        std::optional<std::vector<datetime::Weekday>> weekdays;
        /// Draw only dates in these months, moving a drawn day in another
        /// month to the nearest allowed day inside the bounds as `weekdays`
        /// does. Default: every month.
        std::optional<std::vector<datetime::Month>> months;
    };

    /**
     * @brief Parameters for times() generator.
     */
    struct TimesParams {
        /// Minimum time (inclusive). Default: midnight.
        std::optional<datetime::Time> min_value;
        /// Maximum time (inclusive). Default: `23:59:59.999999999`.
        std::optional<datetime::Time> max_value;
        /// The drawn time is a whole number of these units since midnight.
        datetime::Precision precision = datetime::Precision::Nanoseconds;
    };

    /**
     * @brief Parameters for datetimes() generator.
     */
    struct DatetimesParams {
        /// Minimum datetime (inclusive). Default: the first instant of
        /// hegel::datetime::MIN_YEAR.
        std::optional<datetime::DateTime> min_value;
        /// Maximum datetime (inclusive). Default: the last nanosecond of
        /// hegel::datetime::MAX_YEAR.
        std::optional<datetime::DateTime> max_value;
        /// The drawn time of day is a whole number of these units since
        /// midnight.
        datetime::Precision precision = datetime::Precision::Nanoseconds;
    };

    /**
     * @brief Parameters for timestamps() generator.
     */
    struct TimestampsParams {
        /// Minimum instant (inclusive). Default:
        /// hegel::datetime::MIN_INSTANT.
        std::optional<datetime::Timestamp> min_value;
        /// Maximum instant (inclusive). Default: the last nanosecond of
        /// hegel::datetime::MAX_INSTANT.
        std::optional<datetime::Timestamp> max_value;
        /// The drawn instant is a whole number of these units since the
        /// Unix epoch.
        datetime::Precision precision = datetime::Precision::Nanoseconds;
    };

    /**
     * @brief Parameters for utc_offsets() generator.
     */
    struct UtcOffsetsParams {
        /// Minimum offset (inclusive), which must be known. Default:
        /// -hegel::datetime::MAX_OFFSET.
        std::optional<datetime::UtcOffset> min_value;
        /// Maximum offset (inclusive), which must be known. Default:
        /// hegel::datetime::MAX_OFFSET.
        std::optional<datetime::UtcOffset> max_value;
        /// The drawn offset is a whole number of these units.
        datetime::Granularity granularity = datetime::Granularity::Seconds;
        /// Whether the draw may yield the unknown offset, RFC 3339's
        /// `-00:00`. It shrinks to a known offset first.
        bool unknown = false;
    };

    /**
     * @brief Parameters for timezones() generator.
     */
    struct TimezonesParams {
        /// Draw only these names, each exactly as the IANA database spells
        /// it or `Etc/Unknown`. The `links` and `legacy` flags do not apply
        /// to an explicit list. Default: every name of the bundled database.
        std::optional<std::vector<std::string>> names;
        /// Whether the default set includes link names such as
        /// `Asia/Calcutta`.
        bool links = true;
        /// Whether the default set includes legacy names such as `EST5EDT`
        /// and `US/Eastern`.
        bool legacy = false;
        /// Whether `Etc/Unknown`, the zone with a known instant and an
        /// unknown offset, is drawn.
        bool unknown = false;
        /// Whether a name in an explicit `names` list that the engine's
        /// database lacks is accepted as an opaque zone: one only the
        /// caller's database knows, drawn like any other name but
        /// evaluated like `Etc/Unknown`, with its civil time read as UTC
        /// and an unknown offset.
        bool opaque = false;
    };

    /**
     * @brief Parameters for zoned_datetimes() generator.
     *
     * The bounds are either wall-clock (`min_value` / `max_value`) or
     * instant (`min_instant` / `max_instant`); setting one of each kind is
     * an error.
     */
    struct ZonedDatetimesParams {
        /// The zones to draw from. The zone and the wall time come from one
        /// engine draw, which must know the allowed set of names to classify
        /// wall times and target transitions, so the selection is a
        /// TimezonesParams rather than an arbitrary generator.
        TimezonesParams timezones;
        /// Minimum wall-clock time (inclusive), in whatever zone is drawn.
        std::optional<datetime::DateTime> min_value;
        /// Maximum wall-clock time (inclusive), in whatever zone is drawn.
        std::optional<datetime::DateTime> max_value;
        /// Minimum instant (inclusive).
        std::optional<datetime::Timestamp> min_instant;
        /// Maximum instant (inclusive).
        std::optional<datetime::Timestamp> max_instant;
        /// The drawn wall time is a whole number of these units since
        /// midnight.
        datetime::Precision precision = datetime::Precision::Nanoseconds;
        /// Whether a wall time the clocks skipped may be drawn. Allowed, it
        /// is reported with kind hegel::datetime::Kind::Gap, both offsets
        /// and the instant it resolves to with the pre-transition offset;
        /// disallowed, another wall time is drawn, and a replayed or shrunk
        /// value that lands in a gap is moved past it.
        bool allow_imaginary = true;
        /// Which instant a wall time the clocks repeated means.
        datetime::Ambiguous ambiguous = datetime::Ambiguous::Both;
    };

    /**
     * @brief Parameters for signed_durations() generator.
     */
    struct SignedDurationsParams {
        /// Minimum duration (inclusive). Default: `int64_t` minimum seconds
        /// and -999999999 nanoseconds.
        std::optional<datetime::SignedDuration> min_value;
        /// Maximum duration (inclusive). Default: `int64_t` maximum seconds
        /// and 999999999 nanoseconds.
        std::optional<datetime::SignedDuration> max_value;
        /// The drawn duration is a whole number of these units.
        datetime::Precision precision = datetime::Precision::Nanoseconds;
    };

    /**
     * @brief Inclusive bounds on one unit of a span, for SpansParams.
     */
    struct UnitBounds {
        int64_t min_value; ///< Minimum value (inclusive).
        int64_t max_value; ///< Maximum value (inclusive).
    };

    /**
     * @brief Parameters for spans() generator.
     *
     * A unit with bounds is enabled and drawn within them; a unit without
     * is always 0. When no unit has bounds, every unit is enabled within
     * ±`INT64_MAX`, or `[0, INT64_MAX]` under
     * hegel::datetime::SpanSign::NonNegative.
     */
    struct SpansParams {
        std::optional<UnitBounds> years;        ///< Bounds on years.
        std::optional<UnitBounds> months;       ///< Bounds on months.
        std::optional<UnitBounds> weeks;        ///< Bounds on weeks.
        std::optional<UnitBounds> days;         ///< Bounds on days.
        std::optional<UnitBounds> hours;        ///< Bounds on hours.
        std::optional<UnitBounds> minutes;      ///< Bounds on minutes.
        std::optional<UnitBounds> seconds;      ///< Bounds on seconds.
        std::optional<UnitBounds> milliseconds; ///< Bounds on milliseconds.
        std::optional<UnitBounds> microseconds; ///< Bounds on microseconds.
        std::optional<UnitBounds> nanoseconds;  ///< Bounds on nanoseconds.
        /// How the units' signs are drawn.
        datetime::SpanSign sign = datetime::SpanSign::Uniform;
    };

    /**
     * @brief Parameters for durations() generator.
     * @tparam Duration The `std::chrono::duration` type
     */
    template <typename Duration> struct DurationsParams {
        /// Minimum duration (inclusive). Default: the lowest value both the
        /// type and the engine hold.
        std::optional<Duration> min_value;
        /// Maximum duration (inclusive). Default: the highest value both
        /// the type and the engine hold.
        std::optional<Duration> max_value;
    };

    /// @cond INTERNAL
    namespace detail {

        template <typename T> struct is_chrono_duration : std::false_type {};
        template <typename Rep, typename Period>
        struct is_chrono_duration<std::chrono::duration<Rep, Period>>
            : std::true_type {};

        // The engine precision of a chrono period, by its length in
        // nanoseconds; only the standard units from nanoseconds to hours
        // have one.
        template <int64_t UnitNanos> struct unit_precision {
            static constexpr bool known = false;
            static constexpr datetime::Precision value =
                datetime::Precision::Nanoseconds;
        };
        template <> struct unit_precision<1> {
            static constexpr bool known = true;
            static constexpr datetime::Precision value =
                datetime::Precision::Nanoseconds;
        };
        template <> struct unit_precision<1'000> {
            static constexpr bool known = true;
            static constexpr datetime::Precision value =
                datetime::Precision::Microseconds;
        };
        template <> struct unit_precision<1'000'000> {
            static constexpr bool known = true;
            static constexpr datetime::Precision value =
                datetime::Precision::Milliseconds;
        };
        template <> struct unit_precision<1'000'000'000> {
            static constexpr bool known = true;
            static constexpr datetime::Precision value =
                datetime::Precision::Seconds;
        };
        template <> struct unit_precision<60'000'000'000> {
            static constexpr bool known = true;
            static constexpr datetime::Precision value =
                datetime::Precision::Minutes;
        };
        template <> struct unit_precision<3'600'000'000'000> {
            static constexpr bool known = true;
            static constexpr datetime::Precision value =
                datetime::Precision::Hours;
        };

    } // namespace detail
    /// @endcond

    /// @name Datetime
    /// @{

    /**
     * @brief Generate hegel::datetime::Date values.
     *
     * By default any date of the engine's range, years ±999,999, on any
     * weekday and in any month; shrinks toward 2000-01-01 or the nearest
     * bound. One draw in twenty on average targets an interesting date in
     * the bounds: a bound, the nearest leap day or year boundary, the Unix
     * epoch, the 32-bit and 64-bit boundaries or the Gregorian cutover.
     *
     * @code{.cpp}
     * auto d = tc.draw(gs::dates({.min_value = Date{2024, 1, 1},
     *                             .weekdays = {{Weekday::Saturday,
     *                                           Weekday::Sunday}}}));
     * @endcode
     *
     * @param params Bounds and weekday / month restrictions.
     * @return Generator producing dates.
     * @throws std::invalid_argument for an invalid or inverted bound, or an
     *   empty weekday or month list.
     */
    Generator<datetime::Date> dates(const DatesParams& params = {});

    /**
     * @brief Generate hegel::datetime::Time values.
     *
     * By default any time of day at nanosecond precision; shrinks toward
     * midnight or the lower bound.
     *
     * @param params Bounds and precision.
     * @return Generator producing times of day.
     * @throws std::invalid_argument for an invalid or inverted bound.
     */
    Generator<datetime::Time> times(TimesParams params = {});

    /**
     * @brief Generate hegel::datetime::DateTime values.
     *
     * By default any wall-clock time of the engine's range, years ±999,999,
     * at nanosecond precision; shrinks toward 2000-01-01T00:00:00 or the
     * nearest bound.
     *
     * @param params Bounds and precision.
     * @return Generator producing naive datetimes.
     * @throws std::invalid_argument for an invalid or inverted bound.
     */
    Generator<datetime::DateTime> datetimes(DatetimesParams params = {});

    /**
     * @brief Generate hegel::datetime::Timestamp values.
     *
     * By default any instant of the engine's range, years ±999,999, at
     * nanosecond precision; shrinks toward 2000-01-01T00:00:00Z or the
     * nearest bound. One draw in twenty on average targets an interesting
     * instant: the Unix epoch, the 32-bit and 64-bit boundaries, a leap
     * second's or a time zone transition's.
     *
     * @param params Bounds and precision.
     * @return Generator producing instants.
     * @throws std::invalid_argument for an invalid or inverted bound.
     */
    Generator<datetime::Timestamp> timestamps(TimestampsParams params = {});

    /**
     * @brief Generate hegel::datetime::UtcOffset values.
     *
     * By default any known offset within ±25:59:59 at second granularity;
     * shrinks toward `+00:00`. One draw in twenty on average targets an
     * offset some time zone has used, so `+05:45` and `-00:44:30` appear.
     *
     * @param params Bounds, granularity and the unknown switch.
     * @return Generator producing UTC offsets.
     * @throws std::invalid_argument for an unknown, out-of-range or
     *   inverted bound.
     */
    Generator<datetime::UtcOffset> utc_offsets(UtcOffsetsParams params = {});

    /**
     * @brief Generate hegel::datetime::TimeZone values.
     *
     * By default any name of the engine's bundled IANA database, link names
     * in and legacy names out; shrinks down a ladder of names to `UTC` when
     * it is allowed, otherwise to the lowest-ranked allowed name. Half the
     * draws are uniform over the allowed names and half weighted by the
     * zones' features, so a zone with a rare feature (a fractional offset,
     * a skipped day, negative DST) is drawn at a rate a short run reaches.
     *
     * @code{.cpp}
     * auto zone = tc.draw(gs::timezones({.names = {{"Europe/London",
     *                                              "Asia/Calcutta"}}}));
     * @endcode
     *
     * @param params The allowed names.
     * @return Generator producing time zones.
     * @throws std::invalid_argument for an empty list or a name the engine
     *   does not know and `opaque` does not admit.
     */
    Generator<datetime::TimeZone> timezones(const TimezonesParams& params = {});

    /**
     * @brief Generate hegel::datetime::ZonedDateTime values: a zone, a
     * wall-clock time in it and, when the clocks repeated that time, which
     * repeat is meant.
     *
     * By default any zone timezones() draws and any wall-clock time of the
     * engine's range at nanosecond precision, with skipped wall times
     * reported as hegel::datetime::Kind::Gap and both repeats of a fold
     * drawn; shrinks down the zone ladder, then toward 2000-01-01T00:00,
     * preferring a unique wall time to a repeated one. One draw in twenty on
     * average targets a transition of the zone, which is how gaps, folds
     * and the exact boundary instants are reached. Build a native value
     * from `civil`, the zone name and `fold` (or, for a gap, from
     * `timestamp`), never from the offsets, so that a newer database on
     * your side changes what a value means but never makes it invalid.
     *
     * @code{.cpp}
     * auto z = tc.draw(gs::zoned_datetimes(
     *     {.timezones = {.names =
     * std::vector<std::string>{"America/New_York"}}, .precision =
     * Precision::Seconds, .allow_imaginary = false}));
     * @endcode
     *
     * @param params Zones, bounds and the gap and fold policies.
     * @return Generator producing zoned datetimes.
     * @throws std::invalid_argument for both wall-clock and instant bounds,
     *   an invalid or inverted bound, bounds holding no value at the
     *   precision, or a zone selection timezones() rejects.
     */
    Generator<datetime::ZonedDateTime>
    zoned_datetimes(const ZonedDatetimesParams& params = {});

    /**
     * @brief Generate hegel::datetime::SignedDuration values.
     *
     * By default any duration of ±`int64_t` seconds at nanosecond
     * precision; shrinks toward zero. One draw in twenty on average targets
     * a bound or a duration such as one unit, 24 hours or the `int32_t`
     * seconds limit.
     *
     * @param params Bounds and precision.
     * @return Generator producing signed durations.
     * @throws std::invalid_argument for an invalid or inverted bound.
     */
    Generator<datetime::SignedDuration>
    signed_durations(SignedDurationsParams params = {});

    /**
     * @brief Generate hegel::datetime::Span values.
     *
     * By default every unit from years to nanoseconds within ±`INT64_MAX`
     * (or `[0, INT64_MAX]` under hegel::datetime::SpanSign::NonNegative),
     * all of one drawn sign; shrinks toward the zero span.
     *
     * @code{.cpp}
     * auto s = tc.draw(gs::spans({.days = {{0, 30}}, .hours = {{0, 23}}}));
     * @endcode
     *
     * @param params Per-unit bounds and the sign policy.
     * @return Generator producing calendar spans.
     * @throws std::invalid_argument for inverted bounds or bounds the sign
     *   policy does not admit.
     */
    Generator<datetime::Span> spans(SpansParams params = {});

    /**
     * @brief Generate `std::chrono::duration` values.
     *
     * The precision follows the duration's period, so
     * `durations<std::chrono::seconds>()` draws whole seconds, and the
     * default bounds are the widest both the type and the engine (±`int64_t`
     * seconds) hold; shrinks toward zero. The period must be one of the
     * standard units from nanoseconds to hours; for another period map
     * signed_durations().
     *
     * @code{.cpp}
     * auto d = tc.draw(gs::durations<std::chrono::milliseconds>(
     *     {.min_value = std::chrono::milliseconds{0}}));
     * @endcode
     *
     * @tparam Duration The `std::chrono::duration` type; nanoseconds by
     *   default.
     * @param params Bounds.
     * @return Generator producing durations.
     * @throws std::invalid_argument for a bound the engine does not hold or
     *   inverted bounds.
     */
    template <typename Duration = std::chrono::nanoseconds>
    Generator<Duration> durations(DurationsParams<Duration> params = {}) {
        static_assert(detail::is_chrono_duration<Duration>::value,
                      "durations<Duration>() requires a std::chrono::duration");
        constexpr int64_t unit_nanos =
            datetime::detail::period_nanos<typename Duration::period>::value;
        static_assert(detail::unit_precision<unit_nanos>::known,
                      "durations<Duration>() supports the periods from "
                      "nanoseconds to hours; map signed_durations() for "
                      "another period");
        SignedDurationsParams inner;
        inner.precision = detail::unit_precision<unit_nanos>::value;
        if (params.min_value.has_value()) {
            inner.min_value =
                datetime::SignedDuration::from_chrono(*params.min_value);
            if (!inner.min_value.has_value()) {
                throw std::invalid_argument(
                    "min_value lies outside the engine's duration range");
            }
        } else {
            inner.min_value =
                datetime::SignedDuration::from_chrono(Duration::min());
        }
        if (params.max_value.has_value()) {
            inner.max_value =
                datetime::SignedDuration::from_chrono(*params.max_value);
            if (!inner.max_value.has_value()) {
                throw std::invalid_argument(
                    "max_value lies outside the engine's duration range");
            }
        } else {
            inner.max_value =
                datetime::SignedDuration::from_chrono(Duration::max());
        }
        return signed_durations(inner).map(
            [](const datetime::SignedDuration& d) {
                return *d.template to_chrono<Duration>();
            });
    }

    /// @}

} // namespace hegel::generators
