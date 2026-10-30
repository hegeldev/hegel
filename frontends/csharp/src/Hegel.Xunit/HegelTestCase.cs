using System.ComponentModel;
using Xunit.Abstractions;
using Xunit.Sdk;

namespace Hegel.Xunit;

/// <summary>
/// The xUnit test case of a <see cref="HegelTestAttribute"/> method: one test, whose body the
/// engine runs. Created by <see cref="HegelTestDiscoverer"/>; not used directly.
/// </summary>
public sealed class HegelTestCase : XunitTestCase
{
    /// <summary>Called by xUnit's deserializer.</summary>
    [EditorBrowsable(EditorBrowsableState.Never)]
    [Obsolete("Called by the de-serializer; should only be called by deriving classes for de-serialization purposes")]
    public HegelTestCase()
    {
    }

    /// <summary>Called by <see cref="HegelTestDiscoverer"/>.</summary>
    public HegelTestCase(IMessageSink diagnosticMessageSink, TestMethodDisplay defaultMethodDisplay, TestMethodDisplayOptions defaultMethodDisplayOptions, ITestMethod testMethod)
        : base(diagnosticMessageSink, defaultMethodDisplay, defaultMethodDisplayOptions, testMethod)
    {
    }

    /// <inheritdoc/>
    public override Task<RunSummary> RunAsync(IMessageSink diagnosticMessageSink, IMessageBus messageBus, object[] constructorArguments, ExceptionAggregator aggregator, CancellationTokenSource cancellationTokenSource) =>
        new HegelTestCaseRunner(this, DisplayName, SkipReason, constructorArguments, messageBus, aggregator, cancellationTokenSource).RunAsync();
}
