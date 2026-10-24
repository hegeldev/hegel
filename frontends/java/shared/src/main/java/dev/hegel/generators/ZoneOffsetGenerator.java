package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.TestCase;
import java.time.ZoneOffset;

/**
 * Generates {@link ZoneOffset} values (fixed UTC offsets) within an inclusive {@code [min, max]}
 * range.
 *
 * <p>The default range is the whole legal {@code ZoneOffset} span ({@code -18:00} to {@code
 * +18:00}) at one-second granularity; narrow it with the fluent {@link #min(ZoneOffset)} / {@link
 * #max(ZoneOffset)} methods, and make every drawn offset a whole number of hours, quarter-hours or
 * minutes with {@link #granularity(Granularity)}. Values shrink toward {@link ZoneOffset#UTC}. One
 * draw in twenty on average targets an offset some time zone has used, so {@code +05:45} and
 * {@code -00:44:30} appear, or a limit such as {@code +18:00} or {@code -00:00:01}.
 */
public final class ZoneOffsetGenerator implements Generator<ZoneOffset> {
    private final int minSeconds;
    private final int maxSeconds;
    private final Granularity granularity;

    public ZoneOffsetGenerator(int minSeconds, int maxSeconds) {
        this(minSeconds, maxSeconds, Granularity.SECONDS);
    }

    private ZoneOffsetGenerator(int minSeconds, int maxSeconds, Granularity granularity) {
        if (minSeconds > maxSeconds) {
            throw new IllegalArgumentException("zoneOffsets: min (" + minSeconds + "s) > max (" + maxSeconds + "s)");
        }
        this.minSeconds = minSeconds;
        this.maxSeconds = maxSeconds;
        this.granularity = granularity;
    }

    /**
     * @param min the inclusive lower bound
     * @return a copy with the lower bound set
     */
    public ZoneOffsetGenerator min(ZoneOffset min) {
        return new ZoneOffsetGenerator(min.getTotalSeconds(), maxSeconds, granularity);
    }

    /**
     * @param max the inclusive upper bound
     * @return a copy with the upper bound set
     */
    public ZoneOffsetGenerator max(ZoneOffset max) {
        return new ZoneOffsetGenerator(minSeconds, max.getTotalSeconds(), granularity);
    }

    /**
     * Draw only offsets that are a whole number of {@code granularity} units ({@link
     * Granularity#SECONDS} by default). Bounds holding no such offset are rejected with an {@link
     * IllegalArgumentException} at the first draw.
     *
     * @param granularity the unit every drawn offset is a whole number of
     * @return a copy with the granularity set
     */
    public ZoneOffsetGenerator granularity(Granularity granularity) {
        return new ZoneOffsetGenerator(minSeconds, maxSeconds, granularity);
    }

    /** @hidden */
    @Override
    public ZoneOffset doDraw(TestCase tc) {
        return ZoneOffset.ofTotalSeconds(tc.generateOffset(minSeconds, maxSeconds, granularity.engineValue));
    }
}
