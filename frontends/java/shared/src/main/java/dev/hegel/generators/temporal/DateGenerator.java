package dev.hegel.generators.temporal;

import dev.hegel.EngineAccess;
import dev.hegel.Generator;
import dev.hegel.SpecHandle;
import dev.hegel.TestCase;
import java.time.DayOfWeek;
import java.time.LocalDate;
import java.time.Month;
import java.util.Arrays;
import java.util.Objects;
import java.util.Set;

/**
 * Generates {@link LocalDate} values within an inclusive {@code [min, max]} range.
 *
 * <p>The default range is {@code 0001-01-01} to {@code 9999-12-31}; set either end with the fluent
 * {@link #min(LocalDate)} / {@link #max(LocalDate)} methods, anywhere in the years {@code -999999}
 * to {@code 999999}, and restrict the dates to some days of the week or months with {@link
 * #weekdays(DayOfWeek...)} and {@link #months(Month...)}. Values shrink toward 2000-01-01, or the nearest bound (or allowed
 * day) when that is out of range. One draw in twenty on average targets a date tests tend to get
 * wrong: the leap days nearest the bounds, the year boundaries, the 2038 and 2106 overflow days
 * and the days of an ISO week 53.
 *
 * <p>The fluent methods can be called in any order. Each rejects, with an {@link
 * IllegalArgumentException}, what cannot work whatever is set afterwards: a bound outside the years
 * {@code -999999} to {@code 999999}, an empty set of weekdays or months, and the settings given so
 * far when they do not fit together ({@code min} after {@code max}, or bounds with no date on an
 * allowed weekday and month). A setting that was not given is not held against the others until the
 * first draw, where it counts as its default: {@code min} alone may be beyond the year 9999, since
 * a {@code max} can follow in either order, and without one the first draw fails with an {@link
 * IllegalArgumentException} because the default {@code max} is the last day of 9999.
 */
public final class DateGenerator implements Generator<LocalDate> {
    static final LocalDate DEFAULT_MIN = LocalDate.of(1, 1, 1);
    static final LocalDate DEFAULT_MAX = LocalDate.of(9999, 12, 31);
    static final int MAX_ENGINE_YEAR = 999_999;
    static final LocalDate MIN_ENGINE = LocalDate.of(-MAX_ENGINE_YEAR, 1, 1);
    static final LocalDate MAX_ENGINE = LocalDate.of(MAX_ENGINE_YEAR, 12, 31);

    private final LocalDate min;
    private final LocalDate max;
    private final LocalDate lo;
    private final LocalDate hi;
    private final Set<DayOfWeek> weekdays;
    private final Set<Month> months;
    private final SpecCache options = new SpecCache();

    public DateGenerator() {
        this(null, null, null, null);
    }

    /**
     * @param min the inclusive lower bound
     * @param max the inclusive upper bound
     * @throws IllegalArgumentException if {@code min} is after {@code max}
     */
    public DateGenerator(LocalDate min, LocalDate max) {
        this(Objects.requireNonNull(min), Objects.requireNonNull(max), null, null);
    }

    private DateGenerator(LocalDate min, LocalDate max, Set<DayOfWeek> weekdays, Set<Month> months) {
        this.lo = Bounds.or(min, DEFAULT_MIN);
        this.hi = Bounds.or(max, DEFAULT_MAX);
        validateYear(lo);
        validateYear(hi);
        this.min = min;
        this.max = max;
        this.weekdays = weekdays;
        this.months = months;
        LocalDate widestLo = Bounds.or(min, MIN_ENGINE);
        LocalDate widestHi = Bounds.or(max, MAX_ENGINE);
        Bounds.ordered("dates", min, max, widestLo, widestHi);
        TestCase setup = EngineAccess.setup();
        setup.validateDate(widestLo, widestHi, optionsHandle(setup));
    }

    private static void validateYear(LocalDate date) {
        if (date.getYear() < -MAX_ENGINE_YEAR || date.getYear() > MAX_ENGINE_YEAR) {
            throw new IllegalArgumentException("dates: year of " + date + " is outside [-999999, 999999]");
        }
    }

    private static <E extends Enum<E>> Set<E> nonEmpty(Set<E> values, String what) {
        if (values.isEmpty()) {
            throw new IllegalArgumentException("dates: " + what + " must not be empty");
        }
        return Set.copyOf(values);
    }

    /**
     * @param min the inclusive lower bound
     * @return a copy with the lower bound set
     */
    public DateGenerator min(LocalDate min) {
        return new DateGenerator(Objects.requireNonNull(min), max, weekdays, months);
    }

    /**
     * @param max the inclusive upper bound
     * @return a copy with the upper bound set
     */
    public DateGenerator max(LocalDate max) {
        return new DateGenerator(min, Objects.requireNonNull(max), weekdays, months);
    }

    /**
     * Draw only dates falling on these days of the week. A drawn date on another day is moved to
     * the nearest allowed date inside the bounds rather than rejected, so the draw never filters.
     *
     * @param weekdays the allowed days of the week, at least one
     * @return a copy restricted to those weekdays
     */
    public DateGenerator weekdays(DayOfWeek... weekdays) {
        return weekdays(Set.copyOf(Arrays.asList(weekdays)));
    }

    /**
     * @param weekdays the allowed days of the week, at least one
     * @return a copy restricted to those weekdays
     * @see #weekdays(DayOfWeek...)
     */
    public DateGenerator weekdays(Set<DayOfWeek> weekdays) {
        return new DateGenerator(min, max, nonEmpty(weekdays, "weekdays"), months);
    }

    /**
     * Draw only dates in these months. A drawn date in another month is moved to the nearest
     * allowed date inside the bounds rather than rejected, so the draw never filters.
     *
     * @param months the allowed months, at least one
     * @return a copy restricted to those months
     */
    public DateGenerator months(Month... months) {
        return months(Set.copyOf(Arrays.asList(months)));
    }

    /**
     * @param months the allowed months, at least one
     * @return a copy restricted to those months
     * @see #months(Month...)
     */
    public DateGenerator months(Set<Month> months) {
        return new DateGenerator(min, max, weekdays, nonEmpty(months, "months"));
    }

    private static <E extends Enum<E>> int mask(Set<E> values) {
        int mask = 0;
        if (values != null) {
            for (E value : values) {
                mask |= 1 << value.ordinal();
            }
        }
        return mask;
    }

    private SpecHandle buildOptions(TestCase tc) {
        return tc.dateOptions(mask(weekdays), mask(months));
    }

    private SpecHandle optionsHandle(TestCase tc) {
        return weekdays == null && months == null ? null : options.get(tc, this::buildOptions);
    }

    /** @hidden */
    @Override
    public LocalDate doDraw(TestCase tc) {
        Bounds.ordered("dates", min, max, lo, hi);
        return tc.generateDate(lo, hi, optionsHandle(tc));
    }
}
