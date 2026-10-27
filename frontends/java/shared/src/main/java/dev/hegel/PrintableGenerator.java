package dev.hegel;

/**
 * A {@link Generator} that can describe the values it draws: what {@link TestCase#draw} needs to
 * print a draw in the failure report. It draws and prints in one step, writing to the printer as
 * it draws, so that a container prints element by element and each element's own printing appears
 * inside it.
 *
 * <p>A printable generator is a lambda over the test case and the printer. Draw each part through
 * {@link TestCase#drawAndPrint}, so that it prints itself in place:
 *
 * <pre>{@code
 * PrintableGenerator<Point> points = (tc, p) -> {
 *     p.beginGroup("Point.of(");
 *     int x = tc.drawAndPrint(integers(), p);
 *     p.separator();
 *     int y = tc.drawAndPrint(integers(), p);
 *     p.endGroup(")");
 *     return Point.of(x, y);
 * };
 *
 * Point point = tc.draw(points, "point");   // var point_1 = Point.of(3, -1);
 * }</pre>
 *
 * <p>The silent {@link #doDraw} draws through {@link #doDrawAndPrint} with the {@link
 * Printer#noop()} printer, so both draw the same values from the same choices. A generator whose
 * silent draw is cheaper, or whose parts need not be printable when it is drawn silently, overrides
 * it.
 *
 * @param <T> the type of value produced
 */
@FunctionalInterface
public interface PrintableGenerator<T> extends Generator<T> {
    /**
     * Draw a value, describing it to {@code printer} as it is drawn. User code draws through {@link
     * TestCase#draw}; a printable generator implements this, drawing its parts through {@link
     * TestCase#drawAndPrint}.
     *
     * @param tc the current test case
     * @param printer the printer to describe the value to
     * @return the generated value
     */
    T doDrawAndPrint(TestCase tc, Printer printer);

    /**
     * Draw a value without printing it: {@link #doDrawAndPrint} into the {@link Printer#noop()}
     * printer.
     *
     * @param tc the current test case
     * @return the generated value
     */
    @Override
    default T doDraw(TestCase tc) {
        return doDrawAndPrint(tc, Printer.noop());
    }
}
