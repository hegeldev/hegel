package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.TestCase;

/**
 * Generates {@code true} with probability {@code p} and {@code false} otherwise, shrinking toward
 * {@code false}.
 */
public final class BooleanGenerator extends ValueGenerator<Boolean> {
    private final double p;

    /**
     * @param p the probability of {@code true}, in {@code [0, 1]}
     */
    public BooleanGenerator(double p) {
        if (!(p >= 0.0 && p <= 1.0)) {
            throw new IllegalArgumentException("weightedBooleans: p must be between 0 and 1, got " + p);
        }
        this.p = p;
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.of("hegel.booleans");
    }

    /** @hidden */
    @Override
    public Boolean doDraw(TestCase tc) {
        return tc.generateBoolean(p);
    }
}
