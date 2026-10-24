package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.SpecHandle;
import dev.hegel.TestCase;
import java.time.DayOfWeek;
import java.time.LocalDate;
import java.time.Month;
import java.util.Arrays;
import java.util.Set;

/**
 * Generates {@link LocalDate} values within an inclusive {@code [min, max]} range.
 *
 * <p>The default range is {@code 0001-01-01} to {@code 9999-12-31}; narrow it with the fluent
 * {@link #min(LocalDate)} / {@link #max(LocalDate)} methods, and restrict it to some days of the week
 * or months with {@link #weekdays(DayOfWeek...)} and {@link #months(Month...)}. Values shrink
 * toward 2000-01-01, or the nearest bound (or allowed day) when that is out of range. One draw in
 * twenty on average targets a date tests tend to get wrong: the leap days nearest the bounds, the
 * year boundaries, the 2038 and 2106 overflow days and the days of an ISO week 53.
 */
public final class DateGenerator implements Generator<LocalDate> {
    static final LocalDate DEFAULT_MIN = LocalDate.of(1, 1, 1);
    static final LocalDate DEFAULT_MAX = LocalDate.of(9999, 12, 31);
    static final int MAX_ENGINE_YEAR = 999_999;

    private final LocalDate min;
    private final LocalDate max;
    private final Set<DayOfWeek> weekdays;
    private final Set<Month> months;
    private final SpecCache options = new SpecCache();

    public DateGenerator() {
        this(DEFAULT_MIN, DEFAULT_MAX);
    }

    public DateGenerator(LocalDate min, LocalDate max) {
        this(min, max, null, null);
    }

    private DateGenerator(LocalDate min, LocalDate max, Set<DayOfWeek> weekdays, Set<Month> months) {
        if (min.isAfter(max)) {
            throw new IllegalArgumentException("dates: min (" + min + ") > max (" + max + ")");
        }
        validateYear(min);
        validateYear(max);
        this.min = min;
        this.max = max;
        this.weekdays = weekdays;
        this.months = months;
        if (weekdays != null || months != null) {
            checkSomeDayIsAllowed();
        }
    }

    private static void validateYear(LocalDate date) {
        if (date.getYear() < -MAX_ENGINE_YEAR || date.getYear() > MAX_ENGINE_YEAR) {
            throw new IllegalArgumentException("dates: year of " + date + " is outside [-999999, 999999]");
        }
    }

    private void checkSomeDayIsAllowed() {
        for (LocalDate day = min; !day.isAfter(max); day = day.plusDays(1)) {
            if (allows(day)) {
                return;
            }
        }
        throw new IllegalArgumentException("dates: no date between " + min + " and " + max + " falls on weekdays="
                + weekdays + " and months=" + months);
    }

    private boolean allows(LocalDate day) {
        return (weekdays == null || weekdays.contains(day.getDayOfWeek()))
                && (months == null || months.contains(day.getMonth()));
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
        return new DateGenerator(min, max, weekdays, months);
    }

    /**
     * @param max the inclusive upper bound
     * @return a copy with the upper bound set
     */
    public DateGenerator max(LocalDate max) {
        return new DateGenerator(min, max, weekdays, months);
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

    /** @hidden */
    @Override
    public LocalDate doDraw(TestCase tc) {
        SpecHandle handle = weekdays == null && months == null ? null : options.get(tc, this::buildOptions);
        return tc.generateDate(min, max, handle);
    }
}
