package dev.hegel;

import dev.hegel.lowlevel.Abi;
import java.util.function.Consumer;

/**
 * What both bindings do around the Java body of a zone source callback (a loader or an oracle): no
 * throwable may unwind into the engine, so one becomes the callback's error message and a failure
 * code.
 */
final class ZoneCallbacks {
    private ZoneCallbacks() {}

    static int guard(Runnable body, Consumer<String> setError) {
        try {
            body.run();
            return Abi.OK;
        } catch (Throwable t) {
            setError.accept(message(t));
            return Abi.E_INVALID_ARG;
        }
    }

    static String message(Throwable t) {
        String message = t.getMessage();
        return message == null ? t.toString() : message;
    }
}
