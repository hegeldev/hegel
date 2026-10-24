package dev.hegel.generators;

import dev.hegel.lowlevel.Abi;

/** Validation and defaulting helpers for size bounds. */
final class Sizes {
    private Sizes() {}

    static void validate(long minSize, long maxSize, String what) {
        if (minSize < 0) {
            throw new IllegalArgumentException(what + ": minSize must be >= 0, got " + minSize);
        }
        if (maxSize != Abi.UNBOUNDED && maxSize < minSize) {
            throw new IllegalArgumentException(
                    what + ": maxSize (" + maxSize + ") must be >= minSize (" + minSize + ")");
        }
    }

    /**
     * The {@code max_size} to send for a text or bytes draw: the explicit {@code maxSize} when one
     * was set, otherwise {@link Abi#MAX_SIZE_DEFAULT}, for the engine to apply its default.
     */
    static long engineMax(long maxSize) {
        return maxSize == Abi.UNBOUNDED ? Abi.MAX_SIZE_DEFAULT : maxSize;
    }
}
