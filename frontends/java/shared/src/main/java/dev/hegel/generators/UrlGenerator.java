package dev.hegel.generators;

import dev.hegel.Label;
import dev.hegel.TestCase;

/**
 * Generates syntactically valid (RFC 3986) {@code http}/{@code https} URLs.
 */
public final class UrlGenerator extends ValueGenerator<String> {
    private final HandleCache cache = new HandleCache();

    /** @hidden */
    @Override
    public long label() {
        return Label.of("hegel.urls");
    }

    /** @hidden */
    @Override
    public String doDraw(TestCase tc) {
        return tc.generateString(cache.get(tc, TestCase::urlGenerator));
    }
}
