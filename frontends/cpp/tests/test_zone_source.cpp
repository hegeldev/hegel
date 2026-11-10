// Zone sources: the ZoneSource builder in each of its shapes, the loader,
// the process default, and zone draws against a source.

#include <gtest/gtest.h>

#include "common/utils.h"

#include <atomic>
#include <cstdint>
#include <memory>
#include <optional>
#include <set>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

#include <hegel/hegel.h>

namespace gs = hegel::generators;
namespace temporal = hegel::generators::temporal;

using hegel::tests::common::assert_all_examples;
using hegel::tests::common::find_any;
using temporal::AfterProbe;
using temporal::DateTime;
using temporal::DaylightRule;
using temporal::HoldLast;
using temporal::InferRule;
using temporal::Instant;
using temporal::Month;
using temporal::RuleDate;
using temporal::TransitionKind;
using temporal::Weekday;
using temporal::ZoneData;
using temporal::ZoneOracle;
using temporal::ZoneRule;
using temporal::ZoneSource;
using temporal::ZoneTransition;
using temporal::ZoneType;

namespace {

    const ZoneType est{-5 * 3600, false, "EST"};
    const ZoneType edt{-4 * 3600, true, "EDT"};

    ZoneData eastern() {
        return ZoneData{
            est,
            {},
            ZoneRule{est,
                     DaylightRule{edt,
                                  RuleDate::month_week_day(Month::March, 2,
                                                           Weekday::Sunday),
                                  RuleDate::month_week_day(Month::November, 1,
                                                           Weekday::Sunday)}}};
    }

    ZoneData fixed(int32_t offset_seconds, const std::string& abbreviation) {
        ZoneType type{offset_seconds, false, abbreviation};
        return ZoneData{type, {}, HoldLast{}};
    }

    DateTime datetime(int32_t year, uint8_t month, uint8_t day, uint8_t hour,
                      uint8_t minute) {
        return DateTime{{year, month, day}, {hour, minute, 0, 0}};
    }

    int64_t epoch(int32_t year, uint8_t month, uint8_t day, uint8_t hour) {
        return ZoneSource::bundled()
            .resolve("UTC", datetime(year, month, day, hour, 0))
            .instant.second;
    }

    int32_t offset_in(const ZoneSource& source, const std::string& name,
                      int64_t instant) {
        return source.offset_at(name, Instant{instant, 0}).offset.seconds;
    }

    void put_be(std::vector<uint8_t>& out, uint64_t value, int bytes) {
        for (int i = bytes - 1; i >= 0; --i) {
            out.push_back(static_cast<uint8_t>(value >> (8 * i)));
        }
    }

    std::vector<uint8_t> tzif_header(uint32_t timecnt, uint32_t typecnt,
                                     uint32_t charcnt) {
        std::vector<uint8_t> out{'T', 'Z', 'i', 'f', '2'};
        out.resize(20, 0);
        for (uint32_t count : {0u, 0u, 0u, timecnt, typecnt, charcnt}) {
            put_be(out, count, 4);
        }
        return out;
    }

    std::vector<uint8_t> tzif_from_plus1_to_plus2_at(int64_t instant) {
        std::vector<uint8_t> out = tzif_header(0, 1, 4);
        put_be(out, 0, 4);
        out.push_back(0);
        out.push_back(0);
        for (char c : std::string("UTC", 4)) {
            out.push_back(static_cast<uint8_t>(c));
        }
        std::vector<uint8_t> v2 = tzif_header(1, 2, 8);
        out.insert(out.end(), v2.begin(), v2.end());
        put_be(out, static_cast<uint64_t>(instant), 8);
        out.push_back(1);
        put_be(out, 3600, 4);
        out.push_back(0);
        out.push_back(0);
        put_be(out, 7200, 4);
        out.push_back(0);
        out.push_back(4);
        for (char c : std::string("+01\0+02", 8)) {
            out.push_back(static_cast<uint8_t>(c));
        }
        for (char c : std::string("\n<+02>-2\n")) {
            out.push_back(static_cast<uint8_t>(c));
        }
        return out;
    }

    hegel::Settings quick() {
        return hegel::Settings{.test_cases = 30,
                               .database = hegel::Database::disabled()};
    }

    void draw_once(const gs::Generator<temporal::TimeZone>& zones) {
        hegel::test([&](hegel::TestCase& tc) { (void)tc.draw(zones); },
                    hegel::Settings{.test_cases = 1,
                                    .database = hegel::Database::disabled()});
    }

    // Makes `source` the default until the end of the scope.
    class ScopedDefault {
      public:
        explicit ScopedDefault(const ZoneSource& source)
            : previous_(temporal::set_default_zone_source(source)) {}
        ~ScopedDefault() { temporal::set_default_zone_source(previous_); }
        ScopedDefault(const ScopedDefault&) = delete;
        ScopedDefault& operator=(const ScopedDefault&) = delete;

      private:
        ZoneSource previous_;
    };

    // The message of the argument error the first draw from `gen` raises.
    template <typename T> std::string draw_error(const gs::Generator<T>& gen) {
        try {
            find_any<T>(gen, [](const T&) { return true; });
        } catch (const std::invalid_argument& e) {
            return e.what();
        }
        ADD_FAILURE() << "the draw raised no argument error";
        return "";
    }

    void expect_contains(const std::string& text, const std::string& part) {
        EXPECT_NE(text.find(part), std::string::npos) << text;
    }

} // namespace

TEST(ZoneSource, AFooterOnlyZoneHasGapsAndFolds) {
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());

    auto gap = source.resolve("Etc/TestEastern", datetime(2024, 3, 10, 2, 30));
    EXPECT_EQ(gap.kind, TransitionKind::Gap);
    EXPECT_EQ(gap.offset_before.seconds, -5 * 3600);
    EXPECT_EQ(gap.offset_after.seconds, -4 * 3600);

    auto fold = source.resolve("Etc/TestEastern", datetime(2024, 11, 3, 1, 30));
    EXPECT_EQ(fold.kind, TransitionKind::Fold);
    EXPECT_EQ(fold.zone.name, "Etc/TestEastern");
    EXPECT_EQ(fold.zone.primary_name, "Etc/TestEastern");

    for (int32_t year : {1900, 2024, 3000}) {
        auto summer = source.offset_at("Etc/TestEastern",
                                       Instant{epoch(year, 7, 1, 12), 0});
        EXPECT_EQ(summer.offset.seconds, -4 * 3600);
        EXPECT_TRUE(summer.is_dst);
        EXPECT_EQ(summer.abbreviation, "EDT");
        EXPECT_EQ(offset_in(source, "Etc/TestEastern", epoch(year, 1, 1, 12)),
                  -5 * 3600);
    }
}

TEST(ZoneSource, RuleDatesTakeEveryPosixForm) {
    ZoneType standard{0, false, "STD"};
    ZoneType summer{3600, true, "SUM"};
    ZoneSource source;
    source.add_zone(
        "Etc/TestJulian",
        ZoneData{
            standard,
            {},
            ZoneRule{standard, DaylightRule{summer, RuleDate::julian(60, 0),
                                            RuleDate::julian(305, 0)}}});
    source.add_zone(
        "Etc/TestZeroBased",
        ZoneData{
            standard,
            {},
            ZoneRule{standard, DaylightRule{summer, RuleDate::zero_based(59, 0),
                                            RuleDate::zero_based(304, 0)}}});

    EXPECT_EQ(offset_in(source, "Etc/TestJulian", epoch(2024, 2, 29, 12)), 0);
    EXPECT_EQ(offset_in(source, "Etc/TestJulian", epoch(2024, 3, 1, 12)), 3600);
    EXPECT_EQ(offset_in(source, "Etc/TestZeroBased", epoch(2024, 2, 29, 12)),
              3600);
    EXPECT_EQ(offset_in(source, "Etc/TestZeroBased", epoch(2023, 2, 28, 12)),
              0);
    EXPECT_EQ(offset_in(source, "Etc/TestZeroBased", epoch(2023, 3, 1, 12)),
              3600);
}

TEST(ZoneSource, ExplicitTransitionsHoldOrInferAfterTheLast) {
    std::vector<ZoneTransition> transitions;
    for (int32_t year : {2023, 2024}) {
        auto start = ZoneSource::bundled().resolve(
            "America/New_York",
            datetime(year, 3, year == 2023 ? 12 : 10, 3, 0));
        auto end = ZoneSource::bundled().resolve(
            "America/New_York", datetime(year, 11, year == 2023 ? 5 : 3, 1, 0));
        transitions.push_back(ZoneTransition{start.instant.second, edt});
        transitions.push_back(ZoneTransition{end.instant.second + 3600, est});
    }
    ZoneSource source;
    source.add_zone("Etc/TestHeld", ZoneData{est, transitions, HoldLast{}});
    source.add_zone("Etc/TestInferred",
                    ZoneData{est, transitions, InferRule{2024}});

    int64_t summer_2030 = epoch(2030, 7, 1, 12);
    EXPECT_EQ(offset_in(source, "Etc/TestHeld", summer_2030), -5 * 3600);
    EXPECT_EQ(offset_in(source, "Etc/TestInferred", summer_2030), -4 * 3600);
    EXPECT_EQ(offset_in(source, "Etc/TestHeld", epoch(2024, 7, 1, 12)),
              -4 * 3600);
}

TEST(ZoneSource, AddZoneRejectsBadData) {
    ZoneSource source;
    EXPECT_THROW(source.add_zone("Etc/TestFar", fixed(3 * 86'400, "FAR")),
                 std::invalid_argument);
    EXPECT_THROW(source.add_zone("", fixed(0, "Z")), std::invalid_argument);
    EXPECT_THROW(source.add_zone("Etc/Unknown", fixed(0, "Z")),
                 std::invalid_argument);
    EXPECT_THROW(
        source.add_zone("Etc/TestBackwards", ZoneData{est,
                                                      {ZoneTransition{100, edt},
                                                       ZoneTransition{50, est}},
                                                      HoldLast{}}),
        std::invalid_argument);
}

TEST(ZoneSource, TzifFilesAreParsed) {
    int64_t change = epoch(2000, 1, 1, 0);
    ZoneSource source;
    source.add_tzif("Etc/TestTzif", tzif_from_plus1_to_plus2_at(change));
    EXPECT_EQ(offset_in(source, "Etc/TestTzif", change - 1), 3600);
    EXPECT_EQ(offset_in(source, "Etc/TestTzif", change), 7200);
    EXPECT_EQ(source.offset_at("Etc/TestTzif", Instant{change, 0}).abbreviation,
              "+02");
    EXPECT_THROW(source.add_tzif("Etc/TestBad", {'T', 'Z', 'i', 'f'}),
                 std::invalid_argument);
    EXPECT_THROW(source.add_tzif("Etc/TestEmpty", {}), std::invalid_argument);

    std::vector<uint8_t> bytes = tzif_from_plus1_to_plus2_at(change);
    source.add_tzif("Etc/TestTzifBytes", bytes.data(), bytes.size());
    EXPECT_EQ(offset_in(source, "Etc/TestTzifBytes", change), 7200);
    EXPECT_THROW(source.add_tzif("Etc/TestShort", bytes.data(), 4),
                 std::invalid_argument);
}

TEST(ZoneSource, OraclesAreProbedWithOrWithoutNextTransition) {
    int64_t change = epoch(2000, 1, 1, 0);
    ZoneType before{3600, false, "ONE"};
    ZoneType after{7200, false, "TWO"};
    auto offset = [=](int64_t instant) {
        return instant < change ? before : after;
    };
    ZoneSource source;
    source.add_oracle("Etc/TestStepped",
                      ZoneOracle{offset,
                                 [=](int64_t from) -> std::optional<int64_t> {
                                     if (from < change) {
                                         return change;
                                     }
                                     return std::nullopt;
                                 },
                                 1990, 2010, AfterProbe::Hold});
    source.add_oracle("Etc/TestSampled", ZoneOracle{offset, nullptr, 1999, 2001,
                                                    AfterProbe::Hold});

    for (const char* name : {"Etc/TestStepped", "Etc/TestSampled"}) {
        EXPECT_EQ(offset_in(source, name, change - 1), 3600);
        EXPECT_EQ(offset_in(source, name, change), 7200);
        EXPECT_EQ(offset_in(source, name, epoch(2050, 1, 1, 0)), 7200);
        EXPECT_EQ(source.offset_at(name, Instant{change, 0}).abbreviation,
                  "TWO");
    }
}

TEST(ZoneSource, AnOracleCanInferTheRuleAfterItsRange) {
    ZoneSource bundled = ZoneSource::bundled();
    ZoneSource source;
    source.add_oracle("Etc/TestNewYork",
                      ZoneOracle{[&](int64_t instant) {
                                     auto o =
                                         bundled.offset_at("America/New_York",
                                                           Instant{instant, 0});
                                     return ZoneType{o.offset.seconds, o.is_dst,
                                                     o.abbreviation};
                                 },
                                 nullptr, 2022, 2024, AfterProbe::Infer});
    EXPECT_EQ(offset_in(source, "Etc/TestNewYork", epoch(2040, 7, 1, 12)),
              -4 * 3600);
    EXPECT_EQ(offset_in(source, "Etc/TestNewYork", epoch(2040, 1, 1, 12)),
              -5 * 3600);
}

TEST(ZoneSource, OracleFailuresBecomeInvalidArgument) {
    ZoneSource source;
    try {
        source.add_oracle("Etc/TestThrows",
                          ZoneOracle{[](int64_t) -> ZoneType {
                                         throw std::runtime_error("boom");
                                     },
                                     nullptr, 2000, 2001, AfterProbe::Hold});
        FAIL() << "add_oracle did not throw";
    } catch (const std::invalid_argument& e) {
        EXPECT_NE(std::string(e.what()).find("boom"), std::string::npos)
            << e.what();
    }
    try {
        source.add_oracle("Etc/TestThrowsInt",
                          ZoneOracle{[](int64_t) -> ZoneType { throw 7; },
                                     nullptr, 2000, 2001, AfterProbe::Hold});
        FAIL() << "add_oracle did not throw";
    } catch (const std::invalid_argument& e) {
        EXPECT_NE(std::string(e.what()).find("std::exception"),
                  std::string::npos)
            << e.what();
    }
    EXPECT_THROW(source.add_oracle("Etc/TestMissing", ZoneOracle{}),
                 std::invalid_argument);
    EXPECT_THROW(source.add_oracle(
                     "Etc/TestStuck",
                     ZoneOracle{[](int64_t) { return ZoneType{0, false, "Z"}; },
                                [](int64_t from) -> std::optional<int64_t> {
                                    return from;
                                },
                                2000, 2001, AfterProbe::Hold}),
                 std::invalid_argument);
    EXPECT_THROW(source.add_oracle(
                     "Etc/TestInverted",
                     ZoneOracle{[](int64_t) { return ZoneType{0, false, "Z"}; },
                                nullptr, 2001, 2000, AfterProbe::Hold}),
                 std::invalid_argument);
}

TEST(ZoneSource, LinksShareTheirTargetsData) {
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());
    source.add_link("Etc/TestAlias", "Etc/TestEastern");
    auto described = source.info("Etc/TestAlias");
    ASSERT_TRUE(described.has_value());
    EXPECT_TRUE(described->is_link);
    EXPECT_EQ(described->primary_name, "Etc/TestEastern");
    EXPECT_EQ(
        source.resolve("Etc/TestAlias", datetime(2024, 3, 10, 2, 30)).kind,
        TransitionKind::Gap);
    EXPECT_EQ(source.resolve("Etc/TestAlias", datetime(2024, 1, 1, 0, 0))
                  .zone.primary_name,
              "Etc/TestEastern");
    EXPECT_THROW(source.add_link("Etc/TestSelf", "Etc/TestSelf"),
                 std::invalid_argument);
    EXPECT_FALSE(source.info("Etc/TestNowhere").has_value());
}

TEST(ZoneSource, CataloguedNamesKeepTheirRankAndTags) {
    ZoneSource source;
    source.add_zone("Europe/London", fixed(0, "GMT"));
    source.add_zone("Etc/TestFixed", fixed(5 * 3600 + 1800, "IST"));
    auto london = source.info("Europe/London");
    ASSERT_TRUE(london.has_value());
    EXPECT_EQ(london->rank, temporal::info("Europe/London")->rank);
    auto custom = source.info("Etc/TestFixed");
    ASSERT_TRUE(custom.has_value());
    EXPECT_GT(custom->rank, london->rank);
    EXPECT_FALSE(custom->is_link);
}

namespace {

    ZoneSource lazy_source(const std::shared_ptr<std::atomic<int>>& loads) {
        ZoneSource source;
        source.declare({"America/New_York", "Asia/Tokyo"})
            .set_loader([loads](ZoneSource& s, const std::string& name) {
                ++*loads;
                if (name == "America/New_York") {
                    s.add_zone(name, eastern());
                } else if (name == "Asia/Tokyo") {
                    s.add_zone(name, fixed(9 * 3600, "JST"));
                } else if (name == "Etc/TestLink") {
                    s.add_link(name, "Asia/Tokyo");
                }
            });
        return source;
    }

} // namespace

TEST(ZoneSource, TheLoaderSuppliesNamesOnFirstUse) {
    auto loads = std::make_shared<std::atomic<int>>(0);
    ZoneSource source = lazy_source(loads);
    EXPECT_EQ(loads->load(), 0);
    EXPECT_EQ(offset_in(source, "Asia/Tokyo", 0), 9 * 3600);
    EXPECT_EQ(loads->load(), 1);
    EXPECT_EQ(offset_in(source, "Asia/Tokyo", 0), 9 * 3600);
    EXPECT_EQ(loads->load(), 1);
    EXPECT_EQ(offset_in(source, "Etc/TestLink", 0), 9 * 3600);
    EXPECT_EQ(source.info("Etc/TestLink")->primary_name, "Asia/Tokyo");
    EXPECT_FALSE(source.info("Etc/TestNothing").has_value());
}

TEST(ZoneSource, DrawsLoadOnlyTheZonesTheyLandOn) {
    auto loads = std::make_shared<std::atomic<int>>(0);
    auto zoned = temporal::zoned_datetimes(
        {.zones = temporal::zones({.source = lazy_source(loads)})});
    EXPECT_EQ(loads->load(), 0);
    std::set<std::string> drawn;
    hegel::test(
        [&](hegel::TestCase& tc) {
            auto value = tc.draw(zoned);
            drawn.insert(value.zone.name);
            EXPECT_EQ(value.zone.name, value.zone.primary_name);
        },
        quick());
    EXPECT_EQ(drawn, (std::set<std::string>{"America/New_York", "Asia/Tokyo"}));
    EXPECT_EQ(loads->load(), static_cast<int>(drawn.size()));
}

TEST(ZoneSource, ALoaderIsFreedWhenTheSourceLetsItGo) {
    auto token = std::make_shared<int>(0);
    auto loader = [token](ZoneSource& s, const std::string& name) {
        s.add_zone(name, fixed(*token, "Z"));
    };
    {
        ZoneSource source;
        source.set_loader(loader);
        EXPECT_EQ(token.use_count(), 3);
        source.set_loader([](ZoneSource&, const std::string&) {});
        EXPECT_EQ(token.use_count(), 2);
        source.set_loader(loader);
        EXPECT_EQ(token.use_count(), 3);
        source.set_loader(nullptr);
        EXPECT_EQ(token.use_count(), 2);
        source.set_loader(loader);
        EXPECT_EQ(token.use_count(), 3);
    }
    EXPECT_EQ(token.use_count(), 2);

    {
        ZoneSource source;
        source.declare({"Etc/TestLater"}).set_loader(loader);
        auto zoned = temporal::zoned_datetimes(
            {.zones = temporal::zones({.source = std::move(source)})});
        EXPECT_EQ(token.use_count(), 3);
        assert_all_examples<temporal::ZonedDateTime>(
            zoned, [](const temporal::ZonedDateTime& z) {
                return z.zone.name == "Etc/TestLater";
            });
    }
    EXPECT_EQ(token.use_count(), 2);

    ZoneSource bundled = ZoneSource::bundled();
    EXPECT_THROW(bundled.set_loader(loader), std::invalid_argument);
    EXPECT_EQ(token.use_count(), 2);
}

TEST(ZoneSource, ThreadsLoadAndDrawFromOneSourceTogether) {
    std::vector<std::string> names;
    for (int i = 0; i < 16; ++i) {
        names.push_back("Etc/TestZone" + std::to_string(i));
    }
    auto loads = std::make_shared<std::atomic<int>>(0);
    ZoneSource source;
    source.declare(names).set_loader([loads](ZoneSource& s,
                                             const std::string& name) {
        ++*loads;
        int index = std::stoi(name.substr(std::string("Etc/TestZone").size()));
        s.add_zone(name, fixed(index * 60, "T" + std::to_string(index)));
    });
    auto builtin = temporal::zoned_datetimes(
        {.zones = temporal::zones({.source = source})});
    auto wrapped = temporal::zoned_datetimes(
        {.zones = temporal::zones({.source = source})
                      .filter([](const temporal::TimeZone&) { return true; }),
         .source = source});
    std::atomic<int> wrong{0};
    std::vector<std::thread> threads;
    for (int t = 0; t < 8; ++t) {
        threads.emplace_back([&, t] {
            for (size_t i = 0; i < names.size(); ++i) {
                size_t index = (i + static_cast<size_t>(t)) % names.size();
                if (offset_in(source, names[index], 0) !=
                    static_cast<int32_t>(index) * 60) {
                    ++wrong;
                }
            }
            hegel::test(
                [&](hegel::TestCase& tc) {
                    for (const auto& gen : {builtin, wrapped}) {
                        temporal::ZonedDateTime z = tc.draw(gen);
                        int index = std::stoi(z.zone.name.substr(
                            std::string("Etc/TestZone").size()));
                        if (z.offset.seconds != index * 60) {
                            ++wrong;
                        }
                    }
                },
                quick());
        });
    }
    for (std::thread& thread : threads) {
        thread.join();
    }
    EXPECT_EQ(wrong.load(), 0);
    EXPECT_GE(loads->load(), static_cast<int>(names.size()));
}

TEST(ZoneSource, ALoaderFailureFailsTheQuery) {
    ZoneSource source;
    source.declare({"Etc/TestBroken"})
        .set_loader([](ZoneSource&, const std::string& name) {
            throw std::runtime_error("cannot load " + name);
        });
    try {
        source.offset_at("Etc/TestBroken", Instant{0, 0});
        FAIL() << "offset_at did not throw";
    } catch (const std::invalid_argument& e) {
        EXPECT_NE(std::string(e.what()).find("cannot load Etc/TestBroken"),
                  std::string::npos)
            << e.what();
    }
}

TEST(ZoneSource, WithoutALoaderADeclaredNameHasNoData) {
    ZoneSource source;
    source.declare({"Etc/TestLater"})
        .set_loader([](ZoneSource& s, const std::string& name) {
            s.add_zone(name, fixed(0, "Z"));
        })
        .set_loader(nullptr);
    EXPECT_THROW(source.offset_at("Etc/TestLater", Instant{0, 0}),
                 std::invalid_argument);
    source.add_zone("Etc/TestLater", fixed(60, "M"));
    EXPECT_EQ(offset_in(source, "Etc/TestLater", 0), 60);
    EXPECT_THROW(source.declare({""}), std::invalid_argument);
}

TEST(ZoneSource, CopiesShareOneSource) {
    ZoneSource source;
    ZoneSource copy = source;
    copy.add_zone("Etc/TestFixed", fixed(120, "TWO"));
    EXPECT_EQ(offset_in(source, "Etc/TestFixed", 0), 120);
}

TEST(ZoneSource, VersionsAreRecorded) {
    ZoneSource source;
    EXPECT_EQ(source.version(), "");
    source.set_version("2099z");
    EXPECT_EQ(source.version(), "2099z");
    EXPECT_EQ(ZoneSource::bundled().version(), temporal::tzdb_version());
}

TEST(ZoneSource, TheBundledSourceCannotChange) {
    ZoneSource bundled = ZoneSource::bundled();
    EXPECT_THROW(bundled.add_zone("Etc/TestFixed", fixed(0, "Z")),
                 std::invalid_argument);
    EXPECT_THROW(bundled.set_version("x"), std::invalid_argument);
    EXPECT_THROW(bundled.set_loader(nullptr), std::invalid_argument);
    EXPECT_THROW(bundled.set_loader([](ZoneSource&, const std::string&) {}),
                 std::invalid_argument);
}

TEST(ZoneSourceGenerators, ZonesDrawTheSourcesNames) {
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());
    source.add_zone("Etc/TestFixed", fixed(3600, "ONE"));
    assert_all_examples<temporal::TimeZone>(
        temporal::zones({.source = source}),
        [](const temporal::TimeZone& zone) {
            return zone.name == "Etc/TestEastern" ||
                   zone.name == "Etc/TestFixed";
        });
    assert_all_examples<temporal::TimeZone>(
        temporal::zones({.names = std::vector<std::string>{"Etc/TestFixed"},
                         .source = source}),
        [](const temporal::TimeZone& zone) {
            return zone.name == "Etc/TestFixed";
        });
    EXPECT_THROW(
        temporal::zones({.names = std::vector<std::string>{"Europe/London"},
                         .source = source}),
        std::invalid_argument);
    (void)temporal::zones({.names = std::vector<std::string>{"Europe/London"},
                           .allow_opaque = true,
                           .source = source});
    EXPECT_THROW(temporal::zones({.source = ZoneSource()}),
                 std::invalid_argument);
    ZoneSource legacy_only;
    legacy_only.add_zone("EST5EDT", eastern());
    EXPECT_THROW(temporal::zones({.source = legacy_only}),
                 std::invalid_argument);
    draw_once(temporal::zones({.allow_legacy = true, .source = legacy_only}));

    ZoneSource mars;
    mars.add_zone("Mars/Olympus", fixed(0, "MAR"));
    assert_all_examples<temporal::TimeZone>(
        temporal::zones({.source = mars}), [](const temporal::TimeZone& zone) {
            return zone.name == "Mars/Olympus";
        });
}

TEST(ZoneSourceGenerators, ZonedDatetimesResolveAgainstTheSource) {
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());
    auto gap = find_any<temporal::ZonedDateTime>(
        temporal::zoned_datetimes({.zones = temporal::zones({.source = source}),
                                   .min_value = datetime(2024, 3, 10, 0, 0),
                                   .max_value = datetime(2024, 3, 10, 5, 0)}),
        [](const temporal::ZonedDateTime& z) {
            return z.kind == TransitionKind::Gap;
        });
    EXPECT_EQ(gap.zone.name, "Etc/TestEastern");
    EXPECT_EQ(gap.civil.time.hour, 2);
}

TEST(ZoneSourceGenerators, ZonedDatetimesTakeASourceWithoutZones) {
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());
    auto gap = find_any<temporal::ZonedDateTime>(
        temporal::zoned_datetimes({.source = source,
                                   .min_value = datetime(2024, 3, 10, 0, 0),
                                   .max_value = datetime(2024, 3, 10, 5, 0)}),
        [](const temporal::ZonedDateTime& z) {
            return z.kind == TransitionKind::Gap;
        });
    EXPECT_EQ(gap.zone.name, "Etc/TestEastern");
    EXPECT_EQ(gap.zone.primary_name, "Etc/TestEastern");
}

TEST(ZoneSourceGenerators, AWrappedZonesGeneratorResolvesAgainstTheSource) {
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());
    source.add_zone("Etc/TestFixed", fixed(3600, "ONE"));
    source.add_link("Etc/TestAlias", "Etc/TestEastern");
    auto wrapped = temporal::zones({.source = source})
                       .filter([](const temporal::TimeZone& zone) {
                           return zone.name != "Etc/TestFixed";
                       });
    auto zoned =
        temporal::zoned_datetimes({.zones = wrapped,
                                   .source = source,
                                   .min_value = datetime(2024, 3, 10, 0, 0),
                                   .max_value = datetime(2024, 3, 10, 5, 0)});
    assert_all_examples<temporal::ZonedDateTime>(
        zoned, [&](const temporal::ZonedDateTime& z) {
            return z.zone.name != "Etc/TestFixed" &&
                   z.zone.primary_name == "Etc/TestEastern" && z == [&] {
                       temporal::ZonedDateTime again =
                           source.resolve(z.zone.name, z.civil);
                       again.fold = z.fold;
                       again.offset = z.offset;
                       again.instant = z.instant;
                       return again;
                   }();
        });
    auto gap = find_any<temporal::ZonedDateTime>(
        zoned, [](const temporal::ZonedDateTime& z) {
            return z.kind == TransitionKind::Gap &&
                   z.zone.name == "Etc/TestAlias";
        });
    EXPECT_EQ(gap.civil.time.hour, 2);
    EXPECT_EQ(gap.offset_before.seconds, -5 * 3600);

    for (const auto& zones :
         {gs::just(temporal::TimeZone{"Etc/TestEastern", "Etc/TestEastern"}),
          gs::sampled_from(std::vector<temporal::TimeZone>{
              {"Etc/TestEastern", "Etc/TestEastern"}}),
          gs::one_of<temporal::TimeZone>({temporal::zones(
              {.names = std::vector<std::string>{"Etc/TestEastern"},
               .source = source})})}) {
        auto fold = find_any<temporal::ZonedDateTime>(
            temporal::zoned_datetimes(
                {.zones = zones,
                 .source = source,
                 .min_value = datetime(2024, 11, 3, 0, 0),
                 .max_value = datetime(2024, 11, 3, 3, 0)}),
            [](const temporal::ZonedDateTime& z) {
                return z.kind == TransitionKind::Fold && z.fold;
            });
        EXPECT_EQ(fold.offset.seconds, -5 * 3600);
        EXPECT_EQ(fold.zone.primary_name, "Etc/TestEastern");
    }
}

TEST(ZoneSourceGenerators, ANameTheSourceLacksIsAnError) {
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());
    std::string missing = draw_error(temporal::zoned_datetimes(
        {.zones =
             gs::just(temporal::TimeZone{"Europe/London", "Europe/London"}),
         .source = source}));
    expect_contains(missing, "\"Europe/London\"");
    expect_contains(missing, "`source`");

    std::string not_in_default = draw_error(temporal::zoned_datetimes(
        {.zones = temporal::zones({.source = source})
                      .map([](temporal::TimeZone zone) { return zone; })}));
    expect_contains(not_in_default, "\"Etc/TestEastern\"");
    expect_contains(not_in_default, "`source`");

    std::string opaque = draw_error(temporal::zoned_datetimes(
        {.zones =
             temporal::zones({.names = std::vector<std::string>{"Mars/Olympus"},
                              .allow_opaque = true})
                 .filter([](const temporal::TimeZone&) { return true; })}));
    expect_contains(opaque, "\"Mars/Olympus\"");

    auto unknown = temporal::zoned_datetimes(
        {.zones =
             gs::just(temporal::TimeZone{std::string(temporal::UNKNOWN_ZONE),
                                         std::string(temporal::UNKNOWN_ZONE)}),
         .source = source});
    assert_all_examples<temporal::ZonedDateTime>(
        unknown, [](const temporal::ZonedDateTime& z) {
            return z.offset == temporal::UNKNOWN_OFFSET;
        });
}

TEST(ZoneSourceGenerators, ASourceNextToAZonesGeneratorWithItsOwnIsAnError) {
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());
    for (const auto& zones :
         {temporal::zones(), temporal::zones({.source = source})}) {
        try {
            temporal::zoned_datetimes({.zones = zones, .source = source});
            ADD_FAILURE() << "the source was accepted";
        } catch (const std::invalid_argument& e) {
            expect_contains(e.what(), "Pass the source to zones() instead");
        }
    }
}

TEST(ZoneSourceGenerators, AZoneWithNoValueInTheBoundsRejectsTheTestCase) {
    ZoneSource source;
    source.add_zone("Etc/TestWhole", fixed(3600, "W"));
    source.add_zone("Etc/TestHalf", fixed(30, "H"));
    int64_t noon = epoch(2024, 6, 1, 12);
    auto zoned = temporal::zoned_datetimes(
        {.zones = temporal::zones({.source = source})
                      .filter([](const temporal::TimeZone&) { return true; }),
         .source = source,
         .min_instant = Instant{noon + 1, 0},
         .max_instant = Instant{noon + 59, 0},
         .precision = temporal::Precision::Minutes});
    std::set<std::string> drawn;
    hegel::test(
        [&](hegel::TestCase& tc) {
            temporal::ZonedDateTime z = tc.draw(zoned);
            drawn.insert(z.zone.name);
            EXPECT_EQ(z.instant, (Instant{noon + 30, 0}));
            EXPECT_EQ(z.civil, datetime(2024, 6, 1, 12, 1));
        },
        quick());
    EXPECT_EQ(drawn, (std::set<std::string>{"Etc/TestHalf"}));
}

TEST(ZoneSourceDefault, SettingTheDefaultReturnsThePreviousOne) {
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());
    ZoneSource previous = temporal::set_default_zone_source(source);
    EXPECT_EQ(previous.version(), temporal::tzdb_version());
    EXPECT_TRUE(previous.info("Europe/London").has_value());
    EXPECT_TRUE(temporal::info("Etc/TestEastern").has_value());
    ZoneSource mine = temporal::set_default_zone_source(previous);
    mine.add_zone("Etc/TestFixed", fixed(60, "M"));
    EXPECT_TRUE(source.info("Etc/TestFixed").has_value());
    EXPECT_TRUE(temporal::info("Europe/London").has_value());
    EXPECT_FALSE(temporal::info("Etc/TestEastern").has_value());
}

TEST(ZoneSourceDefault, AGeneratorKeepsTheDefaultItWasBuiltWith) {
    temporal::TimeZone london{"Europe/London", "Europe/London"};
    auto zones_before = temporal::zones();
    auto zoned_before = temporal::zoned_datetimes();
    auto wrapped_before =
        temporal::zoned_datetimes({.zones = gs::just(london)});
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());
    ScopedDefault scoped(source);

    ZoneSource bundled = ZoneSource::bundled();
    assert_all_examples<temporal::TimeZone>(
        zones_before, [&](const temporal::TimeZone& zone) {
            return bundled.info(zone.name).has_value();
        });
    assert_all_examples<temporal::ZonedDateTime>(
        zoned_before, [&](const temporal::ZonedDateTime& z) {
            return bundled.info(z.zone.name).has_value();
        });
    auto summer = find_any<temporal::ZonedDateTime>(
        wrapped_before, [](const temporal::ZonedDateTime& z) {
            return z.offset.seconds == 3600;
        });
    EXPECT_EQ(summer.zone, london);

    expect_contains(
        draw_error(temporal::zoned_datetimes({.zones = gs::just(london)})),
        "\"Europe/London\"");
    assert_all_examples<temporal::TimeZone>(
        temporal::zones(), [](const temporal::TimeZone& zone) {
            return zone.name == "Etc/TestEastern";
        });
}

TEST(ZoneSourceDefault, TheDefaultSourceDrivesZonesAndQueries) {
    EXPECT_EQ(temporal::default_zone_source().version(),
              temporal::tzdb_version());
    ZoneSource source;
    source.add_zone("Etc/TestEastern", eastern());
    ZoneSource previous = temporal::set_default_zone_source(source);

    EXPECT_FALSE(temporal::info("Europe/London").has_value());
    EXPECT_EQ(temporal::offset_at("Etc/TestEastern",
                                  Instant{epoch(2024, 7, 1, 12), 0})
                  .offset.seconds,
              -4 * 3600);
    EXPECT_EQ(
        temporal::resolve("Etc/TestEastern", datetime(2024, 3, 10, 2, 30)).kind,
        TransitionKind::Gap);
    assert_all_examples<temporal::ZonedDateTime>(
        temporal::zoned_datetimes(), [](const temporal::ZonedDateTime& z) {
            return z.zone.name == "Etc/TestEastern";
        });
    auto any_zone = temporal::zoned_datetimes(
        {.zones =
             gs::just(temporal::TimeZone{"Etc/TestEastern", "Etc/TestEastern"}),
         .min_value = datetime(2024, 3, 10, 2, 0),
         .max_value = datetime(2024, 3, 10, 2, 59)});

    temporal::set_default_zone_source(previous);
    assert_all_examples<temporal::ZonedDateTime>(
        any_zone, [](const temporal::ZonedDateTime& z) {
            return z.kind == TransitionKind::Gap &&
                   z.zone.primary_name == "Etc/TestEastern";
        });
    EXPECT_TRUE(temporal::info("Europe/London").has_value());
}
