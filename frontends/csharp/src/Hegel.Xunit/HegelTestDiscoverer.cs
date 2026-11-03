using Xunit.Abstractions;
using Xunit.Sdk;

namespace Hegel.Xunit;

/// <summary>
/// Finds the test case of a <see cref="HegelTestAttribute"/> method for xUnit, named by the
/// attribute's <see cref="XunitTestCaseDiscovererAttribute"/>; not called directly.
/// </summary>
public sealed class HegelTestDiscoverer : IXunitTestCaseDiscoverer
{
    private readonly IMessageSink _diagnosticMessageSink;

    /// <summary>Called by xUnit.</summary>
    public HegelTestDiscoverer(IMessageSink diagnosticMessageSink)
    {
        _diagnosticMessageSink = diagnosticMessageSink;
    }

    /// <inheritdoc/>
    public IEnumerable<IXunitTestCase> Discover(ITestFrameworkDiscoveryOptions discoveryOptions, ITestMethod testMethod, IAttributeInfo factAttribute)
    {
        var display = discoveryOptions.MethodDisplayOrDefault();
        var options = discoveryOptions.MethodDisplayOptionsOrDefault();
        var problem = Problem(testMethod.Method);
        IXunitTestCase testCase = problem == null
            ? new HegelTestCase(_diagnosticMessageSink, display, options, testMethod)
            : new ExecutionErrorTestCase(_diagnosticMessageSink, display, options, testMethod, problem);
        return [testCase];
    }

    internal static string? Problem(IMethodInfo method)
    {
        var parameters = method.GetParameters().ToList();
        if (parameters.Count != 1 || parameters[0].ParameterType.Name != typeof(TestCase).FullName)
        {
            return "[HegelTest] methods must take exactly one parameter, a TestCase.";
        }
        if (method.ReturnType.Name != typeof(void).FullName)
        {
            return "[HegelTest] methods must return void: the engine runs the body synchronously.";
        }
        if (method.IsGenericMethodDefinition)
        {
            return "[HegelTest] methods are not allowed to be generic.";
        }
        return null;
    }
}
