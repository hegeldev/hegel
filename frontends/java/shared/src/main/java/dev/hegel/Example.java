package dev.hegel;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * An explicit example: values a property test runs with before any are generated, in the {@link
 * Phase#EXPLICIT explicit phase}. Use one to pin a known edge case or a past bug, so it is checked
 * on every run whatever the engine generates.
 *
 * <p>The test body's draws take the example's values instead of drawing from their generators:
 * a draw takes the value given for its name ({@link #with}), and otherwise the next positional
 * value ({@link #of}), in order. A draw's name is the one the counterexample output shows: its
 * label ({@code x}, numbered {@code x_2}, {@code x_3} when the label is drawn again) or {@code
 * draw_N} for the N-th unlabelled draw. The generators are not consulted, so a value need not be
 * one they could produce, but it must have the type the body expects.
 *
 * <pre>{@code
 * Hegel.test(tc -> {
 *     List<Integer> xs = tc.draw(lists(integers()), "xs");
 *     assertEquals(xs, reverse(reverse(xs)));
 * }, new Settings().examples(Example.of(List.of()), Example.of().with("xs", List.of(1, 1))));
 * }</pre>
 *
 * <p>An example whose body fails is reported like any other failure, before the engine's own, and
 * with {@link Settings#reportMultipleFailures(boolean)} off it ends the run there. It is a usage
 * error, raised as an {@link IllegalArgumentException}, for an example to lack a value a draw asks
 * for, or to have values no draw takes. An example that fails an {@link TestCase#assume
 * assumption} (or calls {@link TestCase#reject}) is skipped: it neither passes nor fails.
 *
 * <p>The rest of the test-case API works in an example as follows. A {@link TestCase#drawSilent
 * silent draw} has no name, so it takes the next positional value. A {@link TestCase#fork fork}
 * draws from the same example as the test case it was forked from. {@link TestCase#repeat} runs
 * its body once, since an example's values can be drawn only once. {@link TestCase#target} and
 * events are ignored. Spans, pools and {@link Stateful} machines need the engine and are a usage
 * error.
 */
public final class Example {
    private final List<Object> positional;
    private final Map<String, Object> named;

    private Example(List<Object> positional, Map<String, Object> named) {
        this.positional = positional;
        this.named = named;
    }

    /**
     * An example whose draws take {@code values} in order.
     *
     * @param values the positional values (may contain {@code null})
     * @return the example
     */
    public static Example of(Object... values) {
        return new Example(Collections.unmodifiableList(new ArrayList<>(Arrays.asList(values))), Map.of());
    }

    /**
     * This example with {@code value} for the draw named {@code name}.
     *
     * @param name the draw's name
     * @param value its value (may be {@code null})
     * @return a new example
     */
    public Example with(String name, Object value) {
        Map<String, Object> copy = new LinkedHashMap<>(named);
        copy.put(name, value);
        return new Example(positional, Collections.unmodifiableMap(copy));
    }

    List<Object> positional() {
        return positional;
    }

    Map<String, Object> named() {
        return named;
    }

    @Override
    public String toString() {
        List<String> parts = new ArrayList<>();
        for (Object value : positional) {
            parts.add(render(value));
        }
        for (Map.Entry<String, Object> entry : named.entrySet()) {
            parts.add(entry.getKey() + "=" + render(entry.getValue()));
        }
        return "Example(" + String.join(", ", parts) + ")";
    }

    private static String render(Object value) {
        return value instanceof CharSequence s ? Printer.quote(s.toString(), '"') : String.valueOf(value);
    }
}
