package dev.hegel.generators;

import dev.hegel.PrettyPrinter;
import dev.hegel.PrintableGenerator;
import dev.hegel.TestCase;

/**
 * A generator that prints the value it has drawn, with {@link PrettyPrinter#repr}: every leaf generator,
 * and the value-producing ones.
 *
 * @param <T> the value type
 */
abstract class ValueGenerator<T> implements PrintableGenerator<T> {
    @Override
    public abstract T doDraw(TestCase tc);

    /** @hidden */
    @Override
    public final T doDrawAndPrint(TestCase tc, PrettyPrinter printer) {
        T value = doDraw(tc);
        printer.repr(value);
        return value;
    }
}
