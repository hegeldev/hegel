package dev.hegel.generators;

import dev.hegel.AssumeRejected;
import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.TestCase;
import dev.hegel.lowlevel.Abi;
import java.util.function.Predicate;

/**
 * Result of {@link Generator#filter}. Always uses the composite draw path: it retries the source up
 * to {@link #ATTEMPTS} times, bracketing each attempt in a {@link Label#FILTER_ATTEMPT} span, and rejects the
 * whole case if none satisfy the predicate.
 *
 * @param <T> the value type
 */
public final class FilteredGenerator<T> implements Generator<T> {
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

    @Override
    public T doDraw(TestCase tc) {
        for (int attempt = 0; attempt < ATTEMPTS; attempt++) {
            tc.startSpan(Label.FILTER_ATTEMPT);
            boolean discard = true;
            try {
                T value = source.doDraw(tc);
                if (predicate.test(value)) {
                    discard = false;
                    return value;
                }
            } finally {
                tc.stopSpan(discard);
            }
        }
        throw new AssumeRejected();
    }
}
