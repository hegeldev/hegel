using System.Reflection;
using System.Runtime.CompilerServices;
using NUnit.Framework;
using NUnit.Framework.Interfaces;
using NUnit.Framework.Internal;
using NUnit.Framework.Internal.Commands;

namespace Hegel.NUnit;

/// <summary>
/// Marks an NUnit test method as a Hegel property. The method takes one <see cref="TestCase"/>
/// and returns <c>void</c>; the engine runs it many times with generated inputs, shrinks any
/// failure to a minimal counterexample, puts the counterexample in the test's output and fails
/// the test with the assertion itself, as <see cref="Property.Test"/> does:
/// <code>
/// [HegelTest]
/// public void Addition_commutes(TestCase tc)
/// {
///     var a = tc.Draw(Generators.Integers());
///     var b = tc.Draw(Generators.Integers());
///     Assert.That(a + b, Is.EqualTo(b + a));
/// }
/// </code>
/// The property is one test in NUnit's test tree, in a fixture that needs no
/// <c>[TestFixture]</c>; <c>[SetUp]</c> and <c>[TearDown]</c> run once around the whole
/// property, and <c>[Ignore]</c>, <c>[Category]</c> and <c>[Description]</c> work as on a
/// <c>[Test]</c>. <see cref="Profile"/> and <see cref="TestCases"/> adjust its settings; for any
/// other setting, or one only known at runtime, call <see cref="Property.Test"/> from a
/// <c>[Test]</c> instead.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = false)]
public sealed class HegelTestAttribute : NUnitAttribute, ISimpleTestBuilder, IImplyFixture, IWrapTestMethod
{
    /// <summary>
    /// Marks the method as a Hegel property. The parameters are filled in by the compiler with
    /// where the attribute is written, which is the test's location for the database and Antithesis.
    /// </summary>
    public HegelTestAttribute([CallerFilePath] string sourceFile = "", [CallerLineNumber] int sourceLine = 0)
    {
        SourceFile = sourceFile;
        SourceLine = sourceLine;
    }

    /// <summary>The source file the attribute is written in.</summary>
    public string SourceFile { get; }

    /// <summary>The line the attribute is written on.</summary>
    public int SourceLine { get; }

    /// <summary>
    /// The <c>hegel.toml</c> profile (or one registered with <see cref="Settings.RegisterProfile"/>)
    /// the test's settings resolve from, instead of the default profile.
    /// </summary>
    public string? Profile { get; set; }

    /// <summary>
    /// How many test cases to generate. <c>0</c>, the default, leaves the number to the settings
    /// profile and the <c>HEGEL_TEST_CASES</c> environment variable.
    /// </summary>
    public long TestCases { get; set; }

    /// <summary>Called by NUnit to build the test of the method the attribute is on.</summary>
    public TestMethod BuildFrom(IMethodInfo method, Test? suite)
    {
        var test = new TestMethod(method, suite);
        test.ApplyAttributesToTest(method.MethodInfo);
        var problem = Problem(method.MethodInfo);
        if (problem != null)
        {
            test.MakeInvalid(problem);
        }
        return test;
    }

    /// <summary>Called by NUnit to replace the call of the method with a run of the property.</summary>
    public TestCommand Wrap(TestCommand command) => new HegelTestCommand(command.Test, this);

    internal Settings ToSettings()
    {
        var settings = Profile == null ? new Settings() : Settings.FromProfile(Profile);
        return TestCases == 0 ? settings : settings with { TestCases = TestCases };
    }

    internal static string? Problem(MethodInfo method)
    {
        var parameters = method.GetParameters();
        if (parameters.Length != 1 || parameters[0].ParameterType != typeof(TestCase))
        {
            return "[HegelTest] methods must take exactly one parameter, a TestCase.";
        }
        if (method.ReturnType != typeof(void))
        {
            return "[HegelTest] methods must return void: the engine runs the body synchronously.";
        }
        if (method.IsDefined(typeof(AsyncStateMachineAttribute), false))
        {
            return "[HegelTest] methods must not be async: the engine runs the body synchronously.";
        }
        if (method.IsGenericMethodDefinition)
        {
            return "[HegelTest] methods are not allowed to be generic.";
        }
        return null;
    }
}
