package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.TestCase;
import java.util.function.Function;

/**
 * A generator built from imperative code that draws from other generators, backing {@link
 * dev.hegel.Generators#composite(Function)}. It brackets the body in a single {@code composite}
 * span, and prints the value the body returns with {@link dev.hegel.Printer#repr}: the body's own
 * draws are not printed.
 *
 * @param <T> the value type
 */
public final class CompositeGenerator<T> extends ValueGenerator<T> {
    private final Function<TestCase, T> body;

    public CompositeGenerator(Function<TestCase, T> body) {
        this.body = body;
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.COMPOSITE;
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        return body.apply(tc);
    }
}
