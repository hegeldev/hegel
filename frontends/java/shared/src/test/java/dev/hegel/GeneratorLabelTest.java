package dev.hegel;

import static dev.hegel.Generators.arrays;
import static dev.hegel.Generators.bigIntegers;
import static dev.hegel.Generators.binary;
import static dev.hegel.Generators.booleans;
import static dev.hegel.Generators.composite;
import static dev.hegel.Generators.domains;
import static dev.hegel.Generators.doubles;
import static dev.hegel.Generators.emails;
import static dev.hegel.Generators.floats;
import static dev.hegel.Generators.fromLark;
import static dev.hegel.Generators.fromRegex;
import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.ipAddresses;
import static dev.hegel.Generators.just;
import static dev.hegel.Generators.lists;
import static dev.hegel.Generators.longs;
import static dev.hegel.Generators.maps;
import static dev.hegel.Generators.oneOf;
import static dev.hegel.Generators.optional;
import static dev.hegel.Generators.permutations;
import static dev.hegel.Generators.records;
import static dev.hegel.Generators.recursive;
import static dev.hegel.Generators.sampledFrom;
import static dev.hegel.Generators.sets;
import static dev.hegel.Generators.text;
import static dev.hegel.Generators.tuples;
import static dev.hegel.Generators.urls;
import static dev.hegel.Generators.uuids;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

import java.util.List;
import org.junit.jupiter.api.Test;

/** Every standard generator labels itself as the other Hegel frontends do. */
class GeneratorLabelTest {
    private static final class Evens implements Generator<Integer> {
        @Override
        public Integer doDraw(TestCase tc) {
            return tc.drawSilent(integers()) & ~1;
        }
    }

    private static final class Odds implements Generator<Integer> {
        @Override
        public Integer doDraw(TestCase tc) {
            return tc.drawSilent(integers()) | 1;
        }
    }

    record Point(int x, int y) {}

    private static long of(String name) {
        return Label.of(name);
    }

    @Test
    void leafGeneratorsAreLabelledByKind() {
        assertEquals(of("hegel.integers"), integers().label());
        assertEquals(of("hegel.integers"), longs().label());
        assertEquals(of("hegel.integers"), bigIntegers().label());
        assertEquals(of("hegel.floats"), doubles().label());
        assertEquals(of("hegel.floats"), floats().label());
        assertEquals(of("hegel.booleans"), booleans().label());
        assertEquals(of("hegel.text"), text().label());
        assertEquals(of("hegel.binary"), binary().label());
        assertEquals(of("hegel.from_regex"), fromRegex("a").label());
        assertEquals(of("hegel.emails"), emails().label());
        assertEquals(of("hegel.urls"), urls().label());
        assertEquals(of("hegel.domains"), domains().label());
        assertEquals(of("hegel.ip_addresses"), ipAddresses().label());
        assertEquals(of("hegel.uuids"), uuids().label());
        assertEquals(of("hegel.just"), just(1).label());
        assertEquals(Label.SAMPLED_FROM, sampledFrom(1, 2).label());
        assertEquals(Label.PERMUTATION, permutations(1, 2).label());
        assertEquals(Label.COMPOSITE, composite(tc -> 1).label());
        assertEquals(Label.DEFERRED, Generators.<Integer>deferred().label());
        assertEquals(Label.FROM_LARK, fromLark("start: \"a\"").label());
        Generator<Integer> custom = tc -> 1;
        assertEquals(Label.GENERATOR, custom.label());
        assertEquals(Label.of(Evens.class.getName()), new Evens().label());
        assertNotEquals(new Evens().label(), new Odds().label());
    }

    @Test
    void compositeGeneratorsCombineTheirKindWithTheirComponents() {
        long ints = integers().label();
        long strings = text().label();
        Generator<List<Integer>> listOfInts = lists(integers());
        assertEquals(Label.combine(Label.LIST, ints), listOfInts.label());
        assertEquals(listOfInts.label(), listOfInts.label());
        assertNotEquals(listOfInts.label(), lists(text()).label());
        assertEquals(Label.combine(Label.SET, ints), sets(integers()).label());
        assertEquals(
                Label.combine(Label.MAP, ints, strings),
                maps(integers(), text()).label());
        assertEquals(
                Label.combine(Label.ARRAY, ints),
                arrays(integers(), 2, Integer[]::new).label());
        assertEquals(Label.combine(Label.OPTIONAL, ints), optional(integers()).label());
        assertEquals(Label.combine(Label.MAPPED, ints), integers().map(x -> x).label());
        assertEquals(
                Label.combine(Label.FLAT_MAP, ints),
                integers().flatMap(x -> just(x)).label());
        assertEquals(
                Label.combine(Label.FILTER, ints), integers().filter(x -> true).label());
        assertEquals(
                Label.combine(Label.TUPLE, ints, strings),
                tuples(integers(), text()).label());
        assertEquals(
                Label.combine(Label.ONE_OF, ints, ints),
                oneOf(integers(), integers()).label());
        assertEquals(
                Label.combine(Label.FIXED_DICT, of(Point.class.getName())),
                records(Point.class).label());
        assertEquals(
                Label.combine(Label.RECURSIVE, Label.combine(Label.MAPPED, ints)),
                recursive(integers().map(x -> (Object) x), sub -> lists(sub).map(xs -> (Object) xs))
                        .label());
    }

    @Test
    void poolDrawsAreLabelledByHowTheyDraw() {
        Hegel.test(
                tc -> {
                    Pool<Integer> pool = new Pool<>(tc);
                    assertEquals(Label.POOL_VALUES_REUSABLE, pool.reusable().label());
                    assertEquals(Label.POOL_VALUES_CONSUMED, pool.consuming().label());
                    assertEquals(
                            Label.combine(Label.FILTER, Label.POOL_VALUES_CONSUMED),
                            pool.consuming().filter(x -> true).label());
                    ConcurrentPool<Integer> concurrent = new ConcurrentPool<>(tc);
                    assertEquals(
                            Label.POOL_VALUES_REUSABLE, concurrent.reusable().label());
                    assertEquals(
                            Label.POOL_VALUES_CONSUMED, concurrent.consuming().label());
                    assertEquals(
                            Label.combine(Label.FILTER, Label.POOL_VALUES_CONSUMED),
                            concurrent.consuming().filter(x -> true).label());
                },
                new Settings().testCases(1).database(Database.disabled()));
    }
}
