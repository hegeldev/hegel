package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Generates maps by driving the engine's collection API, drawing a key then a value and rejecting
 * duplicate keys.
 *
 * <p>The entry-count range defaults to any size; narrow it with the fluent {@link #minSize(int)} /
 * {@link #maxSize(int)} methods.
 *
 * <p>It prints as {@code Map.of(k, v)}, each key and value printed by its generator as it is drawn;
 * an entry with a rejected duplicate key prints nothing.
 */
public final class MapGenerator<K, V> implements PrintableGenerator<Map<K, V>> {
    private final Generator<K> keys;
    private final Generator<V> values;
    private final long minSize;
    private final long maxSize;
    private final CachedLabel label = new CachedLabel();

    public MapGenerator(Generator<K> keys, Generator<V> values, long minSize, long maxSize) {
        Sizes.validate(minSize, maxSize, "maps");
        this.keys = keys;
        this.values = values;
        this.minSize = minSize;
        this.maxSize = maxSize;
    }

    /**
     * @param minSize the minimum entry count (inclusive)
     * @return a copy with the minimum size set
     */
    public MapGenerator<K, V> minSize(int minSize) {
        return new MapGenerator<>(keys, values, minSize, maxSize);
    }

    /**
     * @param maxSize the maximum entry count (inclusive)
     * @return a copy with the maximum size set
     */
    public MapGenerator<K, V> maxSize(int maxSize) {
        return new MapGenerator<>(keys, values, minSize, maxSize);
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.MAP, keys.label(), values.label()));
    }

    /** @hidden */
    @Override
    public Map<K, V> doDraw(TestCase tc) {
        return draw(tc, null);
    }

    /** @hidden */
    @Override
    public Map<K, V> doDrawAndPrint(TestCase tc, Printer printer) {
        return draw(tc, printer);
    }

    private Map<K, V> draw(TestCase tc, Printer printer) {
        Printer p = Components.out(printer);
        long id = tc.newCollection(minSize, maxSize);
        try {
            Map<K, V> out = new LinkedHashMap<>();
            p.beginGroup("Map.of(");
            while (tc.collectionMore(id)) {
                try (Printer.Speculation speculation = p.speculate()) {
                    if (!out.isEmpty()) {
                        p.separator();
                    }
                    K key = Components.draw(tc, keys, printer);
                    if (out.containsKey(key)) {
                        speculation.abort();
                        tc.collectionReject(id, "duplicate key");
                    } else {
                        p.text(", ");
                        out.put(key, Components.draw(tc, values, printer));
                        speculation.commit();
                    }
                }
            }
            p.endGroup(")");
            return out;
        } finally {
            tc.collectionFree(id);
        }
    }
}
