package dev.hegel;

import dev.hegel.lowlevel.Libhegel;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * Serves a test body's draws from an explicit {@link Example}, printing each drawn value and note
 * into a standalone engine document as a generated case's would be.
 */
final class ExplicitDraws implements AutoCloseable {
    /** The test-case operations an explicit example ignores; every other one is a usage error. */
    private static final Set<String> IGNORED = Set.of("target", "event", "eventValue");

    private final Example example;
    private final Printer printer;
    private final Set<String> used = new HashSet<>();
    private int consumed;

    ExplicitDraws(Libhegel lib, Example example) {
        this.example = example;
        this.printer = new Printer(lib, lib.printerNew());
    }

    /**
     * The engine side of an explicit test case: there is none. A call is a usage error, except for
     * the operations an explicit example ignores.
     */
    static DataSource noEngine() {
        return (DataSource) Proxy.newProxyInstance(
                DataSource.class.getClassLoader(), new Class<?>[] {DataSource.class}, (proxy, method, args) -> {
                    if (IGNORED.contains(method.getName())) {
                        return null;
                    }
                    throw new InvalidExample(method.getName() + " is not supported in an explicit example");
                });
    }

    /** The value of the draw named {@code name}, printed as {@code name = value;}. */
    synchronized Object take(String name) {
        Object value;
        if (example.named().containsKey(name)) {
            used.add(name);
            value = example.named().get(name);
        } else if (consumed < example.positional().size()) {
            value = example.positional().get(consumed++);
        } else {
            throw new InvalidExample("Explicit example " + example + " has no value for draw " + name
                    + ": give it by name with Example.with, or add a positional value");
        }
        printer.text(name + " = ");
        printer.value(value);
        printer.text(";");
        printer.hardBreak();
        return value;
    }

    /** The value of a silent draw: the example's next positional value, not printed. */
    synchronized Object takeSilent() {
        if (consumed < example.positional().size()) {
            return example.positional().get(consumed++);
        }
        throw new InvalidExample("Explicit example " + example
                + " has no value for a silent draw: a silent draw takes the next positional value");
    }

    synchronized void note(String message) {
        for (String line : message.split("\n", -1)) {
            printer.text(line);
            printer.hardBreak();
        }
    }

    /** Fail unless every value of the example was drawn. */
    synchronized void checkConsumed() {
        List<String> problems = new ArrayList<>();
        for (String name : example.named().keySet()) {
            if (!used.contains(name)) {
                problems.add("no draw is named " + name);
            }
        }
        int leftover = example.positional().size() - consumed;
        if (leftover > 0) {
            problems.add(leftover + " positional value(s) were not drawn");
        }
        if (!problems.isEmpty()) {
            throw new InvalidExample(
                    "Explicit example " + example + " was not fully used: " + String.join("; ", problems));
        }
    }

    /** The printed draws and notes, one entry per line. */
    List<String> output() {
        return printer.read().lines().toList();
    }

    @Override
    public void close() {
        printer.close();
    }
}
