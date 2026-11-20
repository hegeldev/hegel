package dev.hegel.generators.temporal;

/** What the generators with a default range share: filling in a bound that was not set, and the order check. */
final class Bounds {
    private Bounds() {}

    /** The bound as it was set, or {@code unset} when it was not. */
    static <T> T or(T bound, T unset) {
        return bound != null ? bound : unset;
    }

    /**
     * Reject a range whose ends are in the wrong order. {@code min} and {@code max} are the bounds
     * as they were set, null for one that was not, and {@code lo} and {@code hi} what they come to
     * with something filled in for a bound that was not set: the engine's extreme as the generator
     * is built, so that only two bounds that were both set can be out of order, and the default at
     * a draw.
     */
    static <T extends Comparable<? super T>> void ordered(String what, T min, T max, T lo, T hi) {
        if (lo.compareTo(hi) > 0) {
            String hint = min == null
                    ? "; min was not set, so it is the default"
                    : max == null ? "; max was not set, so it is the default" : "";
            throw new IllegalArgumentException(what + ": min (" + lo + ") > max (" + hi + ")" + hint);
        }
    }
}
