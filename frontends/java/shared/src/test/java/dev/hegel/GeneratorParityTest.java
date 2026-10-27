package dev.hegel;

import static dev.hegel.Generators.arrays;
import static dev.hegel.Generators.bigIntegers;
import static dev.hegel.Generators.binary;
import static dev.hegel.Generators.characters;
import static dev.hegel.Generators.fromRegex;
import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.lists;
import static dev.hegel.Generators.nulls;
import static dev.hegel.Generators.optional;
import static dev.hegel.Generators.optionalDoubles;
import static dev.hegel.Generators.optionalInts;
import static dev.hegel.Generators.optionalLongs;
import static dev.hegel.Generators.permutations;
import static dev.hegel.Generators.samples;
import static dev.hegel.Generators.subsequences;
import static dev.hegel.Generators.text;
import static dev.hegel.Generators.weightedBooleans;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.generators.TextGenerator;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import java.util.function.UnaryOperator;
import org.junit.jupiter.api.Test;

/** The generators every Hegel frontend offers: big integers, unique lists, permutations, ... */
class GeneratorParityTest {
    private static final Settings SETTINGS = new Settings().database(Database.disabled());
    private static final BigInteger LONG_MAX = BigInteger.valueOf(Long.MAX_VALUE);
    private static final BigInteger TWO_127 = BigInteger.ONE.shiftLeft(127);

    @Test
    void bigIntegersReachBeyondLong() {
        RunReport report = Hegel.run(
                tc -> assertTrue(tc.draw(bigIntegers(), "x").compareTo(LONG_MAX) <= 0),
                SETTINGS.testCases(1000),
                Reporter.silent());
        assertEquals(
                LONG_MAX.add(BigInteger.ONE), report.failures().get(0).draws().get("x"));
    }

    @Test
    void bigIntegersReachBelowLong() {
        BigInteger longMin = BigInteger.valueOf(Long.MIN_VALUE);
        RunReport report = Hegel.run(
                tc -> assertTrue(tc.draw(bigIntegers(), "x").compareTo(longMin) >= 0),
                SETTINGS.testCases(1000),
                Reporter.silent());
        assertEquals(
                longMin.subtract(BigInteger.ONE),
                report.failures().get(0).draws().get("x"));
    }

    @Test
    void bigIntegersStayInTheirBounds() {
        BigInteger lo = BigInteger.ONE.shiftLeft(200).negate();
        BigInteger hi = BigInteger.ONE.shiftLeft(199).negate();
        Hegel.test(
                tc -> {
                    BigInteger x = tc.draw(bigIntegers().min(lo).max(hi));
                    assertTrue(x.compareTo(lo) >= 0 && x.compareTo(hi) <= 0, x::toString);
                    BigInteger small =
                            tc.draw(bigIntegers().min(BigInteger.valueOf(-5)).max(BigInteger.TEN));
                    assertTrue(small.compareTo(BigInteger.valueOf(-5)) >= 0 && small.compareTo(BigInteger.TEN) <= 0);
                },
                SETTINGS);
    }

    @Test
    void anUnsetBigIntegerBoundReachesBeyondTheOther() {
        BigInteger big = TWO_127.multiply(BigInteger.valueOf(4));
        Hegel.test(
                tc -> {
                    assertTrue(tc.draw(bigIntegers().min(big)).compareTo(big) >= 0);
                    assertTrue(tc.draw(bigIntegers().max(big.negate())).compareTo(big.negate()) <= 0);
                    assertTrue(tc.draw(bigIntegers().min(BigInteger.ONE)).signum() > 0);
                    assertTrue(
                            tc.draw(bigIntegers().max(BigInteger.ONE.negate())).signum() < 0);
                },
                SETTINGS);
    }

    @Test
    void bigIntegerBoundsAreValidated() {
        assertThrows(
                IllegalArgumentException.class,
                () -> bigIntegers().min(BigInteger.TEN).max(BigInteger.ONE));
    }

    @Test
    void uniqueListsHaveDistinctElements() {
        Hegel.test(
                tc -> {
                    List<Integer> xs = tc.draw(lists(integers().min(0).max(5)).unique());
                    assertEquals(xs.size(), new HashSet<>(xs).size());
                    List<String> ys = tc.draw(lists(text().maxSize(3)).uniqueBy(String::length));
                    Set<Integer> lengths = new HashSet<>();
                    for (String y : ys) {
                        assertTrue(lengths.add(y.length()), ys::toString);
                    }
                },
                SETTINGS);
    }

    @Test
    void uniqueListsCompareArraysByContent() {
        Hegel.test(
                tc -> {
                    List<byte[]> xs =
                            tc.draw(lists(binary().maxSize(1)).unique().maxSize(5));
                    Set<List<Byte>> contents = new HashSet<>();
                    for (byte[] x : xs) {
                        List<Byte> content = new ArrayList<>();
                        for (byte b : x) {
                            content.add(b);
                        }
                        assertTrue(contents.add(content));
                    }
                    List<Integer[][]> nested = tc.draw(lists(
                                    arrays(arrays(integers().min(0).max(1), 1, Integer[]::new), 1, Integer[][]::new))
                            .uniqueBy(a -> a)
                            .maxSize(3));
                    Set<Integer> firsts = new HashSet<>();
                    for (Integer[][] a : nested) {
                        assertTrue(firsts.add(a[0][0]));
                    }
                },
                SETTINGS);
    }

    @Test
    void uniqueListsOfArraysAreOnlyAsLongAsTheirDistinctContentsAllow() {
        RunReport report = Hegel.run(
                tc -> assertTrue(tc.draw(lists(binary().maxSize(0)).unique()).size() <= 1),
                SETTINGS,
                Reporter.silent());
        assertTrue(report.passed());
    }

    @Test
    void uniqueListsCanStillBeLong() {
        RunReport report = Hegel.run(
                tc -> assertTrue(
                        tc.draw(lists(integers().min(0).max(100)).unique()).size() < 5),
                SETTINGS,
                Reporter.silent());
        assertEquals(1, report.failures().size());
    }

    @Test
    void permutationsReorderEveryElement() {
        Set<List<Integer>> seen = new HashSet<>();
        Hegel.test(
                tc -> {
                    List<Integer> perm = tc.draw(permutations(1, 2, 3, 4));
                    assertEquals(Set.of(1, 2, 3, 4), new HashSet<>(perm));
                    assertEquals(4, perm.size());
                    seen.add(perm);
                    assertEquals(List.of(), tc.draw(permutations(List.of())));
                },
                SETTINGS);
        assertTrue(seen.size() > 1);
    }

    @Test
    void permutationsShrinkTowardTheOriginalOrder() {
        RunReport report = Hegel.run(
                tc -> {
                    List<Integer> perm = tc.draw(permutations(1, 2, 3, 4, 5), "p");
                    assertTrue(perm.get(4) != 1);
                },
                SETTINGS,
                Reporter.silent());
        assertEquals(List.of(2, 3, 4, 5, 1), report.failures().get(0).draws().get("p"));
    }

    @Test
    void aPermutationEndsWithItsCollection() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false);
        assertEquals(List.of(), tc.draw(permutations(1, 2)));
        assertEquals(List.of(7L), fake.freedCollections);
        assertTrue(fake.startedSpans.contains(Label.PERMUTATION));
    }

    @Test
    void arraysHaveTheirLength() {
        Hegel.test(
                tc -> {
                    Integer[] xs = tc.draw(arrays(integers(), 3, Integer[]::new));
                    assertEquals(3, xs.length);
                    assertEquals(0, tc.draw(arrays(integers(), 0, Integer[]::new)).length);
                },
                SETTINGS);
        assertThrows(IllegalArgumentException.class, () -> arrays(integers(), -1, Integer[]::new));
    }

    @Test
    void optionalsAreEmptyOrPresent() {
        boolean[] seen = new boolean[2];
        Hegel.test(
                tc -> {
                    Optional<Integer> x = tc.draw(optional(integers()));
                    seen[x.isPresent() ? 1 : 0] = true;
                    tc.draw(optionalInts(integers()));
                    tc.draw(optionalLongs(Generators.longs()));
                    tc.draw(optionalDoubles(Generators.doubles()));
                },
                SETTINGS);
        assertTrue(seen[0] && seen[1]);
    }

    @Test
    void optionalsOpenAnOptionalSpan() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false);
        assertEquals(Optional.empty(), tc.draw(optional(integers())));
        assertEquals(List.of(Label.combine(Label.OPTIONAL, integers().label())), fake.startedSpans);
    }

    @Test
    void nullsAreNull() {
        Hegel.test(tc -> assertNull(tc.draw(nulls())), SETTINGS.testCases(5));
    }

    @Test
    void textFromAnAlphabet() {
        Hegel.test(
                tc -> {
                    String s = tc.draw(text().alphabet("abé"));
                    assertTrue(s.chars().allMatch(c -> "abé".indexOf(c) >= 0), s);
                },
                SETTINGS);
    }

    @Test
    void textInACodec() {
        Hegel.test(
                tc -> {
                    assertTrue(tc.draw(text().codec("ascii")).chars().allMatch(c -> c < 0x80));
                    assertTrue(tc.draw(text().codec("latin-1")).chars().allMatch(c -> c < 0x100));
                    assertTrue(tc.draw(text().codec("iso-8859-1").codepoints(0x41, 0x5a))
                            .chars()
                            .allMatch(c -> c >= 0x41 && c <= 0x5a));
                    tc.draw(text().codec("utf-8"));
                },
                SETTINGS);
        TextGenerator ebcdic = text().codec("ebcdic");
        IllegalArgumentException invalid = assertThrows(
                IllegalArgumentException.class, () -> Hegel.test(tc -> tc.draw(ebcdic), SETTINGS.testCases(1)));
        assertTrue(invalid.getMessage().contains("ebcdic"), invalid.getMessage());
    }

    @Test
    void anAlphabetExcludesOtherRestrictions() {
        List<UnaryOperator<TextGenerator>> restrictions = new ArrayList<>(List.of(
                t -> t.codec("ascii"),
                t -> t.codepoints(0, 100),
                t -> t.categories("Lu"),
                t -> t.excludeCategories("Lu"),
                t -> t.includeCharacters("x"),
                t -> t.excludeCharacters("x")));
        for (UnaryOperator<TextGenerator> restriction : restrictions) {
            assertThrows(IllegalArgumentException.class, () -> restriction.apply(text().alphabet("ab")));
            assertThrows(
                    IllegalArgumentException.class,
                    () -> restriction.apply(text()).alphabet("ab"));
        }
        TextGenerator onlyMax = new TextGenerator(0, 5, null, 100, null, null, null, null);
        assertThrows(IllegalArgumentException.class, () -> onlyMax.alphabet("ab"));
    }

    @Test
    void regexesDrawFromTheirAlphabet() {
        Hegel.test(
                tc -> {
                    String s = tc.draw(fromRegex("[a-z]+").alphabet(characters().alphabet("xyz")));
                    assertTrue(s.chars().allMatch(c -> "xyz".indexOf(c) >= 0), s);
                    String padded = tc.draw(fromRegex("q")
                            .fullmatch(false)
                            .alphabet(characters().codec("ascii")));
                    assertTrue(padded.contains("q") && padded.chars().allMatch(c -> c < 0x80), padded);
                },
                SETTINGS);
    }

    @Test
    void bigIntegersAreDerived() {
        Hegel.test(tc -> assertInstanceOf(BigInteger.class, tc.draw(Generators.forType(BigInteger.class))), SETTINGS);
    }

    @Test
    void weightedBooleansFollowTheirProbability() {
        int trues = 0;
        int cases = 0;
        List<Integer> seen = new ArrayList<>();
        for (int i = 0; i < 1; i++) {
            Hegel.test(
                    tc -> {
                        assertEquals(false, tc.draw(weightedBooleans(0.0)));
                        assertEquals(true, tc.draw(weightedBooleans(1.0)));
                        seen.add(tc.draw(weightedBooleans(0.9)) ? 1 : 0);
                    },
                    SETTINGS.testCases(100));
        }
        for (int b : seen) {
            trues += b;
            cases++;
        }
        assertTrue(trues > cases / 2, trues + " of " + cases);
    }

    @Test
    void subsequencesKeepTheOriginalOrder() {
        List<String> elements = List.of("a", "b", "c", "d", "e");
        Hegel.test(
                tc -> {
                    List<String> sub = tc.draw(subsequences(elements));
                    assertTrue(sub.size() <= elements.size());
                    int position = -1;
                    for (String item : sub) {
                        int index = elements.indexOf(item);
                        assertTrue(index > position);
                        position = index;
                    }
                    assertTrue(tc.draw(subsequences(elements).minSize(2)).size() >= 2);
                    assertTrue(tc.draw(subsequences(elements).maxSize(1)).size() <= 1);
                    assertEquals(
                            elements, tc.draw(subsequences(elements).minSize(5).maxSize(10)));
                    assertEquals(List.of(), tc.draw(subsequences(List.of())));
                    assertTrue(tc.draw(subsequences(1, 1, 2)).stream()
                                    .filter(x -> x == 1)
                                    .count()
                            <= 2);
                },
                SETTINGS);
    }

    @Test
    void subsequencesShrinkTowardTheEarliestElements() {
        RunReport report = Hegel.run(
                tc -> {
                    List<Integer> sub = tc.draw(subsequences(1, 2, 3, 4, 5), "s");
                    assertTrue(sub.size() < 2);
                },
                SETTINGS,
                Reporter.silent());
        assertEquals(List.of(1, 2), report.failures().get(0).draws().get("s"));
    }

    @Test
    void samplesWithReplacementRepeatElements() {
        Set<List<Integer>> seen = new HashSet<>();
        Hegel.test(
                tc -> {
                    List<Integer> sample = tc.draw(samples(1, 2, 3).minSize(4).maxSize(6));
                    assertTrue(sample.size() >= 4 && sample.size() <= 6);
                    assertTrue(Set.of(1, 2, 3).containsAll(sample));
                    seen.add(sample);
                    assertEquals(List.of(), tc.draw(samples(List.of())));
                    assertEquals(List.of(), tc.draw(samples(List.of()).withReplacement()));
                },
                SETTINGS);
        assertTrue(seen.stream().anyMatch(s -> new HashSet<>(s).size() < s.size()));
    }

    @Test
    void samplesWithoutReplacementAreReorderedSubsets() {
        List<String> elements = List.of("a", "b", "c", "d");
        Hegel.test(
                tc -> {
                    List<String> sample = tc.draw(samples(elements).withoutReplacement());
                    assertTrue(sample.size() <= elements.size());
                    assertEquals(sample.size(), new HashSet<>(sample).size());
                    assertTrue(elements.containsAll(sample));
                    assertEquals(
                            4,
                            tc.draw(samples(elements)
                                            .withoutReplacement()
                                            .minSize(4)
                                            .maxSize(10))
                                    .size());
                    assertTrue(tc.draw(samples(elements).withoutReplacement().maxSize(2))
                                    .size()
                            <= 2);
                },
                SETTINGS);
    }

    @Test
    void samplesShrinkTowardTheEarliestElements() {
        RunReport with = Hegel.run(
                tc -> {
                    List<Integer> sample = tc.draw(samples(5, 6, 7), "s");
                    assertTrue(sample.size() < 2);
                },
                SETTINGS,
                Reporter.silent());
        assertEquals(List.of(5, 5), with.failures().get(0).draws().get("s"));
        RunReport without = Hegel.run(
                tc -> {
                    List<Integer> sample = tc.draw(samples(5, 6, 7).withoutReplacement(), "s");
                    assertTrue(sample.size() < 2);
                },
                SETTINGS,
                Reporter.silent());
        assertEquals(List.of(5, 6), without.failures().get(0).draws().get("s"));
    }

    @Test
    void subsequencesAndSamplesEndWithTheirCollections() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false);
        assertEquals(List.of(), tc.draw(subsequences(1, 2)));
        assertEquals(List.of(), tc.draw(samples(1, 2)));
        assertEquals(List.of(), tc.draw(samples(1, 2).withoutReplacement()));
        assertEquals(List.of(7L, 7L, 7L), fake.freedCollections);
        assertTrue(fake.startedSpans.contains(Label.SUBSEQUENCE));
        assertTrue(fake.startedSpans.contains(Label.SAMPLE));
    }
}
