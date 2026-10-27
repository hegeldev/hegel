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
        tc.startSpan(Label.PERMUTATION);
        try {
            int n = elements.size();
            List<T> remaining = new ArrayList<>(elements);
            List<T> out = new ArrayList<>(n);
            long id = tc.newCollection(n, n);
            try {
                while (!remaining.isEmpty() && tc.collectionMore(id)) {
                    out.add(remaining.remove((int) tc.generateInteger(0, remaining.size() - 1)));
                }
            } finally {
                tc.collectionFree(id);
            }
            return out;
        } finally {
            tc.stopSpan(false);
        }
    }
}
