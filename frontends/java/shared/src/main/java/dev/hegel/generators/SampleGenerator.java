package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.TestCase;
import dev.hegel.lowlevel.Abi;
import java.util.ArrayList;
import java.util.List;

/**
 * Generates samples of a fixed list: lists of elements chosen from it. With replacement (the
 * default) each element of the result is chosen independently from the whole list, so the same
 * value can appear any number of times; {@link #withoutReplacement()} uses each element of the list
 * at most once, so the result is a reordered subset of the list. Shrinks toward fewer elements,
 * taken from earlier in the list.
 *
 * <p>The length range defaults to any size; narrow it with the fluent {@link #minSize(int)} /
 * {@link #maxSize(int)} methods. Without replacement the maximum is additionally capped at the
 * number of elements, and a minimum above it is rejected.
 *
 * @param <T> the element type
 */
public final class SampleGenerator<T> extends ValueGenerator<List<T>> {
    private final List<T> elements;
    private final long minSize;
    private final long maxSize;
    private final boolean replacement;

    /**
     * @param elements the elements to sample from; defensively copied
     * @param minSize the minimum length (inclusive)
     * @param maxSize the maximum length (inclusive), or {@link Abi#UNBOUNDED}
     * @param replacement whether an element may be chosen more than once
     */
    public SampleGenerator(List<T> elements, long minSize, long maxSize, boolean replacement) {
        Sizes.validate(minSize, maxSize, "samples");
        this.elements = List.copyOf(elements);
        int n = this.elements.size();
        if (replacement) {
            if (n == 0 && minSize > 0) {
                throw new IllegalArgumentException("Cannot generate a non-empty sample from an empty sequence");
            }
        } else if (minSize > n) {
            throw new IllegalArgumentException("Cannot generate a sample without replacement: minSize " + minSize
                    + " is larger than the " + n + " elements in the sequence");
        }
        this.minSize = minSize;
        this.maxSize = maxSize;
        this.replacement = replacement;
    }

    /**
     * @param minSize the minimum length (inclusive)
     * @return a copy with the minimum size set
     */
    public SampleGenerator<T> minSize(int minSize) {
        return new SampleGenerator<>(elements, minSize, maxSize, replacement);
    }

    /**
     * @param maxSize the maximum length (inclusive)
     * @return a copy with the maximum size set
     */
    public SampleGenerator<T> maxSize(int maxSize) {
        return new SampleGenerator<>(elements, minSize, maxSize, replacement);
    }

    /**
     * Sample with replacement: each element of the result is chosen independently from the whole
     * list, so the same value can appear any number of times. This is the default.
     *
     * @return a copy sampling with replacement
     */
    public SampleGenerator<T> withReplacement() {
        return new SampleGenerator<>(elements, minSize, maxSize, true);
    }

    /**
     * Sample without replacement: each element of the list is used at most once, so the result is a
     * reordered subset of the list. Duplicates in the list are distinct elements, so they can appear
     * in the result up to as many times as they appear in the list.
     *
     * @return a copy sampling without replacement
     */
    public SampleGenerator<T> withoutReplacement() {
        return new SampleGenerator<>(elements, minSize, maxSize, false);
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.SAMPLE;
    }

    /** @hidden */
    @Override
    public List<T> doDraw(TestCase tc) {
        int n = elements.size();
        List<T> out = new ArrayList<>();
        if (replacement) {
            long id = tc.newCollection(minSize, n == 0 ? 0 : maxSize);
            try {
                while (tc.collectionMore(id)) {
                    out.add(elements.get((int) tc.generateInteger(0, n - 1)));
                }
            } finally {
                tc.collectionFree(id);
            }
        } else {
            long max = maxSize == Abi.UNBOUNDED ? n : Math.min(maxSize, n);
            for (int i : IndexSamples.draw(tc, n, minSize, max)) {
                out.add(elements.get(i));
            }
        }
        return out;
    }
}
