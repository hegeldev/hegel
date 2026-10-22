package dev.hegel.generators;

import dev.hegel.SpecHandle;
import dev.hegel.TestCase;
import java.util.function.Function;

/**
 * Caches an engine configuration handle (date options, a time zone, zoned or span spec) for one
 * generator configuration, so it is built and validated once, not per draw.
 *
 * <p>The handle is rebuilt if a cached one was created by a different binding (a test swapped the
 * engine between draws). Benign racing is fine: two threads may both build, and every built handle
 * is valid and eventually freed by its own {@link SpecHandle} cleaner.
 */
final class SpecCache {
    private volatile SpecHandle handle;

    SpecHandle get(TestCase tc, Function<TestCase, SpecHandle> build) {
        SpecHandle h = handle;
        if (h == null || !tc.ownsSpec(h)) {
            h = build.apply(tc);
            handle = h;
        }
        return h;
    }
}
