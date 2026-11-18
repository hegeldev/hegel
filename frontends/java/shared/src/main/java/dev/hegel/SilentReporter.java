package dev.hegel;

/**
 * The reporter {@link Reporter#silent()} returns. The runner passes the engine no output
 * destination for it, so a verbose or debug run renders nothing nobody would see.
 */
final class SilentReporter implements Reporter {
    static final SilentReporter INSTANCE = new SilentReporter();

    private SilentReporter() {}
}
