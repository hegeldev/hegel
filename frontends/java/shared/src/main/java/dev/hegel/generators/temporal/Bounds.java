package dev.hegel.generators.temporal;

/** The order check the generators with a default range share. */
final class Bounds {
    private Bounds() {}

    /**
     * Reject a range whose ends are in the wrong order. {@code min} and {@code max} are the bounds
     * as they were set, null for one that was not, and {@code lo} and {@code hi} what they come to
     * with the defaults filled in.
     */
    static <T extends Comparable<? super T>> void ordered(String what, T min, T max, T lo, T hi) {
        if (lo.compareTo(hi) > 0) {
            String hint = min == null
                    ? "; min was not set, so it is the default: set it before max"
                    : max == null ? "; max was not set, so it is the default: set it before min" : "";
            throw new IllegalArgumentException(what + ": min (" + lo + ") > max (" + hi + ")" + hint);
        }
    }
}
