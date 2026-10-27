package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;

/**
 * How a structural generator draws its parts. Each has one drawing body, given a {@code null}
 * printer on its silent path, where every part is drawn silently, and the printer to describe the
 * value to on its printing path, where every part draws and prints itself in place. Only the
 * printable form of a structural generator, whose constructor takes printable parts, has a
 * printing path.
 */
final class Components {
    private Components() {}

    @SuppressWarnings("unchecked")
    static <X> X draw(TestCase tc, Generator<X> component, Printer printer) {
        return printer == null ? tc.drawSilent(component) : tc.drawAndPrint((PrintableGenerator<X>) component, printer);
    }

    static Printer out(Printer printer) {
        return printer == null ? Printer.noop() : printer;
    }
}
