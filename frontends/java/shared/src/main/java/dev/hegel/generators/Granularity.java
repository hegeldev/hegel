package dev.hegel.generators;

import dev.hegel.lowlevel.Abi;

/** The unit every offset drawn by {@link ZoneOffsetGenerator} is a whole number of. */
public enum Granularity {
    HOURS(Abi.OFFSET_GRANULARITY_HOURS),
    QUARTER_HOURS(Abi.OFFSET_GRANULARITY_QUARTER_HOURS),
    MINUTES(Abi.OFFSET_GRANULARITY_MINUTES),
    SECONDS(Abi.OFFSET_GRANULARITY_SECONDS);

    final int engineValue;

    Granularity(int engineValue) {
        this.engineValue = engineValue;
    }
}
