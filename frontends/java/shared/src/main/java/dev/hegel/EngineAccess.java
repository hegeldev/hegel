package dev.hegel;

import dev.hegel.lowlevel.Libhegel;

/**
 * The process's engine binding, for the generators outside this package that need it between test
 * cases.
 *
 * @hidden
 */
public final class EngineAccess {
    private EngineAccess() {}

    /** The binding {@link Hegel} runs tests with, loading it on first use. */
    public static Libhegel binding() {
        return Engine.get();
    }
}
