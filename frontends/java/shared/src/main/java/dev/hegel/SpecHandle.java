package dev.hegel;

import dev.hegel.lowlevel.Libhegel;
import java.lang.ref.Cleaner;

/**
 * An opaque engine configuration handle ({@code hegel_date_options_t*}, {@code
 * hegel_timezone_spec_t*}, {@code hegel_zoned_spec_t*} or {@code hegel_span_spec_t*}) plus the
 * binding it was built by and the way to release it.
 *
 * <p>Built once per generator configuration (validating every parameter eagerly) and cached by the
 * date and time generators in {@code dev.hegel.generators}. The handle is immutable after
 * construction and may be shared across test cases and threads. The engine-side allocation is
 * released when this wrapper becomes unreachable (via a {@link Cleaner}), since by then no draw can
 * use it again.
 *
 * @hidden
 */
public final class SpecHandle {
    private static final Cleaner CLEANER = Cleaner.create();

    /** How a kind of handle is released: one of the binding's {@code *Free} methods. */
    @FunctionalInterface
    interface Freer {
        void free(Libhegel lib, long handle);
    }

    final Libhegel lib;
    final long handle;

    SpecHandle(Libhegel lib, long handle, Freer freer) {
        this.lib = lib;
        this.handle = handle;
        CLEANER.register(this, new Free(lib, handle, freer));
    }

    /**
     * The deferred release of the engine-side allocation. A record (not a lambda capturing {@code
     * this}) so the cleanable never keeps its own handle reachable.
     */
    record Free(Libhegel lib, long handle, Freer freer) implements Runnable {
        @Override
        public void run() {
            freer.free(lib, handle);
        }
    }
}
