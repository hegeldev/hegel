package example;

import dev.hegel.Hegel;
import dev.hegel.Reporter;
import dev.hegel.RunReport;
import dev.hegel.Settings;

/** A caller outside {@code dev.hegel}, so {@link Hegel#run} finds a user frame to locate the test at. */
public final class LocationFixture {
    private LocationFixture() {}

    /** Run a trivial property from here. */
    public static RunReport runHere(Settings settings) {
        return Hegel.run(tc -> {}, settings, Reporter.silent());
    }
}
