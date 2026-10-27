package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.List;

/**
 * Chooses among alternative generators of the same type: an index is drawn and the selected
 * alternative is generated inside a ONE_OF span, so the shrinker can swap which branch is taken.
 * A {@code OneOfGenerator} draws silently; {@link Printable}, what {@link
 * dev.hegel.Generators#oneOf} returns for printable alternatives, prints what the chosen alternative
 * prints.
 *
 * @param <T> the value type
 */
public class OneOfGenerator<T> implements Generator<T> {
    private final List<Generator<T>> options;
    private final CachedLabel label = new CachedLabel();

    /** @param options the alternatives */
    public OneOfGenerator(List<? extends Generator<T>> options) {
        if (options.isEmpty()) {
            throw new IllegalArgumentException("oneOf requires at least one generator");
        }
        this.options = List.<Generator<T>>copyOf(options);
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> CachedLabel.combine(Label.ONE_OF, options));
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        return draw(tc, null);
    }

    final T draw(TestCase tc, Printer printer) {
        tc.startSpan(label());
        try {
            int index = (int) tc.generateInteger(0, options.size() - 1);
            return Components.draw(tc, options.get(index), printer);
        } finally {
            tc.stopSpan(false);
        }
    }

    /**
     * A choice among printable alternatives: it prints what the chosen alternative prints.
     *
     * @param <T> the value type
     */
    public static final class Printable<T> extends OneOfGenerator<T> implements PrintableGenerator<T> {
        /** @param options the printable alternatives */
        public Printable(List<? extends PrintableGenerator<T>> options) {
            super(options);
        }

        /** @hidden */
        @Override
        public T doDrawAndPrint(TestCase tc, Printer printer) {
            return draw(tc, printer);
        }
    }
}
