#include <hegel/core.h>
#include <hegel/datetime.h>
#include <hegel/generators/datetime.h>

#include <datetime_ffi.h>
#include <engine.h>
#include <hegel.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

namespace hegel::generators {

    namespace {

        using datetime::Date;
        using datetime::DateTime;
        using datetime::SignedDuration;
        using datetime::Time;
        using datetime::Timestamp;
        using datetime::UtcOffset;

        constexpr DateTime min_datetime{{datetime::MIN_YEAR, 1, 1},
                                        {0, 0, 0, 0}};
        constexpr DateTime max_datetime{{datetime::MAX_YEAR, 12, 31},
                                        {23, 59, 59, 999'999'999}};
        constexpr Timestamp min_timestamp{datetime::MIN_INSTANT, 0};
        constexpr Timestamp max_timestamp{datetime::MAX_INSTANT, 999'999'999};
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
            return d.year >= datetime::MIN_YEAR &&
                   d.year <= datetime::MAX_YEAR && d.month >= 1 &&
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

        bool valid(const Timestamp& t) {
            return t.second >= datetime::MIN_INSTANT &&
                   t.second <= datetime::MAX_INSTANT &&
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

        uint32_t precision_to_c(datetime::Precision precision) {
            switch (precision) {
            case datetime::Precision::Microseconds:
                return HEGEL_PRECISION_MICROSECONDS;
            case datetime::Precision::Milliseconds:
                return HEGEL_PRECISION_MILLISECONDS;
            case datetime::Precision::Seconds:
                return HEGEL_PRECISION_SECONDS;
            case datetime::Precision::Minutes:
                return HEGEL_PRECISION_MINUTES;
            case datetime::Precision::Hours:
                return HEGEL_PRECISION_HOURS;
            default:
                return HEGEL_PRECISION_NANOSECONDS;
            }
        }

        uint32_t granularity_to_c(datetime::Granularity granularity) {
            switch (granularity) {
            case datetime::Granularity::Hours:
                return HEGEL_OFFSET_GRANULARITY_HOURS;
            case datetime::Granularity::QuarterHours:
                return HEGEL_OFFSET_GRANULARITY_QUARTER_HOURS;
            case datetime::Granularity::Minutes:
                return HEGEL_OFFSET_GRANULARITY_MINUTES;
            default:
                return HEGEL_OFFSET_GRANULARITY_SECONDS;
            }
        }

        uint32_t ambiguous_to_c(datetime::Ambiguous ambiguous) {
            switch (ambiguous) {
            case datetime::Ambiguous::Earlier:
                return HEGEL_AMBIGUOUS_EARLIER;
            case datetime::Ambiguous::Later:
                return HEGEL_AMBIGUOUS_LATER;
            default:
                return HEGEL_AMBIGUOUS_BOTH;
            }
        }

        uint32_t sign_to_c(datetime::SpanSign sign) {
            switch (sign) {
            case datetime::SpanSign::Mixed:
                return HEGEL_SPAN_SIGN_MIXED;
            case datetime::SpanSign::NonNegative:
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

        class TimestampsGenerator : public IGenerator<Timestamp> {
          public:
            explicit TimestampsGenerator(const TimestampsParams& params)
                : bounds_(bounds(params.min_value, params.max_value,
                                 min_timestamp, max_timestamp, "timestamp")),
                  precision_(precision_to_c(params.precision)) {}

            Timestamp do_draw(const TestCase& tc) const override {
                return impl::timestamp_from_c(impl::draw_timestamp(
                    tc, impl::timestamp_to_c(bounds_.first),
                    impl::timestamp_to_c(bounds_.second), precision_));
            }

          private:
            std::pair<Timestamp, Timestamp> bounds_;
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
            if (bound->seconds < -datetime::MAX_OFFSET ||
                bound->seconds > datetime::MAX_OFFSET) {
                throw std::invalid_argument(std::string(what) +
                                            " must be within 25:59:59 of UTC");
            }
            return bound->seconds;
        }

        class UtcOffsetsGenerator : public IGenerator<UtcOffset> {
          public:
            explicit UtcOffsetsGenerator(const UtcOffsetsParams& params)
                : min_(known_bound("min_value", params.min_value,
                                   -datetime::MAX_OFFSET)),
                  max_(known_bound("max_value", params.max_value,
                                   datetime::MAX_OFFSET)),
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

        hegel_timezone_spec_t* timezone_spec_of(const TimezonesParams& params) {
            return impl::timezone_spec_new(params.names, params.links,
                                           params.legacy, params.unknown,
                                           params.opaque);
        }

        class TimezonesGenerator : public IGenerator<datetime::TimeZone> {
          public:
            explicit TimezonesGenerator(const TimezonesParams& params)
                : spec_(timezone_spec_of(params)) {}

            datetime::TimeZone do_draw(const TestCase& tc) const override {
                return impl::zone_named(impl::draw_timezone(tc, spec_.get()));
            }

          private:
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
                Timestamp min = params.min_instant.value_or(min_timestamp);
                Timestamp max = params.max_instant.value_or(max_timestamp);
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

        class ZonedDatetimesGenerator
            : public IGenerator<datetime::ZonedDateTime> {
          public:
            explicit ZonedDatetimesGenerator(const ZonedDatetimesParams& params)
                : zones_(timezone_spec_of(params.timezones)),
                  spec_(zoned_spec_of(params)) {}

            datetime::ZonedDateTime do_draw(const TestCase& tc) const override {
                impl::ZonedDraw drawn =
                    impl::draw_zoned_datetime(tc, zones_.get(), spec_.get());
                const hegel_zoned_datetime_t& z = drawn.value;
                return datetime::ZonedDateTime{
                    impl::zone_named(std::move(drawn.zone)),
                    impl::datetime_from_c(z.civil),
                    impl::kind_from_c(z.kind),
                    z.offset_before,
                    z.offset_after,
                    z.fold,
                    impl::offset_from_c(z.offset_seconds, z.offset_known),
                    impl::timestamp_from_c(z.timestamp)};
            }

          private:
            TimezoneSpecHandle zones_;
            ZonedSpecHandle spec_;
        };

        class SignedDurationsGenerator : public IGenerator<SignedDuration> {
          public:
            explicit SignedDurationsGenerator(
                const SignedDurationsParams& params)
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
                int64_t min = params.sign == datetime::SpanSign::NonNegative
                                  ? 0
                                  : -std::numeric_limits<int64_t>::max();
                for (std::optional<impl::SpanUnitBounds>& unit : bounds) {
                    unit = impl::SpanUnitBounds{
                        min, std::numeric_limits<int64_t>::max()};
                }
            }
            return impl::span_spec_new(bounds, sign_to_c(params.sign));
        }

        class SpansGenerator : public IGenerator<datetime::Span> {
          public:
            explicit SpansGenerator(const SpansParams& params)
                : spec_(span_spec_of(params)) {}

            datetime::Span do_draw(const TestCase& tc) const override {
                hegel_span_t s = impl::draw_span(tc, spec_.get());
                return datetime::Span{
                    s.years,        s.months,     s.weeks,   s.days,
                    s.hours,        s.minutes,    s.seconds, s.milliseconds,
                    s.microseconds, s.nanoseconds};
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

    Generator<Timestamp> timestamps(TimestampsParams params) {
        return Generator<Timestamp>(new TimestampsGenerator(params));
    }

    Generator<UtcOffset> utc_offsets(UtcOffsetsParams params) {
        return Generator<UtcOffset>(new UtcOffsetsGenerator(params));
    }

    Generator<datetime::TimeZone> timezones(const TimezonesParams& params) {
        return Generator<datetime::TimeZone>(new TimezonesGenerator(params));
    }

    Generator<datetime::ZonedDateTime>
    zoned_datetimes(const ZonedDatetimesParams& params) {
        return Generator<datetime::ZonedDateTime>(
            new ZonedDatetimesGenerator(params));
    }

    Generator<SignedDuration> signed_durations(SignedDurationsParams params) {
        return Generator<SignedDuration>(new SignedDurationsGenerator(params));
    }

    Generator<datetime::Span> spans(SpansParams params) {
        return Generator<datetime::Span>(new SpansGenerator(params));
    }

} // namespace hegel::generators
