package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.HegelException;
import dev.hegel.Label;
import dev.hegel.PrettyPrinter;
import dev.hegel.PrintableGenerator;
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
    private final Map<String, Generator<?>> overrides;
    private final CachedLabel label = new CachedLabel();
    private volatile Generator<?>[] components;

    public RecordGenerator(Class<T> type, Map<String, Generator<?>> overrides) {
        if (!type.isRecord()) {
            throw new IllegalArgumentException(type.getName() + " is not a record");
        }
        this.type = type;
        this.overrides = overrides;
    }

    /**
     * Override the generator used for one component.
     *
     * @param component the component (field) name
     * @param generator the generator to use for it
     * @return a new record generator with the override applied
     */
    public RecordGenerator<T> with(String component, Generator<?> generator) {
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
        Map<String, Generator<?>> next = new HashMap<>(overrides);
        next.put(component, generator);
        return new RecordGenerator<>(type, next);
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> {
            Generator<?>[] generators = components();
            long[] labels = new long[generators.length + 2];
            labels[0] = Label.FIXED_DICT;
            labels[1] = Label.of(type.getName());
            for (int i = 0; i < generators.length; i++) {
                labels[i + 2] = generators[i].label();
            }
            return Label.combine(labels);
        });
    }

    /**
     * The generator of each component, in declaration order: the override where there is one, and
     * otherwise the generator derived from the component's type, resolved on first use and kept.
     */
    private Generator<?>[] components() {
        Generator<?>[] resolved = components;
        if (resolved == null) {
            RecordComponent[] declared = type.getRecordComponents();
            resolved = new Generator<?>[declared.length];
            for (int i = 0; i < declared.length; i++) {
                Generator<?> g = overrides.get(declared[i].getName());
                resolved[i] = g == null ? Derive.fromType(declared[i].getGenericType()) : g;
            }
            components = resolved;
        }
        return resolved;
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        return draw(tc, null);
    }

    /** @hidden */
    @Override
    public T doDrawAndPrint(TestCase tc, PrettyPrinter printer) {
        return draw(tc, printer);
    }

    private T draw(TestCase tc, PrettyPrinter printer) {
        PrettyPrinter p = Components.out(printer);
        RecordComponent[] declared = type.getRecordComponents();
        Generator<?>[] generators = components();
        Object[] values = new Object[declared.length];
        Class<?>[] paramTypes = new Class<?>[declared.length];
        p.beginGroup("new " + type.getSimpleName() + "(");
        for (int i = 0; i < declared.length; i++) {
            paramTypes[i] = declared[i].getType();
            if (i > 0) {
                p.separator();
            }
            values[i] = Components.draw(tc, generators[i], printer);
        }
        p.endGroup(")");
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
