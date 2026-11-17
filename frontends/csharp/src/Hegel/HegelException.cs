namespace Hegel;

/// <summary>
/// Thrown for engine, configuration and binding errors: anything that is not a property failure.
///
/// <para>A failing property rethrows the test body's own exception, so it integrates with any
/// test framework; this exception signals that something went wrong with Hegel itself (a missing
/// engine library, an internal engine error). <see cref="HegelRunException"/> and its subtypes
/// are raised when the engine aborts a run (a failed health check, a flaky or unsatisfiable
/// test), and <see cref="DidNotReproduceException"/> when no replayed failure blob fails.</para>
/// </summary>
public class HegelException : Exception
{
    /// <summary>Create an exception with a message.</summary>
    public HegelException(string message) : base(message) { }

    /// <summary>Create an exception with a message and a cause.</summary>
    public HegelException(string message, Exception innerException) : base(message, innerException) { }
}

/// <summary>
/// Thrown when the engine aborts a run with an error rather than a verdict on the property.
/// <see cref="RunReport.ErrorKind"/> classifies it, and the subtypes <see cref="HealthCheckFailure"/>,
/// <see cref="FlakyTestException"/> and <see cref="UnsatisfiableException"/> name the common kinds.
/// </summary>
public class HegelRunException : HegelException
{
    /// <summary>Create an exception with the engine's message.</summary>
    public HegelRunException(string message) : base(message) { }
}

/// <summary>
/// Thrown when the engine aborts a run because a <see cref="HealthCheck"/> fired: the test itself
/// is misbehaving (rejecting almost every input, or generating data that is too large or too
/// slow) rather than the property being false. If the behaviour is intentional, disable the
/// offending check with <see cref="Settings.SuppressHealthCheck"/>.
/// </summary>
public sealed class HealthCheckFailure : HegelRunException
{
    internal HealthCheckFailure(string message, HealthCheck? healthCheck) : base(message)
    {
        HealthCheck = healthCheck;
    }

    /// <summary>The health check that failed, or null if the engine named one this version of Hegel does not know.</summary>
    public HealthCheck? HealthCheck { get; }
}

/// <summary>
/// Thrown when, under <see cref="NondeterminismStrictness.Error"/>, the test produced different
/// outcomes, or drew different data, when replayed on identical choices.
/// </summary>
public sealed class FlakyTestException : HegelRunException
{
    internal FlakyTestException(string message) : base(message) { }
}

/// <summary>Thrown when no valid test case was ever generated: every input was rejected.</summary>
public sealed class UnsatisfiableException : HegelRunException
{
    internal UnsatisfiableException(string message) : base(message) { }
}

/// <summary>
/// Thrown by a <see cref="Settings.ReproduceFailure"/> run none of whose blobs reproduced a
/// failure: the failure may have been fixed, or a nondeterministic one may not have recurred.
/// </summary>
public sealed class DidNotReproduceException : HegelException
{
    internal DidNotReproduceException(string message) : base(message) { }
}

/// <summary>
/// Thrown for a usage error the engine or the frontend detects while a test runs: an argument
/// the engine rejects (such as an unknown text codec), a draw from a generator that cannot print,
/// or a test-case handle used after its stateful rule ended. It aborts the run rather than
/// counting as a failure of the property.
/// </summary>
public class HegelUsageException : ArgumentException
{
    /// <summary>Create an exception with a message.</summary>
    public HegelUsageException(string message) : base(message) { }
}

/// <summary>
/// Thrown for a misuse of an explicit <see cref="Example"/>: a draw it has no value for, a value
/// it has that no draw takes, or a test-case operation an explicit test case cannot perform.
/// </summary>
public sealed class InvalidExampleException : HegelUsageException
{
    internal InvalidExampleException(string message) : base(message) { }
}

/// <summary>
/// Thrown by <see cref="Property.Test(Action{TestCase}, Settings?, Reporter?, string, int, string)"/>
/// when a run reports several distinct failures: the message lists them and
/// <see cref="Failures"/> carries each failure's own exception.
/// </summary>
public sealed class MultipleFailuresException : Exception
{
    internal MultipleFailuresException(string message, IReadOnlyList<Exception> failures) : base(message)
    {
        Failures = failures;
    }

    /// <summary>The distinct failures' exceptions, in the order the engine reported them.</summary>
    public IReadOnlyList<Exception> Failures { get; }
}

/// <summary>The engine ended the test case: it ran out of data. Never a counterexample.</summary>
internal sealed class StopTest : Exception
{
    public StopTest() : base("the engine ended this test case") { }
}

/// <summary>The test case was rejected by an assumption. Never a counterexample.</summary>
internal sealed class AssumeRejected : Exception
{
    public AssumeRejected() : base("the test case was rejected by an assumption") { }
}

/// <summary>A <see cref="TestCase.Repeat"/> loop finished: the test case is over.</summary>
internal sealed class LoopDone : Exception
{
    public LoopDone() : base("the repeat loop finished") { }
}
