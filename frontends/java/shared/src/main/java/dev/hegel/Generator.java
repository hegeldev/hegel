package dev.hegel;

import dev.hegel.generators.FilteredGenerator;
import dev.hegel.generators.FlatMappedGenerator;
import dev.hegel.generators.MappedGenerator;
import dev.hegel.generators.PrintedGenerator;
import java.util.function.BiConsumer;
import java.util.function.Function;
import java.util.function.Predicate;

/**
 * Produces values of type {@code T} from a {@link TestCase}.
 *
 * <p>Generators are deterministic functions of the engine's choices, not sources of randomness. Use
 * the factory methods on {@link Generators} to construct them and the combinators here to transform
 * them.
 *
 * <p>A {@code Generator} draws silently: it can be drawn with {@link TestCase#drawSilent}, and used
 * anywhere a value is drawn without being reported. Drawing with {@link TestCase#draw}, which
 * prints the value in the failure report, needs a {@link PrintableGenerator}, which describes the
 * value as it draws it, and this is checked when the test compiles. Every built-in leaf generator
 * is printable, and a structural one, such as {@link Generators#lists}, is printable when its parts
 * are; a hand-written one is made printable by implementing {@link PrintableGenerator}, or by
 * {@link #printWith} or {@link #printAsRepr()}. See the {@linkplain dev.hegel package
 * documentation} for the printing model.
 *
 * <p>A hand-written generator is a lambda over the test case, drawing its parts through it:
 *
 * <pre>{@code
 * Generator<Point> points = tc -> new Point(tc.drawSilent(integers()), tc.drawSilent(integers()));
 * }</pre>
 *
 * @param <T> the type of value produced
 */
@FunctionalInterface
public interface Generator<T> {
    /**
     * Draw a value without printing it. User code draws through {@link TestCase#draw} or {@link
     * TestCase#drawSilent}; a generator implements this, drawing its parts through the test case.
     *
     * @param tc the current test case
     * @return the generated value
     */
    T doDraw(TestCase tc);

    /**
     * The label identifying this generator to the engine. A composite generator labels the span it
     * draws in with its own kind combined with its components' labels ({@link Label#combine}), so
     * that a list of integers and a list of strings get different labels while every list of
     * integers gets the same one, as in every Hegel frontend. A generator that does not override
     * this is labelled {@link Label#GENERATOR}.
     *
     * @return the label
     */
    default long label() {
        return Label.GENERATOR;
    }

    /**
     * Transform each generated value with {@code f}. The result is printable whether or not this
     * generator is: it prints the transformed value with {@link Printer#repr}. Mapping a {@link
     * PrintableGenerator} gives a {@link MappedGenerator.Printable}, whose {@link
     * MappedGenerator.Printable#printAsCall printAsCall} prints the call that made the value
     * instead.
     *
     * @param f the mapping function
     * @param <U> the result type
     * @return a generator of mapped values
     */
    default <U> MappedGenerator<T, U> map(Function<? super T, ? extends U> f) {
        return new MappedGenerator<>(this, f);
    }

    /**
     * Keep only values satisfying {@code predicate}. Prefer constraining a generator at
     * construction time (e.g. bounds) over filtering when possible. The result is printable when
     * this generator is ({@link PrintableGenerator#filter}), printing an accepted value as this
     * generator does and nothing of a rejected one.
     *
     * @param predicate the acceptance test
     * @return a filtered generator
     */
    default Generator<T> filter(Predicate<? super T> predicate) {
        return new FilteredGenerator<>(this, predicate);
    }

    /**
     * Dependent generation: draw a value silently, then draw from the generator {@code f} returns
     * for it. The result prints what that second generator prints, so {@code f} returns a {@link
     * PrintableGenerator}; make a hand-written one printable with {@link #printWith} or {@link
     * #printAsRepr()}.
     *
     * @param f maps a drawn value to the next generator
     * @param <U> the result type
     * @return a generator of dependent values
     */
    default <U> PrintableGenerator<U> flatMap(Function<? super T, ? extends PrintableGenerator<U>> f) {
        return new FlatMappedGenerator<>(this, f);
    }

    /**
     * Print each drawn value with {@code print}, which receives the value and the printer to
     * describe it to: for a custom representation, to mask a secret, or to make a hand-written
     * generator printable. The value is drawn as this generator draws it, silently, and {@code
     * print} is only called when the printer {@linkplain Printer#shouldPrint() prints}.
     *
     * <pre>{@code
     * tc.draw(integers().map(Key::new).printWith((key, p) -> p.text("Key.of(" + key.id() + ")")));
     * }</pre>
     *
     * @param print describes a drawn value to a printer
     * @return a printable generator of the same values
     */
    default PrintableGenerator<T> printWith(BiConsumer<? super T, Printer> print) {
        return new PrintedGenerator<>(this, print);
    }

    /**
     * Print each drawn value with {@link Printer#repr}: the usual way to make a hand-written
     * generator printable.
     *
     * @return a printable generator of the same values
     */
    default PrintableGenerator<T> printAsRepr() {
        return printWith((value, printer) -> printer.repr(value));
    }
}
