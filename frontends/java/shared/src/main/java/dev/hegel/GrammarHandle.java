package dev.hegel;

import dev.hegel.lowlevel.Libhegel;
import java.lang.ref.Cleaner;

/**
 * An opaque compiled engine grammar ({@code hegel_grammar_t*}) plus the binding it was built by.
 *
 * <p>Built once per {@link dev.hegel.generators.LarkGenerator} configuration and cached, so the
 * grammar is parsed and compiled once, not per draw. The grammar is immutable and may be shared
 * across test cases and threads. The engine-side allocation is released when this wrapper becomes
 * unreachable (via a {@link Cleaner}); every draw keeps the wrapper reachable while it runs.
 *
 * @hidden
 */
public final class GrammarHandle {
    private static final Cleaner CLEANER = Cleaner.create();

    final Libhegel lib;
    final long handle;

    GrammarHandle(Libhegel lib, long handle) {
        this.lib = lib;
        this.handle = handle;
        CLEANER.register(this, new Free(lib, handle));
    }

    /** The deferred release of the engine-side allocation; see {@link StringGeneratorHandle.Free}. */
    record Free(Libhegel lib, long handle) implements Runnable {
        @Override
        public void run() {
            lib.grammarFree(handle);
        }
    }
}
