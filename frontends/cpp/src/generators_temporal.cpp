#include <hegel/core.h>
#include <hegel/generators/formats.h>
#include <hegel/generators/temporal.h>
#include <hegel/generators/temporal_types.h>
#include <hegel/generators/zone_source.h>
#include <hegel/internal.h>

#include <engine.h>
#include <hegel.h>
#include <temporal_ffi.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <limits>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

namespace hegel::generators::temporal {

    namespace {

        constexpr DateTime min_datetime{{MIN_YEAR, 1, 1}, {0, 0, 0, 0}};
        constexpr DateTime max_datetime{{MAX_YEAR, 12, 31},
                                        {23, 59, 59, 999'999'999}};
        constexpr Instant lowest_instant{MIN_INSTANT, 0};
        constexpr Instant highest_instant{MAX_INSTANT, 999'999'999};
        constexpr SignedDuration min_duration{
            std::numeric_limits<int64_t>::min(), -999'999'999};
        constexpr SignedDuration max_duration{
            std::numeric_limits<int64_t>::max(), 999'999'999};
        constexpr uint32_t all_weekdays = 0x7f;
        constexpr uint32_t all_months = 0xfff;

        uint32_t precision_to_c(Precision precision) {
            switch (precision) {
            case Precision::Microseconds:
                return HEGEL_PRECISION_MICROSECONDS;
            case Precision::Milliseconds:
                return HEGEL_PRECISION_MILLISECONDS;
            case Precision::Seconds:
                return HEGEL_PRECISION_SECONDS;
            case Precision::Minutes:
                return HEGEL_PRECISION_MINUTES;
            case Precision::Hours:
                return HEGEL_PRECISION_HOURS;
            default:
                return HEGEL_PRECISION_NANOSECONDS;
            }
        }

        uint32_t granularity_to_c(Granularity granularity) {
            switch (granularity) {
            case Granularity::Hours:
                return HEGEL_OFFSET_GRANULARITY_HOURS;
            case Granularity::QuarterHours:
                return HEGEL_OFFSET_GRANULARITY_QUARTER_HOURS;
            case Granularity::Minutes:
                return HEGEL_OFFSET_GRANULARITY_MINUTES;
            default:
                return HEGEL_OFFSET_GRANULARITY_SECONDS;
            }
        }

        uint32_t ambiguous_to_c(Ambiguous ambiguous) {
            switch (ambiguous) {
            case Ambiguous::Earlier:
                return HEGEL_AMBIGUOUS_EARLIER;
            case Ambiguous::Later:
                return HEGEL_AMBIGUOUS_LATER;
            default:
                return HEGEL_AMBIGUOUS_BOTH;
            }
        }

        uint32_t sign_to_c(SpanSign sign) {
            switch (sign) {
            case SpanSign::Mixed:
                return HEGEL_SPAN_SIGN_MIXED;
            case SpanSign::NonNegative:
                return HEGEL_SPAN_SIGN_NONNEGATIVE;
            default:
                return HEGEL_SPAN_SIGN_UNIFORM;
            }
        }

        template <typename H, void (*Free)(H*)> class Handle {
          public:
            explicit Handle(H* handle = nullptr) : handle_(handle) {}
            ~Handle() {
                if (handle_ != nullptr) {
                    Free(handle_);
                }
            }
            Handle(const Handle&) = delete;
            Handle& operator=(const Handle&) = delete;

            const H* get() const { return handle_; }

          private:
            H* handle_;
        };

        using DateOptionsHandle =
            Handle<hegel_date_options_t, impl::date_options_free>;
        using TimezoneSpecHandle =
            Handle<hegel_timezone_spec_t, impl::timezone_spec_free>;
        using ZonedSpecHandle =
            Handle<hegel_zoned_spec_t, impl::zoned_spec_free>;
        using SpanSpecHandle = Handle<hegel_span_spec_t, impl::span_spec_free>;

        template <typename E>
        uint32_t mask_of(const std::optional<std::vector<E>>& values,
                         uint32_t all) {
            if (!values.has_value()) {
                return all;
            }
            uint32_t mask = 0;
            for (E value : *values) {
                mask |= 1u << static_cast<uint32_t>(value);
            }
            return mask;
        }

        hegel_date_options_t* date_options_of(const DatesParams& params) {
            uint32_t weekdays = mask_of(params.weekdays, all_weekdays);
            uint32_t months = mask_of(params.months, all_months);
            if (weekdays == all_weekdays && months == all_months) {
                return nullptr;
            }
            return impl::date_options_new(weekdays, months);
        }

        class DatesGenerator : public generators::detail::ValuePrinted<Date> {
          public:
            explicit DatesGenerator(const DatesParams& params)
                : min_(params.min_value.value_or(min_datetime.date)),
                  max_(params.max_value.value_or(max_datetime.date)),
                  options_(date_options_of(params)) {
                impl::validate_date(impl::date_to_c(min_),
                                    impl::date_to_c(max_), options_.get());
            }

            Date do_draw(const TestCase& tc) const override {
                return impl::date_from_c(
                    impl::draw_date(tc, impl::date_to_c(min_),
                                    impl::date_to_c(max_), options_.get()));
            }

            uint64_t label() const override {
                return internal::label(internal::LabelName::Dates);
            }

          private:
            Date min_;
            Date max_;
            DateOptionsHandle options_;
        };

        class TimesGenerator : public generators::detail::ValuePrinted<Time> {
          public:
            explicit TimesGenerator(const TimesParams& params)
                : min_(params.min_value.value_or(min_datetime.time)),
                  max_(params.max_value.value_or(max_datetime.time)),
                  precision_(precision_to_c(params.precision)) {
                impl::validate_time(impl::time_to_c(min_),
                                    impl::time_to_c(max_), precision_);
            }

            Time do_draw(const TestCase& tc) const override {
                return impl::time_from_c(
                    impl::draw_time(tc, impl::time_to_c(min_),
                                    impl::time_to_c(max_), precision_));
            }

            uint64_t label() const override {
                return internal::label(internal::LabelName::Times);
            }

          private:
            Time min_;
            Time max_;
            uint32_t precision_;
        };

        class DatetimesGenerator
            : public generators::detail::ValuePrinted<DateTime> {
          public:
            explicit DatetimesGenerator(const DatetimesParams& params)
                : min_(params.min_value.value_or(min_datetime)),
                  max_(params.max_value.value_or(max_datetime)),
                  precision_(precision_to_c(params.precision)) {
                impl::validate_datetime(impl::datetime_to_c(min_),
                                        impl::datetime_to_c(max_), precision_);
            }

            DateTime do_draw(const TestCase& tc) const override {
                return impl::datetime_from_c(
                    impl::draw_datetime(tc, impl::datetime_to_c(min_),
                                        impl::datetime_to_c(max_), precision_));
            }

            uint64_t label() const override {
                return internal::label(internal::LabelName::Datetimes);
            }

          private:
            DateTime min_;
            DateTime max_;
            uint32_t precision_;
        };

        class InstantsGenerator
            : public generators::detail::ValuePrinted<Instant> {
          public:
            explicit InstantsGenerator(const InstantsParams& params)
                : min_(params.min_value.value_or(lowest_instant)),
                  max_(params.max_value.value_or(highest_instant)),
                  precision_(precision_to_c(params.precision)) {
                impl::validate_timestamp(impl::timestamp_to_c(min_),
                                         impl::timestamp_to_c(max_),
                                         precision_);
            }

            Instant do_draw(const TestCase& tc) const override {
                return impl::timestamp_from_c(impl::draw_timestamp(
                    tc, impl::timestamp_to_c(min_), impl::timestamp_to_c(max_),
                    precision_));
            }

            uint64_t label() const override {
                return internal::label(internal::LabelName::Instants);
            }

          private:
            Instant min_;
            Instant max_;
            uint32_t precision_;
        };

        int32_t known_bound(const char* what,
                            const std::optional<UtcOffset>& bound,
                            int32_t fallback) {
            if (!bound.has_value()) {
                return fallback;
            }
            if (!bound->known) {
                throw std::invalid_argument(std::string(what) +
                                            " must be a known offset");
            }
            return bound->seconds;
        }

        class OffsetsGenerator
            : public generators::detail::ValuePrinted<UtcOffset> {
          public:
            explicit OffsetsGenerator(const OffsetsParams& params)
                : min_(known_bound("min_value", params.min_value, -MAX_OFFSET)),
                  max_(known_bound("max_value", params.max_value, MAX_OFFSET)),
                  granularity_(granularity_to_c(params.granularity)),
                  allow_unknown_(params.allow_unknown) {
                impl::validate_offset(min_, max_, granularity_, allow_unknown_);
            }

            UtcOffset do_draw(const TestCase& tc) const override {
                hegel_offset_t o = impl::draw_offset(
                    tc, min_, max_, granularity_, allow_unknown_);
                return impl::offset_from_c(o.seconds, o.known);
            }

            uint64_t label() const override {
                return internal::label(internal::LabelName::Offsets);
            }

          private:
            int32_t min_;
            int32_t max_;
            uint32_t granularity_;
            bool allow_unknown_;
        };

        class ZonesGenerator
            : public generators::detail::ValuePrinted<TimeZone> {
          public:
            explicit ZonesGenerator(const ZonesParams& params)
                : source_(params.source.has_value() ? *params.source
                                                    : default_zone_source()),
                  spec_(impl::timezone_spec_new(
                      params.names, params.allow_links, params.allow_legacy,
                      params.allow_unknown, params.allow_opaque,
                      impl::source_handle(source_))) {}

            TimeZone do_draw(const TestCase& tc) const override {
                return impl::zone_named(impl::draw_timezone(tc, spec_.get()),
                                        source_);
            }

            const hegel_timezone_spec_t* spec() const { return spec_.get(); }
            const ZoneSource& source() const { return source_; }

            uint64_t label() const override {
                return internal::label(internal::LabelName::Zones);
            }

          private:
            ZoneSource source_;
            TimezoneSpecHandle spec_;
        };

        hegel_zoned_spec_t* zoned_spec_of(const ZonedDatetimesParams& params) {
            bool civil =
                params.min_value.has_value() || params.max_value.has_value();
            bool instant = params.min_instant.has_value() ||
                           params.max_instant.has_value();
            if (civil && instant) {
                throw std::invalid_argument(
                    "zoned_datetimes: cannot combine wall-clock bounds "
                    "(min_value / max_value) with instant bounds "
                    "(min_instant / max_instant)");
            }
            impl::ZonedSpecParams spec;
            spec.precision = precision_to_c(params.precision);
            if (instant) {
                spec.instant_bounds = {
                    impl::timestamp_to_c(
                        params.min_instant.value_or(lowest_instant)),
                    impl::timestamp_to_c(
                        params.max_instant.value_or(highest_instant))};
            } else if (civil) {
                spec.civil_bounds = {
                    impl::datetime_to_c(
                        params.min_value.value_or(min_datetime)),
                    impl::datetime_to_c(
                        params.max_value.value_or(max_datetime))};
            }
            spec.imaginary = params.allow_imaginary ? HEGEL_IMAGINARY_ALLOW
                                                    : HEGEL_IMAGINARY_AVOID;
            spec.ambiguous = ambiguous_to_c(params.ambiguous);
            return impl::zoned_spec_new(spec);
        }

        AnyGenerator<TimeZone> zones_of(const ZonedDatetimesParams& params) {
            if (params.zones.has_value()) {
                return *params.zones;
            }
            return zones({.source = params.source});
        }

        class ZonedDatetimesGenerator
            : public generators::detail::ValuePrinted<ZonedDateTime> {
          public:
            explicit ZonedDatetimesGenerator(const ZonedDatetimesParams& params)
                : zones_(zones_of(params)),
                  builtin_(dynamic_cast<const ZonesGenerator*>(
                      zones_.inner().get())),
                  source_(source_of(params)), spec_(zoned_spec_of(params)) {}

            ZonedDateTime do_draw(const TestCase& tc) const override {
                if (builtin_ != nullptr) {
                    impl::ZonedDraw drawn = impl::draw_zoned_datetime(
                        tc, builtin_->spec(), spec_.get());
                    return value_of(
                        impl::zone_named(std::move(drawn.zone), source_),
                        drawn.value);
                }
                TimeZone zone = tc.draw_silent(zones_);
                std::optional<ZoneInfo> known = source_.info(zone.name);
                if (!known.has_value()) {
                    throw std::invalid_argument(
                        "zoned_datetimes: the zone source does not know the "
                        "time zone \"" +
                        zone.name +
                        "\" that the `zones` generator drew. Pass the zone "
                        "source its names come from as `source`.");
                }
                zone.primary_name = std::move(known->primary_name);
                hegel_zoned_datetime_t drawn =
                    impl::draw_zoned_datetime_for_zone(
                        tc, impl::source_handle(source_), zone.name,
                        spec_.get());
                return value_of(std::move(zone), drawn);
            }

            uint64_t label() const override {
                uint64_t own =
                    internal::label(internal::LabelName::ZonedDatetimes);
                if (builtin_ != nullptr) {
                    return own;
                }
                return internal::combine_labels({own, zones_.label()});
            }

          private:
            ZoneSource source_of(const ZonedDatetimesParams& params) const {
                if (builtin_ == nullptr) {
                    return params.source.has_value() ? *params.source
                                                     : default_zone_source();
                }
                if (params.zones.has_value() && params.source.has_value()) {
                    throw std::invalid_argument(
                        "zoned_datetimes: `source` cannot be set when `zones` "
                        "is a generator that zones() built, which has a "
                        "source of its own. Pass the source to zones() "
                        "instead.");
                }
                return builtin_->source();
            }

            static ZonedDateTime value_of(TimeZone zone,
                                          const hegel_zoned_datetime_t& z) {
                return ZonedDateTime{
                    std::move(zone),
                    impl::datetime_from_c(z.civil),
                    impl::kind_from_c(z.kind),
                    impl::offset_from_c(z.offset_before, z.offset_known),
                    impl::offset_from_c(z.offset_after, z.offset_known),
                    z.fold,
                    impl::offset_from_c(z.offset_seconds, z.offset_known),
                    impl::timestamp_from_c(z.timestamp)};
            }

            AnyGenerator<TimeZone> zones_;
            const ZonesGenerator* builtin_;
            ZoneSource source_;
            ZonedSpecHandle spec_;
        };

        class SignedDurationsGenerator
            : public generators::detail::ValuePrinted<SignedDuration> {
          public:
            explicit SignedDurationsGenerator(
                const DurationsParams<SignedDuration>& params)
                : min_(params.min_value.value_or(min_duration)),
                  max_(params.max_value.value_or(max_duration)),
                  precision_(precision_to_c(params.precision)) {
                impl::validate_duration(impl::duration_to_c(min_),
                                        impl::duration_to_c(max_), precision_);
            }

            SignedDuration do_draw(const TestCase& tc) const override {
                return impl::duration_from_c(
                    impl::draw_duration(tc, impl::duration_to_c(min_),
                                        impl::duration_to_c(max_), precision_));
            }

            uint64_t label() const override {
                return internal::label(internal::LabelName::Durations);
            }

          private:
            SignedDuration min_;
            SignedDuration max_;
            uint32_t precision_;
        };

        hegel_span_spec_t* span_spec_of(const SpansParams& params) {
            std::array<const std::optional<UnitBounds>*, 10> given = {
                &params.years,       &params.months,       &params.weeks,
                &params.days,        &params.hours,        &params.minutes,
                &params.seconds,     &params.milliseconds, &params.microseconds,
                &params.nanoseconds,
            };
            std::array<std::optional<impl::SpanUnitBounds>, 10> bounds;
            bool any = false;
            for (size_t unit = 0; unit < given.size(); ++unit) {
                const std::optional<UnitBounds>& requested = *given[unit];
                if (requested.has_value()) {
                    any = true;
                    bounds[unit] = impl::SpanUnitBounds{requested->min_value,
                                                        requested->max_value};
                }
            }
            if (!any) {
                int64_t min = params.sign == SpanSign::NonNegative
                                  ? 0
                                  : -std::numeric_limits<int64_t>::max();
                for (std::optional<impl::SpanUnitBounds>& unit : bounds) {
                    unit = impl::SpanUnitBounds{
                        min, std::numeric_limits<int64_t>::max()};
                }
            }
            return impl::span_spec_new(bounds, sign_to_c(params.sign));
        }

        class SpansGenerator : public generators::detail::ValuePrinted<Span> {
          public:
            explicit SpansGenerator(const SpansParams& params)
                : spec_(span_spec_of(params)) {}

            Span do_draw(const TestCase& tc) const override {
                hegel_span_t s = impl::draw_span(tc, spec_.get());
                return Span{s.years,      s.months,       s.weeks,
                            s.days,       s.hours,        s.minutes,
                            s.seconds,    s.milliseconds, s.microseconds,
                            s.nanoseconds};
            }

            uint64_t label() const override {
                return internal::label(internal::LabelName::Spans);
            }

          private:
            SpanSpecHandle spec_;
        };

    } // namespace

    Generator<Date> dates(const DatesParams& params) {
        return Generator<Date>(new DatesGenerator(params));
    }

    Generator<Time> times(TimesParams params) {
        return Generator<Time>(new TimesGenerator(params));
    }

    Generator<DateTime> datetimes(DatetimesParams params) {
        return Generator<DateTime>(new DatetimesGenerator(params));
    }

    Generator<Instant> instants(InstantsParams params) {
        return Generator<Instant>(new InstantsGenerator(params));
    }

    Generator<UtcOffset> offsets(OffsetsParams params) {
        return Generator<UtcOffset>(new OffsetsGenerator(params));
    }

    Generator<TimeZone> zones(const ZonesParams& params) {
        return Generator<TimeZone>(new ZonesGenerator(params));
    }

    Generator<ZonedDateTime>
    zoned_datetimes(const ZonedDatetimesParams& params) {
        return Generator<ZonedDateTime>(new ZonedDatetimesGenerator(params));
    }

    Generator<SignedDuration>
    detail::signed_durations(const DurationsParams<SignedDuration>& params) {
        return Generator<SignedDuration>(new SignedDurationsGenerator(params));
    }

    Generator<Span> spans(SpansParams params) {
        return Generator<Span>(new SpansGenerator(params));
    }

} // namespace hegel::generators::temporal

namespace hegel::generators {

    namespace {

        const temporal::Date first_date{1, 1, 1};
        const temporal::Date last_date{9999, 12, 31};

        std::string iso_date(const temporal::Date& date) {
            return temporal::to_string(date);
        }

        std::string iso_time(const temporal::Time& time) {
            std::array<char, 24> buf{};
            unsigned microsecond = time.nanosecond / 1000;
            if (microsecond != 0) {
                std::snprintf(buf.data(), buf.size(), "%02u:%02u:%02u.%06u",
                              static_cast<unsigned>(time.hour),
                              static_cast<unsigned>(time.minute),
                              static_cast<unsigned>(time.second), microsecond);
            } else {
                std::snprintf(buf.data(), buf.size(), "%02u:%02u:%02u",
                              static_cast<unsigned>(time.hour),
                              static_cast<unsigned>(time.minute),
                              static_cast<unsigned>(time.second));
            }
            return buf.data();
        }

    } // namespace

    Generator<std::string> dates() {
        return temporal::dates(
                   {.min_value = first_date, .max_value = last_date})
            .map(iso_date);
    }

    Generator<std::string> times() {
        return temporal::times({.precision = temporal::Precision::Microseconds})
            .map(iso_time);
    }

    Generator<std::string> datetimes() {
        return temporal::datetimes(
                   {.min_value = temporal::DateTime{first_date, {0, 0, 0, 0}},
                    .max_value =
                        temporal::DateTime{last_date, {23, 59, 59, 999999999}},
                    .precision = temporal::Precision::Microseconds})
            .map([](const temporal::DateTime& value) {
                return iso_date(value.date) + "T" + iso_time(value.time);
            });
    }

} // namespace hegel::generators
