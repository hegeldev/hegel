package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.TestCase;
import java.util.function.Function;

/**
 * Generates an optional value: empty, or wrapping a value from an inner generator, inside a {@link
 * Label#OPTIONAL} span. Shrinks toward empty.
 *
 * @param <T> the inner value type
 * @param <R> the optional type, such as {@code Optional<T>} or {@code OptionalInt}
 */
public final class OptionalGenerator<T, R> implements Generator<R> {
    private final Generator<T> inner;
    private final Function<? super T, ? extends R> present;
    private final R empty;
    private final CachedLabel label = new CachedLabel();

    /**
     * @param inner the generator of present values
     * @param present wraps a present value
     * @param empty the empty value
     */
    public OptionalGenerator(Generator<T> inner, Function<? super T, ? extends R> present, R empty) {
        this.inner = inner;
        this.present = present;
        this.empty = empty;
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.OPTIONAL, inner.label()));
    }

    /** @hidden */
    @Override
    public R doDraw(TestCase tc) {
        tc.startSpan(label());
        try {
            if (tc.generateBoolean(0.5)) {
                return present.apply(inner.doDraw(tc));
            }
            return empty;
        } finally {
            tc.stopSpan(false);
        }
    }
}
