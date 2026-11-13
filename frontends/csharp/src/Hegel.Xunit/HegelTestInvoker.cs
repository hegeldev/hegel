using System.Reflection;
using Xunit.Abstractions;
using Xunit.Sdk;

namespace Hegel.Xunit;

internal sealed class HegelTestInvoker : XunitTestInvoker
{
    private readonly TextWriter _report;

    internal HegelTestInvoker(ITest test, IMessageBus messageBus, Type testClass, object[] constructorArguments, MethodInfo testMethod, IReadOnlyList<BeforeAfterTestAttribute> beforeAfterAttributes, ExceptionAggregator aggregator, CancellationTokenSource cancellationTokenSource, TextWriter report)
        : base(test, messageBus, testClass, constructorArguments, testMethod, new object?[] { null }, beforeAfterAttributes, aggregator, cancellationTokenSource)
    {
        _report = report;
    }

    protected override object? CallTestMethod(object testClassInstance)
    {
        var attribute = TestMethod.GetCustomAttribute<HegelTestAttribute>()!;
        var body = TestMethod.CreateDelegate<Action<TestCase>>(testClassInstance);
        Property.Test(body, attribute.ToSettings(), Reporter.Printing(_report), attribute.SourceFile, attribute.SourceLine, TestMethod.Name);
        return null;
    }
}
