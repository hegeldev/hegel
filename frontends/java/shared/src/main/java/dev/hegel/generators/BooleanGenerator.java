package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.TestCase;

/**
 * Generates {@code true} or {@code false}.
 */
public final class BooleanGenerator implements Generator<Boolean> {
    private final double p;

    public BooleanGenerator() {
        this.p = 0.5;
    }

    public BooleanGenerator(double p) {
        this.p = p;
    }
    
    /** @hidden */
    @Override
    public Boolean doDraw(TestCase tc) {
        return tc.generateBoolean(this.p);
    }
}
