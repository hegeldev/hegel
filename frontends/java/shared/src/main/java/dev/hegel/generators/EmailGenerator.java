package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.TestCase;

/**
 * Generates syntactically valid (RFC 5321/5322) email addresses like {@code alice@example.com}.
 */
public final class EmailGenerator extends ValueGenerator<String> {
    private final HandleCache cache = new HandleCache();

    /** @hidden */
    @Override
    public long label() {
        return Label.of("hegel.emails");
    }

    /** @hidden */
    @Override
    public String doDraw(TestCase tc) {
        return tc.generateString(cache.get(tc, TestCase::emailGenerator));
    }
}
