package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.function.IntFunction;

/**
 * Generates arrays of a fixed length, drawing each element from one generator. It prints as {@code
 * new Integer[] {a, b}}, named for the array's component type, each element printed by the element
 * generator as it is drawn.
 *
 * @param <T> the element type
 */
public final class ArrayGenerator<T> implements PrintableGenerator<T[]> {
    private final Generator<T> element;
    private final int length;
    private final IntFunction<T[]> newArray;
    private final CachedLabel label = new CachedLabel();

    /**
     * @param element the element generator
     * @param length the array length
     * @param newArray creates an array of the given length, such as {@code Integer[]::new}
     */
    public ArrayGenerator(Generator<T> element, int length, IntFunction<T[]> newArray) {
        if (length < 0) {
            throw new IllegalArgumentException("arrays: length must be >= 0, got " + length);
        }
        this.element = element;
        this.length = length;
        this.newArray = newArray;
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.ARRAY, element.label()));
    }

    /** @hidden */
    @Override
    public T[] doDraw(TestCase tc) {
        return draw(tc, null);
    }

    /** @hidden */
    @Override
    public T[] doDrawAndPrint(TestCase tc, Printer printer) {
        return draw(tc, printer);
    }

    private T[] draw(TestCase tc, Printer printer) {
        Printer p = Components.out(printer);
        T[] out = newArray.apply(length);
        p.beginGroup("new " + out.getClass().getComponentType().getSimpleName() + "[] {");
        for (int i = 0; i < length; i++) {
            if (i > 0) {
                p.separator();
            }
            out[i] = Components.draw(tc, element, printer);
        }
        p.endGroup("}");
        return out;
    }
}
