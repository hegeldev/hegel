package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.TestCase;
import dev.hegel.lowlevel.Abi;
import java.util.ArrayList;
import java.util.List;

/**
 * Generates subsequences of a fixed list: each value holds some of the elements, each used at most
 * once, in their original order. Shrinks toward fewer elements, taken from earlier in the list.
 *
 * <p>The length range defaults to anything from none of the elements to all of them; narrow it with
 * the fluent {@link #minSize(int)} / {@link #maxSize(int)} methods. A maximum above the number of
 * elements is capped at it, and a minimum above it is rejected.
 *
 * @param <T> the element type
 */
public final class SubsequenceGenerator<T> extends ValueGenerator<List<T>> {
    private final List<T> elements;
    private final long minSize;
    private final long maxSize;

    /**
     * @param elements the elements to pick from; defensively copied
     * @param minSize the minimum length (inclusive)
     * @param maxSize the maximum length (inclusive), or {@link Abi#UNBOUNDED}
     */
    public SubsequenceGenerator(List<T> elements, long minSize, long maxSize) {
        Sizes.validate(minSize, maxSize, "subsequences");
        this.elements = List.copyOf(elements);
        if (minSize > this.elements.size()) {
            throw new IllegalArgumentException("Cannot generate a subsequence: minSize " + minSize
                    + " is larger than the " + this.elements.size() + " elements in the sequence");
        }
        this.minSize = minSize;
        this.maxSize = maxSize;
    }

    /**
     * @param minSize the minimum length (inclusive)
     * @return a copy with the minimum size set
     */
    public SubsequenceGenerator<T> minSize(int minSize) {
        return new SubsequenceGenerator<>(elements, minSize, maxSize);
    }

    /**
     * @param maxSize the maximum length (inclusive)
     * @return a copy with the maximum size set
     */
    public SubsequenceGenerator<T> maxSize(int maxSize) {
        return new SubsequenceGenerator<>(elements, minSize, maxSize);
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.SUBSEQUENCE;
    }

    /** @hidden */
    @Override
    public List<T> doDraw(TestCase tc) {
        int n = elements.size();
        long max = maxSize == Abi.UNBOUNDED ? n : Math.min(maxSize, n);
        List<Integer> indices = IndexSamples.draw(tc, n, minSize, max);
        indices.sort(null);
        List<T> out = new ArrayList<>(indices.size());
        for (int i : indices) {
            out.add(elements.get(i));
        }
        return out;
    }
}
