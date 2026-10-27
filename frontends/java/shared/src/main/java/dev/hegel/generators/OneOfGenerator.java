package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.List;

/**
 * Chooses among alternative generators of the same type: an index is drawn and the selected
 * alternative is generated, so the shrinker can swap which branch is taken. It prints what the
 * chosen alternative prints.
 */
public final class OneOfGenerator<T> implements PrintableGenerator<T> {
    private final List<Generator<T>> options;
    private final CachedLabel label = new CachedLabel();

    public OneOfGenerator(List<Generator<T>> options) {
        if (options.isEmpty()) {
            throw new IllegalArgumentException("oneOf requires at least one generator");
        }
        this.options = List.copyOf(options);
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

    /** @hidden */
    @Override
    public T doDrawAndPrint(TestCase tc, Printer printer) {
        return draw(tc, printer);
    }

    private T draw(TestCase tc, Printer printer) {
        int index = (int) tc.generateInteger(0, options.size() - 1);
        return Components.draw(tc, options.get(index), printer);
    }
}
