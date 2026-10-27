package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.TestCase;

/**
 * Generates {@code true} or {@code false} with equal probability.
 */
public final class BooleanGenerator extends ValueGenerator<Boolean> {
    /** @hidden */
    @Override
    public long label() {
        return Label.of("hegel.booleans");
    }

    /** @hidden */
    @Override
    public Boolean doDraw(TestCase tc) {
        return tc.generateBoolean(0.5);
    }
}
