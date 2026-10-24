package dev.hegel;

import dev.hegel.lowlevel.Abi;

/** Engine output verbosity. */
public enum Verbosity {
    /**
     * Leave the choice to the engine's settings profile ({@link #NORMAL} unless configured
     * otherwise).
     */
    DEFAULT(null),
    /** Nothing besides the final result. */
    QUIET(Abi.VERBOSITY_QUIET),
    /** A short summary per run. The engine's default. */
    NORMAL(Abi.VERBOSITY_NORMAL),
    /** Per-test-case progress and final replay draws. */
    VERBOSE(Abi.VERBOSITY_VERBOSE),
    /** As verbose, plus shrinker trace output. */
    DEBUG(Abi.VERBOSITY_DEBUG);

    /** The {@code hegel_verbosity_t} value to send, or {@code null} to leave the profile's choice. */
    final Integer code;

    Verbosity(Integer code) {
        this.code = code;
    }

    /** The verbosity the engine reports back for {@code code}. */
    static Verbosity fromCode(int code) {
        for (Verbosity v : values()) {
            if (v.code != null && v.code == code) {
                return v;
            }
        }
        throw new HegelException("unknown hegel_verbosity_t value " + code);
    }
}
