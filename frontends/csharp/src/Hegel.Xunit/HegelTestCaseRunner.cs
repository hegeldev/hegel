using System.Reflection;
using Xunit.Abstractions;
using Xunit.Sdk;

namespace Hegel.Xunit;

internal sealed class HegelTestCaseRunner : XunitTestCaseRunner
{
    internal HegelTestCaseRunner(IXunitTestCase testCase, string displayName, string? skipReason, object[] constructorArguments, IMessageBus messageBus, ExceptionAggregator aggregator, CancellationTokenSource cancellationTokenSource)
        : base(testCase, displayName, skipReason, constructorArguments, [], messageBus, aggregator, cancellationTokenSource)
    {
    }

    protected override XunitTestRunner CreateTestRunner(ITest test, IMessageBus messageBus, Type testClass, object[] constructorArguments, MethodInfo testMethod, object[] testMethodArguments, string skipReason, IReadOnlyList<BeforeAfterTestAttribute> beforeAfterAttributes, ExceptionAggregator aggregator, CancellationTokenSource cancellationTokenSource) =>
        new HegelTestRunner(test, messageBus, testClass, constructorArguments, testMethod, skipReason, beforeAfterAttributes, new ExceptionAggregator(aggregator), cancellationTokenSource);
}
