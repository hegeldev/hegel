package dev.hegel;

import static dev.hegel.Generators.integers;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Consumer;
import java.util.function.IntConsumer;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.junit.jupiter.api.Test;

class MainTest {
    private static final Settings NO_DATABASE = new Settings().database(Database.disabled());
    private static final Consumer<TestCase> PASS = tc -> tc.draw(integers(), "n");
    private static final Consumer<TestCase> FAIL = tc -> {
        throw new AssertionError("failed with n=" + tc.draw(integers(), "n"));
    };

    private record Result(int status, String out, String err) {}

    private static Result run(Libhegel lib, Consumer<TestCase> body, Settings settings, String... args) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        ByteArrayOutputStream err = new ByteArrayOutputStream();
        int status =
                Main.run(lib, args, body, settings, "Workload", new PrintStream(out, true), new PrintStream(err, true));
        return new Result(status, out.toString(StandardCharsets.UTF_8), err.toString(StandardCharsets.UTF_8));
    }

    private static Result run(Consumer<TestCase> body, String... args) {
        return run(Engine.get(), body, NO_DATABASE, args);
    }

    private static String blobIn(String report) {
        Matcher matcher = Pattern.compile("rerun with: --failure-blob (\\S+)").matcher(report);
        assertTrue(matcher.find(), report);
        return matcher.group(1);
    }

    @Test
    void runsOneTestCaseWhateverTheSettings() {
        AtomicInteger cases = new AtomicInteger();
        Result result = run(Engine.get(), tc -> cases.incrementAndGet(), NO_DATABASE.testCases(50));
        assertEquals(0, result.status, result.err);
        assertEquals(1, cases.get());
    }

    @Test
    void helpPrintsUsageWithoutRunning() {
        for (String help : List.of("--help", "-h")) {
            AtomicInteger cases = new AtomicInteger();
            Result result = run(tc -> cases.incrementAndGet(), "--seed", "1", help);
            assertEquals(0, result.status);
            assertTrue(result.out.startsWith("Usage: java Workload [options]\n\n"), result.out);
            assertTrue(result.out.contains("--failure-blob <BLOB>"), result.out);
            assertTrue(result.out.contains("--unbounded-choices <BOOL>"), result.out);
            assertEquals(0, cases.get());
        }
    }

    @Test
    void usageErrors() {
        String[][] cases = {
            {"unknown argument: positional", "positional"},
            {"unknown argument: --", "--"},
            {"unknown argument: --test-cases", "--test-cases", "5"},
            {"unknown argument: --test_cases", "--test_cases=5"},
            {"unknown argument: --no-such-setting", "--no-such-setting", "1"},
            {"unknown argument: --no-such-setting", "--no-such-setting"},
            {"--seed requires a value", "--seed"},
            {"--seed requires a value", "--seed", "--derandomize", "true"},
            {"--seed expects an integer", "--seed=nope"},
            {"--database-key expects a non-empty key", "--database-key="},
            {"--profile requires a value", "--profile"},
            {"--profile requires a value", "--profile="},
            {"--profile can be given only once", "--profile", "ci", "--profile=ci"},
            {"--failure-blob requires a value", "--failure-blob"},
            {"--failure-blob can be given only once", "--failure-blob=a", "--failure-blob=b"},
        };
        for (String[] c : cases) {
            AtomicInteger ran = new AtomicInteger();
            String[] args = Arrays.copyOfRange(c, 1, c.length);
            Result result = run(tc -> ran.incrementAndGet(), args);
            assertEquals(2, result.status, c[0]);
            assertEquals(0, ran.get(), c[0]);
            assertTrue(result.err.startsWith("error: " + c[0]), result.err);
            assertTrue(result.err.contains("Usage: java Workload"), result.err);
        }
    }

    @Test
    void aFailureReportsABlobThatReplaysIt() {
        Result failure = run(FAIL, "--phases", "generate");
        assertEquals(1, failure.status, failure.err);
        assertTrue(failure.err.contains("n = "), failure.err);
        assertTrue(failure.err.contains("AssertionError: failed with n="), failure.err);
        assertFalse(failure.err.contains("error: "), failure.err);
        String blob = blobIn(failure.err);

        Result replay = run(FAIL, "--failure-blob", blob);
        assertEquals(1, replay.status, replay.err);
        assertTrue(replay.err.contains("AssertionError: failed with n="), replay.err);

        Result stale = run(PASS, "--failure-blob=" + blob);
        assertEquals(1, stale.status, stale.err);
        assertTrue(stale.err.contains("error: the failure blob did not reproduce a failure."), stale.err);
        assertTrue(stale.err.contains("stop passing --failure-blob"), stale.err);
    }

    @Test
    void optionsOverrideSettings() {
        Result quiet = run(FAIL, "--verbosity", "quiet", "--print-blob=false");
        assertEquals(1, quiet.status);
        assertEquals("", quiet.err);

        Settings quietSettings = NO_DATABASE.verbosity(Verbosity.QUIET).printBlob(false);
        Result loud = run(Engine.get(), FAIL, quietSettings, "--verbosity", "normal", "--print-blob", "true");
        assertTrue(loud.err.contains("n = "), loud.err);
        blobIn(loud.err);
    }

    @Test
    void errorsExitWithOneWithoutUsage() {
        FakeLibhegel badEnvironment = new FakeLibhegel();
        badEnvironment.settingsNewRc = Abi.E_INVALID_ARG;
        badEnvironment.lastError = "HEGEL_TEST_CASES is not a number";
        Result[] results = {
            run(PASS, "--failure-blob", "not a blob"),
            run(tc -> tc.assume(false)),
            run(badEnvironment, tc -> {}, new Settings()),
        };
        for (Result result : results) {
            assertEquals(1, result.status, result.err);
            assertTrue(result.err.startsWith("error: "), result.err);
            assertFalse(result.err.contains("Usage: "), result.err);
        }
        assertTrue(results[2].err.startsWith("error: HEGEL_TEST_CASES"), results[2].err);
    }

    @Test
    void theDatabaseKeyDefaultsToTheProgramAndAnExplicitNameWins() {
        FakeLibhegel fake = new FakeLibhegel();
        run(fake, tc -> {}, new Settings());
        assertEquals("Workload#main", fake.databaseKey);
        run(fake, tc -> {}, new Settings().name("mine"));
        assertEquals("mine", fake.databaseKey);
    }

    @Test
    void optionsReachTheEngineUnderTheirTomlKeys() {
        FakeLibhegel fake = new FakeLibhegel();
        run(fake, tc -> {}, new Settings(), "--database-key", "mine", "--show-statistics=true");
        assertEquals(List.of("database_key=mine", "show_statistics=true"), fake.settingsFromStringCalls);
    }

    @Test
    void healthChecksThatJudgeARunAreOffAlongsideTheOthers() {
        FakeLibhegel fake = new FakeLibhegel();
        run(fake, tc -> {}, new Settings().suppressHealthCheck(HealthCheck.FILTER_TOO_MUCH));
        assertEquals(Abi.HC_TOO_SLOW | Abi.HC_TEST_CASES_TOO_LARGE, fake.suppressMask & ~Abi.HC_FILTER_TOO_MUCH);
    }

    @Test
    void aProfileIsSelectedBeforeTheRun() {
        FakeLibhegel fake = new FakeLibhegel();
        assertEquals(0, run(fake, tc -> {}, new Settings(), "--profile=nightly").status);
        assertEquals("nightly", fake.defaultProfile);
    }

    @Test
    void aProfileTheEngineRefusesOrCannotResolveIsAUsageError() {
        FakeLibhegel refused = new FakeLibhegel();
        refused.setDefaultProfileRc = Abi.E_INVALID_ARG;
        refused.lastError = "bad name";
        Result result = run(refused, tc -> {}, new Settings(), "--profile=x");
        assertEquals(2, result.status);
        assertTrue(result.err.startsWith("error: --profile: bad name"), result.err);

        FakeLibhegel unresolved = new FakeLibhegel();
        unresolved.unresolvableProfile = "x";
        unresolved.lastError = "no such profile";
        result = run(unresolved, tc -> {}, new Settings(), "--profile=x");
        assertEquals(2, result.status);
        assertTrue(result.err.startsWith("error: --profile: no such profile"), result.err);
        assertEquals("", unresolved.defaultProfile);
    }

    @Test
    void anEnvironmentErrorUnderAProfileIsNotAUsageError() {
        FakeLibhegel badEnvironment = new FakeLibhegel();
        badEnvironment.settingsNewRc = Abi.E_INVALID_ARG;
        badEnvironment.lastError = "HEGEL_TEST_CASES is not a number";
        Result result = run(badEnvironment, tc -> {}, new Settings(), "--profile=ci");
        assertEquals(1, result.status, result.err);
        assertTrue(result.err.startsWith("error: HEGEL_TEST_CASES"), result.err);
        assertFalse(result.err.contains("Usage: "), result.err);
        assertEquals("", badEnvironment.defaultProfile);
    }

    @Test
    void aBlobReplayEndingInAnErrorNamesTheOption() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.runStatus = Abi.RUN_STATUS_ERROR;
        fake.runError = "health check failed";
        Result result = run(fake, tc -> {}, new Settings(), "--failure-blob=blob");
        assertEquals(1, result.status, result.err);
        assertTrue(
                result.err.startsWith("error: the failure blob replay ended in an error: health check failed"),
                result.err);
    }

    @Test
    void anEngineMessageWithoutTheKeyIsKeptWhole() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.settingsFromStringRc = Abi.E_INVALID_ARG;
        fake.lastError = "out of range";
        Result result = run(fake, tc -> {}, new Settings(), "--seed=1");
        assertTrue(result.err.startsWith("error: --seed out of range"), result.err);
    }

    @Test
    void hegelMainExitsWithTheStatusAndNamesTheCaller() {
        List<Integer> statuses = new ArrayList<>();
        IntConsumer exit = Main.exit;
        PrintStream stdout = System.out;
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        Main.exit = statuses::add;
        System.setOut(new PrintStream(out, true));
        try {
            Hegel.main(new String[] {"--help"}, PASS);
        } finally {
            Main.exit = exit;
            System.setOut(stdout);
        }
        assertEquals(List.of(0), statuses);
        assertTrue(out.toString(StandardCharsets.UTF_8).startsWith("Usage: java " + MainTest.class.getName()));
    }

    @Test
    void bindingsSetAndReadTheEnginesSettings() {
        Libhegel lib = Engine.get();
        long[] handle = new long[1];
        assertEquals(Abi.OK, lib.settingsNew(handle));
        long s = handle[0];
        try {
            assertEquals(Abi.OK, lib.settingsFromString(s, "verbosity", "debug"));
            assertEquals(Abi.VERBOSITY_DEBUG, lib.settingsGetVerbosity(s));
            assertEquals(Abi.OK, lib.settingsFromString(s, "suppress_health_check", "too_slow"));
            assertEquals(Abi.HC_TOO_SLOW, lib.settingsGetSuppressHealthCheck(s));
            assertEquals(Abi.E_INVALID_ARG, lib.settingsFromString(s, "no_such_setting", "1"));
            assertTrue(lib.lastErrorMessage().contains("unknown setting"), lib.lastErrorMessage());
        } finally {
            lib.settingsFree(s);
        }
        assertNotEquals(Abi.OK, lib.setDefaultProfile("default"));
        assertEquals(Abi.OK, lib.setDefaultProfile(""));
    }
}
