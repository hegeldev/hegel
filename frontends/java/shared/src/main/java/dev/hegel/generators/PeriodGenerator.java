package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.SpecHandle;
import dev.hegel.TestCase;
import dev.hegel.lowlevel.Abi;
import java.time.Period;

/**
 * Generates {@link Period} values: a count of years, months and days, each independently signed.
 *
 * <p>By default every unit spans the whole {@code int} range. The first of {@link #years(int,
 * int)}, {@link #months(int, int)} and {@link #days(int, int)} called replaces that default with
 * just its unit, and later calls add units, so a period of days and months is {@code
 * periods().days(0, 30).months(0, 11)}; a unit never enabled is always 0. {@link
 * #nonNegative(boolean)} draws magnitudes only. Values shrink toward {@link Period#ZERO}, or the
 * nearest value to it in each unit's bounds.
 */
public final class PeriodGenerator implements Generator<Period> {
    private static final int YEARS = 0;
    private static final int MONTHS = 1;
    private static final int DAYS = 2;
    private static final int[] ENGINE_UNITS = {Abi.SPAN_UNIT_YEARS, Abi.SPAN_UNIT_MONTHS, Abi.SPAN_UNIT_DAYS};
    private static final int[] SPAN_INDEX = {0, 1, 3};

    /** Per unit, {@code {min, max}} when enabled or {@code null}; {@code null} overall before any unit was set. */
    private final int[][] bounds;

    private final boolean nonNegative;
    private final SpecCache spec = new SpecCache();

    public PeriodGenerator() {
        this(null, false);
    }

    private PeriodGenerator(int[][] bounds, boolean nonNegative) {
        this.bounds = bounds;
        this.nonNegative = nonNegative;
    }

    private PeriodGenerator unit(int unit, int min, int max) {
        if (min > max) {
            throw new IllegalArgumentException("periods: min (" + min + ") > max (" + max + ")");
        }
        int[][] next = bounds == null ? new int[3][] : bounds.clone();
        next[unit] = new int[] {min, max};
        return new PeriodGenerator(next, nonNegative);
    }

    /**
     * Enable years within {@code [min, max]}; the first unit enabled replaces the default of all
     * three.
     *
     * @param min the inclusive lower bound
     * @param max the inclusive upper bound
     * @return a copy with the bounds set
     */
    public PeriodGenerator years(int min, int max) {
        return unit(YEARS, min, max);
    }

    /**
     * Enable months within {@code [min, max]}; the first unit enabled replaces the default of all
     * three.
     *
     * @param min the inclusive lower bound
     * @param max the inclusive upper bound
     * @return a copy with the bounds set
     */
    public PeriodGenerator months(int min, int max) {
        return unit(MONTHS, min, max);
    }

    /**
     * Enable days within {@code [min, max]}; the first unit enabled replaces the default of all
     * three.
     *
     * @param min the inclusive lower bound
     * @param max the inclusive upper bound
     * @return a copy with the bounds set
     */
    public PeriodGenerator days(int min, int max) {
        return unit(DAYS, min, max);
    }

    /**
     * @param nonNegative whether every unit is drawn as a magnitude only (off by default: each
     *     unit is signed on its own, as a {@link Period} is); a unit whose lower bound is negative
     *     is then rejected with an {@link IllegalArgumentException} at the first draw
     * @return a copy with the setting applied
     */
    public PeriodGenerator nonNegative(boolean nonNegative) {
        return new PeriodGenerator(bounds, nonNegative);
    }

    private SpecHandle buildSpec(TestCase tc) {
        int[][] enabled = bounds;
        if (enabled == null) {
            int[] whole = {nonNegative ? 0 : Integer.MIN_VALUE, Integer.MAX_VALUE};
            enabled = new int[][] {whole, whole, whole};
        }
        int count = 0;
        for (int[] bound : enabled) {
            if (bound != null) {
                count++;
            }
        }
        int[] units = new int[count];
        long[] mins = new long[count];
        long[] maxs = new long[count];
        int i = 0;
        for (int unit = 0; unit < enabled.length; unit++) {
            if (enabled[unit] != null) {
                units[i] = ENGINE_UNITS[unit];
                mins[i] = enabled[unit][0];
                maxs[i] = enabled[unit][1];
                i++;
            }
        }
        return tc.spanSpec(units, mins, maxs, nonNegative ? Abi.SPAN_SIGN_NONNEGATIVE : Abi.SPAN_SIGN_MIXED);
    }

    /** @hidden */
    @Override
    public Period doDraw(TestCase tc) {
        long[] span = tc.generateSpan(spec.get(tc, this::buildSpec));
        return Period.of((int) span[SPAN_INDEX[YEARS]], (int) span[SPAN_INDEX[MONTHS]], (int) span[SPAN_INDEX[DAYS]]);
    }
}
