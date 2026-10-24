package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.TestCase;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.function.Function;

/**
 * Generates lists by driving the engine's collection API element by element: the engine decides how
 * many elements to produce and the shrinker can delete or reorder them. The element generator opens
 * its own spans, so no per-element span is needed here (matching the engine's canonical client).
 *
 * <p>The length range defaults to any size; narrow it with the fluent {@link #minSize(int)} /
 * {@link #maxSize(int)} methods. {@link #unique()} and {@link #uniqueBy(Function)} make the
 * elements distinct: a drawn duplicate is rejected and the engine draws another in its place.
 */
public final class ListGenerator<T> implements Generator<List<T>> {
    private final Generator<T> element;
    private final long minSize;
    private final long maxSize;
    private final Function<? super T, ?> uniqueKey;

    public ListGenerator(Generator<T> element, long minSize, long maxSize) {
        this(element, minSize, maxSize, null);
    }

    private ListGenerator(Generator<T> element, long minSize, long maxSize, Function<? super T, ?> uniqueKey) {
        Sizes.validate(minSize, maxSize, "lists");
        this.element = element;
        this.minSize = minSize;
        this.maxSize = maxSize;
        this.uniqueKey = uniqueKey;
    }

    /**
     * @param minSize the minimum length (inclusive)
     * @return a copy with the minimum size set
     */
    public ListGenerator<T> minSize(int minSize) {
        return new ListGenerator<>(element, minSize, maxSize, uniqueKey);
    }

    /**
     * @param maxSize the maximum length (inclusive)
     * @return a copy with the maximum size set
     */
    public ListGenerator<T> maxSize(int maxSize) {
        return new ListGenerator<>(element, minSize, maxSize, uniqueKey);
    }

    /**
     * Make the elements distinct by {@link Object#equals}.
     *
     * @return a copy generating lists of distinct elements
     */
    public ListGenerator<T> unique() {
        return uniqueBy(Function.identity());
    }

    /**
     * Make the elements distinct by a key: no two elements of a list have equal keys.
     *
     * @param key extracts the key elements are compared by
     * @return a copy generating lists of elements with distinct keys
     */
    public ListGenerator<T> uniqueBy(Function<? super T, ?> key) {
        return new ListGenerator<>(element, minSize, maxSize, key);
    }

    /** @hidden */
    @Override
    public List<T> doDraw(TestCase tc) {
        tc.startSpan(Label.LIST);
        try {
            long id = tc.newCollection(minSize, maxSize);
            try {
                List<T> out = new ArrayList<>();
                Set<Object> keys = new HashSet<>();
                while (tc.collectionMore(id)) {
                    T value = element.doDraw(tc);
                    if (uniqueKey != null && !keys.add(uniqueKey.apply(value))) {
                        tc.collectionReject(id, "duplicate element");
                    } else {
                        out.add(value);
                    }
                }
                return out;
            } finally {
                tc.collectionFree(id);
            }
        } finally {
            tc.stopSpan(false);
        }
    }
}
