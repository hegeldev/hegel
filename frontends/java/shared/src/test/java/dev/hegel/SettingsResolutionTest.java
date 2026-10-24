package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import example.LocationFixture;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

/**
 * Settings start from the engine's resolution and send only what the caller set, and every engine
 * setting, profiles included, is reachable.
 */
class SettingsResolutionTest {
    private static FakeLibhegel run(Settings settings) {
        FakeLibhegel fake = new FakeLibhegel();
        Runner.run(fake, settings, tc -> {}, Reporter.silent()).throwIfFailed();
        return fake;
    }

    @Test
    void unsetSettingsAreNotSent() {
        FakeLibhegel fake = run(new Settings());
        assertNull(fake.verbosity);
        assertNull(fake.reportMultipleFailures);
        assertNull(fake.stopOnFirstFailure);
        assertNull(fake.showStatistics);
        assertNull(fake.unboundedChoices);
        assertNull(fake.testLocation);
        assertNull(fake.profileRequested);
    }

    @Test
    void stringSettingsAreSentAfterTheTypedOnesInOrder() {
        assertEquals(List.of(), run(new Settings()).stringSettings);
        FakeLibhegel fake = run(new Settings().set("test_cases", "7").set("phases", "generate"));
        assertEquals(List.of("test_cases=7", "phases=generate"), fake.stringSettings);
    }

    @Test
    void stringSettingsAreParsedByTheEngine() {
        Settings[] effective = new Settings[1];
        Reporter capture = new Reporter() {
            @Override
            public void runStarted(Settings settings) {
                effective[0] = settings;
            }
        };
        Hegel.run(tc -> {}, new Settings().testCases(3).set("test_cases", "7").database(Database.disabled()), capture);
        assertEquals(7L, effective[0].testCases);
        IllegalArgumentException unknown =
                assertThrows(IllegalArgumentException.class, () -> new Settings().set("no_such_setting", "1"));
        assertTrue(unknown.getMessage().contains("no_such_setting"), unknown.getMessage());
        assertThrows(IllegalArgumentException.class, () -> new Settings().set("verbosity", "loud"));
    }

    @Test
    void explicitSettingsAreSent() {
        TestLocation location = new TestLocation("a/B.java", 12, "a.B", "prop");
        FakeLibhegel fake = run(new Settings()
                .verbosity(Verbosity.QUIET)
                .reportMultipleFailures(true)
                .stopOnFirstFailure(false)
                .showStatistics(true)
                .unboundedChoices(true)
                .testLocation(location));
        assertEquals(Abi.VERBOSITY_QUIET, fake.verbosity);
        assertEquals(Boolean.TRUE, fake.reportMultipleFailures);
        assertEquals(Boolean.FALSE, fake.stopOnFirstFailure);
        assertEquals(Boolean.TRUE, fake.showStatistics);
        assertEquals(Boolean.TRUE, fake.unboundedChoices);
        assertEquals(location, fake.testLocation);
    }

    @Test
    void reportersSeeTheResolvedVerbosity() {
        Settings[] seen = new Settings[1];
        Reporter recording = new Reporter() {
            @Override
            public void runStarted(Settings settings) {
                seen[0] = settings;
            }
        };
        FakeLibhegel fake = new FakeLibhegel();
        Runner.run(fake, new Settings(), tc -> {}, recording);
        assertEquals(Verbosity.NORMAL, seen[0].verbosity);

        FakeLibhegel unknown = new FakeLibhegel();
        unknown.verbosity = 99;
        HegelException e =
                assertThrows(HegelException.class, () -> Runner.run(unknown, new Settings(), tc -> {}, recording));
        assertTrue(e.getMessage().contains("99"), e.getMessage());
    }

    @Test
    void aProfileRunStartsFromThatProfile() {
        FakeLibhegel fake = new FakeLibhegel();
        Settings settings = Settings.fromProfile("ci").testCases(5);
        Runner.run(fake, settings, tc -> {}, Reporter.silent()).throwIfFailed();
        assertEquals("ci", fake.profileRequested);
        assertEquals(Long.valueOf(5), fake.testCases);
    }

    @Test
    void profileErrorsAreTranslated() {
        IllegalArgumentException unknown =
                assertThrows(IllegalArgumentException.class, () -> Settings.fromProfile("no-such-profile"));
        assertTrue(unknown.getMessage().contains("no-such-profile"), unknown.getMessage());

        FakeLibhegel broken = new FakeLibhegel();
        broken.settingsNewForProfileRc = Abi.E_INTERNAL;
        HegelException e = assertThrows(HegelException.class, () -> Runner.checkProfile(broken, "x"));
        assertTrue(e.getMessage().contains("hegel_settings_new_for_profile"), e.getMessage());

        FakeLibhegel badName = new FakeLibhegel();
        badName.registerProfileRc = Abi.E_INVALID_ARG;
        assertThrows(IllegalArgumentException.class, () -> Runner.registerProfile(badName, "x", new Settings()));
        assertTrue(badName.settingsFreed);

        FakeLibhegel badDefault = new FakeLibhegel();
        badDefault.setDefaultProfileRc = Abi.E_INTERNAL;
        assertThrows(HegelException.class, () -> Runner.setDefaultProfile(badDefault, "x"));
    }

    @Test
    void registeredProfilesAreResolvedByTheEngine() {
        Settings.registerProfile("java-settings-test", new Settings().testCases(7));
        Settings[] seen = new Settings[1];
        Reporter recording = new Reporter() {
            @Override
            public void runStarted(Settings settings) {
                seen[0] = settings;
            }
        };
        RunReport report = Hegel.run(
                tc -> {}, Settings.fromProfile("java-settings-test").database(Database.disabled()), recording);
        assertTrue(report.passed());
        assertEquals(Long.valueOf(7), seen[0].testCases);

        assertThrows(IllegalArgumentException.class, () -> Settings.registerProfile("base", new Settings()));
        assertThrows(IllegalArgumentException.class, () -> Settings.registerProfile("not a name", new Settings()));
    }

    @Test
    void theDefaultProfileCanBeSetAndCleared() {
        Settings.registerProfile("java-default-test", new Settings().testCases(3));
        Settings[] seen = new Settings[1];
        Reporter recording = new Reporter() {
            @Override
            public void runStarted(Settings settings) {
                seen[0] = settings;
            }
        };
        try {
            Settings.setDefaultProfile("java-default-test");
            Hegel.run(tc -> {}, new Settings().database(Database.disabled()), recording);
            assertEquals(Long.valueOf(3), seen[0].testCases);
        } finally {
            Settings.setDefaultProfile("");
        }
        assertThrows(IllegalArgumentException.class, () -> Settings.setDefaultProfile("default"));
    }

    /** The real binding behind a proxy that records every test location sent to the engine. */
    private static List<TestLocation> recordLocations(Runnable action) {
        List<TestLocation> locations = new ArrayList<>();
        Libhegel real = Engine.get();
        Libhegel recording = (Libhegel) Proxy.newProxyInstance(
                Libhegel.class.getClassLoader(), new Class<?>[] {Libhegel.class}, (proxy, method, args) -> {
                    if (method.getName().equals("settingsTestLocation")) {
                        locations.add(
                                new TestLocation((String) args[1], (int) args[2], (String) args[3], (String) args[4]));
                    }
                    try {
                        return method.invoke(real, args);
                    } catch (InvocationTargetException e) {
                        throw e.getCause();
                    }
                });
        Engine.setForTesting(recording);
        try {
            action.run();
        } finally {
            Engine.setForTesting(real);
        }
        return locations;
    }

    @Test
    void runsAreLocatedAtTheirCallSite() {
        List<TestLocation> located = recordLocations(() -> LocationFixture.runHere(
                new Settings().database(Database.disabled()).infrastructurePackages("org.elsewhere.")));
        assertEquals(1, located.size());
        TestLocation location = located.get(0);
        assertEquals("example/LocationFixture.java", location.file());
        assertEquals("example.LocationFixture", location.className());
        assertEquals("runHere", location.function());
        assertTrue(location.beginLine() > 0);

        // Reflection is looked past, an explicit location wins, and a caller made entirely of
        // infrastructure leaves the location unset.
        List<TestLocation> reflective = recordLocations(() -> {
            try {
                Hegel.class
                        .getMethod("run", java.util.function.Consumer.class, Settings.class, Reporter.class)
                        .invoke(
                                null,
                                (java.util.function.Consumer<TestCase>) tc -> {},
                                new Settings(),
                                Reporter.silent());
            } catch (ReflectiveOperationException e) {
                throw new AssertionError(e);
            }
        });
        assertEquals("dev/hegel/SettingsResolutionTest.java", reflective.get(0).file());
        assertEquals(
                List.of(),
                recordLocations(
                        () -> Hegel.run(tc -> {}, new Settings().infrastructurePackages(""), Reporter.silent())));
        TestLocation explicit = new TestLocation("x/Y.java", 3, "x.Y", "z");
        assertEquals(
                List.of(explicit),
                recordLocations(() -> LocationFixture.runHere(new Settings().testLocation(explicit))));
    }

    @Test
    void locationsWithoutASourceFileUseTheTopLevelClassName() {
        assertEquals(
                new TestLocation("a/b/Outer.java", 0, "a.b.Outer$Inner", "f"),
                TestLocation.of("a.b.Outer$Inner", null, 0, "f"));
        assertEquals(new TestLocation("Top.java", 4, "Top", "g"), TestLocation.of("Top", null, 4, "g"));
        assertEquals(new TestLocation("p/Src.java", 1, "p.C", "h"), TestLocation.of("p.C", "Src.java", 1, "h"));
    }
}
