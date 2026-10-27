package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.function.Function;

/**
 * Result of {@link Generator#flatMap}. Always uses the composite draw path: it draws a value
 * silently, asks {@code f} for the next generator, and draws from it, bracketing the pair in a
 * {@code flat_map} span. It prints what the second generator prints.
 *
 * @param <T> the source value type
 * @param <U> the dependent value type
 */
public final class FlatMappedGenerator<T, U> implements PrintableGenerator<U> {
    private final Generator<T> source;
    private final Function<? super T, ? extends Generator<U>> f;
    private final CachedLabel label = new CachedLabel();

    public FlatMappedGenerator(Generator<T> source, Function<? super T, ? extends Generator<U>> f) {
        this.source = source;
        this.f = f;
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.FLAT_MAP, source.label()));
    }

    /** @hidden */
    @Override
    public U doDraw(TestCase tc) {
        return draw(tc, null);
    }

    /** @hidden */
    @Override
    public U doDrawAndPrint(TestCase tc, Printer printer) {
        return draw(tc, printer);
    }

    private U draw(TestCase tc, Printer printer) {
        tc.startSpan(label());
        try {
            T value = tc.drawSilent(source);
            return Components.draw(tc, f.apply(value), printer);
        } finally {
            tc.stopSpan(false);
        }
    }
}
