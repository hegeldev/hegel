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
 * <p>The constants below are the kinds Hegel's own generators label themselves with. They share their
 * {@code hegel.<kind>} names with the other Hegel frontends, and a composite generator's label
 * combines its kind with its components' labels ({@link Generator#label}), as in every frontend.
 * Each is the fixed value {@link #of(String)} gives for its name, so reading one needs no engine.
 * Prefix custom names with your library's to keep clear of them.
 *
 * <p>{@link #of(String)} and {@link #combine(long...)} load the engine on first use, as a run does,
 * and throw a {@link dev.hegel.lowlevel.LibhegelException} if it cannot be loaded.
 */
public final class Label {
    private Label() {}

    /** A variable-length list: {@code of("hegel.lists")}. */
    public static final long LIST = 0x4ccf178a8c755675L;

    /** A set: {@code of("hegel.sets")}. */
    public static final long SET = 0x8c7bf6e4c7f30665L;

    /** A map: {@code of("hegel.dicts")}. */
    public static final long MAP = 0x68e71e450ea5be6dL;

    /** A fixed-arity tuple: {@code of("hegel.tuples")}. */
    public static final long TUPLE = 0xd8fa223d51dda37bL;

    /** A choice between alternative generators: {@code of("hegel.one_of")}. */
    public static final long ONE_OF = 0xf96229f21f87bc20L;

    /** An optional value: {@code of("hegel.optional")}. */
    public static final long OPTIONAL = 0x46ac6fb1a1222720L;

    /** A record-like structure with a fixed set of named fields: {@code of("hegel.fixed_dicts")}. */
    public static final long FIXED_DICT = 0x4588f998f6874808L;

    /** A draw whose generator depends on an earlier draw: {@code of("hegel.flat_map")}. */
    public static final long FLAT_MAP = 0xe85be60d862e687aL;

    /** A draw filtered by a predicate, combined with the label of the generator it filters: {@code of("hegel.filter")}. */
    public static final long FILTER = 0x954d8384f16a15feL;

    /** One attempt of a filtered draw, discarded when the predicate rejects its value: {@code of("hegel.filter.attempt")}. */
    public static final long FILTER_ATTEMPT = 0x8b6a04585682cd19L;

    /** A draw transformed by a function: {@code of("hegel.map")}. */
    public static final long MAPPED = 0x46ef1386be65f262L;

    /** A value sampled from a fixed collection: {@code of("hegel.sampled_from")}. */
    public static final long SAMPLED_FROM = 0xf9573be7fe320c77L;

    /** One rule invocation of a stateful test: {@code of("hegel.stateful.rule")}. */
    public static final long STATEFUL_RULE = 0xbd20e256d6f482f8L;

    /** A recursive value or one of its subtrees ({@link Generators#recursive}): {@code of("hegel.recursive")}. */
    public static final long RECURSIVE = 0xbbdcc6e73a04d5a8L;

    /** A permutation of a fixed list ({@link Generators#permutations}): {@code of("hegel.permutation")}. */
    public static final long PERMUTATION = 0xb027cceaf4155d40L;

    /** A fixed-length array ({@link Generators#arrays}): {@code of("hegel.array")}. */
    public static final long ARRAY = 0x5a6e9dc8e669fff1L;

    /** A value built imperatively from several draws ({@link Generators#composite}): {@code of("hegel.composite")}. */
    public static final long COMPOSITE = 0x28e766540c9ffb4fL;

    /** A generator defined later, for forward references ({@link Generators#deferred}): {@code of("hegel.deferred")}. */
    public static final long DEFERRED = 0xb96dc18ba2ffa9f7L;

    /** A string matching a Lark grammar ({@link Generators#fromLark}): {@code of("hegel.from_lark")}. */
    public static final long FROM_LARK = 0x8fc2df1e0331ed41L;

    /** A non-consuming draw from a stateful test's {@link Pool} ({@link Pool#reusable}): {@code of("hegel.pool.values_reusable")}. */
    public static final long POOL_VALUES_REUSABLE = 0x7f9558f7787b8b82L;

    /** A consuming draw from a stateful test's {@link Pool} ({@link Pool#consuming}): {@code of("hegel.pool.values_consumed")}. */
    public static final long POOL_VALUES_CONSUMED = 0x06d5135fb19072d1L;

    /** A generator that gives no label of its own ({@link Generator#label}): {@code of("hegel.generator")}. */
    public static final long GENERATOR = 0xb86174e143a64e31L;

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
