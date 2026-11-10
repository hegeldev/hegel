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

    /**
     * The exception for a call {@code lib} just rejected with {@code HEGEL_E_INVALID_ARG}: the
     * engine's diagnostic, caused by what a zone source callback threw during the call, if one did.
     */
    public static IllegalArgumentException argumentError(Libhegel lib) {
        String raw = lib.lastErrorMessage();
        String message = raw == null ? "" : raw;
        return new IllegalArgumentException(message, ZoneCallbacks.takeFailure(message));
    }

    /**
     * A test case with no engine test case behind it, through which a generator builds and
     * validates its configuration handles when it is created, so that a bad argument is reported
     * there and not at the first draw. Drawing from it fails.
     */
    public static TestCase setup() {
        return new TestCase(new LiveDataSource(binding(), 0), false, Reporter.silent());
    }

    /** Releases the zone source {@code handle} of {@code lib} once the returned owner is unreachable. */
    public static SpecHandle zoneSource(Libhegel lib, long handle) {
        return new SpecHandle(lib, handle, Libhegel::zoneSourceFree);
    }
}
