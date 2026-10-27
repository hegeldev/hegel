package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;

/**
 * Generates fixed-length heterogeneous tuples. The element values are drawn in order inside a
 * TUPLE span and handed to an {@code assembler} that packs them into the user-facing type {@code
 * T} (a {@code TupleN} record, or the raw {@code List<Object>} for the variadic factory).
 *
 * <p>A {@code TupleGenerator} draws silently; {@link Printable}, what {@link
 * dev.hegel.Generators#tuples} returns for printable element generators, prints each element as it
 * is drawn: a tuple record as its constructor call, {@code new Tuple2(a, b)}, a list as {@code [a,
 * b]}.
 *
 * @param <T> the assembled tuple type
 */
public class TupleGenerator<T> implements Generator<T> {
    private final String record;
    private final List<Generator<?>> elements;
    private final Function<List<Object>, T> assembler;
    private final CachedLabel label = new CachedLabel();

    /**
     * A generator of tuples that print as a list, {@code [a, b]}.
     *
     * @param elements one generator per position
     * @param assembler packs the drawn values into a tuple
     */
    public TupleGenerator(List<? extends Generator<?>> elements, Function<List<Object>, T> assembler) {
        this(null, elements, assembler);
    }

    /**
     * A generator of tuples that print as a call of the constructor of the record {@code record},
     * {@code new record(a, b)}.
     *
     * @param record the simple name of the record the tuple is, such as {@code Tuple2}
     * @param elements one generator per position
     * @param assembler packs the drawn values into a tuple
     */
    public TupleGenerator(String record, List<? extends Generator<?>> elements, Function<List<Object>, T> assembler) {
        this.record = record;
        this.elements = List.<Generator<?>>copyOf(elements);
        this.assembler = assembler;
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> CachedLabel.combine(Label.TUPLE, elements));
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        return draw(tc, null);
    }

    final T draw(TestCase tc, Printer printer) {
        Printer p = Components.out(printer);
        tc.startSpan(label());
        try {
            List<Object> out = new ArrayList<>(elements.size());
            p.beginGroup(record == null ? "[" : "new " + record + "(");
            for (Generator<?> g : elements) {
                if (!out.isEmpty()) {
                    p.separator();
                }
                out.add(Components.draw(tc, g, printer));
            }
            p.endGroup(record == null ? "]" : ")");
            return assembler.apply(out);
        } finally {
            tc.stopSpan(false);
        }
    }

    /**
     * A tuple generator over printable element generators: it prints each element as it is drawn,
     * a tuple record as its constructor call, {@code new Tuple2(a, b)}, a list as {@code [a, b]}.
     *
     * @param <T> the assembled tuple type
     */
    public static final class Printable<T> extends TupleGenerator<T> implements PrintableGenerator<T> {
        /**
         * A generator of tuples that print as a list, {@code [a, b]}.
         *
         * @param elements one printable generator per position
         * @param assembler packs the drawn values into a tuple
         */
        public Printable(List<? extends PrintableGenerator<?>> elements, Function<List<Object>, T> assembler) {
            this(null, elements, assembler);
        }

        /**
         * A generator of tuples that print as a call of the constructor of the record {@code
         * record}, {@code new record(a, b)}.
         *
         * @param record the simple name of the record the tuple is, such as {@code Tuple2}
         * @param elements one printable generator per position
         * @param assembler packs the drawn values into a tuple
         */
        public Printable(
                String record, List<? extends PrintableGenerator<?>> elements, Function<List<Object>, T> assembler) {
            super(record, elements, assembler);
        }

        /** @hidden */
        @Override
        public T doDrawAndPrint(TestCase tc, Printer printer) {
            return draw(tc, printer);
        }
    }
}
