package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.TestCase;
import java.time.Duration;
import java.time.temporal.ChronoUnit;

/**
 * Generates {@link Duration} values within an inclusive {@code [min, max]} range.
 *
 * <p>The default range is the whole signed range {@link Duration} represents, {@code
 * Long.MIN_VALUE} to {@code Long.MAX_VALUE} seconds at nanosecond resolution; narrow it with the
 * fluent {@link #min(Duration)} / {@link #max(Duration)} methods, and make every drawn duration a
 * whole number of some unit with {@link #precision(ChronoUnit)}. Values shrink toward {@link
 * Duration#ZERO}, or the nearest bound when that is out of range. One draw in twenty on average
 * targets a round duration, such as a second, an hour, 25 hours or a week, of either sign.
 */
public final class DurationGenerator implements Generator<Duration> {
    static final Duration MIN = Duration.ofSeconds(Long.MIN_VALUE);
    static final Duration MAX = Duration.ofSeconds(Long.MAX_VALUE, 999_999_999);

    private final Duration min;
    private final Duration max;
    private final ChronoUnit precision;
    private final int precisionCode;

    public DurationGenerator() {
        this(MIN, MAX);
    }

    public DurationGenerator(Duration min, Duration max) {
        this(min, max, ChronoUnit.NANOS);
    }

    private DurationGenerator(Duration min, Duration max, ChronoUnit precision) {
        if (min.compareTo(max) > 0) {
            throw new IllegalArgumentException("durations: min (" + min + ") > max (" + max + ")");
        }
        this.min = min;
        this.max = max;
        this.precision = precision;
        this.precisionCode = Precisions.engine(precision, "durations");
    }

    /**
     * @param min the inclusive lower bound
     * @return a copy with the lower bound set
     */
    public DurationGenerator min(Duration min) {
        return new DurationGenerator(min, max, precision);
    }

    /**
     * @param max the inclusive upper bound
     * @return a copy with the upper bound set
     */
    public DurationGenerator max(Duration max) {
        return new DurationGenerator(min, max, precision);
    }

    /**
     * Draw only durations that are a whole number of {@code precision} units: {@link
     * ChronoUnit#NANOS} (the default), {@link ChronoUnit#MICROS}, {@link ChronoUnit#MILLIS}, {@link
     * ChronoUnit#SECONDS}, {@link ChronoUnit#MINUTES} or {@link ChronoUnit#HOURS}. Bounds holding
     * no such duration are rejected with an {@link IllegalArgumentException} at the first draw.
     *
     * @param precision the unit every drawn duration is a whole number of
     * @return a copy with the precision set
     */
    public DurationGenerator precision(ChronoUnit precision) {
        return new DurationGenerator(min, max, precision);
    }

    /** @hidden */
    @Override
    public Duration doDraw(TestCase tc) {
        return tc.generateDuration(min, max, precisionCode);
    }
}
