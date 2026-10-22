package dev.hegel;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.List;
import java.util.UUID;

/**
 * A {@link DataSource} backed by the real engine for one in-flight test case.
 *
 * <p>Once the engine returns {@code STOP_TEST} (or an assumption is rejected) the source is marked
 * {@code aborted}: value-producing primitives short-circuit by re-raising {@link StopTest} without
 * touching libhegel, and {@link #stopSpan} becomes a no-op so span-closing {@code finally} blocks
 * during unwinding do not call into a case that is already being torn down.
 */
final class LiveDataSource implements DataSource {
    private final Libhegel lib;
    private final long tc;
    private boolean aborted;

    LiveDataSource(Libhegel lib, long tc) {
        this.lib = lib;
        this.tc = tc;
    }

    @Override
    public boolean isAborted() {
        return aborted;
    }

    private void translate(int rc, String op) {
        switch (rc) {
            case Abi.OK:
                return;
            case Abi.E_STOP_TEST:
                aborted = true;
                throw new StopTest();
            case Abi.E_ASSUME:
                aborted = true;
                throw new AssumeRejected();
            case Abi.E_INVALID_ARG:
                throw new IllegalArgumentException(nullToEmpty(lib.lastErrorMessage()));
            case Abi.E_CONCURRENT_USE:
                throw new HegelException("hegel_" + op
                        + ": the test-case handle was used from two threads at once. In a concurrent"
                        + " state machine, draw only through the TestCase handed to the rule and share"
                        + " generated values through a ConcurrentPool.");
            default:
                throw new HegelException(
                        "hegel_" + op + " failed (rc=" + rc + "): " + nullToEmpty(lib.lastErrorMessage()));
        }
    }

    private static String nullToEmpty(String s) {
        return s == null ? "" : s;
    }

    private void checkLive() {
        if (aborted) {
            throw new StopTest();
        }
    }

    @Override
    public boolean generateBoolean(double p) {
        checkLive();
        boolean[] out = new boolean[1];
        translate(lib.generateBoolean(tc, p, out), "generate_boolean");
        return out[0];
    }

    @Override
    public long generateInteger(long min, long max) {
        checkLive();
        long[] out = new long[1];
        translate(lib.generateInteger(tc, min, max, out), "generate_integer");
        return out[0];
    }

    @Override
    public double generateFloat(
            int width,
            double min,
            double max,
            boolean allowNan,
            boolean allowInfinity,
            boolean excludeMin,
            boolean excludeMax,
            double smallestNonzeroMagnitude) {
        checkLive();
        double[] out = new double[1];
        translate(
                lib.generateFloat(
                        tc,
                        width,
                        min,
                        max,
                        allowNan,
                        allowInfinity,
                        excludeMin,
                        excludeMax,
                        smallestNonzeroMagnitude,
                        out),
                "generate_float");
        return out[0];
    }

    @Override
    public byte[] generateBytes(long minSize, long maxSize) {
        checkLive();
        byte[][] out = new byte[1][];
        translate(lib.generateBytes(tc, minSize, maxSize, out), "generate_bytes");
        return out[0];
    }

    @Override
    public String generateString(StringGeneratorHandle generator) {
        checkLive();
        String[] out = new String[1];
        translate(lib.generateString(tc, generator.handle, out), "generate_string");
        return out[0];
    }

    @Override
    public LocalDate generateDate(LocalDate min, LocalDate max, SpecHandle options) {
        checkLive();
        LocalDate[] out = new LocalDate[1];
        translate(lib.generateDate(tc, min, max, options == null ? 0 : options.handle, out), "generate_date");
        return out[0];
    }

    @Override
    public LocalTime generateTime(LocalTime min, LocalTime max, int precision) {
        checkLive();
        LocalTime[] out = new LocalTime[1];
        translate(lib.generateTime(tc, min, max, precision, out), "generate_time");
        return out[0];
    }

    @Override
    public LocalDateTime generateDatetime(LocalDateTime min, LocalDateTime max, int precision) {
        checkLive();
        LocalDateTime[] out = new LocalDateTime[1];
        translate(lib.generateDatetime(tc, min, max, precision, out), "generate_datetime");
        return out[0];
    }

    @Override
    public Instant generateTimestamp(Instant min, Instant max, int precision) {
        checkLive();
        Instant[] out = new Instant[1];
        translate(lib.generateTimestamp(tc, min, max, precision, out), "generate_timestamp");
        return out[0];
    }

    @Override
    public int generateOffset(int minSeconds, int maxSeconds, int granularity) {
        checkLive();
        int[] seconds = new int[1];
        boolean[] known = new boolean[1];
        translate(
                lib.generateOffset(tc, minSeconds, maxSeconds, granularity, false, seconds, known), "generate_offset");
        return seconds[0];
    }

    @Override
    public Duration generateDuration(Duration min, Duration max, int precision) {
        checkLive();
        Duration[] out = new Duration[1];
        translate(lib.generateDuration(tc, min, max, precision, out), "generate_duration");
        return out[0];
    }

    @Override
    public String generateTimezone(SpecHandle spec) {
        checkLive();
        String[] out = new String[1];
        translate(lib.generateTimezone(tc, spec.handle, out), "generate_timezone");
        return out[0];
    }

    @Override
    public Libhegel.ZonedDatetime generateZonedDatetime(SpecHandle tzSpec, SpecHandle zonedSpec) {
        checkLive();
        Libhegel.ZonedDatetime[] out = new Libhegel.ZonedDatetime[1];
        translate(lib.generateZonedDatetime(tc, tzSpec.handle, zonedSpec.handle, out), "generate_zoned_datetime");
        return out[0];
    }

    @Override
    public long[] generateSpan(SpecHandle spec) {
        checkLive();
        long[] out = new long[10];
        translate(lib.generateSpan(tc, spec.handle, out), "generate_span");
        return out;
    }

    private void configure(long handle, SpecHandle.Freer freer, int rc, String op) {
        if (rc != Abi.OK) {
            freer.free(lib, handle);
        }
        translate(rc, op);
    }

    @Override
    public SpecHandle dateOptions(int weekdayMask, int monthMask) {
        checkLive();
        long[] out = new long[1];
        translate(lib.dateOptionsNew(out), "date_options_new");
        SpecHandle.Freer freer = Libhegel::dateOptionsFree;
        if (weekdayMask != 0) {
            configure(out[0], freer, lib.dateOptionsSetWeekdays(out[0], weekdayMask), "date_options_set_weekdays");
        }
        if (monthMask != 0) {
            configure(out[0], freer, lib.dateOptionsSetMonths(out[0], monthMask), "date_options_set_months");
        }
        return new SpecHandle(lib, out[0], freer);
    }

    @Override
    public SpecHandle timezoneSpec(List<String> names, boolean links, boolean legacy) {
        checkLive();
        long[] out = new long[1];
        translate(lib.timezoneSpecNew(out), "timezone_spec_new");
        SpecHandle.Freer freer = Libhegel::timezoneSpecFree;
        configure(out[0], freer, lib.timezoneSpecSetNames(out[0], names), "timezone_spec_set_names");
        lib.timezoneSpecSetLinks(out[0], links);
        lib.timezoneSpecSetLegacy(out[0], legacy);
        return new SpecHandle(lib, out[0], freer);
    }

    @Override
    public SpecHandle zonedSpec(
            LocalDateTime civilMin,
            LocalDateTime civilMax,
            Instant instantMin,
            Instant instantMax,
            int precision,
            int imaginary,
            int ambiguous) {
        checkLive();
        long[] out = new long[1];
        translate(lib.zonedSpecNew(out), "zoned_spec_new");
        long spec = out[0];
        SpecHandle.Freer freer = Libhegel::zonedSpecFree;
        configure(spec, freer, lib.zonedSpecSetPrecision(spec, precision), "zoned_spec_set_precision");
        if (civilMin != null) {
            configure(
                    spec, freer, lib.zonedSpecSetCivilBounds(spec, civilMin, civilMax), "zoned_spec_set_civil_bounds");
        } else {
            configure(
                    spec,
                    freer,
                    lib.zonedSpecSetInstantBounds(spec, instantMin, instantMax),
                    "zoned_spec_set_instant_bounds");
        }
        configure(spec, freer, lib.zonedSpecSetImaginary(spec, imaginary), "zoned_spec_set_imaginary");
        configure(spec, freer, lib.zonedSpecSetAmbiguous(spec, ambiguous), "zoned_spec_set_ambiguous");
        return new SpecHandle(lib, spec, freer);
    }

    @Override
    public SpecHandle spanSpec(int[] units, long[] mins, long[] maxs, int sign) {
        checkLive();
        long[] out = new long[1];
        translate(lib.spanSpecNew(out), "span_spec_new");
        long spec = out[0];
        SpecHandle.Freer freer = Libhegel::spanSpecFree;
        configure(spec, freer, lib.spanSpecSetSign(spec, sign), "span_spec_set_sign");
        for (int i = 0; i < units.length; i++) {
            configure(
                    spec,
                    freer,
                    lib.spanSpecSetUnitBounds(spec, units[i], mins[i], maxs[i]),
                    "span_spec_set_unit_bounds");
        }
        return new SpecHandle(lib, spec, freer);
    }

    @Override
    public boolean ownsSpec(SpecHandle spec) {
        return spec.lib == lib;
    }

    @Override
    public Libhegel.TimezoneInfo timezoneInfo(String name) {
        return lib.timezoneInfo(name);
    }

    @Override
    public UUID generateUuid(Integer version) {
        checkLive();
        byte[] bytes = new byte[16];
        translate(lib.generateUuid(tc, version == null ? 0 : version, version != null, bytes), "generate_uuid");
        long msb = 0;
        long lsb = 0;
        for (int i = 0; i < 8; i++) {
            msb = (msb << 8) | (bytes[i] & 0xffL);
            lsb = (lsb << 8) | (bytes[i + 8] & 0xffL);
        }
        return new UUID(msb, lsb);
    }

    @Override
    public byte[] generateIpv4() {
        checkLive();
        byte[] bytes = new byte[4];
        translate(lib.generateIpv4(tc, bytes), "generate_ipv4");
        return bytes;
    }

    @Override
    public byte[] generateIpv6() {
        checkLive();
        byte[] bytes = new byte[16];
        translate(lib.generateIpv6(tc, bytes), "generate_ipv6");
        return bytes;
    }

    @Override
    public StringGeneratorHandle textGenerator(
            long minSize,
            long maxSize,
            String codec,
            long minCodepoint,
            long maxCodepoint,
            List<String> categories,
            List<String> excludeCategories,
            String includeCharacters,
            String excludeCharacters) {
        checkLive();
        long[] out = new long[1];
        translate(
                lib.stringGeneratorText(
                        minSize,
                        maxSize,
                        codec,
                        minCodepoint,
                        maxCodepoint,
                        categories,
                        excludeCategories,
                        includeCharacters,
                        excludeCharacters,
                        out),
                "string_generator_text");
        return new StringGeneratorHandle(lib, out[0]);
    }

    @Override
    public StringGeneratorHandle regexGenerator(String pattern, boolean fullmatch, StringGeneratorHandle alphabet) {
        checkLive();
        long[] out = new long[1];
        translate(
                lib.stringGeneratorRegex(pattern, fullmatch, alphabet == null ? 0 : alphabet.handle, out),
                "string_generator_regex");
        return new StringGeneratorHandle(lib, out[0]);
    }

    @Override
    public StringGeneratorHandle emailGenerator() {
        checkLive();
        long[] out = new long[1];
        translate(lib.stringGeneratorEmail(out), "string_generator_email");
        return new StringGeneratorHandle(lib, out[0]);
    }

    @Override
    public StringGeneratorHandle urlGenerator() {
        checkLive();
        long[] out = new long[1];
        translate(lib.stringGeneratorUrl(out), "string_generator_url");
        return new StringGeneratorHandle(lib, out[0]);
    }

    @Override
    public StringGeneratorHandle domainGenerator(long maxLength) {
        checkLive();
        long[] out = new long[1];
        translate(lib.stringGeneratorDomain(maxLength, out), "string_generator_domain");
        return new StringGeneratorHandle(lib, out[0]);
    }

    @Override
    public boolean ownsStringGenerator(StringGeneratorHandle generator) {
        return generator.lib == lib;
    }

    @Override
    public void startSpan(long label) {
        checkLive();
        translate(lib.startSpan(tc, label), "start_span");
    }

    @Override
    public void stopSpan(boolean discard) {
        if (aborted) {
            return;
        }
        translate(lib.stopSpan(tc, discard), "stop_span");
    }

    @Override
    public long newCollection(long minSize, long maxSize) {
        checkLive();
        long[] id = new long[1];
        translate(lib.newCollection(tc, minSize, maxSize, id), "new_collection");
        return id[0];
    }

    @Override
    public boolean collectionMore(long id) {
        checkLive();
        boolean[] more = new boolean[1];
        translate(lib.collectionMore(tc, id, more), "collection_more");
        return more[0];
    }

    @Override
    public void collectionReject(long id, String why) {
        checkLive();
        translate(lib.collectionReject(tc, id, why), "collection_reject");
    }

    @Override
    public long newPool() {
        checkLive();
        long[] id = new long[1];
        translate(lib.newPool(tc, id), "new_pool");
        return id[0];
    }

    @Override
    public long poolAdd(long poolId) {
        checkLive();
        long[] id = new long[1];
        translate(lib.poolAdd(tc, poolId, id), "pool_add");
        return id[0];
    }

    @Override
    public long poolGenerate(long poolId, boolean consume) {
        checkLive();
        long[] id = new long[1];
        translate(lib.poolGenerate(tc, poolId, consume, id), "pool_generate");
        return id[0];
    }

    @Override
    public StateMachine newStateMachine(
            List<String> ruleNames,
            long[] ruleGroups,
            double[] ruleWeights,
            List<String> invariantNames,
            boolean[] invariantAlwaysCheck,
            long minConcurrency,
            long maxConcurrency,
            int stepCount) {
        checkLive();
        long[] id = new long[1];
        long[] concurrency = new long[1];
        translate(
                lib.newStateMachine(
                        tc,
                        ruleNames,
                        ruleGroups,
                        ruleWeights,
                        invariantNames,
                        invariantAlwaysCheck,
                        minConcurrency,
                        maxConcurrency,
                        stepCount,
                        id,
                        concurrency),
                "new_state_machine");
        return new StateMachine(id[0], (int) concurrency[0]);
    }

    @Override
    public long stateMachineNextGroup(long stateMachineId) {
        checkLive();
        long[] group = new long[1];
        translate(lib.stateMachineNextGroup(tc, stateMachineId, group), "state_machine_next_group");
        return group[0];
    }

    @Override
    public long stateMachineNextRule(long stateMachineId, long workerIndex) {
        checkLive();
        long[] index = new long[1];
        translate(lib.stateMachineNextRule(tc, stateMachineId, workerIndex, index), "state_machine_next_rule");
        return index[0];
    }

    @Override
    public void stateMachineRuleRejected(long stateMachineId, long workerIndex) {
        checkLive();
        translate(lib.stateMachineRuleRejected(tc, stateMachineId, workerIndex), "state_machine_rule_rejected");
    }

    @Override
    public DataSource cloneForWorker(long workerIndex) {
        checkLive();
        // Cloning consumes a choice position, so it is a draw: on a replay of a shorter sequence
        // it reports STOP_TEST like any other draw, and the case is an overrun.
        long[] clone = new long[1];
        translate(lib.testCaseClone(tc, clone), "test_case_clone");
        try {
            lib.testCaseSetWorker(clone[0], workerIndex);
        } catch (RuntimeException e) {
            lib.testCaseFree(clone[0]);
            throw e;
        }
        return new LiveDataSource(lib, clone[0]);
    }

    @Override
    public void release() {
        // Not gated on `aborted`: the clone handle is caller-owned and must be released exactly once.
        lib.testCaseFree(tc);
    }

    @Override
    public boolean stateMachineShouldCheckInvariant(long stateMachineId, long invariantIndex) {
        checkLive();
        boolean[] check = new boolean[1];
        translate(
                lib.stateMachineShouldCheckInvariant(tc, stateMachineId, invariantIndex, check),
                "state_machine_should_check_invariant");
        return check[0];
    }

    @Override
    public void stateMachineFree(long stateMachineId) {
        // Not gated on `aborted`: the handle outlives the case and must be released exactly once.
        lib.stateMachineFree(stateMachineId);
    }

    @Override
    public void target(double value, String label) {
        checkLive();
        translate(lib.target(tc, value, label), "target");
    }
}
