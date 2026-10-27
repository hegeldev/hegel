package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.function.Function;

/**
 * Generates an optional value: empty, or wrapping a value from an inner generator, inside a {@link
 * Label#OPTIONAL} span. Shrinks toward empty. It prints as {@code Optional[x]}, the inner value
 * printed by the inner generator as it is drawn, or {@code Optional.empty} (with the name of the
 * optional type it makes, such as {@code OptionalInt}).
 *
 * @param <T> the inner value type
 * @param <R> the optional type, such as {@code Optional<T>} or {@code OptionalInt}
 */
public final class OptionalGenerator<T, R> implements PrintableGenerator<R> {
    private final String name;
    private final Generator<T> inner;
    private final Function<? super T, ? extends R> present;
    private final R empty;
    private final CachedLabel label = new CachedLabel();

    /**
     * @param name the simple name of the optional type, such as {@code Optional}
     * @param inner the generator of present values
     * @param present wraps a present value
     * @param empty the empty value
     */
    public OptionalGenerator(String name, Generator<T> inner, Function<? super T, ? extends R> present, R empty) {
        this.name = name;
        this.inner = inner;
        this.present = present;
        this.empty = empty;
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.OPTIONAL, inner.label()));
    }

    /** @hidden */
    @Override
    public R doDraw(TestCase tc) {
        return draw(tc, null);
    }

    /** @hidden */
    @Override
    public R doDrawAndPrint(TestCase tc, Printer printer) {
        return draw(tc, printer);
    }

    private R draw(TestCase tc, Printer printer) {
        Printer p = Components.out(printer);
        tc.startSpan(label());
        try {
            if (tc.generateBoolean(0.5)) {
                p.beginGroup(name + "[");
                R value = present.apply(Components.draw(tc, inner, printer));
                p.endGroup("]");
                return value;
            }
            p.text(name + ".empty");
            return empty;
        } finally {
            tc.stopSpan(false);
        }
    }
}
