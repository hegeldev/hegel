package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.TestCase;
import java.time.LocalTime;
import java.time.temporal.ChronoUnit;

/**
 * Generates {@link LocalTime} values within an inclusive {@code [min, max]} range.
 *
 * <p>The default range is the whole day at nanosecond resolution; narrow it with the fluent {@link
 * #min(LocalTime)} / {@link #max(LocalTime)} methods, and make every drawn time a whole number of
 * some unit with {@link #precision(ChronoUnit)}. Values shrink toward the lower bound. One draw in
 * twenty on average targets an interesting time: a bound, midnight, noon, the last second of the
 * day or a fractional second of a common precision.
 */
public final class TimeGenerator implements Generator<LocalTime> {
    private final LocalTime min;
    private final LocalTime max;
    private final ChronoUnit precision;
    private final int precisionCode;

    public TimeGenerator() {
        this(LocalTime.MIN, LocalTime.MAX);
    }

    public TimeGenerator(LocalTime min, LocalTime max) {
        this(min, max, ChronoUnit.NANOS);
    }

    private TimeGenerator(LocalTime min, LocalTime max, ChronoUnit precision) {
        if (min.isAfter(max)) {
            throw new IllegalArgumentException("times: min (" + min + ") > max (" + max + ")");
        }
        this.min = min;
        this.max = max;
        this.precision = precision;
        this.precisionCode = Precisions.engine(precision, "times");
    }

    /**
     * @param min the inclusive lower bound
     * @return a copy with the lower bound set
     */
    public TimeGenerator min(LocalTime min) {
        return new TimeGenerator(min, max, precision);
    }

    /**
     * @param max the inclusive upper bound
     * @return a copy with the upper bound set
     */
    public TimeGenerator max(LocalTime max) {
        return new TimeGenerator(min, max, precision);
    }

    /**
     * Draw only times that are a whole number of {@code precision} units since midnight: {@link
     * ChronoUnit#NANOS} (the default), {@link ChronoUnit#MICROS}, {@link ChronoUnit#MILLIS}, {@link
     * ChronoUnit#SECONDS}, {@link ChronoUnit#MINUTES} or {@link ChronoUnit#HOURS}. Bounds holding
     * no such time are rejected with an {@link IllegalArgumentException} at the first draw.
     *
     * @param precision the unit every drawn time is a whole number of
     * @return a copy with the precision set
     */
    public TimeGenerator precision(ChronoUnit precision) {
        return new TimeGenerator(min, max, precision);
    }

    /** @hidden */
    @Override
    public LocalTime doDraw(TestCase tc) {
        return tc.generateTime(min, max, precisionCode);
    }
}
