package dev.hegel;

/**
 * A generator of the values of a {@link Pool} or {@link ConcurrentPool}, which prints the value it
 * draws with {@link PrettyPrinter#repr}.
 *
 * @param <T> the type of pooled values
 */
abstract class PoolGenerator<T> implements PrintableGenerator<T> {
    @Override
    public abstract T doDraw(TestCase tc);

    @Override
    public final T doDrawAndPrint(TestCase tc, PrettyPrinter printer) {
        T value = doDraw(tc);
        printer.repr(value);
        return value;
    }
}
