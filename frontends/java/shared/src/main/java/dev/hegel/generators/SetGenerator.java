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
 * <p>It prints as {@code [a, b]}, each element printed by the element generator as it is drawn;
 * a rejected duplicate prints nothing.
 */
public final class SetGenerator<T> implements PrintableGenerator<Set<T>> {
    private final Generator<T> element;
    private final long minSize;
    private final long maxSize;
    private final CachedLabel label = new CachedLabel();

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

    /** @hidden */
    @Override
    public Set<T> doDrawAndPrint(TestCase tc, Printer printer) {
        return draw(tc, printer);
    }

    private Set<T> draw(TestCase tc, Printer printer) {
        Printer p = Components.out(printer);
        tc.startSpan(label());
        try {
            long id = tc.newCollection(minSize, maxSize);
            try {
                Set<T> out = new LinkedHashSet<>();
                p.beginGroup("Set.of(");
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
                p.endGroup(")");
                return out;
            } finally {
                tc.collectionFree(id);
            }
        } finally {
            tc.stopSpan(false);
        }
    }
}
