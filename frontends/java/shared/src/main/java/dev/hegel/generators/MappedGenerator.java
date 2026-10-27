package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.function.Function;

/**
 * Result of {@link Generator#map}. Draws from the source and applies {@code f}, bracketing the pair
 * in a {@code map} span so the shrinker treats them as one unit. It prints the mapped value with
 * {@link Printer#repr}, so the source need not be printable. The map of a printable generator is a
 * {@link Printable}, whose {@link Printable#printAsCall} prints the call that made the value
 * instead.
 *
 * @param <T> the source value type
 * @param <U> the mapped value type
 */
public class MappedGenerator<T, U> extends ValueGenerator<U> {
    private final Generator<T> source;
    private final Function<? super T, ? extends U> f;
    private final CachedLabel label = new CachedLabel();

    /**
     * @param source the generator to map
     * @param f the mapping function
     */
    public MappedGenerator(Generator<T> source, Function<? super T, ? extends U> f) {
        this.source = source;
        this.f = f;
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.MAPPED, source.label()));
    }

    /** @hidden */
    @Override
    public U doDraw(TestCase tc) {
        tc.startSpan(label());
        try {
            return f.apply(tc.drawSilent(source));
        } finally {
            tc.stopSpan(false);
        }
    }

    /**
     * The map of a printable generator. It prints the mapped value with {@link Printer#repr}, as
     * any map does, and {@link #printAsCall} prints the call that made the value instead.
     *
     * @param <T> the source value type
     * @param <U> the mapped value type
     */
    public static final class Printable<T, U> extends MappedGenerator<T, U> {
        private final PrintableGenerator<T> source;
        private final Function<? super T, ? extends U> f;

        /**
         * @param source the printable generator to map
         * @param f the mapping function
         */
        public Printable(PrintableGenerator<T> source, Function<? super T, ? extends U> f) {
            super(source, f);
            this.source = source;
            this.f = f;
        }

        /**
         * Print the call that made each value rather than the value: {@code function(...)} around
         * the source's own printing of its value. For a type that prints badly, or whose
         * representation is not how it is built:
         *
         * <pre>{@code
         * tc.draw(integers().min(0).map(Key::of).printAsCall("Key.of"), "key")   // var key = Key.of(3);
         * }</pre>
         *
         * @param function the name of the function, as the report should show it
         * @return a printable generator of the same values
         * @throws IllegalArgumentException if {@code function} is empty
         */
        public PrintableGenerator<U> printAsCall(String function) {
            if (function.isEmpty()) {
                throw new IllegalArgumentException("printAsCall requires a non-empty function name");
            }
            String open = function + "(";
            return new PrintableGenerator<U>() {
                @Override
                public long label() {
                    return Printable.this.label();
                }

                @Override
                public U doDraw(TestCase tc) {
                    return Printable.this.doDraw(tc);
                }

                @Override
                public U doDrawAndPrint(TestCase tc, Printer printer) {
                    tc.startSpan(label());
                    try {
                        printer.beginGroup(open);
                        T input = tc.drawAndPrint(source, printer);
                        printer.endGroup(")");
                        return f.apply(input);
                    } finally {
                        tc.stopSpan(false);
                    }
                }
            };
        }
    }
}
