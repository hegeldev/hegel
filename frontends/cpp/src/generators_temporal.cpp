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
#include <map>
#include <memory>
#include <mutex>
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

        bool is_leap_year(int32_t year) {
            return year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        }

        unsigned days_in_month(int32_t year, unsigned month) {
            switch (month) {
            case 2:
                return is_leap_year(year) ? 29 : 28;
            case 4:
            case 6:
            case 9:
            case 11:
                return 30;
            default:
                return 31;
            }
        }

        bool valid(const Date& d) {
            return d.year >= MIN_YEAR && d.year <= MAX_YEAR && d.month >= 1 &&
                   d.month <= 12 && d.day >= 1 &&
                   d.day <= days_in_month(d.year, d.month);
        }

        bool valid(const Time& t) {
            return t.hour <= 23 && t.minute <= 59 && t.second <= 59 &&
                   t.nanosecond <= 999'999'999;
        }

        bool valid(const DateTime& dt) {
            return valid(dt.date) && valid(dt.time);
        }

        bool valid(const Instant& t) {
            return t.second >= MIN_INSTANT && t.second <= MAX_INSTANT &&
                   t.nanosecond <= 999'999'999;
        }

        bool valid(const SignedDuration& d) {
            bool consistent = d.seconds == 0 || d.nanoseconds == 0 ||
                              (d.seconds < 0) == (d.nanoseconds < 0);
            return d.nanoseconds > -1'000'000'000 &&
                   d.nanoseconds < 1'000'000'000 && consistent;
        }

        template <typename T>
        std::pair<T, T> bounds(const std::optional<T>& min_value,
                               const std::optional<T>& max_value,
                               const T& default_min, const T& default_max,
                               const char* what) {
            if (min_value.has_value() && !valid(*min_value)) {
                throw std::invalid_argument(
                    std::string("min_value is not a valid ") + what);
            }
            if (max_value.has_value() && !valid(*max_value)) {
                throw std::invalid_argument(
                    std::string("max_value is not a valid ") + what);
            }
            T min = min_value.value_or(default_min);
            T max = max_value.value_or(default_max);
            if (max < min) {
                throw std::invalid_argument(
                    "Cannot have max_value < min_value");
            }
            return {min, max};
        }

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

        class DatesGenerator : public IGenerator<Date> {
          public:
            explicit DatesGenerator(const DatesParams& params)
                : bounds_(bounds(params.min_value, params.max_value,
                                 min_datetime.date, max_datetime.date, "date")),
                  options_(date_options_of(params)) {}

            Date do_draw(const TestCase& tc) const override {
                return impl::date_from_c(impl::draw_date(
                    tc, impl::date_to_c(bounds_.first),
                    impl::date_to_c(bounds_.second), options_.get()));
            }

          private:
            std::pair<Date, Date> bounds_;
            DateOptionsHandle options_;
        };

        class TimesGenerator : public IGenerator<Time> {
          public:
            explicit TimesGenerator(const TimesParams& params)
                : bounds_(bounds(params.min_value, params.max_value,
                                 min_datetime.time, max_datetime.time,
                                 "time of day")),
                  precision_(precision_to_c(params.precision)) {}

            Time do_draw(const TestCase& tc) const override {
                return impl::time_from_c(impl::draw_time(
                    tc, impl::time_to_c(bounds_.first),
                    impl::time_to_c(bounds_.second), precision_));
            }

          private:
            std::pair<Time, Time> bounds_;
            uint32_t precision_;
        };

        class DatetimesGenerator : public IGenerator<DateTime> {
          public:
            explicit DatetimesGenerator(const DatetimesParams& params)
                : bounds_(bounds(params.min_value, params.max_value,
                                 min_datetime, max_datetime, "datetime")),
                  precision_(precision_to_c(params.precision)) {}

            DateTime do_draw(const TestCase& tc) const override {
                return impl::datetime_from_c(impl::draw_datetime(
                    tc, impl::datetime_to_c(bounds_.first),
                    impl::datetime_to_c(bounds_.second), precision_));
            }

          private:
            std::pair<DateTime, DateTime> bounds_;
            uint32_t precision_;
        };

        class InstantsGenerator : public IGenerator<Instant> {
          public:
            explicit InstantsGenerator(const InstantsParams& params)
                : bounds_(bounds(params.min_value, params.max_value,
                                 lowest_instant, highest_instant, "instant")),
                  precision_(precision_to_c(params.precision)) {}

            Instant do_draw(const TestCase& tc) const override {
                return impl::timestamp_from_c(impl::draw_timestamp(
                    tc, impl::timestamp_to_c(bounds_.first),
                    impl::timestamp_to_c(bounds_.second), precision_));
            }

          private:
            std::pair<Instant, Instant> bounds_;
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
            if (bound->seconds < -MAX_OFFSET || bound->seconds > MAX_OFFSET) {
                throw std::invalid_argument(std::string(what) +
                                            " must be within 25:59:59 of UTC");
            }
            return bound->seconds;
        }

        class OffsetsGenerator : public IGenerator<UtcOffset> {
          public:
            explicit OffsetsGenerator(const OffsetsParams& params)
                : min_(known_bound("min_value", params.min_value, -MAX_OFFSET)),
                  max_(known_bound("max_value", params.max_value, MAX_OFFSET)),
                  granularity_(granularity_to_c(params.granularity)),
                  unknown_(params.unknown) {
                if (max_ < min_) {
                    throw std::invalid_argument(
                        "Cannot have max_value < min_value");
                }
            }

            UtcOffset do_draw(const TestCase& tc) const override {
                hegel_offset_t o =
                    impl::draw_offset(tc, min_, max_, granularity_, unknown_);
                return impl::offset_from_c(o.seconds, o.known);
            }

          private:
            int32_t min_;
            int32_t max_;
            uint32_t granularity_;
            bool unknown_;
        };

        class ZonesGenerator : public IGenerator<TimeZone> {
          public:
            explicit ZonesGenerator(const ZonesParams& params)
                : source_(params.source.has_value() ? *params.source
                                                    : default_zone_source()),
                  spec_(impl::timezone_spec_new(
                      params.names, params.links, params.legacy, params.unknown,
                      params.opaque, impl::source_handle(source_))) {}

            TimeZone do_draw(const TestCase& tc) const override {
                return impl::zone_named(impl::draw_timezone(tc, spec_.get()),
                                        source_);
            }

            const hegel_timezone_spec_t* spec() const { return spec_.get(); }
            const ZoneSource& source() const { return source_; }

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
                    "Cannot combine wall-clock bounds (min_value / "
                    "max_value) with instant bounds (min_instant / "
                    "max_instant)");
            }
            impl::ZonedSpecParams spec;
            spec.precision = precision_to_c(params.precision);
            if (instant) {
                Instant min = params.min_instant.value_or(lowest_instant);
                Instant max = params.max_instant.value_or(highest_instant);
                if (valid(min) && valid(max) && max < min) {
                    throw std::invalid_argument(
                        "Cannot have max_instant < min_instant");
                }
                spec.instant_bounds = {impl::timestamp_to_c(min),
                                       impl::timestamp_to_c(max)};
            } else {
                DateTime min = params.min_value.value_or(min_datetime);
                DateTime max = params.max_value.value_or(max_datetime);
                if (valid(min) && valid(max) && max < min) {
                    throw std::invalid_argument(
                        "Cannot have max_value < min_value");
                }
                spec.civil_bounds = {impl::datetime_to_c(min),
                                     impl::datetime_to_c(max)};
            }
            spec.imaginary = params.allow_imaginary ? HEGEL_IMAGINARY_ALLOW
                                                    : HEGEL_IMAGINARY_AVOID;
            spec.ambiguous = ambiguous_to_c(params.ambiguous);
            return impl::zoned_spec_new(spec);
        }

        class ZonedDatetimesGenerator : public IGenerator<ZonedDateTime> {
          public:
            explicit ZonedDatetimesGenerator(const ZonedDatetimesParams& params)
                : zones_(params.zones.has_value() ? params.zones->inner()
                                                  : zones().inner()),
                  builtin_(dynamic_cast<const ZonesGenerator*>(zones_.get())),
                  source_(builtin_ != nullptr ? builtin_->source()
                                              : default_zone_source()),
                  spec_(zoned_spec_of(params)) {}

            ZonedDateTime do_draw(const TestCase& tc) const override {
                if (builtin_ != nullptr) {
                    return from_draw(impl::draw_zoned_datetime(
                        tc, builtin_->spec(), spec_.get()));
                }
                internal::start_span(tc, internal::SpanLabel::ZonedDatetime);
                TimeZone zone = zones_->do_draw(tc);
                ZonedDateTime value = from_draw(impl::draw_zoned_datetime(
                    tc, spec_for(zone.name), spec_.get()));
                internal::stop_span(tc);
                return value;
            }

          private:
            ZonedDateTime from_draw(impl::ZonedDraw drawn) const {
                const hegel_zoned_datetime_t& z = drawn.value;
                return ZonedDateTime{
                    impl::zone_named(std::move(drawn.zone), source_),
                    impl::datetime_from_c(z.civil),
                    impl::kind_from_c(z.kind),
                    impl::offset_from_c(z.offset_before, z.offset_known),
                    impl::offset_from_c(z.offset_after, z.offset_known),
                    z.fold,
                    impl::offset_from_c(z.offset_seconds, z.offset_known),
                    impl::timestamp_from_c(z.timestamp)};
            }

            const hegel_timezone_spec_t*
            spec_for(const std::string& name) const {
                std::lock_guard<std::mutex> lock(single_specs_mutex_);
                auto found = single_specs_.find(name);
                if (found == single_specs_.end()) {
                    auto spec = std::make_unique<TimezoneSpecHandle>(
                        impl::timezone_spec_new(std::vector<std::string>{name},
                                                true, true, false, true,
                                                impl::source_handle(source_)));
                    found = single_specs_.emplace(name, std::move(spec)).first;
                }
                return found->second->get();
            }

            std::shared_ptr<IGenerator<TimeZone>> zones_;
            const ZonesGenerator* builtin_;
            ZoneSource source_;
            ZonedSpecHandle spec_;
            mutable std::mutex single_specs_mutex_;
            mutable std::map<std::string, std::unique_ptr<TimezoneSpecHandle>>
                single_specs_;
        };

        class SignedDurationsGenerator : public IGenerator<SignedDuration> {
          public:
            explicit SignedDurationsGenerator(
                const DurationsParams<SignedDuration>& params)
                : bounds_(bounds(params.min_value, params.max_value,
                                 min_duration, max_duration, "duration")),
                  precision_(precision_to_c(params.precision)) {}

            SignedDuration do_draw(const TestCase& tc) const override {
                return impl::duration_from_c(impl::draw_duration(
                    tc, impl::duration_to_c(bounds_.first),
                    impl::duration_to_c(bounds_.second), precision_));
            }

          private:
            std::pair<SignedDuration, SignedDuration> bounds_;
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

        class SpansGenerator : public IGenerator<Span> {
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
            std::array<char, 16> buf{};
            std::snprintf(buf.data(), buf.size(), "%04d-%02u-%02u",
                          static_cast<int>(date.year),
                          static_cast<unsigned>(date.month),
                          static_cast<unsigned>(date.day));
            return buf.data();
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
