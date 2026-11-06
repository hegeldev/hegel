package dev.hegel.generators;

import dev.hegel.AssumeRejected;
import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrettyPrinter;
import dev.hegel.PrintableGenerator;
import dev.hegel.TestCase;
import dev.hegel.lowlevel.Abi;
import java.util.function.Predicate;

/**
 * Result of {@link Generator#filter}. Always uses the composite draw path: it retries the source up
 * to {@link #ATTEMPTS} times, bracketing each attempt in a {@link Label#FILTER_ATTEMPT} span, and
 * rejects the whole case if none satisfy the predicate. It prints the accepted value as the source
 * does: each attempt prints speculatively, and only the accepted one is kept. A filter of a filter
 * is one filtered draw of the source with both predicates, so a chain costs one round of attempts.
 *
 * @param <T> the value type
 */
public final class FilteredGenerator<T> implements PrintableGenerator<T> {
    /**
     * How many values a filtered draw tries before rejecting the test case: the engine's {@code
     * HEGEL_FILTER_ATTEMPTS}, shared by every Hegel frontend.
     */
    public static final int ATTEMPTS = Abi.FILTER_ATTEMPTS;

    private final Generator<T> source;
    private final Predicate<? super T> predicate;
    private final CachedLabel label = new CachedLabel();

    public FilteredGenerator(Generator<T> source, Predicate<? super T> predicate) {
        this.source = source;
        this.predicate = predicate;
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.FILTER, source.label()));
    }

    /** @hidden */
    @Override
    public PrintableGenerator<T> filter(Predicate<? super T> next) {
        return new FilteredGenerator<>(source, value -> predicate.test(value) && next.test(value));
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        return draw(tc, null);
    }

    /** @hidden */
    @Override
    public T doDrawAndPrint(TestCase tc, PrettyPrinter printer) {
        return draw(tc, printer);
    }

    private T draw(TestCase tc, PrettyPrinter printer) {
        PrettyPrinter out = Components.out(printer);
        for (int attempt = 0; attempt < ATTEMPTS; attempt++) {
            tc.startSpan(Label.FILTER_ATTEMPT);
            boolean discard = true;
            PrettyPrinter.Speculation speculation = out.speculate();
            try {
                T value = Components.draw(tc, source, printer);
                if (predicate.test(value)) {
                    discard = false;
                    speculation.commit();
                    return value;
                }
            } finally {
                try {
                    speculation.close();
                } finally {
                    tc.stopSpan(discard);
                }
            }
        }
        throw new AssumeRejected();
    }
}
