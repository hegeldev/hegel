package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.LinkedHashSet;
import java.util.Set;

/**
 * Generates sets of distinct elements by driving the engine's collection API and rejecting
 * duplicates, so the engine keeps producing until the set reaches its size.
 *
 * <p>The size range defaults to any size; narrow it with the fluent {@link #minSize(int)} / {@link
 * #maxSize(int)} methods.
 *
 * <p>A {@code SetGenerator} draws silently; {@link Printable}, what {@link
 * dev.hegel.Generators#sets} returns for a printable element generator, prints as {@code [a, b]},
 * each element printed by the element generator as it is drawn; a rejected duplicate prints
 * nothing.
 *
 * @param <T> the element type
 */
public class SetGenerator<T> implements Generator<Set<T>> {
    private final Generator<T> element;
    private final long minSize;
    private final long maxSize;
    private final CachedLabel label = new CachedLabel();

    /**
     * @param element the element generator
     * @param minSize the minimum size (inclusive)
     * @param maxSize the maximum size (inclusive), or {@link dev.hegel.lowlevel.Abi#UNBOUNDED}
     */
    public SetGenerator(Generator<T> element, long minSize, long maxSize) {
        Sizes.validate(minSize, maxSize, "sets");
        this.element = element;
        this.minSize = minSize;
        this.maxSize = maxSize;
    }

    /**
     * @param minSize the minimum size (inclusive)
     * @return a copy with the minimum size set
     */
    public SetGenerator<T> minSize(int minSize) {
        return new SetGenerator<>(element, minSize, maxSize);
    }

    /**
     * @param maxSize the maximum size (inclusive)
     * @return a copy with the maximum size set
     */
    public SetGenerator<T> maxSize(int maxSize) {
        return new SetGenerator<>(element, minSize, maxSize);
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.SET, element.label()));
    }

    /** @hidden */
    @Override
    public Set<T> doDraw(TestCase tc) {
        return draw(tc, null);
    }

    final Set<T> draw(TestCase tc, Printer printer) {
        Printer p = Components.out(printer);
        tc.startSpan(label());
        try {
            long id = tc.newCollection(minSize, maxSize);
            try {
                Set<T> out = new LinkedHashSet<>();
                p.beginGroup("[");
                while (tc.collectionMore(id)) {
                    try (Printer.Speculation speculation = p.speculate()) {
                        if (!out.isEmpty()) {
                            p.separator();
                        }
                        T value = Components.draw(tc, element, printer);
                        if (out.add(value)) {
                            speculation.commit();
                        } else {
                            speculation.abort();
                            tc.collectionReject(id, "duplicate element");
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
     * A set generator over a printable element generator: it prints as {@code [a, b]}, each
     * element printed by the element generator as it is drawn, and a rejected duplicate prints
     * nothing. Its builder methods keep it printable.
     *
     * @param <T> the element type
     */
    public static final class Printable<T> extends SetGenerator<T> implements PrintableGenerator<Set<T>> {
        private final PrintableGenerator<T> element;

        /**
         * @param element the printable element generator
         * @param minSize the minimum size (inclusive)
         * @param maxSize the maximum size (inclusive), or {@link dev.hegel.lowlevel.Abi#UNBOUNDED}
         */
        public Printable(PrintableGenerator<T> element, long minSize, long maxSize) {
            super(element, minSize, maxSize);
            this.element = element;
        }

        @Override
        public Printable<T> minSize(int minSize) {
            return new Printable<>(element, minSize, super.maxSize);
        }

        @Override
        public Printable<T> maxSize(int maxSize) {
            return new Printable<>(element, super.minSize, maxSize);
        }

        /** @hidden */
        @Override
        public Set<T> doDrawAndPrint(TestCase tc, Printer printer) {
            return draw(tc, printer);
        }
    }
}
