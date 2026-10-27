package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.TestCase;
import java.math.BigInteger;

/**
 * Generates {@link BigInteger} values within an inclusive {@code [min, max]} range, whose bounds may
 * be of any magnitude.
 *
 * <p>Either bound may be left unset. A missing bound is taken to be {@code 2^127} values beyond the
 * other bound, or beyond zero when that is further, so with neither set the values lie in {@code
 * [-2^127, 2^127)}. Values shrink toward zero, or toward the bound
 * nearest zero when zero is out of range.
 */
public final class BigIntegerGenerator extends ValueGenerator<BigInteger> {
    /** How far an unset bound reaches beyond the other bound, or beyond zero. */
    private static final BigInteger UNBOUNDED_MAGNITUDE = BigInteger.ONE.shiftLeft(127);

    private final BigInteger min;
    private final BigInteger max;
    private final BigInteger lo;
    private final BigInteger hi;

    public BigIntegerGenerator(BigInteger min, BigInteger max) {
        if (min != null && max != null && min.compareTo(max) > 0) {
            throw new IllegalArgumentException("bigIntegers: min (" + min + ") > max (" + max + ")");
        }
        this.min = min;
        this.max = max;
        BigInteger lo = UNBOUNDED_MAGNITUDE.negate();
        if (max != null) {
            lo = lo.min(max.subtract(UNBOUNDED_MAGNITUDE).add(BigInteger.ONE));
        }
        BigInteger hi = UNBOUNDED_MAGNITUDE.subtract(BigInteger.ONE);
        if (min != null) {
            hi = hi.max(min.add(UNBOUNDED_MAGNITUDE).subtract(BigInteger.ONE));
        }
        this.lo = min != null ? min : lo;
        this.hi = max != null ? max : hi;
    }

    /**
     * @param min the inclusive lower bound
     * @return a copy with the lower bound set
     */
    public BigIntegerGenerator min(BigInteger min) {
        return new BigIntegerGenerator(min, max);
    }

    /**
     * @param max the inclusive upper bound
     * @return a copy with the upper bound set
     */
    public BigIntegerGenerator max(BigInteger max) {
        return new BigIntegerGenerator(min, max);
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.of("hegel.integers");
    }

    /** @hidden */
    @Override
    public BigInteger doDraw(TestCase tc) {
        return tc.generateIntegerBig(lo, hi);
    }
}
