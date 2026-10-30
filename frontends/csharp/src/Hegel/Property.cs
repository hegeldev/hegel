using System.Runtime.CompilerServices;

namespace Hegel;

/// <summary>
/// Entry points: <see cref="Test"/> runs a property and throws on failure, as a test method
/// wants; <see cref="Run"/> returns the <see cref="RunReport"/> instead.
/// <code>
/// [Fact]
/// public void Addition_commutes() => Property.Test(tc =>
/// {
///     var a = tc.Draw(Generators.Integers());
///     var b = tc.Draw(Generators.Integers());
///     Assert.Equal(a + b, b + a);
/// });
/// </code>
/// </summary>
public static class Property
{
    /// <summary>
    /// Run <paramref name="body"/> as a property-based test with <paramref name="settings"/>
    /// (the defaults when <c>null</c>), reporting to <paramref name="reporter"/> (standard error
    /// when <c>null</c>), and throw the failure it finds: the minimal failing test case's own
    /// exception, so the test framework shows it with its stack. The remaining parameters are
    /// filled in by the compiler and identify the test to the database.
    /// </summary>
    public static RunReport Test(
        Action<TestCase> body,
        Settings? settings = null,
        Reporter? reporter = null,
        [CallerFilePath] string callerFile = "",
        [CallerLineNumber] int callerLine = 0,
        [CallerMemberName] string callerMember = "")
    {
        var report = Run(body, settings, reporter, callerFile, callerLine, callerMember);
        report.ThrowIfFailed();
        return report;
    }

    /// <summary>
    /// Run <paramref name="body"/> as <see cref="Test"/> does, but return the report instead of
    /// throwing when it fails. A run that cannot complete (a health check, a stale
    /// <see cref="Settings.ReproduceFailure"/> blob) still throws.
    /// </summary>
    public static RunReport Run(
        Action<TestCase> body,
        Settings? settings = null,
        Reporter? reporter = null,
        [CallerFilePath] string callerFile = "",
        [CallerLineNumber] int callerLine = 0,
        [CallerMemberName] string callerMember = "")
    {
        ArgumentNullException.ThrowIfNull(body);
        settings ??= new Settings();
        if (settings.TestLocation == null || settings.Name == null)
        {
            var location = CallerLocation.Of(body, callerFile, callerLine, callerMember);
            settings = settings with
            {
                TestLocation = settings.TestLocation ?? location,
                Name = settings.Name ?? CallerLocation.DatabaseKey(location),
            };
        }
        return Runner.Run(Engine.Get(), settings, body, reporter ?? Reporter.Printing(Console.Error));
    }

    /// <summary>The version of the engine (libhegel) this frontend is running on.</summary>
    public static string EngineVersion => Engine.Get().Version();
}
