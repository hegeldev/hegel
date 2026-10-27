package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
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
 * Settings copy the engine's resolution when they are created and send every setting back at run
 * time, and every engine setting, profiles included, is reachable.
 */
class SettingsResolutionTest {
    private static FakeLibhegel run(Settings settings) {
        FakeLibhegel fake = new FakeLibhegel();
        Runner.run(fake, settings, tc -> {}, Reporter.silent()).throwIfFailed();
        return fake;
    }

    @Test
    void everySettingIsResolvedWhenCreatedAndSentWhenRun() {
        FakeLibhegel engine = new FakeLibhegel();
        engine.resolvedTestCases = 7;
        engine.resolvedSeed = -1L;
        engine.resolvedDerandomize = true;
        engine.resolvedDatabase = "/tmp/hegel-db";
        engine.resolvedSuppressMask = Abi.HC_FILTER_TOO_MUCH;
        engine.resolvedPhases = Abi.PHASE_GENERATE;
        engine.resolvedReportMultipleFailures = true;
        engine.resolvedStopOnFirstFailure = false;
        engine.resolvedShowStatistics = true;
        engine.resolvedUnboundedChoices = true;
        engine.resolvedPrintBlob = true;
        engine.resolvedStrictness = Abi.NONDETERMINISM_WARN;
        engine.resolvedBackend = Abi.BACKEND_URANDOM;
        engine.verbosity = Abi.VERBOSITY_DEBUG;
        Settings settings = Settings.resolved(engine, null);
        assertTrue(engine.settingsFreed);
        assertNull(engine.profileRequested);
        assertEquals(7L, settings.testCases);
        assertTrue(settings.hasSeed);
        assertEquals(-1L, settings.seed);
        assertTrue(settings.derandomize);
        assertEquals(Database.Kind.PATH, settings.database.kind);
        assertEquals("/tmp/hegel-db", settings.database.path);
        assertEquals(Abi.HC_FILTER_TOO_MUCH, settings.suppressMask);
        assertEquals(Abi.PHASE_GENERATE, settings.phasesMask);
        assertEquals(Verbosity.DEBUG, settings.verbosity);
        assertEquals(Backend.URANDOM, settings.backend);
        assertTrue(settings.reportMultipleFailures);
        assertFalse(settings.stopOnFirstFailure);
        assertTrue(settings.showStatistics);
        assertTrue(settings.unboundedChoices);
        assertTrue(settings.printBlob);
        assertEquals(NondeterminismStrictness.WARN, settings.nondeterminismStrictness);

        FakeLibhegel fake = run(settings);
        assertEquals(Long.valueOf(7), fake.testCases);
        assertEquals(Long.valueOf(-1), fake.seed);
        assertEquals(Boolean.TRUE, fake.derandomize);
        assertEquals("/tmp/hegel-db", fake.databasePath);
        assertEquals(Abi.HC_FILTER_TOO_MUCH, fake.suppressMask);
        assertEquals(Abi.PHASE_GENERATE, fake.phasesMask);
        assertEquals(Integer.valueOf(Abi.VERBOSITY_DEBUG), fake.verbosity);
        assertEquals(Integer.valueOf(Abi.BACKEND_URANDOM), fake.backendCode);
        assertEquals(Boolean.TRUE, fake.reportMultipleFailures);
        assertEquals(Boolean.FALSE, fake.stopOnFirstFailure);
        assertEquals(Boolean.TRUE, fake.showStatistics);
        assertEquals(Boolean.TRUE, fake.unboundedChoices);
        assertEquals(Boolean.TRUE, fake.printBlob);
        assertEquals(Integer.valueOf(Abi.NONDETERMINISM_WARN), fake.strictness);
        assertNull(fake.testLocation);
    }

    @Test
    void theDefaultsOfTheFakeAreResolvedAndSent() {
        FakeLibhegel engine = new FakeLibhegel();
        Settings settings = Settings.resolved(engine, null);
        assertFalse(settings.hasSeed);
        assertEquals(Database.Kind.DEFAULT_LOCATION, settings.database.kind);
        assertEquals(0, settings.suppressMask);
        assertEquals(Verbosity.NORMAL, settings.verbosity);
        assertEquals(Backend.DEFAULT, settings.backend);
        assertTrue(settings.stopOnFirstFailure);
        assertFalse(settings.showStatistics);
        assertFalse(settings.unboundedChoices);
        engine.resolvedDatabase = "";
        assertEquals(Database.Kind.DISABLED, Settings.resolved(engine, null).database.kind);

        FakeLibhegel fake = run(settings);
        assertTrue(fake.seedSent);
        assertNull(fake.seed);
        assertTrue(fake.databaseSent);
        assertNull(fake.databasePath);
        assertEquals(0, fake.suppressMask);
        assertEquals(Integer.valueOf(Abi.VERBOSITY_NORMAL), fake.verbosity);
        assertEquals(Boolean.TRUE, fake.stopOnFirstFailure);
    }

    @Test
    void theRealEngineResolvesTheShippedDefaults() {
        Settings base = Settings.fromProfile("base");
        assertEquals(100L, base.testCases);
        assertFalse(base.hasSeed);
        assertFalse(base.derandomize);
        assertEquals(Database.Kind.DEFAULT_LOCATION, base.database.kind);
        assertEquals(0, base.suppressMask);
        assertEquals(Abi.PHASE_ALL, base.phasesMask);
        assertEquals(Verbosity.NORMAL, base.verbosity);
        assertEquals(Backend.DEFAULT, base.backend);
        assertFalse(base.reportMultipleFailures);
        assertTrue(base.stopOnFirstFailure);
        assertFalse(base.showStatistics);
        assertFalse(base.unboundedChoices);
        assertTrue(base.printBlob);
        assertEquals(NondeterminismStrictness.QUIET, base.nondeterminismStrictness);

        Settings ci = Settings.fromProfile("ci");
        assertTrue(ci.derandomize);
        assertEquals(Database.Kind.DISABLED, ci.database.kind);
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
        Runner.run(fake, new Settings().verbosity(Verbosity.QUIET).set("verbosity", "debug"), tc -> {}, recording);
        assertEquals(Verbosity.QUIET, seen[0].verbosity);
        Runner.run(fake, new Settings().verbosity(Verbosity.QUIET), tc -> {}, recording);
        assertEquals(Verbosity.QUIET, seen[0].verbosity);

        FakeLibhegel unknown = new FakeLibhegel();
        unknown.verbosity = 99;
        HegelException e = assertThrows(HegelException.class, () -> Settings.resolved(unknown, null));
        assertTrue(e.getMessage().contains("99"), e.getMessage());
    }

    @Test
    void aProfileIsResolvedWhenTheSettingsAreCreated() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.resolvedTestCases = 9;
        Settings settings = Settings.resolved(fake, "ci");
        assertEquals("ci", fake.profileRequested);
        assertEquals(9L, settings.testCases);
        FakeLibhegel run = run(settings.testCases(5));
        assertNull(run.profileRequested);
        assertEquals(Long.valueOf(5), run.testCases);
    }

    @Test
    void registrationIsNotRetroactive() {
        Settings[] seen = new Settings[1];
        Reporter recording = new Reporter() {
            @Override
            public void runStarted(Settings settings) {
                seen[0] = settings;
            }
        };
        Settings.registerProfile(
                "java-snapshot-test", new Settings().testCases(3).database(Database.disabled()));
        Settings settings = Settings.fromProfile("java-snapshot-test");
        Settings.registerProfile(
                "java-snapshot-test", new Settings().testCases(9).database(Database.disabled()));
        Hegel.run(tc -> {}, settings, recording);
        assertEquals(3L, seen[0].testCases);
        Hegel.run(tc -> {}, Settings.fromProfile("java-snapshot-test"), recording);
        assertEquals(9L, seen[0].testCases);

        Settings.registerProfile(
                "java-default-snapshot", new Settings().testCases(2).database(Database.disabled()));
        Settings before = new Settings().testCases(4).database(Database.disabled());
        try {
            Settings.setDefaultProfile("java-default-snapshot");
            Hegel.run(tc -> {}, before, recording);
            assertEquals(4L, seen[0].testCases);
            Hegel.run(tc -> {}, new Settings(), recording);
            assertEquals(2L, seen[0].testCases);
        } finally {
            Settings.setDefaultProfile("");
        }
    }

    @Test
    void profileErrorsAreTranslated() {
        IllegalArgumentException unknown =
                assertThrows(IllegalArgumentException.class, () -> Settings.fromProfile("no-such-profile"));
        assertTrue(unknown.getMessage().contains("no-such-profile"), unknown.getMessage());

        FakeLibhegel broken = new FakeLibhegel();
        broken.settingsNewForProfileRc = Abi.E_INTERNAL;
        HegelException e = assertThrows(HegelException.class, () -> Settings.resolved(broken, "x"));
        assertTrue(e.getMessage().contains("hegel_settings_new_for_profile"), e.getMessage());

        FakeLibhegel malformed = new FakeLibhegel();
        malformed.settingsNewRc = Abi.E_INVALID_ARG;
        malformed.lastError = "HEGEL_TEST_CASES must be a positive integer, got \"lots\"";
        IllegalArgumentException bad =
                assertThrows(IllegalArgumentException.class, () -> Settings.resolved(malformed, null));
        assertEquals(malformed.lastError, bad.getMessage());

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
        assertEquals(7L, seen[0].testCases);

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
            assertEquals(3L, seen[0].testCases);
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
