package dev.hegel;

import dev.hegel.lowlevel.Libhegel;
import dev.hegel.lowlevel.LibraryLoader;
import java.nio.file.Path;

/**
 * Process-wide lazy holder for the loaded libhegel shared object.
 *
 * <p>This is deliberately a singleton: it caches only the loaded native library and its resolved
 * function pointers, which are immutable and thread-safe to share. It holds no per-test state —
 * each property test gets its own engine ({@code hegel_run_t}, created per run in {@link Runner}),
 * so nothing leaks between tests. The library is located and opened exactly once on first use.
 * Tests may install a fake binding via {@link #setForTesting} and restore the real one with {@link
 * #reset}; {@link #real()} always answers with the real binding, for computations such as {@link
 * Label}'s that must agree with the engine whatever binding a test installed.
 */
final class Engine {
    private Engine() {}

    private static Libhegel loaded;
    private static Libhegel override;

    /** The binding runs go through: the one a test installed, or else the real one. */
    static synchronized Libhegel get() {
        return override != null ? override : real();
    }

    /** The real binding, loaded on first use. */
    static synchronized Libhegel real() {
        if (loaded == null) {
            // Each frontend jar bundles exactly one binding, registered as a LibhegelBackend
            // service provider; Libhegel.load finds it.
            Path path = LibraryLoader.fromEnvironment().resolve();
            Libhegel lib = Libhegel.load(path);
            LibraryLoader.warnOnVersionMismatch(lib.version(), LibraryLoader.targetEngineVersion(), System.err);
            loaded = lib;
        }
        return loaded;
    }

    /** Install a binding (real or fake) for the rest of the process or until {@link #reset}. */
    static synchronized void setForTesting(Libhegel binding) {
        override = binding;
    }

    /** Forget the installed and the loaded binding, so the next {@link #get()} re-resolves it. */
    static synchronized void reset() {
        override = null;
        loaded = null;
    }
}
