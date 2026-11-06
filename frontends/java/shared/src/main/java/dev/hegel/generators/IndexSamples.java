package dev.hegel.generators;

import dev.hegel.TestCase;
import java.util.ArrayList;
import java.util.List;

/** The index draw shared by permutations, subsequences and samples without replacement. */
final class IndexSamples {
    private IndexSamples() {}

    /**
     * Draws between {@code minSize} and {@code maxSize} distinct indices into {@code 0..n}, without
     * replacement, in draw order. Shrinks toward fewer indices and toward the earliest not-yet-taken
     * index, so the minimal sample is {@code 0..minSize} in increasing order.
     */
    static List<Integer> draw(TestCase tc, int n, long minSize, long maxSize) {
        List<Integer> remaining = new ArrayList<>(n);
        for (int i = 0; i < n; i++) {
            remaining.add(i);
        }
        List<Integer> chosen = new ArrayList<>();
        long id = tc.newCollection(minSize, maxSize);
        try {
            while (!remaining.isEmpty() && tc.collectionMore(id)) {
                chosen.add(remaining.remove((int) tc.generateInteger(0, remaining.size() - 1)));
            }
        } finally {
            tc.collectionFree(id);
        }
        return chosen;
    }
}
