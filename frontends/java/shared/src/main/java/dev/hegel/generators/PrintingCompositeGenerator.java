package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.function.BiFunction;

/**
 * A generator built from imperative code that draws from other generators and prints what it
 * builds, backing {@link dev.hegel.Generators#composite(BiFunction)}. It brackets the body in a
 * single {@code composite} span.
 *
 * @param <T> the value type
 */
public final class PrintingCompositeGenerator<T> implements PrintableGenerator<T> {
    private final BiFunction<TestCase, Printer, T> body;

    public PrintingCompositeGenerator(BiFunction<TestCase, Printer, T> body) {
        this.body = body;
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.COMPOSITE;
    }

    /** @hidden */
    @Override
    public T doDrawAndPrint(TestCase tc, Printer printer) {
        tc.startSpan(Label.COMPOSITE);
        try {
            return body.apply(tc, printer);
        } finally {
            tc.stopSpan(false);
        }
    }
}
