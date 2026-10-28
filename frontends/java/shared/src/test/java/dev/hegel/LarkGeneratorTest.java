package dev.hegel;

import static dev.hegel.Generators.characters;
import static dev.hegel.Generators.fromLark;
import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.text;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.generators.LarkGenerator;
import dev.hegel.lowlevel.Abi;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.regex.Pattern;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class LarkGeneratorTest {
    private static final Settings SETTINGS = new Settings().database(Database.disabled());

    private static final String ARITHMETIC = """
            ?start: sum
            ?sum: product | sum "+" product
            ?product: atom | product "*" atom
            ?atom: NUMBER | "(" sum ")"
            %import common.NUMBER
            %import common.WS
            %ignore WS
            """;

    private static void assertMatches(String regex, String value) {
        assertTrue(Pattern.matches(regex, value), () -> value + " does not match " + regex);
    }

    @Test
    void stringsMatchTheGrammar() {
        Hegel.test(tc -> assertMatches("a+b", tc.draw(fromLark("start: A+ \"b\"\nA: \"a\""))), SETTINGS);
    }

    @Test
    void bundledImportsStandInForMissingFiles() {
        Hegel.test(
                tc -> {
                    String expression = tc.draw(fromLark(ARITHMETIC).maxSize(100));
                    assertMatches("[0-9eE.+*()\\s-]*", expression);
                    assertTrue(expression.length() <= 100, expression);
                },
                SETTINGS);
    }

    @Test
    void startSelectsTheRule() {
        Hegel.test(
                tc -> assertEquals(
                        "y", tc.draw(fromLark("start: \"x\"\nother: \"y\"").start("other"))),
                SETTINGS);
    }

    @Test
    void sizeAndDepthBoundTheStrings() {
        Hegel.test(
                tc -> {
                    assertTrue(tc.draw(fromLark("start: \"a\"*").maxSize(5)).length() <= 5);
                    String nested =
                            tc.draw(fromLark("start: \"(\" start \")\" | \"x\"").maxDepth(3));
                    assertMatches("\\(*x\\)*", nested);
                    assertTrue(nested.chars().filter(c -> c == '(').count() < 3, nested);
                },
                SETTINGS);
    }

    @Test
    void theAlphabetConstrainsPatternTerminals() {
        Hegel.test(
                tc -> assertMatches(
                        "[a-c]+",
                        tc.draw(fromLark("start: /[a-z]+/")
                                .alphabet(characters().codepoints('a', 'c')))),
                SETTINGS);
    }

    @Test
    void explicitTerminalsTakeTheirValuesFromGenerators() {
        LarkGenerator names = fromLark("start: NAME (\",\" NAME)*\n%declare NAME")
                .explicit("NAME", text().alphabet("xyz").minSize(1).maxSize(3))
                .maxSize(50);
        boolean[] sawSeveral = {false};
        Hegel.test(
                tc -> {
                    String value = tc.draw(names);
                    assertMatches("[xyz]{1,3}(,[xyz]{1,3})*", value);
                    assertTrue(value.length() <= 50, value);
                    sawSeveral[0] |= value.contains(",");
                },
                SETTINGS);
        assertTrue(sawSeveral[0]);
    }

    @Test
    void explicitTerminalsCanOverrideAPattern() {
        Hegel.test(
                tc -> assertMatches(
                        "<[0-9]+>",
                        tc.draw(fromLark("start: \"<\" DIGITS \">\"\nDIGITS: /[a-z]+/")
                                .explicit("DIGITS", integers().min(0).max(999).map(String::valueOf)))),
                SETTINGS);
    }

    @Test
    void failuresShrinkThroughTheGrammar() {
        RunReport report = Hegel.run(
                tc -> assertTrue(tc.draw(fromLark("start: A+\nA: \"a\""), "s").length() < 3),
                SETTINGS.derandomize(true),
                Reporter.silent());
        assertEquals("aaa", report.failures().get(0).draws().get("s_1"));
    }

    @Test
    void valuesThatNeverFitRejectEveryCase() {
        LarkGenerator tooLong = fromLark("start: NAME\n%declare NAME")
                .explicit("NAME", text().alphabet("x").minSize(10).maxSize(10))
                .maxSize(5);
        RunReport report = Hegel.run(tc -> tc.draw(tooLong), SETTINGS, Reporter.silent());
        assertEquals(RunStatus.ERROR, report.status());
    }

    @Test
    void aRejectedTerminalValueRejectsTheCase() {
        LarkGenerator never = fromLark("start: NAME\n%declare NAME").explicit("NAME", text().filter(s -> false));
        RunReport report = Hegel.run(tc -> tc.draw(never), SETTINGS, Reporter.silent());
        assertEquals(RunStatus.ERROR, report.status());
    }

    @Test
    void builderArgumentsAreCheckedEagerly() {
        LarkGenerator g = fromLark("start: \"a\"");
        assertThrows(IllegalArgumentException.class, () -> g.maxSize(0));
        assertThrows(IllegalArgumentException.class, () -> g.maxDepth(0));
        LarkGenerator named = g.explicit("A", text());
        IllegalArgumentException e = assertThrows(IllegalArgumentException.class, () -> named.explicit("A", text()));
        assertTrue(e.getMessage().contains("more than one generator"), e.getMessage());
        named.explicit("B", text());
    }

    @Test
    void invalidGrammarsAreReportedOnTheFirstDraw() {
        Hegel.test(
                tc -> {
                    IllegalArgumentException undefined = assertThrows(
                            IllegalArgumentException.class, () -> tc.draw(fromLark("start: undefined_rule")));
                    assertTrue(undefined.getMessage().contains("undefined_rule"), undefined.getMessage());
                    assertThrows(IllegalArgumentException.class, () -> tc.draw(fromLark("start: (")));
                    assertThrows(
                            IllegalArgumentException.class,
                            () -> tc.draw(fromLark("start: \"a\"").explicit("NOPE", text())));
                    assertThrows(
                            IllegalArgumentException.class, () -> tc.draw(fromLark("%import .missing.X\nstart: X")));
                },
                SETTINGS.testCases(1));
    }

    @Test
    void importsAreReadFromTheImportPath(@TempDir Path dir) throws IOException {
        Files.createDirectories(dir.resolve("lib"));
        Files.writeString(dir.resolve("lib/defs.lark"), "WORD: \"hello\"\n");
        Files.createDirectories(dir.resolve("common.lark"));
        LarkGenerator g = fromLark("%import lib.defs.WORD\n%import common.INT\nstart: WORD INT")
                .importPath(dir);
        Hegel.test(tc -> assertMatches("hello[0-9]+", tc.draw(g)), SETTINGS);
    }

    @Test
    void anUnreadableImportIsAnInvalidArgument(@TempDir Path dir) throws IOException {
        Files.write(dir.resolve("bad.lark"), new byte[] {(byte) 0xff, (byte) 0xfe});
        LarkGenerator g = fromLark("%import bad.X\nstart: X").importPath(dir);
        Hegel.test(
                tc -> {
                    IllegalArgumentException e = assertThrows(IllegalArgumentException.class, () -> tc.draw(g));
                    assertTrue(e.getMessage().contains("could not read grammar"), e.getMessage());
                },
                SETTINGS.testCases(1));
    }

    @Test
    void theCompiledGrammarIsCachedPerBinding() {
        LarkGenerator g = fromLark("start: \"a\"");
        FakeLibhegel first = new FakeLibhegel();
        TestCase tc = new TestCase(new LiveDataSource(first, FakeLibhegel.TC), false);
        tc.draw(g);
        tc.draw(g);
        assertEquals(1, first.grammarsBuilt);
        assertEquals(2, first.freedGrammarDraws);
        FakeLibhegel second = new FakeLibhegel();
        new TestCase(new LiveDataSource(second, FakeLibhegel.TC), false).draw(g);
        assertEquals(1, second.grammarsBuilt);
    }

    @Test
    void anAbortedCaseDrawsNothing() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.grammarDrawNextRc = Abi.E_STOP_TEST;
        TestCase tc = new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false);
        LarkGenerator g = fromLark("start: \"a\"");
        assertThrows(StopTest.class, () -> tc.draw(g));
        assertEquals(1, fake.freedGrammarDraws);
        assertThrows(StopTest.class, () -> tc.draw(g));
        assertThrows(StopTest.class, () -> tc.draw(fromLark("start: \"b\"")));
        assertEquals(1, fake.freedGrammarDraws);
    }
}
