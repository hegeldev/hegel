package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;

/** Covers {@link LiveDataSource} return-code translation and the abort short-circuit. */
class BindingErrorPathsTest {
    private static final LocalDate DATE = LocalDate.of(2000, 1, 1);
    private static final LocalTime TIME = LocalTime.NOON;
    private static final LocalDateTime DATETIME = LocalDateTime.of(DATE, TIME);

    private LiveDataSource source(FakeLibhegel fake) {
        return new LiveDataSource(fake, FakeLibhegel.TC);
    }

    @Test
    void okPathsReturnTheEngineValues() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.booleanValue = true;
        fake.integerValue = 9L;
        fake.floatValue = 2.5;
        fake.stringValue = "drawn";
        LiveDataSource ds = source(fake);
        assertTrue(ds.generateBoolean(0.5));
        assertEquals(9, ds.generateInteger(0, 10));
        assertEquals(2.5, ds.generateFloat(64, 0, 10, false, false, false, false, Double.MIN_VALUE));
        assertArrayEquals(new byte[] {1, 2}, ds.generateBytes(0, 4));
        assertEquals("drawn", ds.generateString(ds.emailGenerator()));
        assertEquals(DATE, ds.generateDate(DATE, DATE, null));
        assertEquals(TIME, ds.generateTime(TIME, TIME, Abi.PRECISION_NANOSECONDS));
        assertEquals(DATETIME, ds.generateDatetime(DATETIME, DATETIME, Abi.PRECISION_NANOSECONDS));
        assertEquals(Instant.EPOCH, ds.generateTimestamp(Instant.EPOCH, Instant.EPOCH, Abi.PRECISION_SECONDS));
        assertEquals(-3600, ds.generateOffset(-3600, 3600, Abi.OFFSET_GRANULARITY_HOURS));
        assertEquals(Duration.ZERO, ds.generateDuration(Duration.ZERO, Duration.ZERO, Abi.PRECISION_NANOSECONDS));
        SpecHandle zones = ds.timezoneSpec(List.of("UTC"), false, true);
        assertEquals(List.of("UTC"), fake.timezoneNames);
        assertEquals(false, fake.timezoneLinks);
        assertEquals(true, fake.timezoneLegacy);
        assertEquals("UTC", ds.generateTimezone(zones));
        SpecHandle zoned = ds.zonedSpec(
                DATETIME, DATETIME, null, null, Abi.PRECISION_MINUTES, Abi.IMAGINARY_AVOID, Abi.AMBIGUOUS_LATER);
        assertEquals(DATETIME, fake.civilBounds[0]);
        assertEquals(Abi.PRECISION_MINUTES, fake.zonedPrecision);
        assertEquals(Abi.IMAGINARY_AVOID, fake.zonedImaginary);
        assertEquals(Abi.AMBIGUOUS_LATER, fake.zonedAmbiguous);
        assertEquals(fake.zonedValue, ds.generateZonedDatetime(zones, zoned));
        SpecHandle instants = ds.zonedSpec(
                null, null, Instant.EPOCH, Instant.EPOCH, Abi.PRECISION_HOURS, Abi.IMAGINARY_ALLOW, Abi.AMBIGUOUS_BOTH);
        assertEquals(Instant.EPOCH, fake.instantBounds[1]);
        assertTrue(ds.ownsSpec(instants));
        SpecHandle span =
                ds.spanSpec(new int[] {Abi.SPAN_UNIT_DAYS}, new long[] {1}, new long[] {2}, Abi.SPAN_SIGN_MIXED);
        assertEquals(Abi.SPAN_SIGN_MIXED, fake.spanSign);
        assertArrayEquals(new long[] {Abi.SPAN_UNIT_DAYS, 1, 2}, fake.spanUnitBounds.get(0));
        assertArrayEquals(new long[10], ds.generateSpan(span));
        SpecHandle options = ds.dateOptions(0b1, 0b11);
        assertEquals(1, fake.weekdayMask);
        assertEquals(3, fake.monthMask);
        assertEquals(DATE, ds.generateDate(DATE, DATE, options));
        assertEquals(FakeLibhegel.DATE_OPTIONS, fake.dateOptionsPassed);
        assertTrue(ds.timezoneInfo("UTC").known());
        assertFalse(ds.timezoneInfo("Mars/Olympus_Mons").known());
        assertEquals(new UUID(0, 1), ds.generateUuid(null));
        assertArrayEquals(new byte[] {127, 0, 0, 1}, ds.generateIpv4());
        assertArrayEquals(new byte[16], ds.generateIpv6());
        assertEquals(7, ds.newCollection(0, 5));
        assertFalse(ds.collectionMore(7));
        ds.collectionReject(7, "dup");
        ds.startSpan(Label.LIST);
        ds.stopSpan(false);
        ds.target(1.0, "l");
        assertEquals(3, ds.newPool());
        assertEquals(0, ds.poolAdd(3));
        assertEquals(0, ds.poolGenerate(3, true));
        fake.stateMachineConcurrency = 3;
        DataSource.StateMachine sm =
                ds.newStateMachine(List.of("r"), new long[] {0}, null, List.of("i"), new boolean[] {false}, 1, 4, 50);
        assertEquals(5, sm.id());
        assertEquals(3, sm.concurrency());
        assertEquals(1, fake.stateMachineMinConcurrency);
        assertEquals(4, fake.stateMachineMaxConcurrency);
        assertEquals(Abi.STATE_MACHINE_DONE, ds.stateMachineNextGroup(5));
        assertEquals(Abi.STATE_MACHINE_DONE, ds.stateMachineNextRule(5, 2));
        ds.stateMachineRuleRejected(5, 2);
        assertEquals(List.of(2L), fake.nextRuleWorkers);
        assertEquals(List.of(2L), fake.rejectedWorkers);
        assertTrue(ds.stateMachineShouldCheckInvariant(5, 0));
        ds.stateMachineFree(5);
        assertEquals(1, fake.freedStateMachines);
    }

    @Test
    void clonesDrawThroughTheirOwnHandleAndAreReleased() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.integerValue = 4L;
        LiveDataSource ds = source(fake);
        DataSource clone = ds.cloneForWorker(2);
        assertEquals(List.of(FakeLibhegel.TC), fake.clonesMade);
        assertEquals(2L, fake.workersSet.get(FakeLibhegel.CLONE_BASE));
        assertEquals(4, clone.generateInteger(0, 10));
        clone.release();
        assertEquals(List.of(FakeLibhegel.CLONE_BASE), fake.freedClones);
        // The root handle is untouched by the clone's release.
        assertEquals(1, fake.freedTestCases);
    }

    @Test
    void cloneFailuresPropagateAndLeakNothing() {
        FakeLibhegel cloneFails = new FakeLibhegel();
        cloneFails.cloneRc = Abi.E_BACKEND;
        assertThrows(HegelException.class, () -> source(cloneFails).cloneForWorker(0));
        assertTrue(cloneFails.freedClones.isEmpty());

        // A clone past the end of a replayed sequence is an overrun, like any other draw.
        FakeLibhegel exhausted = new FakeLibhegel();
        exhausted.cloneRc = Abi.E_STOP_TEST;
        LiveDataSource ds0 = source(exhausted);
        assertThrows(StopTest.class, () -> ds0.cloneForWorker(0));
        assertTrue(ds0.isAborted());

        FakeLibhegel workerFails = new FakeLibhegel();
        workerFails.setWorkerFails = true;
        assertThrows(HegelException.class, () -> source(workerFails).cloneForWorker(0));
        // The clone was made, so it is released before the failure surfaces.
        assertEquals(List.of(FakeLibhegel.CLONE_BASE), workerFails.freedClones);

        // Cloning is a draw: it short-circuits once the case is aborted.
        FakeLibhegel aborted = new FakeLibhegel();
        aborted.generateIntegerRc = Abi.E_STOP_TEST;
        LiveDataSource ds = source(aborted);
        assertThrows(StopTest.class, () -> ds.generateInteger(0, 1));
        assertThrows(StopTest.class, () -> ds.cloneForWorker(0));
        assertTrue(aborted.clonesMade.isEmpty());
    }

    @Test
    void concurrentUseOfOneHandleIsExplained() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.poolAddRc = Abi.E_CONCURRENT_USE;
        HegelException e = assertThrows(HegelException.class, () -> source(fake).poolAdd(3));
        assertTrue(e.getMessage().contains("two threads"), e.getMessage());
        assertTrue(e.getMessage().contains("ConcurrentPool"), e.getMessage());
    }

    @Test
    void stringGeneratorConstructionAndOwnership() {
        FakeLibhegel fake = new FakeLibhegel();
        LiveDataSource ds = source(fake);
        StringGeneratorHandle text = ds.textGenerator(0, 10, null, 0, Abi.NO_MAX_CODEPOINT, null, null, null, null);
        StringGeneratorHandle regex = ds.regexGenerator("[a-z]", true, text);
        StringGeneratorHandle regexNoAlphabet = ds.regexGenerator("[a-z]", true, null);
        StringGeneratorHandle email = ds.emailGenerator();
        StringGeneratorHandle url = ds.urlGenerator();
        StringGeneratorHandle domain = ds.domainGenerator(255);
        for (StringGeneratorHandle h : List.of(text, regex, regexNoAlphabet, email, url, domain)) {
            assertNotNull(h);
            assertTrue(ds.ownsStringGenerator(h));
        }
        // A handle built by a different binding is not owned, so caches rebuild it.
        assertFalse(source(new FakeLibhegel()).ownsStringGenerator(text));
    }

    @Test
    void stopTestUnwindsAndAbortsEverything() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.generateIntegerRc = Abi.E_STOP_TEST;
        LiveDataSource ds = source(fake);
        assertThrows(StopTest.class, () -> ds.generateInteger(0, 1));
        assertTrue(ds.isAborted());
        // Subsequent value-producing primitives short-circuit to StopTest without touching libhegel.
        assertThrows(StopTest.class, () -> ds.generateBoolean(0.5));
        assertThrows(StopTest.class, () -> ds.generateInteger(0, 1));
        assertThrows(StopTest.class, () -> ds.generateFloat(64, 0, 1, false, false, false, false, 1e-300));
        assertThrows(StopTest.class, () -> ds.generateBytes(0, 1));
        assertThrows(StopTest.class, () -> ds.generateString(null));
        assertThrows(StopTest.class, () -> ds.generateDate(DATE, DATE, null));
        assertThrows(StopTest.class, () -> ds.generateTime(TIME, TIME, Abi.PRECISION_NANOSECONDS));
        assertThrows(StopTest.class, () -> ds.generateDatetime(DATETIME, DATETIME, Abi.PRECISION_NANOSECONDS));
        assertThrows(StopTest.class, () -> ds.generateTimestamp(Instant.EPOCH, Instant.EPOCH, Abi.PRECISION_SECONDS));
        assertThrows(StopTest.class, () -> ds.generateOffset(0, 0, Abi.OFFSET_GRANULARITY_SECONDS));
        assertThrows(
                StopTest.class, () -> ds.generateDuration(Duration.ZERO, Duration.ZERO, Abi.PRECISION_NANOSECONDS));
        assertThrows(StopTest.class, () -> ds.generateTimezone(null));
        assertThrows(StopTest.class, () -> ds.generateZonedDatetime(null, null));
        assertThrows(StopTest.class, () -> ds.generateSpan(null));
        assertThrows(StopTest.class, () -> ds.dateOptions(1, 1));
        assertThrows(StopTest.class, () -> ds.timezoneSpec(List.of("UTC"), true, false));
        assertThrows(
                StopTest.class,
                () -> ds.zonedSpec(
                        DATETIME,
                        DATETIME,
                        null,
                        null,
                        Abi.PRECISION_NANOSECONDS,
                        Abi.IMAGINARY_ALLOW,
                        Abi.AMBIGUOUS_BOTH));
        assertThrows(StopTest.class, () -> ds.spanSpec(new int[0], new long[0], new long[0], Abi.SPAN_SIGN_MIXED));
        assertThrows(StopTest.class, () -> ds.generateUuid(4));
        assertThrows(StopTest.class, () -> ds.generateIpv4());
        assertThrows(StopTest.class, () -> ds.generateIpv6());
        assertThrows(
                StopTest.class, () -> ds.textGenerator(0, 1, null, 0, Abi.NO_MAX_CODEPOINT, null, null, null, null));
        assertThrows(StopTest.class, () -> ds.regexGenerator("x", true, null));
        assertThrows(StopTest.class, () -> ds.emailGenerator());
        assertThrows(StopTest.class, () -> ds.urlGenerator());
        assertThrows(StopTest.class, () -> ds.domainGenerator(10));
        assertThrows(StopTest.class, () -> ds.startSpan(Label.LIST));
        assertThrows(StopTest.class, () -> ds.newCollection(0, 1));
        assertThrows(StopTest.class, () -> ds.collectionMore(1));
        assertThrows(StopTest.class, () -> ds.collectionReject(1, "x"));
        assertThrows(StopTest.class, () -> ds.newPool());
        assertThrows(StopTest.class, () -> ds.poolAdd(1));
        assertThrows(StopTest.class, () -> ds.poolGenerate(1, false));
        assertThrows(
                StopTest.class,
                () -> ds.newStateMachine(List.of("r"), new long[] {0}, null, List.of(), new boolean[0], 1, 1, 50));
        assertThrows(StopTest.class, () -> ds.stateMachineNextGroup(1));
        assertThrows(StopTest.class, () -> ds.stateMachineNextRule(1, 0));
        assertThrows(StopTest.class, () -> ds.stateMachineRuleRejected(1, 0));
        assertThrows(StopTest.class, () -> ds.stateMachineShouldCheckInvariant(1, 0));
        // Freeing the machine is not a draw: it must still work once the case is aborted.
        ds.stateMachineFree(1);
        assertEquals(1, fake.freedStateMachines);
        assertThrows(StopTest.class, () -> ds.target(1.0, "l"));
        // stopSpan is a no-op once aborted (used by span-closing finally blocks).
        ds.stopSpan(false);
    }

    @Test
    void assumeUnwindsAndAborts() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.generateStringRc = Abi.E_ASSUME;
        LiveDataSource ds = source(fake);
        StringGeneratorHandle email = ds.emailGenerator();
        assertThrows(AssumeRejected.class, () -> ds.generateString(email));
        assertTrue(ds.isAborted());
    }

    @Test
    void rejectedSpecSettersReleaseTheHandle() {
        FakeLibhegel weekdays = new FakeLibhegel();
        weekdays.dateOptionsSetWeekdaysRc = Abi.E_INVALID_ARG;
        weekdays.lastError = "empty mask";
        IllegalArgumentException e = assertThrows(
                IllegalArgumentException.class, () -> source(weekdays).dateOptions(1, 0));
        assertTrue(e.getMessage().contains("empty mask"));
        assertEquals(1, weekdays.freedDateOptions);

        FakeLibhegel months = new FakeLibhegel();
        months.dateOptionsSetMonthsRc = Abi.E_INVALID_ARG;
        assertThrows(IllegalArgumentException.class, () -> source(months).dateOptions(0, 1));
        assertEquals(1, months.freedDateOptions);

        FakeLibhegel names = new FakeLibhegel();
        names.timezoneSpecSetNamesRc = Abi.E_INVALID_ARG;
        assertThrows(
                IllegalArgumentException.class, () -> source(names).timezoneSpec(List.of("Mars/Olympus"), true, false));
        assertEquals(1, names.freedTimezoneSpecs);

        FakeLibhegel bounds = new FakeLibhegel();
        bounds.zonedSpecSetInstantBoundsRc = Abi.E_INVALID_ARG;
        assertThrows(
                IllegalArgumentException.class,
                () -> source(bounds)
                        .zonedSpec(
                                null,
                                null,
                                Instant.EPOCH,
                                Instant.EPOCH,
                                Abi.PRECISION_NANOSECONDS,
                                Abi.IMAGINARY_ALLOW,
                                Abi.AMBIGUOUS_BOTH));
        assertEquals(1, bounds.freedZonedSpecs);

        FakeLibhegel sign = new FakeLibhegel();
        sign.spanSpecSetSignRc = Abi.E_INVALID_ARG;
        assertThrows(
                IllegalArgumentException.class,
                () -> source(sign).spanSpec(new int[] {Abi.SPAN_UNIT_DAYS}, new long[] {-1}, new long[] {1}, 7));
        assertEquals(1, sign.freedSpanSpecs);
        assertTrue(sign.spanUnitBounds.isEmpty());
    }

    @Test
    void invalidArgBecomesIllegalArgumentException() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.stringGeneratorTextRc = Abi.E_INVALID_ARG;
        fake.lastError = "empty alphabet";
        LiveDataSource ds = source(fake);
        IllegalArgumentException e = assertThrows(
                IllegalArgumentException.class,
                () -> ds.textGenerator(0, 1, null, 0, Abi.NO_MAX_CODEPOINT, List.of(), null, null, null));
        assertTrue(e.getMessage().contains("empty alphabet"));
        assertFalse(ds.isAborted());
    }

    @Test
    void backendErrorBecomesHegelException() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.generateBooleanRc = Abi.E_BACKEND;
        fake.lastError = "boom";
        LiveDataSource ds = source(fake);
        HegelException e = assertThrows(HegelException.class, () -> ds.generateBoolean(0.5));
        assertTrue(e.getMessage().contains("boom"));
        assertFalse(ds.isAborted());
    }

    @Test
    void backendErrorWithNullMessage() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.generateBooleanRc = Abi.E_BACKEND;
        fake.lastError = null;
        assertThrows(HegelException.class, () -> source(fake).generateBoolean(0.5));
    }

    @Test
    void spanAndCollectionErrorsPropagate() {
        FakeLibhegel startSpan = new FakeLibhegel();
        startSpan.startSpanRc = Abi.E_STOP_TEST;
        assertThrows(StopTest.class, () -> source(startSpan).startSpan(Label.LIST));

        FakeLibhegel stopSpan = new FakeLibhegel();
        stopSpan.stopSpanRc = Abi.E_INVALID_HANDLE;
        assertThrows(HegelException.class, () -> source(stopSpan).stopSpan(false));

        FakeLibhegel newCollection = new FakeLibhegel();
        newCollection.newCollectionRc = Abi.E_STOP_TEST;
        assertThrows(StopTest.class, () -> source(newCollection).newCollection(0, 5));

        FakeLibhegel more = new FakeLibhegel();
        more.moreSequence = new boolean[] {true, false};
        LiveDataSource ds = source(more);
        assertTrue(ds.collectionMore(1));
        assertFalse(ds.collectionMore(1));

        FakeLibhegel moreError = new FakeLibhegel();
        moreError.collectionMoreRc = Abi.E_BACKEND;
        assertThrows(HegelException.class, () -> source(moreError).collectionMore(1));

        FakeLibhegel reject = new FakeLibhegel();
        reject.collectionRejectRc = Abi.E_STOP_TEST;
        assertThrows(StopTest.class, () -> source(reject).collectionReject(1, "dup"));

        FakeLibhegel target = new FakeLibhegel();
        target.targetRc = Abi.E_ASSUME;
        assertThrows(AssumeRejected.class, () -> source(target).target(1.0, "l"));
    }

    @Test
    void poolAndStateMachineErrorsPropagate() {
        FakeLibhegel pool = new FakeLibhegel();
        pool.newPoolRc = Abi.E_BACKEND;
        assertThrows(HegelException.class, () -> source(pool).newPool());

        FakeLibhegel add = new FakeLibhegel();
        add.poolAddRc = Abi.E_STOP_TEST;
        assertThrows(StopTest.class, () -> source(add).poolAdd(1));

        FakeLibhegel gen = new FakeLibhegel();
        gen.poolGenerateRc = Abi.E_ASSUME;
        assertThrows(AssumeRejected.class, () -> source(gen).poolGenerate(1, true));

        FakeLibhegel sm = new FakeLibhegel();
        sm.newStateMachineRc = Abi.E_INVALID_ARG;
        assertThrows(
                IllegalArgumentException.class,
                () -> source(sm)
                        .newStateMachine(List.of("r"), new long[] {0}, null, List.of(), new boolean[0], 1, 1, 50));

        FakeLibhegel group = new FakeLibhegel();
        group.stateMachineNextGroupRc = Abi.E_STOP_TEST;
        assertThrows(StopTest.class, () -> source(group).stateMachineNextGroup(1));

        FakeLibhegel next = new FakeLibhegel();
        next.stateMachineNextRuleRc = Abi.E_STOP_TEST;
        assertThrows(StopTest.class, () -> source(next).stateMachineNextRule(1, 0));

        FakeLibhegel rejected = new FakeLibhegel();
        rejected.stateMachineRuleRejectedRc = Abi.E_INVALID_ARG;
        assertThrows(IllegalArgumentException.class, () -> source(rejected).stateMachineRuleRejected(1, 0));

        FakeLibhegel check = new FakeLibhegel();
        check.stateMachineShouldCheckInvariantRc = Abi.E_STOP_TEST;
        assertThrows(StopTest.class, () -> source(check).stateMachineShouldCheckInvariant(1, 0));
    }
}
