package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.function.BiConsumer;

/**
 * Result of {@link Generator#printWith} and {@link Generator#printAsRepr()}: draws as its source
 * does, silently, and prints the drawn value with a callback.
 *
 * @param <T> the value type
 */
public final class PrintedGenerator<T> implements PrintableGenerator<T> {
    private final Generator<T> source;
    private final BiConsumer<? super T, Printer> print;

    public PrintedGenerator(Generator<T> source, BiConsumer<? super T, Printer> print) {
        this.source = source;
        this.print = print;
    }

    /** @hidden */
    @Override
    public long label() {
        return source.label();
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        return source.doDraw(tc);
    }

    /** @hidden */
    @Override
    public T doDrawAndPrint(TestCase tc, Printer printer) {
        T value = source.doDraw(tc);
        if (printer.shouldPrint()) {
            print.accept(value, printer);
        }
        return value;
    }
}
