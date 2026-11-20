namespace Hegel.Native;

/// <summary>
/// Constants from the libhegel C ABI (<c>libhegel/include/hegel.h</c>): return codes, run and case
/// statuses, phase and health-check bit masks, the sentinels the structured primitives use and the
/// temporal draws' policies.
/// Kept in sync with the engine header.
/// </summary>
internal static class Abi
{
    internal const int Ok = 0;
    internal const int EStopTest = -1;
    internal const int EAssume = -2;
    internal const int EBackend = -3;
    internal const int EInvalidHandle = -4;
    internal const int EInvalidArg = -5;
    internal const int EAlreadyComplete = -6;
    internal const int ENotComplete = -7;
    internal const int EInternal = -8;
    internal const int EConcurrentUse = -9;
    internal const int ERetry = -10;
    internal const int ENotFound = -11;
    internal const int EUnknownSetting = -12;

    internal const int RunStatusPassed = 0;
    internal const int RunStatusFailed = 1;
    internal const int RunStatusError = 2;

    internal const int RunErrorNone = 0;
    internal const int RunErrorHealthCheck = 1;
    internal const int RunErrorFlaky = 2;
    internal const int RunErrorNondeterministic = 3;
    internal const int RunErrorUnsatisfiable = 4;
    internal const int RunErrorUsage = 5;
    internal const int RunErrorInternal = 6;

    internal const int NondeterminismQuiet = 0;
    internal const int NondeterminismWarn = 1;
    internal const int NondeterminismError = 2;

    internal const int PhaseExplicit = 1 << 0;
    internal const int PhaseReuse = 1 << 1;
    internal const int PhaseGenerate = 1 << 2;
    internal const int PhaseTarget = 1 << 3;
    internal const int PhaseShrink = 1 << 4;
    internal const int PhaseAll = 31;

    internal const int HcFilterTooMuch = 1 << 0;
    internal const int HcTooSlow = 1 << 1;
    internal const int HcTestCasesTooLarge = 1 << 2;
    internal const int HcLargeInitialTestCase = 1 << 3;

    internal const int BackendDefault = 1;
    internal const int BackendUrandom = 2;

    internal const int VerbosityNormal = 0;
    internal const int VerbosityQuiet = 1;
    internal const int VerbosityVerbose = 2;
    internal const int VerbosityDebug = 3;

    internal const int StatusValid = 0;
    internal const int StatusInvalid = 1;
    internal const int StatusOverrun = 2;
    internal const int StatusInteresting = 3;

    internal const long StateMachineDone = long.MinValue;

    /// <summary>An unset size bound: <c>UINT64_MAX</c>.</summary>
    internal const ulong Unbounded = ulong.MaxValue;

    /// <summary>
    /// <c>HEGEL_MAX_SIZE_DEFAULT</c>: the <c>max_size</c> of a text or bytes draw whose user gave
    /// no maximum, for the engine to apply its default.
    /// </summary>
    internal const ulong MaxSizeDefault = ulong.MaxValue - 1;

    /// <summary><c>HEGEL_FILTER_ATTEMPTS</c>: how many values a filtered draw tries before rejecting the test case.</summary>
    internal const int FilterAttempts = 3;

    internal const uint NoMaxCodepoint = 0xFFFFFFFF;

    internal const int PrecisionNanoseconds = 0;
    internal const int PrecisionMicroseconds = 1;
    internal const int PrecisionMilliseconds = 2;
    internal const int PrecisionSeconds = 3;
    internal const int PrecisionMinutes = 4;
    internal const int PrecisionHours = 5;

    internal const uint ImaginaryAllow = 0;
    internal const uint ImaginaryAvoid = 1;

    internal const int AmbiguousBoth = 0;
    internal const int AmbiguousEarlier = 1;
    internal const int AmbiguousLater = 2;

    internal const int OffsetGranularityHours = 0;
    internal const int OffsetGranularityQuarterHours = 1;
    internal const int OffsetGranularityMinutes = 2;
    internal const int OffsetGranularitySeconds = 3;

    /// <summary>The widest offset the engine draws, in seconds: 25:59:59.</summary>
    internal const int OffsetLimitSeconds = 93599;

    internal const uint SpanUnitYears = 0;
    internal const uint SpanUnitMonths = 1;
    internal const uint SpanUnitWeeks = 2;
    internal const uint SpanUnitDays = 3;
    internal const uint SpanUnitHours = 4;
    internal const uint SpanUnitMinutes = 5;
    internal const uint SpanUnitSeconds = 6;
    internal const uint SpanUnitMilliseconds = 7;
    internal const uint SpanUnitMicroseconds = 8;
    internal const uint SpanUnitNanoseconds = 9;
    internal const int SpanUnitCount = 10;

    internal const int SpanSignUniform = 0;
    internal const int SpanSignMixed = 1;
    internal const int SpanSignNonNegative = 2;

    internal const int ZoneAfterHold = 0;
    internal const int ZoneAfterInfer = 2;

    internal const uint TimezoneKindUnique = 0;
    internal const uint TimezoneKindGap = 1;
    internal const uint TimezoneKindFold = 2;

    internal const uint TimezoneTagLegacy = 1 << 14;
}
