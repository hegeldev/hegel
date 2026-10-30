package dev.hegel;

/**
 * Thrown by {@link Hegel#test} when the engine aborted the run rather than reporting a pass or a
 * failure of the property. Subtypes say why: {@link HealthCheckFailure}, {@link
 * FlakyTestException} and {@link UnsatisfiableException}; this type itself covers the rest (a
 * usage error, a bug in Hegel). The message is the engine's explanation. {@link RunReport#errorKind()}
 * classifies the same error on a report from {@link Hegel#run}.
 */
public sealed class HegelRunException extends HegelException
        permits HealthCheckFailure, FlakyTestException, UnsatisfiableException {
    HegelRunException(String message) {
        super(message);
    }
}
