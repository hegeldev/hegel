#include <hegel/generators/temporal_types.h>
#include <hegel/generators/zone_source.h>

#include <engine.h>
#include <hegel.h>
#include <temporal_ffi.h>

#include <cstddef>
#include <cstdint>
#include <exception>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <variant>
#include <vector>

namespace hegel::generators::temporal {

    struct ZoneSource::Impl : std::enable_shared_from_this<ZoneSource::Impl> {
        explicit Impl(hegel_zone_source_t* source) : handle(source) {}
        ~Impl() {
            hegel_context_t* ctx = hegel_context_new();
            hegel_zone_source_free(ctx, handle);
            hegel_context_free(ctx);
        }
        Impl(const Impl&) = delete;
        Impl& operator=(const Impl&) = delete;
        Impl(Impl&&) = delete;
        Impl& operator=(Impl&&) = delete;

        struct Loader {
            Impl* impl;
            ZoneLoader load;
        };

        hegel_zone_source_t* handle;
        std::mutex loaders_mutex;
        std::vector<std::unique_ptr<Loader>> loaders;
    };

    namespace {

        void report(hegel_context_t* ctx, std::string_view message) {
            hegel_context_set_error(ctx, impl::borrow(message));
        }

        template <typename F>
        hegel_result_t guarded(hegel_context_t* ctx, F&& body) {
            impl::CallbackContextScope scope(ctx);
            try {
                body();
                return HEGEL_OK;
            } catch (const std::exception& e) {
                report(ctx, e.what());
            } catch (...) {
                report(ctx, "the callback threw an exception that does not "
                            "derive from std::exception");
            }
            return HEGEL_E_INVALID_ARG;
        }

        hegel_zone_type_t type_to_c(const ZoneType& type) {
            return hegel_zone_type_t{type.offset_seconds, type.is_dst,
                                     impl::borrow(type.abbreviation)};
        }

        hegel_zone_rule_date_t rule_date_to_c(const RuleDate& date) {
            hegel_zone_rule_date_t c{};
            c.time = date.time;
            switch (date.kind) {
            case RuleDate::Kind::Julian:
                c.kind = HEGEL_ZONE_RULE_DAY_JULIAN;
                c.day = date.day;
                break;
            case RuleDate::Kind::ZeroBased:
                c.kind = HEGEL_ZONE_RULE_DAY_ZERO_BASED;
                c.day = date.day;
                break;
            default:
                c.kind = HEGEL_ZONE_RULE_DAY_MONTH_WEEK_DAY;
                c.month =
                    static_cast<uint8_t>(static_cast<int>(date.month) + 1);
                c.week = date.week;
                c.weekday = static_cast<uint8_t>(
                    (static_cast<int>(date.weekday) + 1) % 7);
                break;
            }
            return c;
        }

        struct OracleState {
            const ZoneOracle* oracle;
            std::string abbreviation;
        };

        hegel_result_t oracle_offset_at(void* user_data, hegel_context_t* ctx,
                                        int64_t instant,
                                        hegel_zone_type_t* out_type) {
            auto* state = static_cast<OracleState*>(user_data);
            return guarded(ctx, [&] {
                ZoneType type = state->oracle->offset_at(instant);
                state->abbreviation = std::move(type.abbreviation);
                *out_type =
                    hegel_zone_type_t{type.offset_seconds, type.is_dst,
                                      impl::borrow(state->abbreviation)};
            });
        }

        hegel_result_t oracle_next_transition(void* user_data,
                                              hegel_context_t* ctx,
                                              int64_t after, bool* out_found,
                                              int64_t* out_instant) {
            auto* state = static_cast<OracleState*>(user_data);
            return guarded(ctx, [&] {
                std::optional<int64_t> next =
                    state->oracle->next_transition(after);
                *out_found = next.has_value();
                if (next.has_value()) {
                    *out_instant = *next;
                }
            });
        }

        hegel_result_t load_zone(void* user_data, hegel_context_t* ctx,
                                 hegel_zone_source_t*, hegel_string_t name) {
            auto* loader = static_cast<ZoneSource::Impl::Loader*>(user_data);
            return guarded(ctx, [&] {
                ZoneSource source(loader->impl->shared_from_this());
                loader->load(source, std::string(name.data, name.len));
            });
        }

        std::mutex default_mutex;

        ZoneSource& default_source() {
            static ZoneSource* source = new ZoneSource(ZoneSource::bundled());
            return *source;
        }

    } // namespace

    ZoneSource::ZoneSource(std::shared_ptr<Impl> impl)
        : impl_(std::move(impl)) {}

    ZoneSource::ZoneSource() {
        hegel_context_t* ctx = impl::thread_context();
        hegel_zone_source_t* handle = nullptr;
        impl::require_configured(ctx, "ZoneSource",
                                 hegel_zone_source_new(ctx, &handle));
        impl_ = std::make_shared<Impl>(handle);
    }

    ZoneSource ZoneSource::bundled() {
        hegel_context_t* ctx = impl::thread_context();
        hegel_zone_source_t* handle = nullptr;
        impl::require_configured(ctx, "ZoneSource::bundled",
                                 hegel_zone_source_bundled(ctx, &handle));
        return ZoneSource(std::make_shared<Impl>(handle));
    }

    ZoneSource& ZoneSource::add_tzif(std::string_view name,
                                     const std::vector<uint8_t>& bytes) {
        hegel_context_t* ctx = impl::thread_context();
        impl::require_configured(
            ctx, "add_tzif",
            hegel_zone_source_add_tzif(ctx, impl_->handle, impl::borrow(name),
                                       bytes.data(), bytes.size()));
        return *this;
    }

    ZoneSource& ZoneSource::add_zone(std::string_view name,
                                     const ZoneData& data) {
        std::vector<hegel_zone_transition_t> transitions;
        transitions.reserve(data.transitions.size());
        for (const ZoneTransition& transition : data.transitions) {
            transitions.push_back(hegel_zone_transition_t{
                transition.instant, type_to_c(transition.type)});
        }
        hegel_zone_data_t c{};
        c.initial = type_to_c(data.initial);
        c.transitions = transitions.data();
        c.transition_count = transitions.size();
        c.after = HEGEL_ZONE_AFTER_HOLD;
        if (const auto* rule = std::get_if<ZoneRule>(&data.after)) {
            c.after = HEGEL_ZONE_AFTER_RULE;
            c.standard = type_to_c(rule->standard);
            if (rule->daylight.has_value()) {
                c.has_dst = true;
                c.dst = type_to_c(rule->daylight->type);
                c.dst_start = rule_date_to_c(rule->daylight->start);
                c.dst_end = rule_date_to_c(rule->daylight->end);
            }
        } else if (const auto* infer = std::get_if<InferRule>(&data.after)) {
            c.after = HEGEL_ZONE_AFTER_INFER;
            c.last_year = infer->last_year;
        }
        hegel_context_t* ctx = impl::thread_context();
        impl::require_configured(ctx, "add_zone",
                                 hegel_zone_source_add_zone(ctx, impl_->handle,
                                                            impl::borrow(name),
                                                            &c));
        return *this;
    }

    ZoneSource& ZoneSource::add_oracle(std::string_view name,
                                       const ZoneOracle& oracle) {
        OracleState state{&oracle, {}};
        hegel_zone_oracle_t c{};
        c.offset_at = oracle.offset_at ? oracle_offset_at : nullptr;
        c.next_transition =
            oracle.next_transition ? oracle_next_transition : nullptr;
        c.user_data = &state;
        c.first_year = oracle.first_year;
        c.last_year = oracle.last_year;
        c.after = oracle.after == AfterProbe::Infer ? HEGEL_ZONE_AFTER_INFER
                                                    : HEGEL_ZONE_AFTER_HOLD;
        hegel_context_t* ctx = impl::thread_context();
        impl::require_configured(
            ctx, "add_oracle",
            hegel_zone_source_add_oracle(ctx, impl_->handle, impl::borrow(name),
                                         &c));
        return *this;
    }

    ZoneSource& ZoneSource::add_link(std::string_view name,
                                     std::string_view target) {
        hegel_context_t* ctx = impl::thread_context();
        impl::require_configured(
            ctx, "add_link",
            hegel_zone_source_add_link(ctx, impl_->handle, impl::borrow(name),
                                       impl::borrow(target)));
        return *this;
    }

    ZoneSource& ZoneSource::declare(const std::vector<std::string>& names) {
        std::vector<hegel_string_t> strings;
        strings.reserve(names.size());
        for (const std::string& name : names) {
            strings.push_back(impl::borrow(name));
        }
        hegel_context_t* ctx = impl::thread_context();
        impl::require_configured(ctx, "declare",
                                 hegel_zone_source_declare(ctx, impl_->handle,
                                                           strings.data(),
                                                           strings.size()));
        return *this;
    }

    ZoneSource& ZoneSource::set_loader(ZoneLoader loader) {
        hegel_context_t* ctx = impl::thread_context();
        if (!loader) {
            impl::require_configured(ctx, "set_loader",
                                     hegel_zone_source_set_loader(
                                         ctx, impl_->handle, nullptr, nullptr));
            return *this;
        }
        auto state = std::make_unique<Impl::Loader>(
            Impl::Loader{impl_.get(), std::move(loader)});
        impl::require_configured(
            ctx, "set_loader",
            hegel_zone_source_set_loader(ctx, impl_->handle, load_zone,
                                         state.get()));
        std::lock_guard<std::mutex> lock(impl_->loaders_mutex);
        impl_->loaders.push_back(std::move(state));
        return *this;
    }

    ZoneSource& ZoneSource::set_version(std::string_view version) {
        hegel_context_t* ctx = impl::thread_context();
        impl::require_configured(
            ctx, "set_version",
            hegel_zone_source_set_version(ctx, impl_->handle,
                                          impl::borrow(version)));
        return *this;
    }

    std::string ZoneSource::version() const {
        hegel_context_t* ctx = impl::thread_context();
        hegel_string_t version{nullptr, 0};
        impl::require_configured(
            ctx, "version",
            hegel_zone_source_version(ctx, impl_->handle, &version));
        return std::string(version.data, version.len);
    }

    std::optional<ZoneInfo> ZoneSource::info(std::string_view name) const {
        hegel_timezone_info_t c = impl::timezone_info(impl_->handle, name);
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

    ZonedDateTime ZoneSource::resolve(std::string_view name,
                                      const DateTime& civil) const {
        hegel_timezone_resolution_t r = impl::timezone_resolve(
            impl_->handle, name, impl::datetime_to_c(civil));
        return ZonedDateTime{
            impl::zone_named(std::string(name), *this),
            civil,
            impl::kind_from_c(r.kind),
            impl::offset_from_c(r.offset_before, r.offset_known),
            impl::offset_from_c(r.offset_after, r.offset_known),
            false,
            impl::offset_from_c(r.offset_seconds, r.offset_known),
            impl::timestamp_from_c(r.timestamp)};
    }

    ZoneOffset ZoneSource::offset_at(std::string_view name,
                                     const Instant& instant) const {
        hegel_timezone_offset_t o = impl::timezone_offset_at(
            impl_->handle, name, impl::timestamp_to_c(instant));
        return ZoneOffset{impl::offset_from_c(o.offset_seconds, o.offset_known),
                          o.is_dst,
                          std::string(o.abbreviation.data, o.abbreviation.len)};
    }

    ZoneSource default_zone_source() {
        std::lock_guard<std::mutex> lock(default_mutex);
        return default_source();
    }

    void set_default_zone_source(const ZoneSource& source) {
        std::lock_guard<std::mutex> lock(default_mutex);
        hegel_context_t* ctx = impl::thread_context();
        impl::require_configured(
            ctx, "set_default_zone_source",
            hegel_set_default_zone_source(ctx, source.impl()->handle));
        default_source() = source;
    }

    std::optional<ZoneInfo> info(std::string_view name) {
        return default_zone_source().info(name);
    }

    ZonedDateTime resolve(std::string_view name, const DateTime& civil) {
        return default_zone_source().resolve(name, civil);
    }

    ZoneOffset offset_at(std::string_view name, const Instant& instant) {
        return default_zone_source().offset_at(name, instant);
    }

} // namespace hegel::generators::temporal

namespace hegel::impl {

    const hegel_zone_source_t*
    source_handle(const generators::temporal::ZoneSource& source) {
        return source.impl()->handle;
    }

    generators::temporal::TimeZone
    zone_named(std::string name,
               const generators::temporal::ZoneSource& source) {
        std::string primary_name = name;
        if (std::optional<generators::temporal::ZoneInfo> zone_info =
                source.info(name)) {
            primary_name = std::move(zone_info->primary_name);
        }
        return generators::temporal::TimeZone{std::move(name),
                                              std::move(primary_name)};
    }

} // namespace hegel::impl
