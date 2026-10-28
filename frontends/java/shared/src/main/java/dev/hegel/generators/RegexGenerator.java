package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.TestCase;

/**
 * Generates strings matching a (Python-compatible) regular expression. By default the entire
 * string matches the pattern; use {@link #fullmatch(boolean) fullmatch(false)} to generate strings
 * that merely contain a match. The pattern is validated by the engine when the first value is
 * drawn.
 */
public final class RegexGenerator extends ValueGenerator<String> {
    private final String pattern;
    private final boolean fullmatch;
    private final TextGenerator alphabet;
    private final HandleCache cache = new HandleCache();

    public RegexGenerator(String pattern, boolean fullmatch) {
        this(pattern, fullmatch, null);
    }

    private RegexGenerator(String pattern, boolean fullmatch, TextGenerator alphabet) {
        this.pattern = pattern;
        this.fullmatch = fullmatch;
        this.alphabet = alphabet;
    }

    /**
     * @param fullmatch whether the entire string must match the pattern (the default), or merely
     *     contain a match somewhere within it
     * @return a copy with the fullmatch behaviour set
     */
    public RegexGenerator fullmatch(boolean fullmatch) {
        return new RegexGenerator(pattern, fullmatch, alphabet);
    }

    /**
     * Constrain which characters may appear in generated strings, padding included, to those
     * {@code characters} can generate, for example {@code characters().codec("ascii")}. Only its
     * character restrictions matter; its sizes are ignored.
     *
     * @param characters the generator whose characters make up the alphabet
     * @return a copy with the alphabet set
     */
    public RegexGenerator alphabet(TextGenerator characters) {
        return new RegexGenerator(pattern, fullmatch, characters);
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.of("hegel.from_regex");
    }

    /** @hidden */
    @Override
    public String doDraw(TestCase tc) {
        return tc.generateString(cache.get(
                tc, t -> t.regexGenerator(pattern, fullmatch, alphabet == null ? null : alphabet.alphabetHandle(t))));
    }
}
