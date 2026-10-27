package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;

import dev.hegel.lowlevel.Libhegel;
import java.io.File;
import java.net.URI;
import java.net.URISyntaxException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * {@link TestCase#draw} takes only a {@link PrintableGenerator}, so drawing a generator that cannot
 * print fails to compile. Each snippet is compiled against the frontend's own classes.
 */
class PrintabilityCompilationTest {
    @TempDir
    Path out;

    private static final String HEADER = """
            import static dev.hegel.Generators.*;
            import dev.hegel.*;
            import dev.hegel.generators.*;
            import java.util.*;

            record Point(int x, int y) {}

            class Snippet {
                static void body(TestCase tc, Generator<Integer> silent) {
            """;

    private static final String FOOTER = """
                }
            }
            """;

    private static String classpath(Class<?>... classes) throws URISyntaxException {
        List<String> entries = new ArrayList<>();
        for (Class<?> c : classes) {
            entries.add(Path.of(c.getProtectionDomain()
                            .getCodeSource()
                            .getLocation()
                            .toURI())
                    .toString());
        }
        return String.join(File.pathSeparator, entries);
    }

    private List<Diagnostic<? extends JavaFileObject>> compile(String body) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        assertNotNull(compiler);
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        JavaFileObject source =
                new SimpleJavaFileObject(URI.create("string:///Snippet.java"), JavaFileObject.Kind.SOURCE) {
                    @Override
                    public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                        return HEADER + body + FOOTER;
                    }
                };
        Path classes = Files.createTempDirectory(out, "classes");
        try (StandardJavaFileManager files = compiler.getStandardFileManager(diagnostics, Locale.ROOT, null)) {
            compiler.getTask(
                            null,
                            files,
                            diagnostics,
                            List.of(
                                    "-proc:none",
                                    "-classpath",
                                    classpath(Generator.class, Libhegel.class),
                                    "-d",
                                    classes.toString()),
                            null,
                            List.of(source))
                    .call();
        }
        return diagnostics.getDiagnostics().stream()
                .filter(d -> d.getKind() == Diagnostic.Kind.ERROR)
                .toList();
    }

    private void assertCompiles(String body) throws Exception {
        assertEquals(
                List.of(),
                compile(body).stream().map(d -> d.getMessage(Locale.ROOT)).toList(),
                body);
    }

    private void assertDoesNotCompile(String body) throws Exception {
        assertFalse(compile(body).isEmpty(), body);
    }

    @Test
    void printableGeneratorsCanBeDrawn() throws Exception {
        assertCompiles("""
                tc.draw(integers());
                tc.draw(lists(integers()).unique().minSize(1).maxSize(3), "xs");
                tc.draw(sets(text()).maxSize(2).filter(s -> true));
                tc.draw(maps(integers(), booleans()).minSize(1));
                tc.draw(optional(integers()));
                tc.draw(arrays(integers(), 2, Integer[]::new));
                tc.draw(tuples(integers(), text(), booleans()));
                tc.draw(oneOf(integers(), just(1)));
                tc.draw(integers().flatMap(n -> lists(integers()).minSize(n)));
                tc.draw(integers().map(n -> n + 1).printAsCall("inc"));
                tc.draw(silent.map(n -> n + 1));
                tc.draw(silent.printAsRepr());
                tc.draw(silent.printWith((v, p) -> p.text("v")));
                tc.draw(records(Point.class).with("x", integers().min(0)));
                tc.draw(forType(Point.class));
                tc.draw(recursive(integers(), sub -> lists(sub).maxSize(2).map(l -> l.size())).maxLeaves(5));
                tc.draw((PrintableGenerator<Integer>) (t, p) -> t.drawAndPrint(integers(), p));
                Deferred<Integer> forward = deferred();
                forward.set(integers());
                tc.draw(forward);
                tc.drawSilent(silent);
                tc.drawSilent(lists(silent).unique().minSize(1));
                tc.drawSilent(recursive(silent, sub -> tc2 -> tc2.drawSilent(sub)).maxDepth(2));
                """);
    }

    @Test
    void anUnprintableGeneratorCannotBeDrawn() throws Exception {
        assertDoesNotCompile("tc.draw(silent);");
        assertDoesNotCompile("tc.draw(silent, \"x\");");
        assertDoesNotCompile("tc.draw(t -> 1);");
        assertDoesNotCompile("tc.drawAndPrint(silent, Printer.noop());");
        assertDoesNotCompile("Generator<Integer> upcast = integers(); tc.draw(upcast);");
    }

    @Test
    void aStructuralGeneratorOverUnprintablePartsCannotBeDrawn() throws Exception {
        assertDoesNotCompile("tc.draw(lists(silent));");
        assertDoesNotCompile("tc.draw(lists(silent).unique().minSize(1));");
        assertDoesNotCompile("tc.draw(sets(silent).maxSize(2));");
        assertDoesNotCompile("tc.draw(maps(integers(), silent));");
        assertDoesNotCompile("tc.draw(maps(silent, integers()).minSize(1));");
        assertDoesNotCompile("tc.draw(optional(silent));");
        assertDoesNotCompile("tc.draw(optionalInts(silent));");
        assertDoesNotCompile("tc.draw(arrays(silent, 2, Integer[]::new));");
        assertDoesNotCompile("tc.draw(tuples(integers(), silent));");
        assertDoesNotCompile("tc.draw(tuples(integers(), integers(), silent));");
        assertDoesNotCompile("tc.draw(tuples(new Generator<?>[] {integers(), silent}));");
        assertDoesNotCompile("tc.draw(oneOf(integers(), silent));");
        assertDoesNotCompile("tc.draw(silent.filter(x -> true));");
        assertDoesNotCompile("tc.draw(recursive(silent, sub -> lists(sub).map(l -> l.size())));");
    }

    @Test
    void printingCombinatorsNeedPrintableParts() throws Exception {
        assertDoesNotCompile("silent.map(x -> x).printAsCall(\"f\");");
        assertDoesNotCompile("integers().flatMap(n -> silent);");
        assertDoesNotCompile("records(Point.class).with(\"x\", silent);");
        assertDoesNotCompile("Deferred<Integer> forward = deferred(); forward.set(silent);");
        assertDoesNotCompile("recursive(integers(), sub -> silent);");
    }
}
