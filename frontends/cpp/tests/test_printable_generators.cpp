#include <gtest/gtest.h>

#include <array>
#include <cstdint>
#include <functional>
#include <map>
#include <memory>
#include <optional>
#include <regex>
#include <set>
#include <sstream>
#include <stdexcept>
#include <string>
#include <thread>
#include <tuple>
#include <variant>
#include <vector>

#include <hegel/hegel.h>
#include <hegel/repr.h>

namespace gs = hegel::generators;

class KeyData {
  public:
    static KeyData from_ffi(int raw) { return KeyData(raw); }
    int raw() const { return raw_; }

  private:
    explicit KeyData(int raw) : raw_(raw) {}
    int raw_;
};

struct Point {
    int x;
    int y;
};

class PointGenerator : public gs::IPrintableGenerator<Point> {
  public:
    Point do_draw_and_print(const hegel::TestCase& tc,
                            hegel::Printer& p) const override {
        p.begin_group(6, "Point(");
        int x = tc.draw_and_print(coordinate_, p);
        p.separator();
        int y = tc.draw_and_print(coordinate_, p);
        p.end_group(")");
        return {x, y};
    }

  private:
    gs::Generator<int> coordinate_ =
        gs::integers<int>({.min_value = 0, .max_value = 9});
};

struct Holder {
    hegel::Printer slot;
};

struct Span {
    Span(int a, int b) : lo(a), hi(b) {}
    int lo;
    int hi;
};

class HolderGenerator
    : public gs::IPrintableGenerator<std::shared_ptr<Holder>> {
  public:
    std::shared_ptr<Holder>
    do_draw_and_print(const hegel::TestCase&,
                      hegel::Printer& p) const override {
        p.text("Holder(");
        auto holder = std::make_shared<Holder>(Holder{p.deferred()});
        p.text(")");
        return holder;
    }
};

namespace {
    using Body = std::function<void(hegel::TestCase&)>;

    std::vector<std::string> split_lines(const std::string& text) {
        std::vector<std::string> lines;
        std::istringstream stream(text);
        std::string line;
        while (std::getline(stream, line)) {
            lines.push_back(line);
        }
        return lines;
    }

    hegel::Settings settings_at(hegel::Verbosity verbosity,
                                uint64_t test_cases = 50) {
        hegel::Settings settings;
        settings.test_cases = test_cases;
        settings.verbosity = verbosity;
        settings.derandomize = true;
        settings.database = hegel::Database::disabled();
        return settings;
    }

    std::vector<std::string> failing_lines(const Body& body) {
        testing::internal::CaptureStderr();
        bool failed = false;
        try {
            hegel::test(body, settings_at(hegel::Verbosity::Normal));
        } catch (...) {
            failed = true;
        }
        std::string out = testing::internal::GetCapturedStderr();
        EXPECT_TRUE(failed) << out;
        std::vector<std::string> lines;
        for (const std::string& line : split_lines(out)) {
            if (line.empty()) {
                break;
            }
            lines.push_back(line);
        }
        return lines;
    }

    std::string verbose_output(const Body& body, uint64_t test_cases = 20) {
        testing::internal::CaptureStderr();
        try {
            hegel::test(body,
                        settings_at(hegel::Verbosity::Verbose, test_cases));
        } catch (...) {
        }
        return testing::internal::GetCapturedStderr();
    }

    void boom() { throw std::runtime_error("boom"); }

    gs::Generator<int> zero() {
        return gs::integers<int>({.min_value = 0, .max_value = 0});
    }

    gs::Generator<int> hashed() {
        return gs::integers<int>({.min_value = 0, .max_value = 9})
            .print_with([](int v, hegel::Printer& p) {
                p.text("#" + std::to_string(v));
            });
    }

    auto key_data() {
        return zero()
            .map([](int v) { return KeyData::from_ffi(v); })
            .print_as_call("KeyData::from_ffi");
    }

    using Lines = std::vector<std::string>;
} // namespace

TEST(PrintableGenerators, LeafDrawsPrintTheirShrunkValue) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(
                      gs::integers<int>({.min_value = 0, .max_value = 100}));
                  boom();
              }),
              Lines{"auto draw_1 = 0;"});
}

TEST(PrintableGenerators, NotesAndDrawsInterleaveInOrder) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(gs::booleans());
                  tc.note("mid");
                  (void)tc.draw(gs::booleans());
                  boom();
              }),
              (Lines{"auto draw_1 = false;", "mid", "auto draw_2 = false;"}));
}

TEST(PrintableGenerators, ACustomPrintedElementPrintsInsideAVector) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(
                      gs::vectors(hashed(), {.min_size = 2, .max_size = 2}));
                  boom();
              }),
              Lines{"auto draw_1 = std::vector<int>{#0, #0};"});
}

TEST(PrintableGenerators, AUserGeneratorPrintsInsideAVector) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(
                      gs::vectors(gs::make_generator<PointGenerator>(),
                                  {.min_size = 2, .max_size = 2}));
                  boom();
              }),
              Lines{"auto draw_1 = std::vector<Point>{Point(0, 0), Point(0, "
                    "0)};"});
}

TEST(PrintableGenerators, CustomPrintingSurvivesEveryCombinator) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw("o", gs::optional(hashed()).filter(
                                         [](const std::optional<int>& v) {
                                             return v.has_value();
                                         }));
                  (void)tc.draw("t", gs::tuples(hashed(), gs::booleans()));
                  (void)tc.draw("m", gs::maps(hashed(), hashed(),
                                              {.min_size = 1, .max_size = 1}));
                  (void)tc.draw("one", gs::one_of({hashed(), hashed()}));
                  (void)tc.draw("f", hashed().filter([](int) { return true; }));
                  (void)tc.draw("fm", gs::booleans().flat_map(
                                          [](bool) { return hashed(); }));
                  (void)tc.draw(
                      "r", gs::recursive(hashed(), [](gs::Generator<int> sub) {
                          return gs::one_of({sub, sub});
                      }));
                  auto d = gs::deferred<int>();
                  d.set(hashed());
                  (void)tc.draw("d", d.generator());
                  boom();
              }),
              (Lines{"auto o_1 = std::optional<int>{#0};",
                     "auto t_1 = std::tuple<int, bool>{#0, false};",
                     "auto m_1 = std::map<int, int>{{#0, #0}};",
                     "auto one_1 = #0;", "auto f_1 = #0;", "auto fm_1 = #0;",
                     "auto r_1 = #0;", "auto d_1 = #0;"}));
}

#if HEGEL_HAS_REFLECTION
struct Pair {
    int a;
    bool b;
};

TEST(PrintableGenerators, CustomPrintingSurvivesAFieldOverride) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(gs::default_generator<Pair>().override(
                      gs::field<&Pair::a>(hashed())));
                  boom();
              }),
              Lines{"auto draw_1 = Pair{.a = #0, .b = false};"});
}

TEST(PrintableGenerators, DerivedGeneratorsPrintFieldByField) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(gs::default_generator<std::vector<Pair>>());
                  (void)tc.draw(gs::default_generator<Pair>());
                  boom();
              }),
              (Lines{"auto draw_1 = std::vector<Pair>{};",
                     "auto draw_2 = Pair{.a = 0, .b = false};"}));
}

TEST(PrintableGenerators, AFieldOverrideDrawsTheFieldOnce) {
    int calls = 0;
    hegel::test(
        [&calls](hegel::TestCase& tc) {
            calls = 0;
            (void)tc.draw(gs::default_generator<Pair>().override(
                gs::field<&Pair::a>(gs::integers<int>().map([&calls](int v) {
                    ++calls;
                    return v;
                }))));
            EXPECT_EQ(calls, 1);
        },
        settings_at(hegel::Verbosity::Quiet, 5));
}

TEST(PrintableGenerators, DerivedPrintingMatchesTheValuesRepr) {
    std::vector<Pair> pairs;
    Pair pair{};
    Lines lines = failing_lines([&](hegel::TestCase& tc) {
        pairs = tc.draw(gs::default_generator<std::vector<Pair>>());
        pair = tc.draw(gs::default_generator<Pair>());
        if (!pairs.empty() && pair.a != 0) {
            boom();
        }
    });
    EXPECT_EQ(
        lines,
        (Lines{"auto draw_1 = " + hegel::internal::render(pairs, 79) + ";",
               "auto draw_2 = " + hegel::internal::render(pair, 79) + ";"}));
}
#endif

TEST(PrintableGenerators, ARandomGeneratorPrintsWhatItHandsOutLater) {
    EXPECT_EQ(
        failing_lines([](hegel::TestCase& tc) {
            auto rngs =
                tc.draw("rngs", gs::vectors(gs::randoms(),
                                            {.min_size = 2, .max_size = 2}));
            (void)rngs[0]();
            (void)rngs[0]();
            (void)tc.draw("n", zero());
            boom();
        }),
        (Lines{"auto rngs_1 = std::vector<hegel::generators::HegelRandom>{",
               "    HegelRandom{.consumed = {0, 0}},",
               "    HegelRandom{.consumed = {}}", "};", "auto n_1 = 0;"}));
}

TEST(PrintableGenerators, ADeferredPrinterCanBeFilledFromAThread) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  auto holder = tc.draw(gs::make_generator<HolderGenerator>());
                  (void)tc.draw("n", zero());
                  std::thread([holder] {
                      holder->slot.text("from a thread");
                  }).join();
                  boom();
              }),
              (Lines{"auto draw_1 = Holder(from a thread);", "auto n_1 = 0;"}));
}

TEST(PrintableGenerators, ATrueRandomGeneratorPrintsItsSeed) {
    Lines lines = failing_lines([](hegel::TestCase& tc) {
        (void)tc.draw("rng", gs::randoms({.use_true_random = true}));
        boom();
    });
    ASSERT_EQ(lines.size(), 1u);
    EXPECT_EQ(lines[0].rfind("auto rng_1 = HegelRandom{.seed = ", 0), 0u)
        << lines[0];
}

TEST(PrintableGenerators, SilentDrawsPrintNothing) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  auto rng = tc.draw_silent(gs::randoms());
                  (void)rng();
                  (void)tc.draw_silent(gs::integers<int>());
                  (void)tc.draw("n", zero());
                  boom();
              }),
              Lines{"auto n_1 = 0;"});
}

TEST(PrintableGenerators, MapPrintsTheFinishedValue) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  auto three =
                      gs::integers<int>({.min_value = 3, .max_value = 3});
                  (void)tc.draw(three.map([](int x) { return x * 2; }));
                  (void)tc.draw(three
                                    .print_with([](int, hegel::Printer& p) {
                                        p.text("custom");
                                    })
                                    .map([](int x) { return x * 2; }));
                  boom();
              }),
              (Lines{"auto draw_1 = 6;", "auto draw_2 = 6;"}));
}

TEST(PrintableGenerators, PrintAsCallPrintsTheMappedExpression) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw("key", key_data());
                  (void)tc.draw(
                      "keys",
                      gs::vectors(key_data(), {.min_size = 2, .max_size = 2}));
                  boom();
              }),
              (Lines{"auto key_1 = KeyData::from_ffi(0);",
                     "auto keys_1 = std::vector<KeyData>{KeyData::from_ffi(0), "
                     "KeyData::from_ffi(0)};"}));
}

TEST(PrintableGenerators, PrintAsCallBreaksAWideListOneItemPerLine) {
    EXPECT_EQ(
        failing_lines([](hegel::TestCase& tc) {
            (void)tc.draw("keys", gs::vectors(key_data(),
                                              {.min_size = 4, .max_size = 4}));
            boom();
        }),
        (Lines{"auto keys_1 = std::vector<KeyData>{",
               "    KeyData::from_ffi(0),", "    KeyData::from_ffi(0),",
               "    KeyData::from_ffi(0),", "    KeyData::from_ffi(0)", "};"}));
}

TEST(PrintableGenerators, PrintAsCallWrapsWideInputsInsideTheCall) {
    auto wide = gs::vectors(gs::just(std::string(30, 'a')),
                            {.min_size = 2, .max_size = 2})
                    .map([](std::vector<std::string> v) { return v.size(); })
                    .print_as_call("Wrapped::from_parts");
    EXPECT_EQ(failing_lines([wide](hegel::TestCase& tc) {
                  (void)tc.draw("w", wide);
                  boom();
              }),
              (Lines{"auto w_1 = Wrapped::from_parts(std::vector<std::string>{",
                     "                        std::string("
                     "\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"),",
                     "                        std::string("
                     "\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\")",
                     "                    });"}));
}

TEST(PrintableGenerators, PrintAsCallNeedsAName) {
    EXPECT_THROW((void)zero().map([](int v) { return v; }).print_as_call(""),
                 std::invalid_argument);
}

TEST(PrintableGenerators, PrintAsReprMatchesThePlainDraw) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(zero().print_as_repr());
                  (void)tc.draw(gs::vectors(zero().print_as_repr().filter(
                                                [](int v) { return v == 0; }),
                                            {.min_size = 1, .max_size = 1}));
                  (void)tc.draw(gs::compose([](const hegel::TestCase& inner) {
                                    return inner.draw(zero()) + 1;
                                }).print_as_repr());
                  boom();
              }),
              (Lines{"auto draw_1 = 0;", "auto draw_2 = std::vector<int>{0};",
                     "auto draw_3 = 1;"}));
}

TEST(PrintableGenerators, ComposeInnerDrawsAreSilent) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(gs::compose([](const hegel::TestCase& inner) {
                      return std::vector<int>{inner.draw(zero()),
                                              inner.draw(zero())};
                  }));
                  boom();
              }),
              Lines{"auto draw_1 = std::vector<int>{0, 0};"});
}

TEST(PrintableGenerators, BuildsPrintsAConstructorCall) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(gs::builds<Span>(zero(), hashed()));
                  (void)tc.draw(gs::vectors(gs::builds<Span>(zero(), hashed()),
                                            {.min_size = 1, .max_size = 1}));
                  boom();
              }),
              (Lines{"auto draw_1 = Span(0, #0);",
                     "auto draw_2 = std::vector<Span>{Span(0, #0)};"}));
}

TEST(PrintableGenerators, SetsAndMapsPrintInDrawOrder) {
    auto descending =
        gs::integers<int>({.min_value = 0, .max_value = 9}).map([](int x) {
            return 9 - x;
        });
    EXPECT_EQ(failing_lines([descending](hegel::TestCase& tc) {
                  (void)tc.draw("s", gs::sets(descending,
                                              {.min_size = 2, .max_size = 2}));
                  (void)tc.draw("m", gs::maps(descending, zero(),
                                              {.min_size = 2, .max_size = 2}));
                  boom();
              }),
              (Lines{"auto s_1 = std::set<int>{9, 8};",
                     "auto m_1 = std::map<int, int>{{9, 0}, {8, 0}};"}));
}

TEST(PrintableGenerators, FilteredDrawsPrintOnlyTheAcceptedValue) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(
                      gs::integers<int>({.min_value = 0, .max_value = 10})
                          .filter([](int v) { return v > 4; })
                          .filter([](int v) { return v % 2 == 0; }));
                  boom();
              }),
              Lines{"auto draw_1 = 6;"});
}

TEST(PrintableGenerators, FilteredPoolDrawsPrintOnlyTheAcceptedValue) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  hegel::stateful::Pool<int> pool(tc);
                  pool.add(1);
                  pool.add(2);
                  pool.add(3);
                  (void)tc.draw(hegel::stateful::values_consumed(pool).filter(
                      [](int v) { return v == 2; }));
                  boom();
              }),
              Lines{"auto draw_1 = 2;"});
}

TEST(PrintableGenerators, RejectedAttemptsNeverCorruptVerboseOutput) {
    std::string out = verbose_output([](hegel::TestCase& tc) {
        (void)tc.draw("f", gs::integers<int>({.min_value = 0, .max_value = 9})
                               .filter([](int v) { return v % 3 == 0; }));
        (void)tc.draw(
            "u",
            gs::vectors(gs::integers<int>({.min_value = 0, .max_value = 3}),
                        {.min_size = 3, .max_size = 3, .unique = true}));
        (void)tc.draw(
            "s", gs::sets(gs::integers<int>({.min_value = 0, .max_value = 2}),
                          {.min_size = 2, .max_size = 2}));
        (void)tc.draw(
            "m", gs::maps(gs::integers<int>({.min_value = 0, .max_value = 2}),
                          zero(), {.min_size = 2, .max_size = 2}));
    });
    std::regex shape(
        R"(auto f_1 = [0369];|auto u_1 = std::vector<int>\{[0-3], [0-3], [0-3]\};|auto s_1 = std::set<int>\{[0-2], [0-2]\};|auto m_1 = std::map<int, int>\{\{[0-2], 0\}, \{[0-2], 0\}\};)");
    int draws = 0;
    for (const std::string& line : split_lines(out)) {
        if (line.rfind("auto ", 0) == 0) {
            ++draws;
            EXPECT_TRUE(std::regex_match(line, shape)) << line;
        }
    }
    EXPECT_GT(draws, 0) << out;
}

TEST(PrintableGenerators, ExhaustedFilterRetriesPrintNothing) {
    std::string never = verbose_output(
        [](hegel::TestCase& tc) {
            (void)tc.draw(
                "never", gs::integers<int>().filter([](int) { return false; }));
        },
        5);
    EXPECT_EQ(never.find("auto never"), std::string::npos) << never;
    std::string points = verbose_output(
        [](hegel::TestCase& tc) {
            (void)tc.draw("points", gs::make_generator<PointGenerator>().filter(
                                        [](const Point&) { return false; }));
        },
        5);
    EXPECT_EQ(points.find("auto points"), std::string::npos) << points;
}

TEST(PrintableGenerators, AThrowingPrintWithLeavesNoPartialLine) {
    testing::internal::CaptureStderr();
    EXPECT_THROW(hegel::test(
                     [](hegel::TestCase& tc) {
                         tc.note("before");
                         (void)tc.draw(
                             zero().print_with([](int, hegel::Printer& p) {
                                 p.text("partial");
                                 throw std::logic_error("cannot print");
                             }));
                     },
                     settings_at(hegel::Verbosity::Verbose, 1)),
                 std::logic_error);
    std::string out = testing::internal::GetCapturedStderr();
    EXPECT_NE(out.find("before"), std::string::npos) << out;
    EXPECT_EQ(out.find("partial"), std::string::npos) << out;
}

TEST(PrintableGenerators, SmallTuplesOptionalsAndVariantsPrint) {
    EXPECT_EQ(
        failing_lines([](hegel::TestCase& tc) {
            (void)tc.draw(gs::tuples(gs::booleans()));
            (void)tc.draw(gs::optional(zero()));
            (void)tc.draw(gs::variant(zero(), gs::booleans()));
            boom();
        }),
        (Lines{"auto draw_1 = std::tuple<bool>{false};",
               "auto draw_2 = std::optional<int>{};",
               "auto draw_3 = std::variant<int, bool>{std::in_place_index<0>, "
               "0};"}));
}

TEST(PrintableGenerators, ADeferredSilentGeneratorDrawsSilently) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  auto d = gs::deferred_silent<int>();
                  d.set(gs::AnyGenerator<int>(zero()));
                  (void)tc.draw_silent(d.generator());
                  (void)tc.draw("n", zero());
                  boom();
              }),
              Lines{"auto n_1 = 0;"});
}

TEST(PrintableGenerators, ARecursiveGeneratorWithASilentLeafDrawsSilently) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  auto trees = gs::recursive(
                      gs::AnyGenerator<int>(zero()),
                      [](gs::AnyGenerator<int> sub) {
                          return gs::vectors(sub, {.max_size = 2})
                              .map([](const std::vector<int>& v) {
                                  return static_cast<int>(v.size());
                              });
                      });
                  (void)tc.draw_silent(trees);
                  (void)tc.draw("n", zero());
                  boom();
              }),
              Lines{"auto n_1 = 0;"});
}

TEST(PrintableGenerators, ARecursiveGeneratorPrintsThroughItsBranches) {
    EXPECT_EQ(
        failing_lines([](hegel::TestCase& tc) {
            auto trees = gs::recursive(hashed(), [](gs::Generator<int> sub) {
                return gs::vectors(sub, {.min_size = 2, .max_size = 2})
                    .map([](const std::vector<int>& v) {
                        return 100 + v[0] + v[1];
                    })
                    .print_as_call("node");
            });
            if (tc.draw(trees) >= 100) {
                boom();
            }
        }),
        Lines{"auto draw_1 = node(std::vector<int>{#0, #0});"});
}

TEST(PrintableGenerators, ACustomPrintedElementPrintsInsideAnArray) {
    EXPECT_EQ(failing_lines([](hegel::TestCase& tc) {
                  (void)tc.draw(gs::arrays<int, 2>(hashed()));
                  boom();
              }),
              Lines{"auto draw_1 = std::array<int, 2>{#0, #0};"});
}

TEST(PrintableGenerators, ACustomPrintedBranchPrintsInsideAVariant) {
    EXPECT_EQ(
        failing_lines([](hegel::TestCase& tc) {
            (void)tc.draw(gs::variant(hashed(), gs::booleans()));
            boom();
        }),
        Lines{"auto draw_1 = std::variant<int, bool>{std::in_place_index<0>, "
              "#0};"});
}

TEST(PrintableGenerators,
     DrawSilentAndDrawAndPrintAreUsageErrorsInExplicitCases) {
    auto run = [](const Body& body) {
        testing::internal::CaptureStderr();
        try {
            hegel::test(body, settings_at(hegel::Verbosity::Quiet, 1), {},
                        {hegel::ExplicitTestCase(1)});
        } catch (const std::invalid_argument& error) {
            testing::internal::GetCapturedStderr();
            return std::string(error.what());
        }
        testing::internal::GetCapturedStderr();
        return std::string("<no error>");
    };
    EXPECT_EQ(run([](hegel::TestCase& tc) { (void)tc.draw_silent(zero()); }),
              "draw_silent() is not supported in an explicit test case");
    EXPECT_EQ(run([](hegel::TestCase& tc) {
                  hegel::Printer printer = hegel::Printer::noop();
                  (void)tc.draw_and_print(zero(), printer);
              }),
              "draw_and_print() is not supported in an explicit test case");
}

TEST(PrintableGenerators, LabelsArePublic) {
    EXPECT_EQ(gs::label_from_name("x"), gs::label_from_name("x"));
    EXPECT_NE(gs::label_from_name("x"), gs::label_from_name("y"));
    EXPECT_NE(gs::combine_labels({1, 2}), gs::combine_labels({2, 1}));
}
