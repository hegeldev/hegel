#pragma once

#include <cstdint>
#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <variant>
#include <vector>

#include "hegel/generators/temporal_types.h"

namespace hegel::generators::temporal {

    /**
     * @brief A local time type of a zone: an offset, its daylight-saving flag
     * and its abbreviation.
     */
    struct ZoneType {
        /// Seconds east of UT, less than two days in magnitude.
        int32_t offset_seconds = 0;
        /// Whether the type is daylight-saving time. Informational:
        /// offset_at() reports it, but it does not change how a wall time
        /// resolves.
        bool is_dst = false;
        /// The abbreviation, such as `EST`; no NUL bytes.
        std::string abbreviation;
    };

    /**
     * @brief An explicit transition: from `instant` on, `type` is in force.
     */
    struct ZoneTransition {
        /// Seconds since the Unix epoch.
        int64_t instant = 0;
        /// The type in force from `instant` on.
        ZoneType type;
    };

    /**
     * @brief The day and time of day a yearly rule changes the clocks on, as
     * a POSIX TZ string names them (RFC 9636 §3.3.1).
     *
     * Build one with julian(), zero_based() or month_week_day().
     */
    struct RuleDate {
        /// How the day is named.
        enum class Kind {
            /// `Jn`: day `day` of the year in `[1, 365]`, never counting
            /// February 29.
            Julian,
            /// `n`: day `day` of the year in `[0, 365]`, counting February
            /// 29.
            ZeroBased,
            /// `Mm.w.d`: `weekday` of week `week` of `month`.
            MonthWeekDay,
        };
        /// How the day is named.
        Kind kind = Kind::MonthWeekDay;
        /// The day of the year, for Kind::Julian and Kind::ZeroBased.
        uint16_t day = 0;
        /// The month, for Kind::MonthWeekDay.
        Month month = Month::January;
        /// The week in `[1, 5]`, 5 meaning the last, for Kind::MonthWeekDay.
        uint8_t week = 1;
        /// The weekday, for Kind::MonthWeekDay.
        Weekday weekday = Weekday::Sunday;
        /// Seconds after local midnight of the day, within ±167 hours, in
        /// the offset in force before the change.
        int32_t time = 7200;

        /// Day `day` in `[1, 365]` of every year, February 29 not counted,
        /// so that `julian(60)` is always March 1.
        /// @param day The day of the year.
        /// @param time Seconds after local midnight. Default: 02:00.
        /// @return The rule date.
        static RuleDate julian(uint16_t day, int32_t time = 7200) {
            return RuleDate{Kind::Julian,    day, Month::January, 1,
                            Weekday::Sunday, time};
        }

        /// Day `day` in `[0, 365]` of every year, February 29 counted.
        /// @param day The zero-based day of the year.
        /// @param time Seconds after local midnight. Default: 02:00.
        /// @return The rule date.
        static RuleDate zero_based(uint16_t day, int32_t time = 7200) {
            return RuleDate{Kind::ZeroBased, day, Month::January, 1,
                            Weekday::Sunday, time};
        }

        /// The `week`-th `weekday` of `month`, week 5 meaning the last, so
        /// that `month_week_day(Month::March, 2, Weekday::Sunday)` is the
        /// second Sunday of March.
        /// @param month The month.
        /// @param week The week in `[1, 5]`.
        /// @param weekday The weekday.
        /// @param time Seconds after local midnight. Default: 02:00.
        /// @return The rule date.
        static RuleDate month_week_day(Month month, uint8_t week,
                                       Weekday weekday, int32_t time = 7200) {
            return RuleDate{Kind::MonthWeekDay, 0, month, week, weekday, time};
        }
    };

    /**
     * @brief Daylight-saving time of a yearly rule: its type, and the dates it
     * starts and ends on each year.
     */
    struct DaylightRule {
        ZoneType type;  ///< The daylight-saving type.
        RuleDate start; ///< When the daylight-saving type comes in.
        RuleDate end;   ///< When the standard type comes back.
    };

    /**
     * @brief A yearly rule in force after a zone's explicit transitions: the
     * footer of a TZif file.
     */
    struct ZoneRule {
        /// The standard type.
        ZoneType standard;
        /// Daylight-saving time, or none for a fixed offset.
        std::optional<DaylightRule> daylight;
    };

    /**
     * @brief The last type in force, held for ever after the explicit
     * transitions.
     */
    struct HoldLast {};

    /**
     * @brief A yearly rule the engine infers from the transitions of
     * `last_year` and the year before it.
     *
     * Neither year changing anything gives a fixed offset; each changing
     * twice, between the same two types, on dates one rule names in both
     * years gives that rule; anything else holds the last type.
     */
    struct InferRule {
        /// The last year the transitions cover, in `[MIN_YEAR, MAX_YEAR]`.
        int32_t last_year = 0;
    };

    /**
     * @brief A zone's data given as fields: the TZif model of RFC 9636.
     *
     * A zone with one rule for all time, such as one from a POSIX TZ string,
     * has no transitions and its rule in `after`:
     *
     * @code{.cpp}
     * ZoneType est{-5 * 3600, false, "EST"};
     * ZoneType edt{-4 * 3600, true, "EDT"};
     * ZoneData data{
     *     est, {},
     *     ZoneRule{est, DaylightRule{
     *         edt,
     *         RuleDate::month_week_day(Month::March, 2, Weekday::Sunday),
     *         RuleDate::month_week_day(Month::November, 1, Weekday::Sunday)}}};
     * @endcode
     */
    struct ZoneData {
        /// The type in force before the first transition.
        ZoneType initial;
        /// The transitions, strictly ascending by instant.
        std::vector<ZoneTransition> transitions;
        /// What is in force after the last transition.
        std::variant<HoldLast, ZoneRule, InferRule> after;
    };

    /// What an oracle's zone has in force after the last year it was
    /// probed over; see ZoneOracle.
    enum class AfterProbe {
        /// The last type, for ever. Right for a library that itself holds
        /// the last offset after its table ends.
        Hold,
        /// A yearly rule inferred from the last two probed years, as for
        /// InferRule. Closer for a library that applies the current rule
        /// for ever.
        Infer,
    };

    /**
     * @brief A library that answers offset lookups but does not expose its
     * zone data, probed into explicit transitions over the years
     * `first_year` to `last_year`.
     *
     * With `next_transition` the probe asks for each transition in turn;
     * without it, it samples `offset_at` once a day and bisects each change
     * to the second, which misses a pair of changes less than a day apart.
     * Before `first_year` the type at its start holds; after `last_year`,
     * `after` decides. Both callbacks are called on the thread that calls
     * ZoneSource::add_oracle, during the call only. An exception a callback
     * throws fails the call with `std::invalid_argument` carrying its
     * message.
     */
    struct ZoneOracle {
        /// Required: the type in force at an instant, in seconds since the
        /// Unix epoch.
        std::function<ZoneType(int64_t instant)> offset_at;
        /// Optional: the first instant after `after` at which the type in
        /// force changes, or `std::nullopt` when it never changes again.
        std::function<std::optional<int64_t>(int64_t after)> next_transition;
        /// The first year probed.
        int32_t first_year = 1900;
        /// The last year probed; at least `first_year + 1` for
        /// AfterProbe::Infer.
        int32_t last_year = 2100;
        /// What is in force after `last_year`.
        AfterProbe after = AfterProbe::Hold;
    };

    class ZoneSource;

    /**
     * @brief Supplies a zone source's data for a name the first time a draw
     * or a query needs it.
     *
     * It adds that name's zone (or a link) to `source` with the
     * `ZoneSource::add_*` functions, or adds nothing when it does not know
     * the name. It is called from whichever thread draws or queries, so it
     * must be safe to call from any thread, possibly concurrently and more
     * than once for one name. It must not draw. An exception it throws fails
     * the draw or query with `std::invalid_argument` carrying its message.
     * Use the `source` argument rather than capturing the source, which
     * would keep it alive for ever.
     */
    using ZoneLoader =
        std::function<void(ZoneSource& source, const std::string& name)>;

    /**
     * @brief A source of time zone data: the database the zone and zoned
     * datetime generators classify, target and resolve wall times against.
     *
     * By default every draw and query uses the engine's bundled IANA
     * database, ZoneSource::bundled(). When the code under test uses
     * another database, give its data to a ZoneSource and either pass it to
     * one generator (`zones({.source = source})`) or make it the default
     * with set_default_zone_source(). The zone names, their link and legacy
     * flags and the order they shrink in always come from the bundled
     * catalogue, so a stored failure shrinks the same way everywhere; only
     * what a wall time means in a zone comes from the source. A name the
     * catalogue lacks is drawn too, after every catalogued name, and is
     * never a legacy name, whatever its area (such as `Mars/Olympus`).
     *
     * A source takes data in three shapes: TZif files (add_tzif()), fields
     * (add_zone()) and an offset oracle (add_oracle()), plus links
     * (add_link()) and names whose data a loader supplies on first use
     * (declare() and set_loader()).
     *
     * @code{.cpp}
     * ZoneSource source;
     * source.declare({"America/New_York", "Europe/London"})
     *     .set_loader([](ZoneSource& s, const std::string& name) {
     *         s.add_tzif(name, read_file("/usr/share/zoneinfo/" + name));
     *     });
     * auto zone = tc.draw(zones({.source = source}));
     * @endcode
     *
     * Copies share one source: data added through one copy is visible
     * through every other. A source may be used and filled from any number
     * of threads. Add or declare names before building generators that draw
     * from them: a generator reads the names a source knows when it is
     * built.
     */
    class ZoneSource {
      public:
        /// An empty source, to be filled with the `add_*` functions.
        ZoneSource();

        /// The engine's bundled IANA database. It cannot be changed: each
        /// `add_*` and `set_*` function on it throws
        /// `std::invalid_argument`.
        /// @return The bundled source.
        static ZoneSource bundled();

        /// Add a zone from a TZif file (RFC 9636, version 2 or later), such
        /// as one under `/usr/share/zoneinfo`. Its 64-bit block and its
        /// footer are read.
        /// @param name The name the zone is drawn and queried by: not
        ///   empty, without NUL bytes and not `Etc/Unknown`.
        /// @param bytes The file's contents.
        /// @return This source.
        /// @throws std::invalid_argument for a malformed file, an offset of
        ///   two days or more or a bad name.
        ZoneSource& add_tzif(std::string_view name,
                             const std::vector<uint8_t>& bytes);

        /// Add a zone from its data given as fields.
        /// @param name As for add_tzif().
        /// @param data The zone's data.
        /// @return This source.
        /// @throws std::invalid_argument for transitions out of order, an
        ///   offset of two days or more, a field out of range or a bad name.
        ZoneSource& add_zone(std::string_view name, const ZoneData& data);

        /// Add a zone by probing an offset oracle; see ZoneOracle.
        /// @param name As for add_tzif().
        /// @param oracle The callbacks and the probe range.
        /// @return This source.
        /// @throws std::invalid_argument for a missing `offset_at`, a probe
        ///   range out of order or outside the engine's years, a callback
        ///   that throws or answers an invalid type, a next transition that
        ///   is not later than the instant asked about, more than 100000
        ///   transitions or a bad name.
        ZoneSource& add_oracle(std::string_view name, const ZoneOracle& oracle);

        /// Make `name` a link to `target`, whose data then applies to both.
        /// The target need not be known yet.
        /// @param name The link's name.
        /// @param target The name it links to.
        /// @return This source.
        /// @throws std::invalid_argument for a bad name or a link to itself.
        ZoneSource& add_link(std::string_view name, std::string_view target);

        /// Declare names whose data comes later: from the loader the first
        /// time a draw or query needs it, or from an `add_*` call before a
        /// zoned datetime is drawn in that zone. A name the source already
        /// has keeps what it has.
        /// @param names The names, each as for add_tzif().
        /// @return This source.
        /// @throws std::invalid_argument for a bad name.
        ZoneSource& declare(const std::vector<std::string>& names);

        /// Set the loader asked for the data of a name the source lacks;
        /// see ZoneLoader. An empty function removes it. A name the loader
        /// adds nothing for is unknown from then on.
        /// @param loader The loader.
        /// @return This source.
        ZoneSource& set_loader(ZoneLoader loader);

        /// Record which release of its database the source holds. Failure
        /// output does not report it.
        /// @param version The release, such as `"2026e"`.
        /// @return This source.
        ZoneSource& set_version(std::string_view version);

        /// @return The release of the source's database: for the bundled
        ///   source tzdb_version(), for another what set_version() recorded,
        ///   or an empty string.
        std::string version() const;

        /// What this source knows about `name`, as info() does for the
        /// default source. A name the catalogue lacks reports the source's
        /// link target, the rank after every catalogued name and the tags of
        /// its data and spelling.
        /// @param name The zone name to look up.
        /// @return The description, or `std::nullopt` for a name the source
        ///   does not know.
        /// @throws std::invalid_argument when loading the name's data fails.
        std::optional<ZoneInfo> info(std::string_view name) const;

        /// Resolve a wall time in a zone of this source, as resolve() does
        /// for the default source.
        /// @param name The zone name.
        /// @param civil The wall-clock time to resolve.
        /// @return The resolved zoned datetime.
        /// @throws std::invalid_argument as resolve() does, or when the
        ///   source has no data for `name`.
        ZonedDateTime resolve(std::string_view name,
                              const DateTime& civil) const;

        /// The offset a zone of this source has in force at an instant, as
        /// offset_at() does for the default source.
        /// @param name The zone name.
        /// @param instant The instant to look up.
        /// @return The offset in force.
        /// @throws std::invalid_argument as offset_at() does, or when the
        ///   source has no data for `name`.
        ZoneOffset offset_at(std::string_view name,
                             const Instant& instant) const;

        /// @cond INTERNAL
        struct Impl;
        explicit ZoneSource(std::shared_ptr<Impl> impl);
        const std::shared_ptr<Impl>& impl() const { return impl_; }
        /// @endcond

      private:
        std::shared_ptr<Impl> impl_;
    };

    /**
     * @brief The process-wide default zone source: the one zones(), the
     * default zoned_datetimes() and the info(), resolve() and offset_at()
     * queries use when given no source.
     *
     * It is ZoneSource::bundled() until set_default_zone_source() sets
     * another.
     *
     * @return The default source.
     */
    ZoneSource default_zone_source();

    /**
     * @brief Replace the process-wide default zone source.
     *
     * A generator reads the default when it is built, so generators built
     * before the call keep the source they read.
     *
     * @param source The new default; ZoneSource::bundled() restores the
     *   engine's database.
     */
    void set_default_zone_source(const ZoneSource& source);

} // namespace hegel::generators::temporal
