package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.LibraryLoader;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.List;
import java.util.OptionalLong;
import org.junit.jupiter.api.Test;

/** Covers {@link RealLibhegel} edge branches that the normal engine path does not reach. */
class RealLibhegelTest {

    static void throwsError() {
        throw new AssertionError("boom"); // an Error (Throwable, not RuntimeException)
    }

    static void throwsRuntime() {
        throw new IllegalStateException("rt");
    }

    @Test
    void invokeWrapsNonRuntimeThrowable() throws Exception {
        MethodHandle h = MethodHandles.lookup()
                .findStatic(RealLibhegelTest.class, "throwsError", MethodType.methodType(void.class));
        assertThrows(HegelException.class, () -> RealLibhegel.invoke(h));
    }

    @Test
    void invokeRethrowsRuntimeException() throws Exception {
        MethodHandle h = MethodHandles.lookup()
                .findStatic(RealLibhegelTest.class, "throwsRuntime", MethodType.methodType(void.class));
        assertThrows(IllegalStateException.class, () -> RealLibhegel.invoke(h));
    }

    @Test
    void readStringHandlesNullDataAndInteriorNul() {
        try (Arena a = Arena.ofConfined()) {
            // A zeroed struct is {NULL, 0}: absent rather than empty.
            MemorySegment string = a.allocate(16);
            assertNull(RealLibhegel.readString(string));
            // A length-delimited string keeps an interior NUL and ignores the terminator.
            byte[] bytes = "a\0b\0".getBytes(StandardCharsets.UTF_8);
            MemorySegment data = a.allocate(bytes.length);
            MemorySegment.copy(bytes, 0, data, ValueLayout.JAVA_BYTE, 0, bytes.length);
            string.set(ValueLayout.ADDRESS, 0, data);
            string.set(ValueLayout.JAVA_LONG, 8, 3);
            assertEquals("a\0b", RealLibhegel.readString(string));
            // Through a `const hegel_string_t *` slot, NULL is absent and a pointer is read.
            MemorySegment slot = a.allocate(ValueLayout.ADDRESS);
            assertNull(RealLibhegel.readStringPtr(slot));
            slot.set(ValueLayout.ADDRESS, 0, string);
            assertEquals("a\0b", RealLibhegel.readStringPtr(slot));
        }
    }

    @Test
    void findSymbolReturnsPresentAndThrowsOnMissing() {
        Path lib = LibraryLoader.fromEnvironment().resolve();
        try (Arena a = Arena.ofShared()) {
            SymbolLookup lookup = SymbolLookup.libraryLookup(lib, a);
            assertNotNull(RealLibhegel.findSymbol(lookup, "hegel_version"));
            assertThrows(HegelException.class, () -> RealLibhegel.findSymbol(lookup, "no_such_symbol_xyz"));
        }
    }

    @Test
    void constructorRejectsBadPath() {
        assertThrows(HegelException.class, () -> new RealLibhegel(Path.of("/nonexistent/libhegel.so")));
    }

    private static RealLibhegel real() {
        return new RealLibhegel(LibraryLoader.fromEnvironment().resolve());
    }

    @Test
    void infrastructureCallsReportNullHandles() {
        RealLibhegel lib = real();
        // A NULL handle on an infra call surfaces as a HegelException carrying the engine's
        // diagnostic rather than undefined behaviour.
        assertThrows(HegelException.class, () -> lib.runResultStatus(0));
        assertThrows(HegelException.class, () -> lib.runStart(0, null));
        assertThrows(HegelException.class, () -> lib.runStartBlob(0, "blob", null));
        assertThrows(HegelException.class, () -> lib.testCaseShouldCapture(0));
        assertThrows(HegelException.class, () -> lib.testCaseSetWorker(0, 0));
        long[] clone = {7};
        assertEquals(Abi.E_INVALID_HANDLE, lib.testCaseClone(0, clone));
        assertEquals(7, clone[0]);
    }

    @Test
    void outputCallsReportNullHandles() {
        RealLibhegel lib = real();
        // Out-slots are seeded with sentinels: a failed call must not copy anything out.
        long[] handle = {7};
        assertEquals(Abi.E_INVALID_HANDLE, lib.testCaseBlock(0, 2, handle));
        assertEquals(Abi.E_INVALID_HANDLE, lib.testCasePrinter(0, handle));
        assertEquals(7, handle[0]);
        String[] text = {"unchanged"};
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerValue(0, text));
        assertEquals("unchanged", text[0]);
        assertEquals(Abi.E_INVALID_HANDLE, lib.note(0, "n"));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerText(0, "t"));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerBreakable(0, " "));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerHardBreak(0));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerBeginGroup(0, 1, "["));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerEndGroup(0, "]"));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerReflow(0, "x"));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerBeginSpeculative(0));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerCommitSpeculative(0));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerAbortSpeculative(0));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerResolve(0));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerIfBreak(0, ","));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerComment(0, "  // c"));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerShiftIndent(0, 2));
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerDeferred(0, handle));
        assertEquals(7, handle[0]);
        boolean[] live = {true};
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerIsLive(0, live));
        assertTrue(live[0]);
        assertEquals(Abi.E_INVALID_HANDLE, lib.printerOptionsSetMaxWidth(0, 10));
        lib.printerOptionsFree(0);
        lib.printerFree(0);
    }

    @Test
    void aDocumentWithOptionsLaysOutAndSplicesDeferredRegions() {
        RealLibhegel lib = real();
        long options = lib.printerOptionsNew();
        assertEquals(Abi.E_INVALID_ARG, lib.printerOptionsSetMaxWidth(options, 0));
        assertEquals(Abi.OK, lib.printerOptionsSetMaxWidth(options, 8));
        long printer = lib.printerNew(options);
        lib.printerOptionsFree(options);
        assertEquals(Abi.OK, lib.printerBeginGroup(printer, 1, "["));
        assertEquals(Abi.OK, lib.printerText(printer, "1000,"));
        assertEquals(Abi.OK, lib.printerShiftIndent(printer, 0));
        assertEquals(Abi.OK, lib.printerBreakable(printer, " "));
        long[] slot = new long[1];
        assertEquals(Abi.OK, lib.printerDeferred(printer, slot));
        assertEquals(Abi.OK, lib.printerIfBreak(printer, ","));
        assertEquals(Abi.OK, lib.printerEndGroup(printer, "]"));
        assertEquals(Abi.OK, lib.printerComment(printer, "  // c"));
        assertEquals(Abi.E_INVALID_ARG, lib.printerComment(printer, "a\nb"));
        boolean[] live = new boolean[1];
        assertEquals(Abi.OK, lib.printerIsLive(slot[0], live));
        assertTrue(live[0]);
        assertEquals(Abi.OK, lib.printerText(slot[0], "2000"));
        assertEquals(Abi.OK, lib.printerResolve(printer));
        assertEquals(Abi.OK, lib.printerIsLive(slot[0], live));
        assertFalse(live[0]);
        String[] text = new String[1];
        assertEquals(Abi.OK, lib.printerValue(printer, text));
        assertEquals("[1000,\n 2000,]  // c", text[0]);
        lib.printerFree(slot[0]);
        lib.printerFree(printer);
    }

    @Test
    void aStandaloneDocumentRoundTrips() {
        RealLibhegel lib = real();
        long printer = lib.printerNew();
        assertEquals(Abi.OK, lib.printerBeginGroup(printer, 1, "["));
        assertEquals(Abi.OK, lib.printerText(printer, "a"));
        assertEquals(Abi.OK, lib.printerText(printer, ","));
        assertEquals(Abi.OK, lib.printerBreakable(printer, " "));
        assertEquals(Abi.OK, lib.printerReflow(printer, "Point(x=1, y=\"\u00e9\")"));
        assertEquals(Abi.OK, lib.printerEndGroup(printer, "]"));
        assertEquals(Abi.E_INVALID_ARG, lib.printerResolve(printer));
        String[] text = new String[1];
        assertEquals(Abi.OK, lib.printerValue(printer, text));
        assertEquals("[a, Point(x=1, y=\"\u00e9\")]", text[0]);
        lib.printerFree(printer);
    }

    @Test
    void structuredDrawsReportNullHandles() {
        RealLibhegel lib = real();
        java.time.LocalDate d = java.time.LocalDate.of(2000, 1, 1);
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateDate(0, d, d, new java.time.LocalDate[1]));
        java.time.LocalTime t = java.time.LocalTime.NOON;
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateTime(0, t, t, new java.time.LocalTime[1]));
        java.time.LocalDateTime dt = java.time.LocalDateTime.of(d, t);
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateDatetime(0, dt, dt, new java.time.LocalDateTime[1]));
    }

    @Test
    void booleanDrawReportsNullHandle() {
        RealLibhegel lib = real();
        // Seeded true because the draw's scratch segment is arena-allocated and therefore zeroed:
        // a copy-out on this failed call would read back false rather than leave the value alone.
        boolean[] out = {true};
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateBoolean(0, 0.5, out));
        assertTrue(out[0]);
    }

    @Test
    void fixedBytesDrawsReportNullHandles() {
        RealLibhegel lib = real();
        // A NULL test case is rejected before the engine writes anything, so fixedBytesDraw never
        // reaches its copy-out and each buffer keeps the caller's bytes.
        byte[] uuid = sentinel(16);
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateUuid(0, 0, false, uuid));
        assertArrayEquals(sentinel(16), uuid);

        byte[] ipv4 = sentinel(4);
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateIpv4(0, ipv4));
        assertArrayEquals(sentinel(4), ipv4);

        byte[] ipv6 = sentinel(16);
        assertEquals(Abi.E_INVALID_HANDLE, lib.generateIpv6(0, ipv6));
        assertArrayEquals(sentinel(16), ipv6);
    }

    /**
     * A buffer of non-zero bytes. The draw's scratch segment is arena-allocated and therefore
     * zeroed, so an unwanted copy-out would blank the buffer rather than leave it as it was.
     */
    private static byte[] sentinel(int length) {
        byte[] b = new byte[length];
        java.util.Arrays.fill(b, (byte) 0x7f);
        return b;
    }

    @Test
    void stateMachineCallsReportNullHandle() {
        RealLibhegel lib = real();
        // Both handles are NULL: the engine rejects the call on the test case before it
        // dereferences the state machine, so this is a clean error rather than undefined behaviour.
        // Out-parameters are read only on success, so a failed call leaves the caller's values alone.
        long[] out = {7};
        long[] concurrency = {9};
        boolean[] check = {true};
        assertEquals(
                Abi.E_INVALID_HANDLE,
                lib.newStateMachine(
                        0,
                        List.of("r"),
                        new long[] {0},
                        null,
                        List.of("i"),
                        new boolean[] {true},
                        1,
                        1,
                        50,
                        out,
                        concurrency));
        assertEquals(7, out[0]);
        assertEquals(9, concurrency[0]);
        // Explicit weights are marshalled too (NULL above keeps the engine's all-equal default).
        assertEquals(
                Abi.E_INVALID_HANDLE,
                lib.newStateMachine(
                        0,
                        List.of("r"),
                        new long[] {0},
                        new double[] {2.5},
                        List.of("i"),
                        new boolean[] {true},
                        1,
                        1,
                        50,
                        out,
                        concurrency));
        assertEquals(7, out[0]);
        assertEquals(Abi.E_INVALID_HANDLE, lib.stateMachineNextGroup(0, 0, out));
        assertEquals(7, out[0]);
        assertEquals(Abi.E_INVALID_HANDLE, lib.stateMachineNextRule(0, 0, 0, out));
        assertEquals(7, out[0]);
        assertEquals(Abi.E_INVALID_HANDLE, lib.stateMachineRuleRejected(0, 0, 0));
        assertEquals(Abi.E_INVALID_HANDLE, lib.stateMachineShouldCheckInvariant(0, 0, 0, check));
        assertTrue(check[0]);
        // Freeing NULL is a documented no-op.
        lib.stateMachineFree(0);
    }

    @Test
    void profilesAndTheNewerSettingsCrossTheBoundary() {
        RealLibhegel lib = real();
        long[] out = {7};
        assertEquals(Abi.E_INVALID_ARG, lib.settingsNewForProfile("no-such-profile", out));
        assertEquals(0, out[0]);
        assertFalse(lib.lastErrorMessage().isEmpty());
        assertEquals(Abi.OK, lib.settingsNewForProfile("base", out));
        long s = out[0];
        lib.settingsVerbosity(s, Abi.VERBOSITY_DEBUG);
        assertEquals(Abi.VERBOSITY_DEBUG, lib.settingsGetVerbosity(s));
        lib.settingsStopOnFirstFailure(s, false);
        lib.settingsShowStatistics(s, true);
        lib.settingsUnboundedChoices(s, true);
        lib.settingsTestLocation(s, "a/B.java", 3, "a.B", "f");
        assertEquals(Abi.E_INVALID_ARG, lib.settingsRegisterProfile("base", s));
        assertEquals(Abi.OK, lib.settingsRegisterProfile("reallibhegel-binding-test", s));
        lib.settingsFree(s);
        assertEquals(Abi.E_INVALID_ARG, lib.setDefaultProfile("default"));
        assertEquals(Abi.OK, lib.setDefaultProfile(""));
    }

    @Test
    void labelsAndErrorKindsCrossTheBoundary() {
        RealLibhegel lib = real();
        long list = lib.labelFromName("dev.hegel.list");
        assertEquals(0x588C222984E69C23L, list);
        assertEquals(-0x6BA08FBF26A1271AL, lib.labelCombine(new long[] {list, -0x57664BBC5FB7D3A5L}));
        assertEquals(-0x340D631B7BDDDCDBL, lib.labelCombine(new long[0]));
        assertThrows(HegelException.class, () -> lib.runResultErrorKind(0, new int[1]));
    }

    @Test
    void collectionAndPoolFreesAcceptNull() {
        RealLibhegel lib = real();
        lib.collectionFree(0);
        lib.repeatFree(0);
        lib.poolFree(0);
    }

    @Test
    void repeatCallsOnANullHandleAreRejected() {
        RealLibhegel lib = real();
        assertEquals(Abi.E_INVALID_HANDLE, lib.newRepeat(0, new long[1]));
        assertEquals(Abi.E_INVALID_HANDLE, lib.repeatMore(0, 0, new boolean[1]));
        assertEquals(Abi.E_INVALID_HANDLE, lib.repeatFailed(0, 0));
    }

    @Test
    void releasedThreadContextIsRecreatedOnNextUse() {
        RealLibhegel lib = real();
        lib.releaseThreadContext();
        assertEquals(Abi.E_INVALID_HANDLE, lib.stopSpan(0, false));
        assertFalse(lib.lastErrorMessage().isEmpty());
        lib.releaseThreadContext();
        assertEquals("", lib.lastErrorMessage());
        lib.releaseThreadContext();
    }

    @Test
    void anEndedThreadsContextIsFreedWithoutBeingReleased() throws InterruptedException {
        RealLibhegel lib = real();
        Thread thread = new Thread(() -> lib.version());
        thread.start();
        thread.join();
        assertEquals(1, lib.liveContexts());
        thread = null;
        long deadline = System.nanoTime() + 60_000_000_000L;
        while (lib.liveContexts() > 0 && System.nanoTime() < deadline) {
            System.gc();
            Thread.sleep(10);
        }
        assertEquals(0, lib.liveContexts());
    }

    @Test
    void aCloneDrivenFromAnotherThreadHasItsContextFreedOnceTheThreadEnds() throws InterruptedException {
        RealLibhegel lib = real();
        lib.version();
        Thread[] cloneThread = new Thread[1];
        Runner.run(
                        lib,
                        new Settings().database(Database.disabled()).testCases(1),
                        tc -> {
                            TestCase clone = tc.clone();
                            cloneThread[0] = new Thread(() -> clone.draw(Generators.integers()));
                            cloneThread[0].start();
                            try {
                                cloneThread[0].join();
                            } catch (InterruptedException e) {
                                throw new AssertionError(e);
                            }
                        },
                        Reporter.silent())
                .throwIfFailed();
        assertEquals(2, lib.liveContexts());
        cloneThread[0] = null;
        long deadline = System.nanoTime() + 60_000_000_000L;
        while (lib.liveContexts() > 1 && System.nanoTime() < deadline) {
            System.gc();
            Thread.sleep(10);
        }
        assertEquals(1, lib.liveContexts());
    }

    @Test
    void releasingAThreadsContextFreesIt() {
        RealLibhegel lib = real();
        lib.version();
        assertEquals(1, lib.liveContexts());
        lib.releaseThreadContext();
        assertEquals(0, lib.liveContexts());
    }

    @Test
    void settingsAreSetFromStrings() {
        RealLibhegel lib = real();
        long s = newSettings(lib);
        assertEquals(Abi.OK, lib.settingsSetFromString(s, "test_cases", "11"));
        assertEquals(11, lib.settingsGetTestCases(s));
        assertEquals(Abi.E_INVALID_ARG, lib.settingsSetFromString(s, "test_cases", "many"));
        assertTrue(lib.lastErrorMessage().contains("test_cases"), lib.lastErrorMessage());
        assertEquals(11, lib.settingsGetTestCases(s));
        lib.settingsFree(s);
    }

    @Test
    void settingsGettersReadTheHandleBack() {
        RealLibhegel lib = real();
        long s = newSettings(lib);
        lib.settingsTestCases(s, 7);
        assertEquals(7, lib.settingsGetTestCases(s));
        lib.settingsPrintBlob(s, false);
        assertFalse(lib.settingsGetPrintBlob(s));
        lib.settingsPrintBlob(s, true);
        assertTrue(lib.settingsGetPrintBlob(s));
        lib.settingsNondeterminismStrictness(s, Abi.NONDETERMINISM_ERROR);
        assertEquals(Abi.NONDETERMINISM_ERROR, lib.settingsGetNondeterminismStrictness(s));
        lib.settingsNondeterminismStrictness(s, Abi.NONDETERMINISM_WARN);
        assertEquals(Abi.NONDETERMINISM_WARN, lib.settingsGetNondeterminismStrictness(s));
        lib.settingsSeed(s, -1L, true);
        assertEquals(OptionalLong.of(-1L), lib.settingsGetSeed(s));
        lib.settingsSeed(s, 0L, false);
        assertEquals(OptionalLong.empty(), lib.settingsGetSeed(s));
        lib.settingsDerandomize(s, true);
        assertTrue(lib.settingsGetDerandomize(s));
        lib.settingsDerandomize(s, false);
        assertFalse(lib.settingsGetDerandomize(s));
        lib.settingsDatabase(s, "/tmp/hegel-db");
        assertEquals("/tmp/hegel-db", lib.settingsGetDatabase(s));
        lib.settingsDatabase(s, "");
        assertEquals("", lib.settingsGetDatabase(s));
        lib.settingsDatabase(s, null);
        assertNull(lib.settingsGetDatabase(s));
        lib.settingsSuppressHealthCheck(s, Abi.HC_TOO_SLOW | Abi.HC_FILTER_TOO_MUCH);
        assertEquals(Abi.HC_TOO_SLOW | Abi.HC_FILTER_TOO_MUCH, lib.settingsGetSuppressHealthCheck(s));
        lib.settingsStopOnFirstFailure(s, false);
        assertFalse(lib.settingsGetStopOnFirstFailure(s));
        lib.settingsStopOnFirstFailure(s, true);
        assertTrue(lib.settingsGetStopOnFirstFailure(s));
        lib.settingsShowStatistics(s, true);
        assertTrue(lib.settingsGetShowStatistics(s));
        lib.settingsUnboundedChoices(s, true);
        assertTrue(lib.settingsGetUnboundedChoices(s));
        lib.settingsBackend(s, Abi.BACKEND_URANDOM);
        assertEquals(Abi.BACKEND_URANDOM, lib.settingsGetBackend(s));
        lib.settingsFree(s);
    }

    private static long newSettings(RealLibhegel lib) {
        long[] out = new long[1];
        assertEquals(Abi.OK, lib.settingsNew(out));
        return out[0];
    }

    @Test
    void undecodableBlobWithDefaultOutputIsRejected() {
        RealLibhegel lib = real();
        long s = newSettings(lib);
        long[] out = new long[1];
        // A null output callback leaves replay output on stderr; the garbage blob is rejected.
        assertEquals(Abi.E_INVALID_ARG, lib.testCaseFromBlob(s, "not-a-blob!!!", null, out));
        lib.settingsFree(s);
    }

    @Test
    void blobRunWithDefaultOutputReplaysTheBlob() {
        RealLibhegel lib = real();
        RunReport report = Hegel.run(
                tc -> {
                    tc.draw(Generators.integers(), "x");
                    throw new AssertionError("boom");
                },
                new Settings().database(Database.disabled()).seed(1),
                Reporter.silent());
        String blob = report.failures().get(0).reproduceBlob().orElseThrow();
        long s = newSettings(lib);
        lib.settingsDatabase(s, "");
        // A null output callback leaves the run's output on stderr. Marking every replay valid
        // pumps the run dry without a failure, which is the engine's "stale blob" verdict.
        long run = lib.runStartBlob(s, blob, null);
        assertNotEquals(0, run);
        long tc;
        while ((tc = lib.nextTestCase(run)) != 0) {
            assertEquals(Abi.OK, lib.markComplete(tc, Abi.STATUS_VALID, null));
            lib.testCaseFree(tc);
        }
        long result = lib.runResult(run);
        assertEquals(Abi.RUN_STATUS_PASSED, lib.runResultStatus(result));
        lib.runResultFree(result);
        lib.runFree(run);
        lib.settingsFree(s);
    }

    @Test
    void undecodableBlobRunIsRejectedBeforeItStarts() {
        RealLibhegel lib = real();
        long s = newSettings(lib);
        // Since libhegel 0.44.1 hegel_run_start_blob rejects a blob it cannot decode with
        // E_INVALID_ARG instead of starting a run that ends in an error.
        HegelException e = assertThrows(HegelException.class, () -> lib.runStartBlob(s, "not-a-blob!!!", null));
        assertTrue(e.getMessage().contains("could not be decoded"), e.getMessage());
        lib.settingsFree(s);
    }

    @Test
    void aGrammarDrawIsReadOnlyOnceComplete() {
        RealLibhegel lib = real();
        long[] builder = new long[1];
        assertEquals(Abi.OK, lib.grammarBuilderNew("start: \"a\"", builder));
        String[] path = new String[1];
        assertEquals(Abi.E_NOT_FOUND, lib.grammarBuilderFile(builder[0], 0, path));
        assertNull(path[0]);
        long[] grammar = new long[1];
        assertEquals(Abi.OK, lib.grammarNew(builder[0], null, 0, 0, 0, List.of(), grammar));
        lib.grammarBuilderFree(builder[0]);
        long draw = lib.grammarDrawNew(grammar[0], 0);
        String[] out = new String[1];
        assertEquals(Abi.E_NOT_COMPLETE, lib.grammarDrawResult(draw, out));
        assertNull(out[0]);
        assertEquals(Abi.E_INVALID_ARG, lib.grammarDrawProvide(draw, "x"));
        lib.grammarDrawFree(draw);
        lib.grammarFree(grammar[0]);
        assertThrows(HegelException.class, () -> lib.grammarDrawNew(0, 0));
        lib.grammarDrawFree(0);
        lib.grammarFree(0);
        lib.grammarBuilderFree(0);
    }

    @Test
    void regexGeneratorAcceptsATextAlphabet() {
        RealLibhegel lib = real();
        long[] alphabet = new long[1];
        assertEquals(
                Abi.OK,
                lib.stringGeneratorText(0, 5, "ascii", 0, Abi.NO_MAX_CODEPOINT, null, null, null, null, alphabet));
        long[] regex = new long[1];
        assertEquals(Abi.OK, lib.stringGeneratorRegex("[a-z]+", true, alphabet[0], regex));
        lib.stringGeneratorFree(regex[0]);
        lib.stringGeneratorFree(alphabet[0]);
    }

    @Test
    void regexPatternCrossesAsALengthDelimitedBuffer() {
        // A NUL inside the pattern must survive the call (the pattern crosses as a hegel_string_t:
        // pointer + byte length), and a null pattern crosses as {NULL, 0}, which the engine reads
        // as the empty pattern rather than crashing on it.
        RealLibhegel lib = real();
        long[] regex = new long[1];
        assertEquals(Abi.OK, lib.stringGeneratorRegex("a\0b", true, 0, regex));
        assertTrue(regex[0] != 0);
        lib.stringGeneratorFree(regex[0]);
        assertEquals(Abi.OK, lib.stringGeneratorRegex(null, true, 0, regex));
        assertTrue(regex[0] != 0);
        lib.stringGeneratorFree(regex[0]);
    }

    @Test
    void stringsRoundTripThroughTheEngine() {
        // Strings now cross in both directions as {data, len} structs: a by-value input (the
        // target label), a nullable pointer (mark_complete's origin, the database path), a
        // contiguous array (state-machine names), and the by-value/by-pointer outputs the result
        // readers decode. A failure's origin coming back as the string sent proves the round trip.
        RealLibhegel lib = real();
        assertNotNull(lib.version());
        String message = lib.lastErrorMessage();
        assertTrue(message == null || message.isEmpty(), String.valueOf(message));
        long s = newSettings(lib);
        lib.settingsDatabase(s, "");
        lib.settingsTestCases(s, 5);
        long run = lib.runStart(s, line -> {});
        long tc;
        while ((tc = lib.nextTestCase(run)) != 0) {
            long[] value = new long[1];
            int code = lib.generateInteger(tc, 0, 10, value);
            if (code == Abi.OK) {
                assertEquals(Abi.OK, lib.target(tc, value[0], "héllo wörld"));
                assertEquals(Abi.OK, lib.markComplete(tc, Abi.STATUS_INTERESTING, "origin ✓"));
            } else {
                assertEquals(Abi.OK, lib.markComplete(tc, Abi.STATUS_OVERRUN, null));
            }
            lib.testCaseFree(tc);
        }
        long result = lib.runResult(run);
        assertEquals(Abi.RUN_STATUS_FAILED, lib.runResultStatus(result));
        assertNull(lib.runResultError(result));
        assertEquals(1, lib.runResultFailureCount(result));
        assertEquals("origin ✓", lib.failureOrigin(result, 0));
        assertNotNull(lib.failureBlob(result, 0));
        assertNull(lib.failureCaveat(result, 0));
        lib.runResultFree(result);
        lib.runFree(run);
        lib.settingsFree(s);
    }
}
