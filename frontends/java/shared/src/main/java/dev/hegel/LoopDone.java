package dev.hegel;

/**
 * Control-flow signal: a {@link TestCase#repeat(Runnable)} loop has run every repetition the engine
 * asked for. Unwinds the test body; the runner marks the case VALID. Never a property failure.
 */
final class LoopDone extends RuntimeException {
    LoopDone() {
        super(null, null, false, false);
    }
}
