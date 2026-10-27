package dev.hegel.generators;

import dev.hegel.Generators;
import dev.hegel.Label;
import dev.hegel.TestCase;

/**
 * Generates 64-bit {@code double} values with full control over bounds and special values. For
 * 32-bit {@code float} values use {@link FloatGenerator} ({@link Generators#floats()}).
 *
 * <p>A bound is inclusive ({@link #min(double) min}, {@link #max(double) max}) or exclusive ({@link
 * #minValueExclusive(double) minValueExclusive}, {@link #maxValueExclusive(double) maxValueExclusive}:
 * the bound itself is never drawn). Both forms set the same bound, so the last call wins.
 *
 * <p>Defaults mirror the engine. With no bounds, NaN and the infinities are allowed. Setting any
 * bound excludes NaN; setting <em>both</em> bounds also excludes the infinities (a single bound
 * still allows the infinity on the open side). Subnormal values are allowed whenever the bounds
 * admit any; disable them with {@link #allowSubnormal(boolean) allowSubnormal(false)} when the code
 * under test may run with flush-to-zero floating point (e.g. compiled with {@code -ffast-math}).
 * {@code allowNan}/{@code allowInfinity} override these defaults where the combination is valid.
 * Validation of conflicting options happens at construction time.
 */
public final class DoubleGenerator extends ValueGenerator<Double> {
    private final Floats.DrawParams params;
    private final boolean excludeMin;
    private final boolean excludeMax;

    private final Double min;
    private final Double max;
    private final Boolean allowNan;
    private final Boolean allowInfinity;
    private final Boolean allowSubnormal;

    /** The unbounded generator: any {@code double}, NaN and the infinities included. */
    public DoubleGenerator() {
        this(null, null, null, null, null, false, false);
    }

    private DoubleGenerator(
            Double min,
            Double max,
            Boolean allowNan,
            Boolean allowInfinity,
            Boolean allowSubnormal,
            boolean excludeMin,
            boolean excludeMax) {
        this.params = Floats.resolve(
                "doubles", 64, min, max, allowNan, allowInfinity, allowSubnormal, excludeMin, excludeMax);
        this.min = min;
        this.max = max;
        this.allowNan = allowNan;
        this.allowInfinity = allowInfinity;
        this.allowSubnormal = allowSubnormal;
        this.excludeMin = excludeMin;
        this.excludeMax = excludeMax;
    }

    /**
     * @param min the inclusive lower bound
     * @return a copy with the lower bound set
     */
    public DoubleGenerator min(double min) {
        return new DoubleGenerator(min, max, allowNan, allowInfinity, allowSubnormal, false, excludeMax);
    }

    /**
     * @param max the inclusive upper bound
     * @return a copy with the upper bound set
     */
    public DoubleGenerator max(double max) {
        return new DoubleGenerator(min, max, allowNan, allowInfinity, allowSubnormal, excludeMin, false);
    }

    /**
     * @param min the exclusive lower bound: every value is above it
     * @return a copy with the lower bound set
     */
    public DoubleGenerator minValueExclusive(double min) {
        return new DoubleGenerator(min, max, allowNan, allowInfinity, allowSubnormal, true, excludeMax);
    }

    /**
     * @param max the exclusive upper bound: every value is below it
     * @return a copy with the upper bound set
     */
    public DoubleGenerator maxValueExclusive(double max) {
        return new DoubleGenerator(min, max, allowNan, allowInfinity, allowSubnormal, excludeMin, true);
    }

    /**
     * @param allow whether NaN may be generated
     * @return a copy with the NaN policy set
     */
    public DoubleGenerator allowNan(boolean allow) {
        return new DoubleGenerator(min, max, allow, allowInfinity, allowSubnormal, excludeMin, excludeMax);
    }

    /**
     * @param allow whether infinities may be generated
     * @return a copy with the infinity policy set
     */
    public DoubleGenerator allowInfinity(boolean allow) {
        return new DoubleGenerator(min, max, allowNan, allow, allowSubnormal, excludeMin, excludeMax);
    }

    /**
     * @param allow whether subnormal ("denormalised") values may be generated
     * @return a copy with the subnormal policy set
     */
    public DoubleGenerator allowSubnormal(boolean allow) {
        return new DoubleGenerator(min, max, allowNan, allowInfinity, allow, excludeMin, excludeMax);
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.of("hegel.floats");
    }

    /** @hidden */
    @Override
    public Double doDraw(TestCase tc) {
        return tc.generateFloat(
                64,
                params.min(),
                params.max(),
                params.allowNan(),
                params.allowInfinity(),
                excludeMin,
                excludeMax,
                params.smallestNonzeroMagnitude());
    }
}
