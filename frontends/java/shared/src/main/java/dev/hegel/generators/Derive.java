package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Generators;
import dev.hegel.HegelException;
import dev.hegel.PrintableGenerator;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.RecordComponent;
import java.lang.reflect.Type;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.List;
import java.util.stream.Collectors;

/**
 * Reflection-based generator derivation. Maps a Java type to a default generator: scalar wrappers
 * to their primitive generators, {@code OptionalInt}/{@code OptionalLong}/{@code OptionalDouble} to
 * optionals over them, enums to a choice over their constants, records to a generator over their
 * components, and {@code List}/{@code Set}/{@code Optional}/{@code Map} to the matching collection
 * generators (using the declared element types).
 *
 * <p>A record whose components refer back to it, directly or through other records and
 * collections, has no derived generator: deriving one would recurse without bound. Such a type is
 * rejected with a {@link HegelException} naming it; build its generator with {@link
 * Generators#recursive} or {@link Generators#deferred()}, or override the recursive component with
 * {@link RecordGenerator#with}.
 */
public final class Derive {
    private Derive() {}

    /** Erase a generator's element type; the derived value is consumed as {@code Object}. */
    @SuppressWarnings("unchecked")
    private static PrintableGenerator<Object> erase(Generator<?> g) {
        return (PrintableGenerator<Object>) g;
    }

    public static PrintableGenerator<Object> fromType(Type type) {
        if (type instanceof ParameterizedType pt) {
            return parameterized(pt);
        }
        if (type instanceof Class<?> cls) {
            return fromClass(cls);
        }
        throw new HegelException("Cannot derive a generator for type: " + type);
    }

    private static PrintableGenerator<Object> fromClass(Class<?> cls) {
        Generator<?> g = scalar(cls);
        if (g != null) {
            return erase(g);
        }
        if (cls.isEnum()) {
            return erase(Generators.sampledFrom(List.of(cls.getEnumConstants())));
        }
        if (cls.isRecord()) {
            checkAcyclic(cls, new ArrayDeque<>());
            return erase(new RecordGenerator<>(cls, java.util.Map.of()));
        }
        throw new HegelException("No default generator for "
                + cls.getName()
                + "; supply one explicitly (e.g. a Generator parameter or a record override).");
    }

    /**
     * Walks the component types of {@code record}, and of the records and collections they hold,
     * and throws when a record on {@code path} comes up again.
     */
    private static void checkAcyclic(Class<?> record, Deque<Class<?>> path) {
        if (path.contains(record)) {
            String cycle = path.stream().map(Class::getSimpleName).collect(Collectors.joining(" -> "));
            throw new HegelException("Cannot derive a generator for " + record.getName()
                    + ": it refers to itself through its components (" + cycle + " -> "
                    + record.getSimpleName() + "), so deriving one would recurse without bound. Build its"
                    + " generator with Generators.recursive or Generators.deferred, or give the recursive"
                    + " component a generator with records(" + record.getSimpleName() + ".class).with(...)");
        }
        path.addLast(record);
        for (RecordComponent rc : record.getRecordComponents()) {
            checkAcyclic(rc.getGenericType(), path);
        }
        path.removeLast();
    }

    private static void checkAcyclic(Type type, Deque<Class<?>> path) {
        if (type instanceof ParameterizedType pt) {
            for (Type arg : pt.getActualTypeArguments()) {
                checkAcyclic(arg, path);
            }
        } else if (type instanceof Class<?> cls && cls.isRecord()) {
            checkAcyclic(cls, path);
        }
    }

    private static Generator<?> scalar(Class<?> cls) {
        if (cls == int.class || cls == Integer.class) {
            return Generators.integers();
        }
        if (cls == long.class || cls == Long.class) {
            return Generators.longs();
        }
        if (cls == boolean.class || cls == Boolean.class) {
            return Generators.booleans();
        }
        if (cls == float.class || cls == Float.class) {
            return Generators.floats();
        }
        if (cls == double.class || cls == Double.class) {
            return Generators.doubles();
        }
        if (cls == java.util.OptionalInt.class) {
            return Generators.optionalInts(Generators.integers());
        }
        if (cls == java.util.OptionalLong.class) {
            return Generators.optionalLongs(Generators.longs());
        }
        if (cls == java.util.OptionalDouble.class) {
            return Generators.optionalDoubles(Generators.doubles());
        }
        if (cls == java.math.BigInteger.class) {
            return Generators.bigIntegers();
        }
        if (cls == String.class) {
            return Generators.text();
        }
        if (cls == java.util.UUID.class) {
            return Generators.uuids();
        }
        if (cls == byte[].class) {
            return Generators.binary();
        }
        if (cls == java.time.Duration.class) {
            return Generators.durations();
        }
        if (cls == java.time.LocalDate.class) {
            return Generators.dates();
        }
        if (cls == java.time.LocalTime.class) {
            return Generators.times();
        }
        if (cls == java.time.LocalDateTime.class) {
            return Generators.datetimes();
        }
        if (cls == java.time.OffsetDateTime.class) {
            return Generators.datetimes().offsets(Generators.zoneOffsets());
        }
        if (cls == java.time.ZonedDateTime.class) {
            return Generators.datetimes().timezones(Generators.zoneIds());
        }
        if (cls == java.time.ZoneOffset.class) {
            return Generators.zoneOffsets();
        }
        if (cls == java.time.ZoneId.class) {
            return Generators.zoneIds();
        }
        return null;
    }

    private static PrintableGenerator<Object> parameterized(ParameterizedType pt) {
        Class<?> raw = (Class<?>) pt.getRawType();
        Type[] args = pt.getActualTypeArguments();
        if (raw == List.class) {
            return erase(Generators.lists(fromType(args[0])));
        }
        if (raw == java.util.Set.class) {
            return erase(Generators.sets(fromType(args[0])));
        }
        if (raw == java.util.Optional.class) {
            return erase(Generators.optional(fromType(args[0])));
        }
        if (raw == java.util.Map.class) {
            return erase(Generators.maps(fromType(args[0]), fromType(args[1])));
        }
        throw new HegelException("Cannot derive a generator for generic type: " + pt);
    }
}
