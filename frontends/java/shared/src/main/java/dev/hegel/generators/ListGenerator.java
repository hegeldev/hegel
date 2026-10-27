package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.function.Function;

/**
 * Generates lists by driving the engine's collection API element by element: the engine decides how
 * many elements to produce and the shrinker can delete or reorder them. The element generator opens
 * its own spans, so no per-element span is needed here (matching the engine's canonical client).
 *
 * <p>The length range defaults to any size; narrow it with the fluent {@link #minSize(int)} /
 * {@link #maxSize(int)} methods. {@link #unique()} and {@link #uniqueBy(Function)} make the
 * elements distinct: a drawn duplicate is rejected and the engine draws another in its place.
 *
 * <p>A {@code ListGenerator} draws silently; {@link Printable}, what {@link
 * dev.hegel.Generators#lists} returns for a printable element generator, prints as {@code [a, b]},
 * each element printed by the element generator as it is drawn; a rejected duplicate prints
 * nothing.
 *
 * @param <T> the element type
 */
public class ListGenerator<T> implements Generator<List<T>> {
    private final Generator<T> element;
    private final long minSize;
    private final long maxSize;
    private final Function<? super T, ?> uniqueKey;
    private final CachedLabel label = new CachedLabel();

    /**
     * @param element the element generator
     * @param minSize the minimum length (inclusive)
     * @param maxSize the maximum length (inclusive), or {@link dev.hegel.lowlevel.Abi#UNBOUNDED}
     */
    public ListGenerator(Generator<T> element, long minSize, long maxSize) {
        this(element, minSize, maxSize, null);
    }

    private ListGenerator(Generator<T> element, long minSize, long maxSize, Function<? super T, ?> uniqueKey) {
        Sizes.validate(minSize, maxSize, "lists");
        this.element = element;
        this.minSize = minSize;
        this.maxSize = maxSize;
        this.uniqueKey = uniqueKey;
    }

    /**
     * @param minSize the minimum length (inclusive)
     * @return a copy with the minimum size set
     */
    public ListGenerator<T> minSize(int minSize) {
        return new ListGenerator<>(element, minSize, maxSize, uniqueKey);
    }

    /**
     * @param maxSize the maximum length (inclusive)
     * @return a copy with the maximum size set
     */
    public ListGenerator<T> maxSize(int maxSize) {
        return new ListGenerator<>(element, minSize, maxSize, uniqueKey);
    }

    /**
     * Make the elements distinct by {@link Object#equals}, comparing arrays (nested ones included)
     * by their contents.
     *
     * @return a copy generating lists of distinct elements
     */
    public ListGenerator<T> unique() {
        return uniqueBy(Function.identity());
    }

    /**
     * Make the elements distinct by a key: no two elements of a list have equal keys. Keys are
     * compared by {@link Object#equals}, and arrays (nested ones included) by their contents.
     *
     * @param key extracts the key elements are compared by
     * @return a copy generating lists of elements with distinct keys
     */
    public ListGenerator<T> uniqueBy(Function<? super T, ?> key) {
        return new ListGenerator<>(element, minSize, maxSize, key);
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.LIST, element.label()));
    }

    /** @hidden */
    @Override
    public List<T> doDraw(TestCase tc) {
        return draw(tc, null);
    }

    final List<T> draw(TestCase tc, Printer printer) {
        Printer p = Components.out(printer);
        tc.startSpan(label());
        try {
            long id = tc.newCollection(minSize, maxSize);
            try {
                List<T> out = new ArrayList<>();
                Set<Object> keys = new HashSet<>();
                p.beginGroup("[");
                while (tc.collectionMore(id)) {
                    try (Printer.Speculation speculation = p.speculate()) {
                        if (!out.isEmpty()) {
                            p.separator();
                        }
                        T value = Components.draw(tc, element, printer);
                        if (uniqueKey != null && !keys.add(new ContentKey(uniqueKey.apply(value)))) {
                            speculation.abort();
                            tc.collectionReject(id, "duplicate element");
                        } else {
                            speculation.commit();
                            out.add(value);
                        }
                    }
                }
                p.endGroup("]");
                return out;
            } finally {
                tc.collectionFree(id);
            }
        } finally {
            tc.stopSpan(false);
        }
    }

    /**
     * A list generator over a printable element generator: it prints as {@code [a, b]}, each
     * element printed by the element generator as it is drawn, and a rejected duplicate prints
     * nothing. Its builder methods keep it printable.
     *
     * @param <T> the element type
     */
    public static final class Printable<T> extends ListGenerator<T> implements PrintableGenerator<List<T>> {
        private final PrintableGenerator<T> element;

        /**
         * @param element the printable element generator
         * @param minSize the minimum length (inclusive)
         * @param maxSize the maximum length (inclusive), or {@link dev.hegel.lowlevel.Abi#UNBOUNDED}
         */
        public Printable(PrintableGenerator<T> element, long minSize, long maxSize) {
            this(element, minSize, maxSize, null);
        }

        private Printable(PrintableGenerator<T> element, long minSize, long maxSize, Function<? super T, ?> uniqueKey) {
            super(element, minSize, maxSize, uniqueKey);
            this.element = element;
        }

        @Override
        public Printable<T> minSize(int minSize) {
            return new Printable<>(element, minSize, super.maxSize, super.uniqueKey);
        }

        @Override
        public Printable<T> maxSize(int maxSize) {
            return new Printable<>(element, super.minSize, maxSize, super.uniqueKey);
        }

        @Override
        public Printable<T> unique() {
            return uniqueBy(Function.identity());
        }

        @Override
        public Printable<T> uniqueBy(Function<? super T, ?> key) {
            return new Printable<>(element, super.minSize, super.maxSize, key);
        }

        /** @hidden */
        @Override
        public List<T> doDrawAndPrint(TestCase tc, Printer printer) {
            return draw(tc, printer);
        }
    }

    private record ContentKey(Object key) {
        @Override
        public boolean equals(Object other) {
            return Objects.deepEquals(key, ((ContentKey) other).key);
        }

        @Override
        public int hashCode() {
            return Arrays.deepHashCode(new Object[] {key});
        }
    }
}
