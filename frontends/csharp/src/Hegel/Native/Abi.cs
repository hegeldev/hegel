namespace Hegel.Native;

/// <summary>
/// Constants from the libhegel C ABI (<c>libhegel/include/hegel.h</c>): return codes, run and case
/// statuses, phase and health-check bit masks, and the sentinels the structured primitives use.
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
}
