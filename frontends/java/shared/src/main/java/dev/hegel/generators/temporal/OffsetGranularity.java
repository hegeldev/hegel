package dev.hegel.generators.temporal;

import dev.hegel.lowlevel.Abi;

/** The unit every offset drawn by {@link OffsetGenerator} is a whole number of. */
public enum OffsetGranularity {
    HOURS(Abi.OFFSET_GRANULARITY_HOURS),
    QUARTER_HOURS(Abi.OFFSET_GRANULARITY_QUARTER_HOURS),
    MINUTES(Abi.OFFSET_GRANULARITY_MINUTES),
    SECONDS(Abi.OFFSET_GRANULARITY_SECONDS);

    final int engineValue;

    OffsetGranularity(int engineValue) {
        this.engineValue = engineValue;
    }
}
