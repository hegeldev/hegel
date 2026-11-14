using System.Reflection;
using Hegel.Native;
using Hegel.Tests.Support;
using NUnit.Framework.Api;
using NUnit.Framework.Interfaces;
using NUnit.Framework.Internal;
using Xunit;
using HegelTest = Hegel.NUnit.HegelTestAttribute;
using NUnitFramework = NUnit.Framework;

namespace Hegel.Tests;

public class NUnitTests
{
    static NUnitTests() => NUnitSamples.RegisterProfiles();

    [Fact]
    public void APassingPropertyPasses()
    {
        var result = Harness.Run(nameof(NUnitSamples.Passing));
        Assert.Equal(ResultState.Success, result.ResultState);
        Assert.Equal("", result.Output);
    }

    [Fact]
    public void AStaticPropertyRunsToo() => Assert.Equal(ResultState.Success, Harness.Run(nameof(NUnitSamples.Static)).ResultState);

    [Fact]
    public void AFailingPropertyReportsTheShrunkDrawsAndTheAssertion()
    {
        var result = Harness.Run(nameof(NUnitSamples.Failing));
        Assert.Equal(ResultState.Failure, result.ResultState);
        Assert.Contains("Expected: less than 10", result.Message);
        Assert.Contains("But was:  10", result.Message);
        Assert.Contains("var x_1 = 10;", result.Output);
        Assert.Contains("NUnitSamples.FailAboveTen", result.StackTrace);
    }

    [Fact]
    public void TheCasesTheEngineTriesLeaveNoAssertionsBehind()
    {
        NUnitSamples.Runs = 0;
        var result = Harness.Run(nameof(NUnitSamples.FailingAfterManyCases));
        Assert.Equal(ResultState.Failure, result.ResultState);
        Assert.True(NUnitSamples.Runs > 1);
        Assert.DoesNotContain("Multiple failures", result.Message);
        Assert.True(result.AssertionResults.Count <= 1);
    }

    [Fact]
    public void TestCasesOnTheAttributeIsTheBudget()
    {
        NUnitSamples.Runs = 0;
        Assert.Equal(ResultState.Success, Harness.Run(nameof(NUnitSamples.Counting)).ResultState);
        Assert.Equal(7, NUnitSamples.Runs);
    }

    [Fact]
    public void ABadTestCaseCountFailsTheTest()
    {
        var result = Harness.Run(nameof(NUnitSamples.NegativeTestCases));
        Assert.Equal(ResultState.Error, result.ResultState);
        Assert.StartsWith("System.ArgumentOutOfRangeException", result.Message);
    }

    [Fact]
    public void AnUnknownProfileFailsTheTest()
    {
        var result = Harness.Run(nameof(NUnitSamples.UnknownProfile));
        Assert.Equal(ResultState.Error, result.ResultState);
        Assert.StartsWith("System.ArgumentException", result.Message);
        Assert.Contains("hegel-nunit-no-such-profile", result.Message);
    }

    [Fact]
    public void WithoutAProfileTheDefaultSettingsApply()
    {
        Settings.SetDefaultProfile(NUnitSamples.Quiet);
        try
        {
            Assert.Equal(ResultState.Success, Harness.Run(nameof(NUnitSamples.DefaultSettings)).ResultState);
        }
        finally
        {
            Settings.SetDefaultProfile("");
        }
    }

    [Fact]
    public void NUnitsOwnAttributesWorkAsOnATest()
    {
        var result = Harness.Run(nameof(NUnitSamples.Ignored));
        Assert.Equal(ResultState.Ignored, result.ResultState);
        Assert.Equal("not today", result.Message);
        Assert.Equal("a described property", result.Test.Properties.Get(PropertyNames.Description));
        Assert.Equal("slow", result.Test.Properties.Get(PropertyNames.Category));
    }

    [Fact]
    public void SetUpAndTearDownRunOnceAroundTheProperty()
    {
        NUnitSamples.SetUps = 0;
        NUnitSamples.TearDowns = 0;
        NUnitSamples.Runs = 0;
        Assert.Equal(ResultState.Success, Harness.Run(nameof(NUnitSamples.Counting)).ResultState);
        Assert.Equal((1, 1, 7), (NUnitSamples.SetUps, NUnitSamples.TearDowns, NUnitSamples.Runs));
    }

    [Fact]
    public void TheMethodsOwnFileAndLineAreTheTestLocation()
    {
        var recorder = new RecordingLocations(Engines.Real);
        using var engine = Engines.Override(recorder);
        Harness.Run(nameof(NUnitSamples.Failing));
        var attribute = NUnitSamples.Attribute(nameof(NUnitSamples.Failing));
        var lines = File.ReadAllLines(attribute.SourceFile);
        Assert.Contains("[HegelTest(", lines[attribute.SourceLine - 1]);
        Assert.Contains("void Failing(TestCase tc)", lines[attribute.SourceLine]);
        var expected = (CallerLocation.ProjectPath(attribute.SourceFile), attribute.SourceLine, typeof(NUnitSamples).FullName!, "Failing");
        Assert.Equal([expected], recorder.Locations.Distinct());
    }

    [Fact]
    public void TheTestLocationIsTheDatabaseKey()
    {
        using var fromAttribute = new TempDir(NUnitSamples.DbDir);
        Assert.Equal(ResultState.Failure, Harness.Run(nameof(NUnitSamples.InDatabase)).ResultState);
        var keys = KeyDirs(fromAttribute.Path);
        Assert.NotEmpty(keys);
        var attribute = NUnitSamples.Attribute(nameof(NUnitSamples.InDatabase));
        var expectedKey = $"{CallerLocation.ProjectPath(attribute.SourceFile)}::{nameof(NUnitSamples.InDatabase)}";
        using var sameKey = new TempDir();
        Property.Run(NUnitSamples.FailAboveTen, Engines.Quiet with { Name = expectedKey, Database = Database.At(sameKey.Path) }, Reporter.Silent);
        Assert.Equal(keys, KeyDirs(sameKey.Path));
    }

    [Fact]
    public void AMethodOfTheWrongShapeIsNotRunnable()
    {
        foreach (var (name, problem) in new[]
        {
            (nameof(NUnitSamples.NoParameters), "[HegelTest] methods must take exactly one parameter, a TestCase."),
            (nameof(NUnitSamples.TwoParameters), "[HegelTest] methods must take exactly one parameter, a TestCase."),
            (nameof(NUnitSamples.WrongParameterType), "[HegelTest] methods must take exactly one parameter, a TestCase."),
            (nameof(NUnitSamples.ReturnsInt), "[HegelTest] methods must return void: the engine runs the body synchronously."),
            (nameof(NUnitSamples.Async), "[HegelTest] methods must not be async: the engine runs the body synchronously."),
            (nameof(NUnitSamples.Generic), "[HegelTest] methods are not allowed to be generic."),
        })
        {
            var result = Harness.Run(name);
            Assert.Equal(ResultState.NotRunnable, result.ResultState);
            Assert.Equal(problem, result.Message);
        }
    }

    private static List<string> KeyDirs(string root) => Directory.GetDirectories(root).Select(Path.GetFileName).Select(name => name!).Order().ToList();

    private sealed class RecordingLocations(ILibhegel inner) : DelegatingLibhegel(inner)
    {
        internal List<(string File, int Line, string Class, string Function)> Locations { get; } = [];

        public override void SettingsTestLocation(nint settings, string file, int beginLine, string className, string function)
        {
            Locations.Add((file, beginLine, className, function));
            base.SettingsTestLocation(settings, file, beginLine, className, function);
        }
    }

    private sealed class TempDir : IDisposable
    {
        internal TempDir()
            : this(Directory.CreateTempSubdirectory("hegel-nunit-").FullName)
        {
        }

        internal TempDir(string path)
        {
            Path = path;
            Directory.CreateDirectory(path);
        }

        internal string Path { get; }

        public void Dispose() => Directory.Delete(Path, true);
    }

    private static class Harness
    {
        private static readonly Lazy<NUnitTestAssemblyRunner> Runner = new(() =>
        {
            var runner = new NUnitTestAssemblyRunner(new DefaultTestAssemblyBuilder());
            runner.Load(typeof(NUnitSamples).Assembly, new Dictionary<string, object> { ["NumberOfTestWorkers"] = 0 });
            return runner;
        });

        internal static ITestResult Run(string methodName) =>
            Find(Runner.Value.Run(TestListener.NULL, new SampleFilter(methodName)), methodName)!;

        private static ITestResult? Find(ITestResult result, string methodName) =>
            result.Test.MethodName == methodName ? result : result.Children.Select(child => Find(child, methodName)).FirstOrDefault(found => found != null);

        private sealed class SampleFilter(string methodName) : TestFilter
        {
            public override bool Match(ITest test) => test.TypeInfo?.Type == typeof(NUnitSamples) && test.MethodName == methodName;

            public override TNode AddToXml(TNode parentNode, bool recursive) => parentNode.AddElement("filter");
        }
    }

    private sealed class NUnitSamples
    {
        internal const string Quiet = "hegel-nunit-quiet";
        internal const string Reporting = "hegel-nunit-reporting";
        internal const string WithDatabase = "hegel-nunit-database";
        internal static readonly string DbDir = Path.Combine(Path.GetTempPath(), $"hegel-nunit-db-{Environment.ProcessId}");
        private static readonly Lazy<bool> Registered = new(() =>
        {
            Settings.RegisterProfile(Quiet, Engines.Quiet);
            Settings.RegisterProfile(Reporting, Engines.Quiet with { Verbosity = Verbosity.Normal });
            Settings.RegisterProfile(WithDatabase, Engines.Quiet with { Database = Database.At(DbDir) });
            return true;
        });

        internal static int Runs;
        internal static int SetUps;
        internal static int TearDowns;

        internal static void RegisterProfiles() => _ = Registered.Value;

        internal static HegelTest Attribute(string methodName) => typeof(NUnitSamples).GetMethod(methodName)!.GetCustomAttribute<HegelTest>()!;

        internal static void FailAboveTen(TestCase tc)
        {
            var x = tc.Draw(Generators.Integers().Min(0), "x");
            NUnitFramework.Assert.That(x, NUnitFramework.Is.LessThan(10));
        }

        [NUnitFramework.SetUp]
        public void SetUp() => SetUps++;

        [NUnitFramework.TearDown]
        public void TearDown() => TearDowns++;

        [HegelTest(Profile = Quiet)]
        public void Passing(TestCase tc) => NUnitFramework.Assert.That(tc.Draw(Generators.Integers().Min(0)), NUnitFramework.Is.AtLeast(0));

        [HegelTest(Profile = Quiet, TestCases = 3)]
        public static void Static(TestCase tc) => NUnitFramework.Assert.That(tc.Draw(Generators.Integers().Min(0)), NUnitFramework.Is.AtLeast(0));

        [HegelTest(Profile = Reporting)]
        public void Failing(TestCase tc) => FailAboveTen(tc);

        [HegelTest(Profile = Quiet)]
        public void FailingAfterManyCases(TestCase tc)
        {
            Runs++;
            NUnitFramework.Assert.Multiple(() => FailAboveTen(tc));
        }

        [HegelTest(Profile = Quiet, TestCases = 7)]
        public void Counting(TestCase tc)
        {
            tc.Draw(Generators.Integers());
            Runs++;
        }

        [HegelTest(Profile = Quiet, TestCases = -1)]
        public void NegativeTestCases(TestCase tc)
        {
        }

        [HegelTest(Profile = "hegel-nunit-no-such-profile")]
        public void UnknownProfile(TestCase tc)
        {
        }

        [HegelTest]
        public void DefaultSettings(TestCase tc) => tc.Draw(Generators.Booleans());

        [HegelTest(Profile = Quiet)]
        [NUnitFramework.Ignore("not today")]
        [NUnitFramework.Description("a described property")]
        [NUnitFramework.Category("slow")]
        public void Ignored(TestCase tc) => NUnitFramework.Assert.Fail("ignored tests do not run");

        [HegelTest(Profile = WithDatabase)]
        public void InDatabase(TestCase tc) => FailAboveTen(tc);

        [HegelTest(Profile = Quiet)]
        public void NoParameters()
        {
        }

        [HegelTest(Profile = Quiet)]
        public void TwoParameters(TestCase tc, int extra)
        {
        }

        [HegelTest(Profile = Quiet)]
        public void WrongParameterType(int notATestCase)
        {
        }

        [HegelTest(Profile = Quiet)]
        public int ReturnsInt(TestCase tc) => 0;

        [HegelTest(Profile = Quiet)]
        public async void Async(TestCase tc) => await Task.Yield();

        [HegelTest(Profile = Quiet)]
        public void Generic<T>(TestCase tc)
        {
        }
    }
}
