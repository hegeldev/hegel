package dev.hegel;

/**
 * Span labels for {@link TestCase#span(long, java.util.function.Supplier)} and {@link
 * TestCase#startSpan(long)}.
 *
 * <p>A label identifies the generator that opened a span, and has no meaning beyond identity: the
 * engine treats two spans with the same label as coming from the same generator — candidates for
 * swapping, duplicating and reordering with each other when it shrinks and mutates test cases — and
 * does nothing else with it. Any 64-bit value works as long as the same generator always uses the
 * same one. Derive a label from a name with {@link #of(String)}, and give a generator built from
 * other generators a label {@link #combine(long...) combined} from its own and its components', so
 * that a list of integers and a list of strings get different labels while every list of integers
 * gets the same one. Both are computed by the engine ({@code hegel_label_from_name} and {@code
 * hegel_label_combine}), so labels minted here agree with those of every other Hegel frontend.
 *
 * <p>The constants below are the labels Hegel's own composite generators open their spans with. The
 * standard ones share their {@code hegel.<kind>} names with the other Hegel frontends, so a list or
 * a {@code oneOf} is labelled the same in every language; the ones only Java has are named {@code
 * dev.hegel.<kind>}. Prefix custom names with your library's to keep clear of both.
 */
public final class Label {
    private Label() {}

    /** A variable-length list. */
    public static final long LIST = of("hegel.lists");

    /** One element of a list. */
    public static final long LIST_ELEMENT = of("dev.hegel.list_element");

    /** A set. */
    public static final long SET = of("hegel.sets");

    /** One element of a set. */
    public static final long SET_ELEMENT = of("dev.hegel.set_element");

    /** A map. */
    public static final long MAP = of("hegel.dicts");

    /** One key/value entry of a map. */
    public static final long MAP_ENTRY = of("dev.hegel.map_entry");

    /** A fixed-arity tuple. */
    public static final long TUPLE = of("hegel.tuple");

    /** A choice between alternative generators. */
    public static final long ONE_OF = of("hegel.one_of");

    /** An optional value. */
    public static final long OPTIONAL = of("hegel.optional");

    /** A record-like structure with a fixed set of named fields. */
    public static final long FIXED_DICT = of("hegel.fixed_dicts");

    /** A draw whose generator depends on an earlier draw. */
    public static final long FLAT_MAP = of("hegel.flat_map");

    /** A draw filtered by a predicate. */
    public static final long FILTER = of("hegel.filter");

    /** A draw transformed by a function. */
    public static final long MAPPED = of("hegel.map");

    /** A value sampled from a fixed collection. */
    public static final long SAMPLED_FROM = of("hegel.sampled_from");

    /** One variant of an enum-like type. */
    public static final long ENUM_VARIANT = of("dev.hegel.enum_variant");

    /** One rule invocation of a stateful test. */
    public static final long STATEFUL_RULE = of("hegel.stateful.rule");

    /** A value built imperatively from several draws ({@link Generators#composite}). */
    public static final long COMPOSITE = of("hegel.composite");

    /**
     * Mint a stable label from a name: the engine's {@code hegel_label_from_name}, the 64-bit FNV-1a
     * hash of its UTF-8 bytes. The same name always yields the same label, so a frontend can label
     * its generators by fully-qualified name.
     *
     * @param name the label's name
     * @return the label
     */
    public static long of(String name) {
        return Engine.real().labelFromName(name);
    }

    /**
     * The label for a generator built from other generators: the engine's {@code
     * hegel_label_combine}, a hash of the given labels, in order. Pass the generator's own label
     * (from {@link #of(String)}) first and its components' labels after it. Combining is
     * order-sensitive, and combining a single label does not return it unchanged.
     *
     * @param labels the generator's own label followed by its components'
     * @return the combined label
     */
    public static long combine(long... labels) {
        return Engine.real().labelCombine(labels);
    }
}
