using NUnit.Framework.Interfaces;
using NUnit.Framework.Internal;
using NUnit.Framework.Internal.Commands;

namespace Hegel.NUnit;

internal sealed class HegelTestCommand : TestCommand
{
    private readonly HegelTestAttribute _attribute;

    internal HegelTestCommand(Test test, HegelTestAttribute attribute)
        : base(test)
    {
        _attribute = attribute;
    }

    public override TestResult Execute(TestExecutionContext context)
    {
        var method = Test.Method!.MethodInfo;
        var body = method.CreateDelegate<Action<TestCase>>(method.IsStatic ? null : context.TestObject);
        var report = new StringWriter();
        try
        {
            Property.Test(tc => RunIsolated(body, tc), _attribute.ToSettings(), Reporter.Printing(report), _attribute.SourceFile, _attribute.SourceLine, method.Name);
        }
        finally
        {
            context.OutWriter.Write(report);
        }
        context.CurrentResult.SetResult(ResultState.Success);
        return context.CurrentResult;
    }

    private static void RunIsolated(Action<TestCase> body, TestCase tc)
    {
        using var isolated = new TestExecutionContext.IsolatedContext();
        body(tc);
    }
}
