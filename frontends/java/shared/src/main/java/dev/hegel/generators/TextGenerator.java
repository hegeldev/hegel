package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.StringGeneratorHandle;
import dev.hegel.TestCase;
import dev.hegel.lowlevel.Abi;
import java.util.ArrayList;
import java.util.List;

/**
 * Generates strings with fine-grained control over length and character selection.
 *
 * <p>Surrogate codepoints (Unicode category {@code Cs}) are excluded by default so generated
 * strings round-trip cleanly through Java; request specific categories to override the default
 * exclusion.
 *
 * <p>Lengths default to {@code [0, 100]} characters (or {@code [minSize, minSize + 100]} for a
 * larger minimum); set an explicit {@link #maxSize(int)} for longer strings.
 *
 * <p>Characters come either from a fixed {@link #alphabet(String) alphabet} or from the
 * restrictions the other methods set ({@link #codec(String) codec}, {@link #codepoints(int, int)
 * codepoints}, {@link #categories(String...) categories} and so on); the two cannot be combined.
 */
public final class TextGenerator implements Generator<String> {
    /** The default length cap when no explicit {@code maxSize} is set. */
    static final long DEFAULT_MAX_SIZE = 100;

    /** The codecs {@link #codec(String)} accepts, as the engine names them. */
    private static final List<String> CODECS = List.of("ascii", "latin-1", "iso-8859-1", "utf-8");

    private final long minSize;
    private final long maxSize;
    private final String codec;
    private final Integer minCodepoint;
    private final Integer maxCodepoint;
    private final List<String> categories;
    private final List<String> excludeCategories;
    private final String includeChars;
    private final String excludeChars;
    private final String alphabet;
    private final HandleCache cache = new HandleCache();

    public TextGenerator(
            long minSize,
            long maxSize,
            Integer minCodepoint,
            Integer maxCodepoint,
            List<String> categories,
            List<String> excludeCategories,
            String includeChars,
            String excludeChars) {
        this(
                minSize,
                maxSize,
                null,
                minCodepoint,
                maxCodepoint,
                categories,
                excludeCategories,
                includeChars,
                excludeChars,
                null);
    }

    private TextGenerator(
            long minSize,
            long maxSize,
            String codec,
            Integer minCodepoint,
            Integer maxCodepoint,
            List<String> categories,
            List<String> excludeCategories,
            String includeChars,
            String excludeChars,
            String alphabet) {
        Sizes.validate(minSize, maxSize, "text");
        if (alphabet != null
                && (codec != null
                        || minCodepoint != null
                        || maxCodepoint != null
                        || categories != null
                        || excludeCategories != null
                        || includeChars != null
                        || excludeChars != null)) {
            throw new IllegalArgumentException("text: an alphabet cannot be combined with character restrictions");
        }
        this.minSize = minSize;
        this.maxSize = maxSize;
        this.codec = codec;
        this.minCodepoint = minCodepoint;
        this.maxCodepoint = maxCodepoint;
        this.categories = validateCategories(categories);
        this.excludeCategories = excludeCategories;
        this.includeChars = includeChars;
        this.excludeChars = excludeChars;
        this.alphabet = alphabet;
    }

    private static List<String> validateCategories(List<String> categories) {
        if (categories != null) {
            for (String c : categories) {
                if (c.equals("Cs") || c.equals("C")) {
                    throw new IllegalArgumentException(
                            "text: category \"" + c + "\" includes surrogate codepoints, unsupported");
                }
            }
        }
        return categories;
    }

    private TextGenerator with(
            long minSize,
            long maxSize,
            String codec,
            Integer minCodepoint,
            Integer maxCodepoint,
            List<String> categories,
            List<String> excludeCategories,
            String includeChars,
            String excludeChars,
            String alphabet) {
        return new TextGenerator(
                minSize,
                maxSize,
                codec,
                minCodepoint,
                maxCodepoint,
                categories,
                excludeCategories,
                includeChars,
                excludeChars,
                alphabet);
    }

    /**
     * @param minSize the minimum codepoint length
     * @return a copy with the minimum size set
     */
    public TextGenerator minSize(int minSize) {
        return with(
                minSize,
                maxSize,
                codec,
                minCodepoint,
                maxCodepoint,
                categories,
                excludeCategories,
                includeChars,
                excludeChars,
                alphabet);
    }

    /**
     * @param maxSize the maximum codepoint length
     * @return a copy with the maximum size set
     */
    public TextGenerator maxSize(int maxSize) {
        return with(
                minSize,
                maxSize,
                codec,
                minCodepoint,
                maxCodepoint,
                categories,
                excludeCategories,
                includeChars,
                excludeChars,
                alphabet);
    }

    /**
     * Use only the characters of {@code chars}: every character of a generated string is one of
     * them. Cannot be combined with the other character restrictions.
     *
     * @param chars the alphabet
     * @return a copy drawing from the alphabet
     */
    public TextGenerator alphabet(String chars) {
        return with(
                minSize,
                maxSize,
                codec,
                minCodepoint,
                maxCodepoint,
                categories,
                excludeCategories,
                includeChars,
                excludeChars,
                chars);
    }

    /**
     * Restrict to characters the named codec can encode: {@code "ascii"} (codepoints up to {@code
     * U+007F}), {@code "latin-1"} or its alias {@code "iso-8859-1"} (up to {@code U+00FF}), or
     * {@code "utf-8"} (no restriction). The codec's range intersects with any {@link
     * #codepoints(int, int) codepoint range}.
     *
     * @param codec the codec's name
     * @return a copy with the codec set
     * @throws IllegalArgumentException for any other codec name
     */
    public TextGenerator codec(String codec) {
        if (!CODECS.contains(codec)) {
            throw new IllegalArgumentException("text: unsupported codec \"" + codec + "\"; expected one of " + CODECS);
        }
        return with(
                minSize,
                maxSize,
                codec,
                minCodepoint,
                maxCodepoint,
                categories,
                excludeCategories,
                includeChars,
                excludeChars,
                alphabet);
    }

    /**
     * Restrict to the inclusive Unicode codepoint range {@code [min, max]}.
     *
     * @param min the minimum codepoint
     * @param max the maximum codepoint
     * @return a copy with the codepoint range set
     */
    public TextGenerator codepoints(int min, int max) {
        if (min > max) {
            throw new IllegalArgumentException("text: minCodepoint > maxCodepoint");
        }
        return with(
                minSize, maxSize, codec, min, max, categories, excludeCategories, includeChars, excludeChars, alphabet);
    }

    /**
     * Restrict to the listed Unicode general categories (e.g. {@code "Lu"}, {@code "Nd"}).
     *
     * @param cats the allowed categories
     * @return a copy with the categories set
     */
    public TextGenerator categories(String... cats) {
        return with(
                minSize,
                maxSize,
                codec,
                minCodepoint,
                maxCodepoint,
                List.of(cats),
                excludeCategories,
                includeChars,
                excludeChars,
                alphabet);
    }

    /**
     * @param cats categories to exclude
     * @return a copy with excluded categories set
     */
    public TextGenerator excludeCategories(String... cats) {
        return with(
                minSize,
                maxSize,
                codec,
                minCodepoint,
                maxCodepoint,
                categories,
                List.of(cats),
                includeChars,
                excludeChars,
                alphabet);
    }

    /**
     * @param chars characters always eligible for inclusion
     * @return a copy with the include set
     */
    public TextGenerator includeCharacters(String chars) {
        return with(
                minSize,
                maxSize,
                codec,
                minCodepoint,
                maxCodepoint,
                categories,
                excludeCategories,
                chars,
                excludeChars,
                alphabet);
    }

    /**
     * @param chars characters never included
     * @return a copy with the exclude set
     */
    public TextGenerator excludeCharacters(String chars) {
        return with(
                minSize,
                maxSize,
                codec,
                minCodepoint,
                maxCodepoint,
                categories,
                excludeCategories,
                includeChars,
                chars,
                alphabet);
    }

    /** @hidden */
    @Override
    public String doDraw(TestCase tc) {
        return tc.generateString(
                cache.get(tc, t -> buildHandle(t, minSize, Sizes.resolveMax(minSize, maxSize, DEFAULT_MAX_SIZE))));
    }

    /** A handle over this generator's characters alone, as a regex's alphabet. */
    StringGeneratorHandle alphabetHandle(TestCase tc) {
        return buildHandle(tc, 0, 0);
    }

    private StringGeneratorHandle buildHandle(TestCase tc, long min, long max) {
        if (alphabet != null) {
            return tc.textGenerator(min, max, null, 0, Abi.NO_MAX_CODEPOINT, List.of(), null, alphabet, null);
        }
        List<String> exclude;
        if (categories != null) {
            exclude = null;
        } else {
            // Surrogates are excluded by default so drawn strings are valid Java strings.
            exclude = new ArrayList<>(excludeCategories == null ? List.of() : excludeCategories);
            if (!exclude.contains("Cs")) {
                exclude.add("Cs");
            }
        }
        return tc.textGenerator(
                min,
                max,
                codec,
                minCodepoint == null ? 0 : minCodepoint,
                maxCodepoint == null ? Abi.NO_MAX_CODEPOINT : maxCodepoint,
                categories,
                exclude,
                includeChars,
                excludeChars);
    }
}
