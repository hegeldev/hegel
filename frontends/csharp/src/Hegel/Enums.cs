using Hegel.Native;

namespace Hegel;

/// <summary>How much a run reports: the engine's verbosity setting.</summary>
public enum Verbosity
{
    /// <summary>Only failures.</summary>
    Quiet = Abi.VerbosityQuiet,
    /// <summary>Failures and the engine's own warnings.</summary>
    Normal = Abi.VerbosityNormal,
    /// <summary>Every test case's draws as it runs.</summary>
    Verbose = Abi.VerbosityVerbose,
    /// <summary>Everything, including the engine's internal decisions.</summary>
    Debug = Abi.VerbosityDebug,
}

/// <summary>Where the engine's randomness comes from.</summary>
public enum Backend
{
    /// <summary>The engine's own pseudo-random generator.</summary>
    Default = Abi.BackendDefault,
    /// <summary>The operating system's randomness (for example under Antithesis).</summary>
    Urandom = Abi.BackendUrandom,
}

/// <summary>What the engine does when a test turns out to be nondeterministic.</summary>
public enum NondeterminismStrictness
{
    /// <summary>Nothing.</summary>
    Quiet = Abi.NondeterminismQuiet,
    /// <summary>Warn.</summary>
    Warn = Abi.NondeterminismWarn,
    /// <summary>Report the run as an error.</summary>
    Error = Abi.NondeterminismError,
}

/// <summary>The phases of a run; <see cref="Settings.Phases"/> is a set of them.</summary>
[Flags]
public enum Phase
{
    /// <summary>No phase.</summary>
    None = 0,
    /// <summary>Run the <see cref="Settings.Examples"/>.</summary>
    Explicit = Abi.PhaseExplicit,
    /// <summary>Replay failures saved in the database.</summary>
    Reuse = Abi.PhaseReuse,
    /// <summary>Generate new test cases.</summary>
    Generate = Abi.PhaseGenerate,
    /// <summary>Optimise <see cref="TestCase.Target(double, string)"/> scores.</summary>
    Target = Abi.PhaseTarget,
    /// <summary>Shrink failures.</summary>
    Shrink = Abi.PhaseShrink,
    /// <summary>Every phase.</summary>
    All = Explicit | Reuse | Generate | Target | Shrink,
}

/// <summary>The engine's health checks; <see cref="Settings.SuppressHealthCheck"/> is a set of them.</summary>
[Flags]
public enum HealthCheck
{
    /// <summary>No health check.</summary>
    None = 0,
    /// <summary>Too many test cases were rejected by assumptions or filters.</summary>
    FilterTooMuch = Abi.HcFilterTooMuch,
    /// <summary>Test cases take too long.</summary>
    TooSlow = Abi.HcTooSlow,
    /// <summary>Test cases draw too much data.</summary>
    TestCasesTooLarge = Abi.HcTestCasesTooLarge,
    /// <summary>The first test case drew too much data.</summary>
    LargeInitialTestCase = Abi.HcLargeInitialTestCase,
}

/// <summary>How a run ended.</summary>
public enum RunStatus
{
    /// <summary>No test case failed.</summary>
    Passed,
    /// <summary>At least one test case failed; see <see cref="RunReport.Failures"/>.</summary>
    Failed,
    /// <summary>The run could not complete; see <see cref="RunReport.Error"/>.</summary>
    Error,
}

/// <summary>Why a run ended in <see cref="RunStatus.Error"/>.</summary>
public enum RunErrorKind
{
    /// <summary>A health check failed.</summary>
    HealthCheck = Abi.RunErrorHealthCheck,
    /// <summary>A failure could not be reproduced.</summary>
    Flaky = Abi.RunErrorFlaky,
    /// <summary>The test drew differently on replay.</summary>
    Nondeterministic = Abi.RunErrorNondeterministic,
    /// <summary>No valid test case could be generated.</summary>
    Unsatisfiable = Abi.RunErrorUnsatisfiable,
    /// <summary>The test used the engine wrongly.</summary>
    Usage = Abi.RunErrorUsage,
    /// <summary>The engine failed internally.</summary>
    Internal = Abi.RunErrorInternal,
}

/// <summary>How one test case ended.</summary>
public enum CaseOutcome
{
    /// <summary>It ran to completion.</summary>
    Valid = Abi.StatusValid,
    /// <summary>It was rejected by an assumption.</summary>
    Invalid = Abi.StatusInvalid,
    /// <summary>It ran out of data.</summary>
    Overrun = Abi.StatusOverrun,
    /// <summary>It failed.</summary>
    Interesting = Abi.StatusInteresting,
}

internal static class EnumCodes
{
    internal static Verbosity VerbosityFrom(int code) => code is >= Abi.VerbosityNormal and <= Abi.VerbosityDebug
        ? (Verbosity)code
        : throw new HegelException($"unknown hegel_verbosity_t value {code}");

    internal static Backend BackendFrom(int code) => code is Abi.BackendDefault or Abi.BackendUrandom
        ? (Backend)code
        : throw new HegelException($"unknown hegel_backend_t value {code}");

    internal static NondeterminismStrictness StrictnessFrom(int code) => code is >= Abi.NondeterminismQuiet and <= Abi.NondeterminismError
        ? (NondeterminismStrictness)code
        : throw new HegelException($"unknown hegel_nondeterminism_strictness_t value {code}");

    internal static RunErrorKind? ErrorKindFrom(int code) => code is >= Abi.RunErrorHealthCheck and <= Abi.RunErrorInternal
        ? (RunErrorKind)code
        : null;

    internal static HealthCheck? HealthCheckFrom(int bit) => bit is Abi.HcFilterTooMuch or Abi.HcTooSlow or Abi.HcTestCasesTooLarge or Abi.HcLargeInitialTestCase
        ? (HealthCheck)bit
        : null;
}
