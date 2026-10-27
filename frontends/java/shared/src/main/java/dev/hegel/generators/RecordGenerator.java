package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.HegelException;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.lang.reflect.RecordComponent;
import java.util.HashMap;
import java.util.Map;

/**
 * Generates instances of a record type by drawing each component from its derived (or overridden)
 * generator and invoking the canonical constructor. It prints as that constructor call, {@code
 * new Name(x, y)}, each component printed by its generator as it is drawn, so an override's own
 * printing appears inside it.
 *
 * @param <T> the record type
 */
public final class RecordGenerator<T> implements PrintableGenerator<T> {
    private final Class<T> type;
    private final Map<String, PrintableGenerator<?>> overrides;
    private final CachedLabel label = new CachedLabel();

    /**
     * @param type the record type
     * @param overrides printable generators for some of its components, by name
     */
    public RecordGenerator(Class<T> type, Map<String, PrintableGenerator<?>> overrides) {
        if (!type.isRecord()) {
            throw new IllegalArgumentException(type.getName() + " is not a record");
        }
        this.type = type;
        this.overrides = overrides;
    }

    /**
     * Override the generator used for one component. The record prints each component as its
     * generator does, so the override is printable; make a hand-written one printable with {@link
     * Generator#printWith} or {@link Generator#printAsRepr()}.
     *
     * @param component the component (field) name
     * @param generator the printable generator to use for it
     * @return a new record generator with the override applied
     */
    public RecordGenerator<T> with(String component, PrintableGenerator<?> generator) {
        boolean known = false;
        for (RecordComponent rc : type.getRecordComponents()) {
            if (rc.getName().equals(component)) {
                known = true;
                break;
            }
        }
        if (!known) {
            throw new IllegalArgumentException(type.getName() + " has no record component named '" + component + "'");
        }
        Map<String, PrintableGenerator<?>> next = new HashMap<>(overrides);
        next.put(component, generator);
        return new RecordGenerator<>(type, next);
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.FIXED_DICT, Label.of(type.getName())));
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        return draw(tc, null);
    }

    /** @hidden */
    @Override
    public T doDrawAndPrint(TestCase tc, Printer printer) {
        return draw(tc, printer);
    }

    private T draw(TestCase tc, Printer printer) {
        Printer p = Components.out(printer);
        RecordComponent[] components = type.getRecordComponents();
        Object[] values = new Object[components.length];
        Class<?>[] paramTypes = new Class<?>[components.length];
        tc.startSpan(label());
        try {
            p.beginGroup("new " + type.getSimpleName() + "(");
            for (int i = 0; i < components.length; i++) {
                RecordComponent rc = components[i];
                paramTypes[i] = rc.getType();
                PrintableGenerator<?> g = overrides.get(rc.getName());
                if (g == null) {
                    g = Derive.fromType(rc.getGenericType());
                }
                if (i > 0) {
                    p.separator();
                }
                values[i] = Components.draw(tc, g, printer);
            }
            p.endGroup(")");
        } finally {
            tc.stopSpan(false);
        }
        return construct(paramTypes, values);
    }

    private T construct(Class<?>[] paramTypes, Object[] values) {
        try {
            var ctor = type.getDeclaredConstructor(paramTypes);
            ctor.setAccessible(true);
            return ctor.newInstance(values);
        } catch (ReflectiveOperationException e) {
            throw new HegelException("Failed to construct " + type.getName(), e);
        }
    }
}
