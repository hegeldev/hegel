package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.GrammarHandle;
import dev.hegel.Label;
import dev.hegel.TestCase;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Generates strings matching a grammar written in the syntax of the <a
 * href="https://lark-parser.readthedocs.io/en/stable/grammar.html">Lark</a> parsing library. Created
 * by {@link dev.hegel.Generators#fromLark(String)}.
 *
 * <p>The grammar is checked and compiled by the engine on the first draw; a grammar Lark would
 * reject, an undefined start rule, a cap below the smallest string the grammar can produce, or an
 * {@link #explicit explicit} generator for a terminal the grammar does not use is reported then as
 * an {@link IllegalArgumentException}.
 */
public final class LarkGenerator implements Generator<String> {
    private final String grammar;
    private final String start;
    private final List<Explicit> explicit;
    private final TextGenerator alphabet;
    private final int maxSize;
    private final int maxDepth;
    private final Path importPath;
    private volatile GrammarHandle handle;

    /** A terminal whose values come from a generator of the user's rather than from the grammar. */
    private record Explicit(String name, Generator<String> generator) {}

    public LarkGenerator(String grammar) {
        this(grammar, null, List.of(), null, 0, 0, null);
    }

    private LarkGenerator(
            String grammar,
            String start,
            List<Explicit> explicit,
            TextGenerator alphabet,
            int maxSize,
            int maxDepth,
            Path importPath) {
        this.grammar = grammar;
        this.start = start;
        this.explicit = explicit;
        this.alphabet = alphabet;
        this.maxSize = maxSize;
        this.maxDepth = maxDepth;
        this.importPath = importPath;
    }

    /**
     * Generate strings of the rule named {@code rule} rather than of Lark's default start rule,
     * {@code start}.
     *
     * @param rule the start rule
     * @return a copy with the start rule set
     */
    public LarkGenerator start(String rule) {
        return new LarkGenerator(grammar, rule, explicit, alphabet, maxSize, maxDepth, importPath);
    }

    /**
     * Draw the terminal {@code name} from {@code generator} instead of from the terminal's own
     * pattern. Required for a terminal the grammar {@code %declare}s without defining; any other
     * terminal may be overridden.
     *
     * <p>The engine draws the grammar string and asks for a value for each occurrence of the
     * terminal, in order, which it splices in. The values are beyond the lexing checks the engine
     * makes on the rest of the string: it is the generator's part to draw values that read as the
     * terminal where they land (a value under a quoted-string terminal should contain no quote,
     * say). The values count toward {@link #maxSize(int) maxSize} like any other characters: a
     * string they push over it is drawn again, with less of the cap for the grammar's own text, and
     * a draw that cannot fit the values after a few tries rejects the test case.
     *
     * @param name the terminal's name
     * @param generator the generator its values come from
     * @return a copy with the terminal drawn from {@code generator}
     * @throws IllegalArgumentException if {@code name} already has a generator
     */
    public LarkGenerator explicit(String name, Generator<String> generator) {
        for (Explicit e : explicit) {
            if (e.name().equals(name)) {
                throw new IllegalArgumentException(
                        "fromLark: explicit terminal '" + name + "' is given more than one generator");
            }
        }
        List<Explicit> more = new ArrayList<>(explicit);
        more.add(new Explicit(name, generator));
        return new LarkGenerator(grammar, start, List.copyOf(more), alphabet, maxSize, maxDepth, importPath);
    }

    /**
     * Constrain which characters terminals drawn from their patterns may contain to those {@code
     * characters} can generate, as {@link RegexGenerator#alphabet(TextGenerator)} does for a single
     * pattern. Only its character restrictions matter; its sizes are ignored.
     *
     * @param characters the generator whose characters make up the alphabet
     * @return a copy with the alphabet set
     */
    public LarkGenerator alphabet(TextGenerator characters) {
        return new LarkGenerator(grammar, start, explicit, characters, maxSize, maxDepth, importPath);
    }

    /**
     * Set the maximum length in characters of the generated strings. The default is 8192, and the
     * most allowed is 1048576.
     *
     * @param maxSize the maximum length, at least 1
     * @return a copy with the maximum length set
     */
    public LarkGenerator maxSize(int maxSize) {
        if (maxSize < 1) {
            throw new IllegalArgumentException("fromLark: maxSize must be at least 1, got " + maxSize);
        }
        return new LarkGenerator(grammar, start, explicit, alphabet, maxSize, maxDepth, importPath);
    }

    /**
     * Set how deeply rules may nest in a generated string. The default is 200, and the most allowed
     * is 500.
     *
     * @param maxDepth the maximum nesting, at least 1
     * @return a copy with the maximum nesting set
     */
    public LarkGenerator maxDepth(int maxDepth) {
        if (maxDepth < 1) {
            throw new IllegalArgumentException("fromLark: maxDepth must be at least 1, got " + maxDepth);
        }
        return new LarkGenerator(grammar, start, explicit, alphabet, maxSize, maxDepth, importPath);
    }

    /**
     * Set the directory {@code %import}ed grammars are read from. {@code %import a.b.X} reads
     * {@code a/b.lark} from it, and Lark's bundled {@code common}, {@code python}, {@code unicode}
     * and {@code lark} grammars stand in when it has no such file. A relative import ({@code
     * %import .lib.X}) in {@code a/b.lark} reads {@code a/lib.lark}, and in the grammar text itself
     * reads {@code lib.lark}; no bundled grammar stands in for it, as in Lark. Unlike Lark's {@code
     * import_paths}, there is one directory and no search path.
     *
     * @param dir the directory imports are read from
     * @return a copy with the import directory set
     */
    public LarkGenerator importPath(Path dir) {
        return new LarkGenerator(grammar, start, explicit, alphabet, maxSize, maxDepth, dir);
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.FROM_LARK;
    }

    /** @hidden */
    @Override
    public String doDraw(TestCase tc) {
        return tc.generateGrammar(
                compiled(tc), i -> tc.drawSilent(explicit.get(i).generator()));
    }

    private GrammarHandle compiled(TestCase tc) {
        GrammarHandle h = handle;
        if (h == null || !tc.ownsGrammar(h)) {
            h = tc.grammar(
                    grammar,
                    start,
                    maxSize,
                    maxDepth,
                    alphabet == null ? null : alphabet.alphabetHandle(tc),
                    explicit.stream().map(Explicit::name).toList(),
                    this::readImport);
            handle = h;
        }
        return h;
    }

    /** The text of the imported file at {@code path}, or {@code null} when there is none. */
    private String readImport(String path) {
        if (importPath == null) {
            return null;
        }
        Path file = importPath.resolve(path);
        if (!Files.isRegularFile(file)) {
            return null;
        }
        try {
            return Files.readString(file);
        } catch (IOException e) {
            throw new IllegalArgumentException("fromLark: could not read grammar " + file + ": " + e, e);
        }
    }
}
