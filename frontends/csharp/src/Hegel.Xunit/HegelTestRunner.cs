using System.Reflection;
using Xunit.Abstractions;
using Xunit.Sdk;

namespace Hegel.Xunit;

internal sealed class HegelTestRunner : XunitTestRunner
{
    private readonly StringWriter _report = new();

    internal HegelTestRunner(ITest test, IMessageBus messageBus, Type testClass, object[] constructorArguments, MethodInfo testMethod, string? skipReason, IReadOnlyList<BeforeAfterTestAttribute> beforeAfterAttributes, ExceptionAggregator aggregator, CancellationTokenSource cancellationTokenSource)
        : base(test, messageBus, testClass, constructorArguments, testMethod, [], skipReason, beforeAfterAttributes, aggregator, cancellationTokenSource)
    {
    }

    protected override async Task<Tuple<decimal, string>> InvokeTestAsync(ExceptionAggregator aggregator)
    {
        var (time, output) = await base.InvokeTestAsync(aggregator);
        return Tuple.Create(time, output + _report);
    }

    protected override Task<decimal> InvokeTestMethodAsync(ExceptionAggregator aggregator) =>
        new HegelTestInvoker(Test, MessageBus, TestClass, ConstructorArguments, TestMethod, BeforeAfterAttributes, aggregator, CancellationTokenSource, _report).RunAsync();
}
