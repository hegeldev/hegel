package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.TestCase;
import java.util.ArrayList;
import java.util.List;

/**
 * Generates permutations of a fixed list: each value holds every element, in an order the engine
 * chooses, shrinking toward the original order.
 *
 * @param <T> the element type
 */
public final class PermutationGenerator<T> extends ValueGenerator<List<T>> {
    private final List<T> elements;

    /**
     * @param elements the elements to permute; defensively copied
     */
    public PermutationGenerator(List<T> elements) {
        this.elements = List.copyOf(elements);
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.PERMUTATION;
    }

    /** @hidden */
    @Override
    public List<T> doDraw(TestCase tc) {
        int n = elements.size();
        List<T> out = new ArrayList<>(n);
        for (int i : IndexSamples.draw(tc, n, n, n)) {
            out.add(elements.get(i));
        }
        return out;
    }
}
