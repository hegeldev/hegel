#pragma once

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <optional>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

#include "hegel/core.h"
#include "hegel/generators/temporal_types.h"
#include "hegel/generators/zone_source.h"

namespace hegel::generators::temporal {

    // =============================================================================
    // Parameter structs
    // =============================================================================

    /**
     * @brief Parameters for dates() generator.
     */
    struct DatesParams {
        /// Minimum date (inclusive). Default: January 1 of MIN_YEAR.
        std::optional<Date> min_value;
        /// Maximum date (inclusive). Default: December 31 of MAX_YEAR.
        std::optional<Date> max_value;
        /// Draw only dates on these weekdays. A drawn day on another weekday is
        /// moved to the nearest allowed day inside the bounds rather than
        /// rejected. Default: every weekday.
        std::optional<std::vector<Weekday>> weekdays;
        /// Draw only dates in these months, moving a drawn day in another month
        /// to the nearest allowed day inside the bounds as `weekdays` does.
        /// Default: every month.
        std::optional<std::vector<Month>> months;
    };

    /**
     * @brief Parameters for times() generator.
     */
    struct TimesParams {
        /// Minimum time (inclusive). Default: midnight.
        std::optional<Time> min_value;
        /// Maximum time (inclusive). Default: `23:59:59.999999999`.
        std::optional<Time> max_value;
        /// The drawn time is a whole number of these units since midnight.
        Precision precision = Precision::Nanoseconds;
    };

    /**
     * @brief Parameters for datetimes() generator.
     */
    struct DatetimesParams {
        /// Minimum datetime (inclusive). Default: the first instant of
        /// MIN_YEAR.
        std::optional<DateTime> min_value;
        /// Maximum datetime (inclusive). Default: the last nanosecond of
        /// MAX_YEAR.
        std::optional<DateTime> max_value;
        /// The drawn time of day is a whole number of these units since
        /// midnight.
        Precision precision = Precision::Nanoseconds;
    };

    /**
     * @brief Parameters for instants() generator over a
     * `std::chrono::system_clock` time point.
     * @tparam TimePoint The `std::chrono::time_point` type, or Instant for
     *   the specialization that also takes a precision.
     */
    template <typename TimePoint = Instant> struct InstantsParams {
        /// Minimum time point (inclusive). Default: the lowest value both
        /// the type and the engine hold.
        std::optional<TimePoint> min_value;
        /// Maximum time point (inclusive). Default: the highest value both
        /// the type and the engine hold.
        std::optional<TimePoint> max_value;
    };

    /**
     * @brief Parameters for instants() generator over Instant.
     */
    template <> struct InstantsParams<Instant> {
        /// Minimum instant (inclusive). Default: MIN_INSTANT.
        std::optional<Instant> min_value;
        /// Maximum instant (inclusive). Default: the last nanosecond of
        /// MAX_INSTANT.
        std::optional<Instant> max_value;
        /// The drawn instant is a whole number of these units since the Unix
        /// epoch.
        Precision precision = Precision::Nanoseconds;
    };

    /**
     * @brief Parameters for offsets() generator.
     */
    struct OffsetsParams {
        /// Minimum offset (inclusive), which must be known. Default:
        /// -MAX_OFFSET.
        std::optional<UtcOffset> min_value;
        /// Maximum offset (inclusive), which must be known. Default:
        /// MAX_OFFSET.
        std::optional<UtcOffset> max_value;
        /// The drawn offset is a whole number of these units.
        Granularity granularity = Granularity::Seconds;
        /// Whether the draw may yield the unknown offset, RFC 3339's `-00:00`.
        /// It shrinks to a known offset first.
        bool allow_unknown = false;
    };

    /**
     * @brief Parameters for zones() generator.
     */
    struct ZonesParams {
        /// Draw only these names, each exactly as the source spells it or
        /// `Etc/Unknown`. The `allow_links` and `allow_legacy` flags do not
        /// apply to an explicit list. Default: every name of the source.
        std::optional<std::vector<std::string>> names;
        /// Whether the default set includes link names such as `Asia/Calcutta`.
        bool allow_links = true;
        /// Whether the default set includes legacy names such as `EST5EDT` and
        /// `US/Eastern`.
        bool allow_legacy = false;
        /// Whether `Etc/Unknown`, the zone with a known instant and an unknown
        /// offset, is drawn.
        bool allow_unknown = false;
        /// Whether a name in an explicit `names` list that the source lacks
        /// is accepted as an opaque zone: one only the caller's database
        /// knows, drawn like any other name but evaluated like
        /// `Etc/Unknown`, with its civil time read as UTC and an unknown
        /// offset.
        bool allow_opaque = false;
        /// The zone data the names come from and zoned datetimes in these
        /// zones resolve against. Default: default_zone_source() as it is
        /// when the generator is built.
        std::optional<ZoneSource> source;
    };

    /**
     * @brief Parameters for zoned_datetimes() generator.
     *
     * The bounds are either wall-clock (`min_value` / `max_value`) or
     * instant (`min_instant` / `max_instant`); setting one of each kind is
     * an error.
     */
    struct ZonedDatetimesParams {
        /// The generator the zone comes from. Default: zones() over
        /// `source`.
        ///
        /// A generator that zones() returns gives its names and its source to
        /// the engine, and the zone and the wall time come from one engine
        /// draw. Any other generator, such as a zones() generator that was
        /// filtered or mapped, draws a zone first, then the wall time in
        /// that zone, resolved against `source`. A name `source` does not
        /// know throws `std::invalid_argument` on the draw: it never
        /// becomes an opaque zone. For opaque zones, use zones() with
        /// `allow_opaque`.
        std::optional<AnyGenerator<TimeZone>> zones;
        /// The zone source the names `zones` draws are resolved against.
        /// Default: default_zone_source() as it is when the generator is
        /// built.
        ///
        /// A generator that zones() returns has a source of its own:
        /// setting `source` as well throws `std::invalid_argument`. Pass
        /// the source to zones() instead.
        std::optional<ZoneSource> source;
        /// Minimum wall-clock time (inclusive), in whatever zone is drawn.
        std::optional<DateTime> min_value;
        /// Maximum wall-clock time (inclusive), in whatever zone is drawn.
        std::optional<DateTime> max_value;
        /// Minimum instant (inclusive).
        std::optional<Instant> min_instant;
        /// Maximum instant (inclusive).
        std::optional<Instant> max_instant;
        /// The drawn wall time is a whole number of these units since midnight.
        Precision precision = Precision::Nanoseconds;
        /// Whether a wall time the clocks skipped may be drawn. Allowed, it is
        /// reported with kind TransitionKind::Gap, both offsets and the instant
        /// it resolves to with the pre-transition offset; disallowed, another
        /// wall time is drawn, and a replayed or shrunk value that lands in a
        /// gap is moved past it.
        bool allow_imaginary = true;
        /// Which instant a wall time the clocks repeated means.
        Ambiguous ambiguous = Ambiguous::Both;
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
     * ±`INT64_MAX`, or `[0, INT64_MAX]` under SpanSign::NonNegative.
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
        SpanSign sign = SpanSign::Uniform;
    };

    /**
     * @brief Parameters for durations() generator over a
     * `std::chrono::duration`.
     * @tparam Duration The `std::chrono::duration` type, or SignedDuration
     *   for the specialization that also takes a precision.
     */
    template <typename Duration = SignedDuration> struct DurationsParams {
        /// Minimum duration (inclusive). Default: the lowest value both the
        /// type and the engine hold.
        std::optional<Duration> min_value;
        /// Maximum duration (inclusive). Default: the highest value both the
        /// type and the engine hold.
        std::optional<Duration> max_value;
    };

    /**
     * @brief Parameters for durations() generator over SignedDuration.
     */
    template <> struct DurationsParams<SignedDuration> {
        /// Minimum duration (inclusive). Default: `int64_t` minimum seconds and
        /// -999999999 nanoseconds.
        std::optional<SignedDuration> min_value;
        /// Maximum duration (inclusive). Default: `int64_t` maximum seconds and
        /// 999999999 nanoseconds.
        std::optional<SignedDuration> max_value;
        /// The drawn duration is a whole number of these units.
        Precision precision = Precision::Nanoseconds;
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
            static constexpr Precision value = Precision::Nanoseconds;
        };
        template <> struct unit_precision<1> {
            static constexpr bool known = true;
            static constexpr Precision value = Precision::Nanoseconds;
        };
        template <> struct unit_precision<1'000> {
            static constexpr bool known = true;
            static constexpr Precision value = Precision::Microseconds;
        };
        template <> struct unit_precision<1'000'000> {
            static constexpr bool known = true;
            static constexpr Precision value = Precision::Milliseconds;
        };
        template <> struct unit_precision<1'000'000'000> {
            static constexpr bool known = true;
            static constexpr Precision value = Precision::Seconds;
        };
        template <> struct unit_precision<60'000'000'000> {
            static constexpr bool known = true;
            static constexpr Precision value = Precision::Minutes;
        };
        template <> struct unit_precision<3'600'000'000'000> {
            static constexpr bool known = true;
            static constexpr Precision value = Precision::Hours;
        };

        inline constexpr Instant lowest_instant{MIN_INSTANT, 0};
        inline constexpr Instant highest_instant{MAX_INSTANT, 999'999'999};

        // The engine precision of an instant draw for a time point type:
        // the finest one that is a whole number of the type's ticks, so
        // that the type holds every instant at it. `known` is false for
        // a type that does not convert to Instant, and for a tick that
        // does not divide an hour.
        template <typename TimePoint, bool = holds_instants<TimePoint>::value>
        struct time_point_precision {
            static constexpr bool known = false;
            static constexpr Precision value = Precision::Nanoseconds;
        };
        template <typename TimePoint>
        struct time_point_precision<TimePoint, true> {
            static constexpr int64_t tick =
                period_nanos<typename TimePoint::period>::value;
            static constexpr bool known = 3'600'000'000'000 % tick == 0;
            static constexpr Precision value =
                1 % tick == 0                ? Precision::Nanoseconds
                : 1'000 % tick == 0          ? Precision::Microseconds
                : 1'000'000 % tick == 0      ? Precision::Milliseconds
                : 1'000'000'000 % tick == 0  ? Precision::Seconds
                : 60'000'000'000 % tick == 0 ? Precision::Minutes
                                             : Precision::Hours;
        };

        // The Instant at a time since the epoch, inside the engine's range
        // or not; nullopt when its seconds do not fit in int64_t.
        template <typename Rep, typename Period>
        std::optional<Instant>
        instant_at(std::chrono::duration<Rep, Period> since_epoch) {
            std::optional<std::pair<int64_t, int64_t>> parts =
                parts_from_duration(since_epoch, /*floor=*/true);
            if (!parts) {
                return std::nullopt;
            }
            return Instant{parts->first, static_cast<uint32_t>(parts->second)};
        }

    } // namespace detail
    /// @endcond

    /**
     * @brief Generate Date values.
     *
     * By default any date of the engine's range, years ±999,999, on any
     * weekday and in any month; shrinks toward 2000-01-01 or the nearest
     * bound. One draw in twenty on average targets an interesting date in
     * the bounds: a bound, the nearest leap day or year boundary, the Unix
     * epoch, the 32-bit and 64-bit boundaries or the Gregorian cutover.
     *
     * @code{.cpp}
     * auto d = tc.draw(gs::temporal::dates(
     *     {.min_value = Date{2024, 1, 1},
     *      .weekdays = {{Weekday::Saturday, Weekday::Sunday}}}));
     * @endcode
     *
     * @param params Bounds and weekday / month restrictions.
     * @return Generator producing dates.
     * @throws std::invalid_argument for an invalid or inverted bound, an
     *   empty weekday or month list, or bounds holding no day the lists
     *   allow.
     */
    Generator<Date> dates(const DatesParams& params = {});

    /**
     * @brief Generate Time values.
     *
     * By default any time of day at nanosecond precision; shrinks toward
     * midnight or the lower bound.
     *
     * @param params Bounds and precision.
     * @return Generator producing times of day.
     * @throws std::invalid_argument for an invalid or inverted bound, or
     *   bounds holding no value at the precision.
     */
    Generator<Time> times(TimesParams params = {});

    /**
     * @brief Generate DateTime values.
     *
     * By default any wall-clock time of the engine's range, years ±999,999,
     * at nanosecond precision; shrinks toward 2000-01-01T00:00:00 or the
     * nearest bound.
     *
     * @param params Bounds and precision.
     * @return Generator producing naive datetimes.
     * @throws std::invalid_argument for an invalid or inverted bound, or
     *   bounds holding no value at the precision.
     */
    Generator<DateTime> datetimes(DatetimesParams params = {});

    /**
     * @brief Generate UtcOffset values.
     *
     * By default any known offset within ±25:59:59 at second granularity;
     * shrinks toward `+00:00`. One draw in twenty on average targets an
     * offset some time zone has used, so `+05:45` and `-00:44:30` appear.
     *
     * @param params Bounds, granularity and the unknown switch.
     * @return Generator producing UTC offsets.
     * @throws std::invalid_argument for an unknown, out-of-range or
     *   inverted bound, or bounds holding no value at the granularity.
     */
    Generator<UtcOffset> offsets(OffsetsParams params = {});

    /**
     * @brief Generate TimeZone values.
     *
     * By default any name of the default zone source (the engine's bundled
     * IANA database unless set_default_zone_source() set another), link
     * names in and legacy names out; shrinks down a ladder of names to `UTC`
     * when it is allowed, otherwise to the lowest-ranked allowed name. Half
     * the draws are uniform over the allowed names and half weighted by the
     * zones' features, so a zone with a rare feature (a fractional offset,
     * a skipped day, negative DST) is drawn at a rate a short run reaches.
     *
     * @code{.cpp}
     * auto zone = tc.draw(gs::temporal::zones(
     *     {.names = {{"Europe/London", "Asia/Calcutta"}}}));
     * @endcode
     *
     * With a ZoneSource, the names come from it, and zoned_datetimes() over
     * this generator resolves wall times against its data:
     *
     * @code{.cpp}
     * auto zone = tc.draw(gs::temporal::zones({.source = my_source}));
     * @endcode
     *
     * @param params The allowed names and their source.
     * @return Generator producing time zones.
     * @throws std::invalid_argument for an empty list, a name the source
     *   does not know and `allow_opaque` does not admit, or a source with
     *   no name the `allow_links` and `allow_legacy` flags allow.
     */
    Generator<TimeZone> zones(const ZonesParams& params = {});

    /**
     * @brief Generate ZonedDateTime values: a zone, a wall-clock time in it
     * and, when the clocks repeated that time, which repeat is meant.
     *
     * By default any zone zones() draws and any wall-clock time of the
     * engine's range at nanosecond precision, with skipped wall times
     * reported as TransitionKind::Gap and both repeats of a fold drawn;
     * shrinks down the zone ladder, then toward 2000-01-01T00:00, preferring
     * a unique wall time to a repeated one. One draw in twenty on
     * average targets a transition of the zone, which is how gaps, folds
     * and the exact boundary instants are reached. Build a native value
     * from `civil`, the zone name and `fold` (or, for a gap, from
     * `instant`), never from the offsets, so that a newer database on
     * your side changes what a value means but never makes it invalid.
     *
     * @code{.cpp}
     * auto z = tc.draw(gs::temporal::zoned_datetimes(
     *     {.zones = gs::temporal::zones({.names = {{"America/New_York"}}}),
     *      .precision = Precision::Seconds,
     *      .allow_imaginary = false}));
     * @endcode
     *
     * Any generator of zones works, such as one of your own zones. Its
     * names are resolved against `source`, or against the default zone
     * source when there is none, and a name that source does not know makes
     * the draw throw `std::invalid_argument`:
     *
     * @code{.cpp}
     * auto local = tc.draw(gs::temporal::zoned_datetimes(
     *     {.zones = gs::just(TimeZone{"Europe/Paris", "Europe/Paris"})}));
     * auto mine = tc.draw(gs::temporal::zoned_datetimes(
     *     {.zones = gs::temporal::zones({.source = my_source})
     *                   .filter([](const TimeZone& zone) {
     *                       return zone.name != "Etc/Skipped";
     *                   }),
     *      .source = my_source}));
     * @endcode
     *
     * To draw every zone of a ZoneSource, pass it alone:
     *
     * @code{.cpp}
     * auto theirs =
     *     tc.draw(gs::temporal::zoned_datetimes({.source = my_source}));
     * @endcode
     *
     * @param params Zones, their source, bounds and the gap and fold
     *   policies.
     * @return Generator producing zoned datetimes.
     * @throws std::invalid_argument for both wall-clock and instant bounds,
     *   an invalid or inverted bound, bounds holding no value at the
     *   precision, or a `source` next to a `zones` generator that zones()
     *   returned.
     */
    Generator<ZonedDateTime>
    zoned_datetimes(const ZonedDatetimesParams& params = {});

    /**
     * @brief Generate Span values.
     *
     * By default every unit from years to nanoseconds within ±`INT64_MAX`
     * (or `[0, INT64_MAX]` under SpanSign::NonNegative), all of one drawn
     * sign; shrinks toward the zero span.
     *
     * @code{.cpp}
     * auto s = tc.draw(
     *     gs::temporal::spans({.days = {{0, 30}}, .hours = {{0, 23}}}));
     * @endcode
     *
     * @param params Per-unit bounds and the sign policy.
     * @return Generator producing calendar spans.
     * @throws std::invalid_argument for inverted bounds or bounds the sign
     *   policy does not admit.
     */
    Generator<Span> spans(SpansParams params = {});

    /// @cond INTERNAL
    namespace detail {

        Generator<Instant>
        engine_instants(const InstantsParams<Instant>& params);

        Generator<SignedDuration>
        signed_durations(const DurationsParams<SignedDuration>& params);

        template <typename TimePoint>
        class TimePointsGenerator
            : public generators::detail::ValuePrinted<TimePoint> {
          public:
            explicit TimePointsGenerator(Generator<Instant> instants)
                : instants_(std::move(instants)) {}

            TimePoint do_draw(const TestCase& tc) const override {
                return *instants_.do_draw(tc)
                            .template to_chrono<typename TimePoint::duration>();
            }

            uint64_t label() const override { return instants_.label(); }

          private:
            Generator<Instant> instants_;
        };

        template <typename TimePoint>
        Generator<TimePoint>
        time_points(const InstantsParams<TimePoint>& params) {
            using Duration = typename TimePoint::duration;
            InstantsParams<Instant> inner;
            inner.precision = time_point_precision<TimePoint>::value;
            if (params.min_value.has_value()) {
                inner.min_value =
                    instant_at(params.min_value->time_since_epoch());
                if (!inner.min_value.has_value()) {
                    throw std::invalid_argument(
                        "min_value lies outside the engine's instant range");
                }
            } else {
                inner.min_value = std::max(
                    instant_at(Duration::min()).value_or(lowest_instant),
                    lowest_instant);
            }
            if (params.max_value.has_value()) {
                inner.max_value =
                    instant_at(params.max_value->time_since_epoch());
                if (!inner.max_value.has_value()) {
                    throw std::invalid_argument(
                        "max_value lies outside the engine's instant range");
                }
            } else {
                inner.max_value = std::min(
                    instant_at(Duration::max()).value_or(highest_instant),
                    highest_instant);
            }
            return Generator<TimePoint>(
                new TimePointsGenerator<TimePoint>(engine_instants(inner)));
        }

    } // namespace detail
    /// @endcond

    /**
     * @brief Generate Instant values, or `std::chrono::system_clock` time
     * points when `TimePoint` is one.
     *
     * For Instant, the default: by default any instant of the engine's
     * range, years ±999,999, at nanosecond precision; shrinks toward
     * 2000-01-01T00:00:00Z or the nearest bound. One draw in twenty on
     * average targets an interesting instant: the Unix epoch, the 32-bit
     * and 64-bit boundaries, a leap second's or a time zone transition's.
     *
     * For a `std::chrono::time_point` of `std::chrono::system_clock`, the
     * same draw at the precision and within the range of the type, so
     * that the type holds every drawn instant exactly. The precision is
     * the finest of the engine's (Precision) that is a whole number of
     * the duration's ticks: `std::chrono::milliseconds` draws whole
     * milliseconds, and a tick of 100 nanoseconds whole microseconds. The
     * default bounds are the widest both the type and the engine hold: a
     * 64-bit count of nanoseconds gives the years 1678 to 2262, and one
     * of seconds the engine's range. A failure report shows a drawn time
     * point as its type and the text of the instant:
     * `std::chrono::sys_time<std::chrono::milliseconds>{"2024-03-10T02:30:00.5Z"}`.
     *
     * The duration must have an integer count of at most 64 bits and a
     * tick that is a whole number of nanoseconds and divides a second,
     * or a whole number of seconds that divides an hour. Any other
     * duration, such as `std::chrono::days` or one finer than
     * nanoseconds, does not compile: draw `std::chrono::hours` and
     * convert. Only `std::chrono::system_clock` is supported, the one
     * clock whose epoch every toolchain fixes at the Unix epoch; convert
     * to another clock with `std::chrono::clock_cast` where the standard
     * library has it.
     *
     * @code{.cpp}
     * auto at = tc.draw(
     *     gs::temporal::instants({.precision = Precision::Seconds}));
     * using sys_ms = std::chrono::sys_time<std::chrono::milliseconds>;
     * auto ms = tc.draw(gs::temporal::instants<sys_ms>(
     *     {.min_value = sys_ms{std::chrono::milliseconds{0}}}));
     * auto now = tc.draw(
     *     gs::temporal::instants<std::chrono::system_clock::time_point>());
     * @endcode
     *
     * @tparam TimePoint Instant (the default) or a `std::chrono::time_point`
     *   of `std::chrono::system_clock`.
     * @param params Bounds and, for Instant, the precision.
     * @return Generator producing instants.
     * @throws std::invalid_argument for an invalid or inverted bound, a
     *   bound outside the engine's range, or bounds holding no value at the
     *   precision.
     */
    template <typename TimePoint = Instant>
    Generator<TimePoint> instants(InstantsParams<TimePoint> params = {}) {
        if constexpr (std::is_same_v<TimePoint, Instant>) {
            return detail::engine_instants(params);
        } else {
            static_assert(detail::is_system_time_point<TimePoint>::value,
                          "instants<TimePoint>() requires Instant or a "
                          "std::chrono::time_point of "
                          "std::chrono::system_clock");
            static_assert(detail::time_point_precision<TimePoint>::known,
                          "instants<TimePoint>() requires a duration with an "
                          "integer count of at most 64 bits and a tick that "
                          "is a whole number of nanoseconds and divides a "
                          "second, or a whole number of seconds that divides "
                          "an hour; for another duration draw "
                          "std::chrono::hours or finer and convert");
            return detail::time_points(params);
        }
    }

    /**
     * @brief Generate SignedDuration values, or `std::chrono::duration`
     * values when `Duration` is one.
     *
     * For SignedDuration, the default: by default any duration of
     * ±`int64_t` seconds at nanosecond precision; shrinks toward zero. One
     * draw in twenty on average targets a bound or a duration such as one
     * unit, 24 hours or the `int32_t` seconds limit.
     *
     * For a `std::chrono::duration`, the precision follows the duration's
     * period, so `durations<std::chrono::seconds>()` draws whole seconds,
     * and the default bounds are the widest both the type and the engine
     * (±`int64_t` seconds) hold; shrinks toward zero. The period must be
     * one of the standard units from nanoseconds to hours; for another
     * period map `durations()`.
     *
     * @code{.cpp}
     * auto d = tc.draw(
     *     gs::temporal::durations({.precision = Precision::Seconds}));
     * auto ms = tc.draw(gs::temporal::durations<std::chrono::milliseconds>(
     *     {.min_value = std::chrono::milliseconds{0}}));
     * @endcode
     *
     * @tparam Duration SignedDuration (the default) or a
     *   `std::chrono::duration` type.
     * @param params Bounds and, for SignedDuration, the precision.
     * @return Generator producing durations.
     * @throws std::invalid_argument for an invalid or inverted bound, a
     *   bound the engine does not hold, or bounds holding no value at the
     *   precision.
     */
    template <typename Duration = SignedDuration>
    Generator<Duration> durations(DurationsParams<Duration> params = {}) {
        if constexpr (std::is_same_v<Duration, SignedDuration>) {
            return detail::signed_durations(params);
        } else {
            static_assert(detail::is_chrono_duration<Duration>::value,
                          "durations<Duration>() requires SignedDuration or a "
                          "std::chrono::duration");
            constexpr int64_t unit_nanos =
                detail::period_nanos<typename Duration::period>::value;
            static_assert(detail::unit_precision<unit_nanos>::known,
                          "durations<Duration>() supports the periods from "
                          "nanoseconds to hours; map durations() for "
                          "another period");
            DurationsParams<SignedDuration> inner;
            inner.precision = detail::unit_precision<unit_nanos>::value;
            if (params.min_value.has_value()) {
                inner.min_value =
                    SignedDuration::from_chrono(*params.min_value);
                if (!inner.min_value.has_value()) {
                    throw std::invalid_argument(
                        "min_value lies outside the engine's duration range");
                }
            } else {
                inner.min_value = SignedDuration::from_chrono(Duration::min());
            }
            if (params.max_value.has_value()) {
                inner.max_value =
                    SignedDuration::from_chrono(*params.max_value);
                if (!inner.max_value.has_value()) {
                    throw std::invalid_argument(
                        "max_value lies outside the engine's duration range");
                }
            } else {
                inner.max_value = SignedDuration::from_chrono(Duration::max());
            }
            return detail::signed_durations(inner).map(
                [](const SignedDuration& d) {
                    return *d.template to_chrono<Duration>();
                });
        }
    }

} // namespace hegel::generators::temporal
