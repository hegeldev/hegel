package dev.hegel;

import dev.hegel.lowlevel.Abi;
import java.util.function.Consumer;

/**
 * What both bindings do around the Java body of a zone source callback (a loader or an oracle): no
 * throwable may unwind into the engine, so one becomes the callback's error message and a failure
 * code. The throwable itself is kept for the thread, whose failing engine call the callback ran
 * inside, so the exception that reports that failure can carry it as its cause.
 */
final class ZoneCallbacks {
    private ZoneCallbacks() {}

    private static final ThreadLocal<Throwable> FAILURE = new ThreadLocal<>();

    static int guard(Runnable body, Consumer<String> setError) {
        try {
            body.run();
            return Abi.OK;
        } catch (Throwable t) {
            FAILURE.set(t);
            setError.accept(message(t));
            return Abi.E_INVALID_ARG;
        }
    }

    /**
     * What the last callback on this thread threw, when {@code message} (the engine's error) is
     * reporting it: the engine quotes the message the callback set.
     */
    static Throwable takeFailure(String message) {
        Throwable failure = FAILURE.get();
        FAILURE.remove();
        return failure != null && message.contains(message(failure)) ? failure : null;
    }

    static String message(Throwable t) {
        String message = t.getMessage();
        return message == null ? t.toString() : message;
    }
}
