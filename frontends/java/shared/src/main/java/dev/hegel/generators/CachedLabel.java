package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import java.util.List;
import java.util.function.LongSupplier;

/** A composite generator's label, computed by the engine on first use and kept for later draws. */
final class CachedLabel {
    private volatile Long value;

    /** The label of a generator of kind {@code own} built from {@code components}, in order. */
    static long combine(long own, List<? extends Generator<?>> components) {
        long[] labels = new long[components.size() + 1];
        labels[0] = own;
        for (int i = 0; i < components.size(); i++) {
            labels[i + 1] = components.get(i).label();
        }
        return Label.combine(labels);
    }

    /** The label, computed by {@code compute} the first time it is asked for. */
    long get(LongSupplier compute) {
        Long label = value;
        if (label == null) {
            label = compute.getAsLong();
            value = label;
        }
        return label;
    }
}
