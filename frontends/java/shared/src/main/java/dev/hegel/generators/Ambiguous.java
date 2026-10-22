package dev.hegel.generators;

import dev.hegel.lowlevel.Abi;

/**
 * Which instant a {@link ZonedDateTimeGenerator} means by a wall-clock time the clocks repeated,
 * such as 01:30 on the night daylight-saving time ends.
 */
public enum Ambiguous {
    /** Either, decided by a drawn choice that shrinks to the earlier. */
    BOTH(Abi.AMBIGUOUS_BOTH),
    /** The earlier instant, with the offset in force before the transition. */
    EARLIER(Abi.AMBIGUOUS_EARLIER),
    /** The later instant, with the offset in force after the transition. */
    LATER(Abi.AMBIGUOUS_LATER);

    final int engineValue;

    Ambiguous(int engineValue) {
        this.engineValue = engineValue;
    }
}
