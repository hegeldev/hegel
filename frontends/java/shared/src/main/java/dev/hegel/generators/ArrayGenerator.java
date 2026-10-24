package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.TestCase;
import java.util.function.IntFunction;

/**
 * Generates arrays of a fixed length, drawing each element from one generator.
 *
 * @param <T> the element type
 */
public final class ArrayGenerator<T> implements Generator<T[]> {
    private final Generator<T> element;
    private final int length;
    private final IntFunction<T[]> newArray;

    /**
     * @param element the element generator
     * @param length the array length
     * @param newArray creates an array of the given length, such as {@code Integer[]::new}
     */
    public ArrayGenerator(Generator<T> element, int length, IntFunction<T[]> newArray) {
        if (length < 0) {
            throw new IllegalArgumentException("arrays: length must be >= 0, got " + length);
        }
        this.element = element;
        this.length = length;
        this.newArray = newArray;
    }

    /** @hidden */
    @Override
    public T[] doDraw(TestCase tc) {
        tc.startSpan(Label.ARRAY);
        try {
            T[] out = newArray.apply(length);
            for (int i = 0; i < length; i++) {
                out[i] = element.doDraw(tc);
            }
            return out;
        } finally {
            tc.stopSpan(false);
        }
    }
}
