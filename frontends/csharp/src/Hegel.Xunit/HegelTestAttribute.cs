using System.Runtime.CompilerServices;
using Xunit;
using Xunit.Sdk;

namespace Hegel.Xunit;

/// <summary>
/// Marks an xUnit test method as a Hegel property. The method takes one <see cref="TestCase"/>
/// and returns <c>void</c>; the engine runs it many times with generated inputs, shrinks any
/// failure to a minimal counterexample, puts the counterexample in the test's output and fails
/// the test with the assertion itself, as <see cref="Property.Test"/> does:
/// <code>
/// [HegelTest]
/// public void Addition_commutes(TestCase tc)
/// {
///     var a = tc.Draw(Generators.Integers());
///     var b = tc.Draw(Generators.Integers());
///     Assert.Equal(a + b, b + a);
/// }
/// </code>
/// The property is one test in xUnit's test tree. <see cref="Profile"/> and <see cref="TestCases"/>
/// adjust its settings; for any other setting, or one only known at runtime, call
/// <see cref="Property.Test"/> from a <c>[Fact]</c> instead.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = false)]
[XunitTestCaseDiscoverer("Hegel.Xunit.HegelTestDiscoverer", "Hegel.Xunit")]
public sealed class HegelTestAttribute : FactAttribute
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

    internal Settings ToSettings()
    {
        var settings = Profile == null ? new Settings() : Settings.FromProfile(Profile);
        return TestCases == 0 ? settings : settings with { TestCases = TestCases };
    }
}
