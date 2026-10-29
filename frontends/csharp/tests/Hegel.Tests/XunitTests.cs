using System.Reflection;
using Hegel.Native;
using Hegel.Tests.Support;
using Hegel.Xunit;
using Xunit;
using Xunit.Abstractions;
using Xunit.Sdk;

namespace Hegel.Tests;

public class XunitTests
{
    static XunitTests() => XunitSamples.RegisterProfiles();

    [HegelTest(Profile = XunitSamples.Quiet)]
    public void APropertyRunsThroughXunitsOwnDiscovery(TestCase tc)
    {
        var x = tc.Draw(Generators.Integers());
        Assert.Equal(x, x);
    }

    [HegelTest(Profile = XunitSamples.Quiet, TestCases = 3)]
    public static void AStaticPropertyRunsToo(TestCase tc) => Assert.True(tc.Draw(Generators.Integers().Min(0)) >= 0);

    [Fact]
    public async Task APassingPropertyPasses()
    {
        var messages = await Harness.Run(nameof(XunitSamples.Passing));
        Assert.Single(messages.OfType<ITestPassed>());
        Assert.Empty(messages.OfType<ITestFailed>());
    }

    [Fact]
    public async Task AFailingPropertyReportsTheShrunkDrawsAndTheAssertion()
    {
        var failed = Assert.Single((await Harness.Run(nameof(XunitSamples.Failing))).OfType<ITestFailed>());
        Assert.Equal("Xunit.Sdk.TrueException", failed.ExceptionTypes[0]);
        Assert.StartsWith("Assert.True() Failure", failed.Messages[0]);
        Assert.Contains("var x_1 = 10;", failed.Output);
        Assert.Contains("XunitSamples.Failing", failed.StackTraces[0]);
    }

    [Fact]
    public async Task TestCasesOnTheAttributeIsTheBudget()
    {
        XunitSamples.Runs = 0;
        Assert.Single((await Harness.Run(nameof(XunitSamples.Counting))).OfType<ITestPassed>());
        Assert.Equal(7, XunitSamples.Runs);
    }

    [Fact]
    public async Task ABadTestCaseCountFailsTheTest()
    {
        var failed = Assert.Single((await Harness.Run(nameof(XunitSamples.NegativeTestCases))).OfType<ITestFailed>());
        Assert.Equal("System.ArgumentOutOfRangeException", failed.ExceptionTypes[0]);
    }

    [Fact]
    public async Task AnUnknownProfileFailsTheTest()
    {
        var failed = Assert.Single((await Harness.Run(nameof(XunitSamples.UnknownProfile))).OfType<ITestFailed>());
        Assert.Equal("System.ArgumentException", failed.ExceptionTypes[0]);
        Assert.Contains("hegel-xunit-no-such-profile", failed.Messages[0]);
    }

    [Fact]
    public async Task WithoutAProfileTheDefaultSettingsApply()
    {
        Settings.SetDefaultProfile(XunitSamples.Quiet);
        try
        {
            Assert.Single((await Harness.Run(nameof(XunitSamples.DefaultSettings))).OfType<ITestPassed>());
        }
        finally
        {
            Settings.SetDefaultProfile("");
        }
    }

    [Fact]
    public async Task SkipAndDisplayNameWorkAsOnAFact()
    {
        var skipped = Assert.Single((await Harness.Run(nameof(XunitSamples.Skipped))).OfType<ITestSkipped>());
        Assert.Equal("not today", skipped.Reason);
        Assert.Equal("a custom name", skipped.Test.DisplayName);
    }

    [Fact]
    public async Task TheMethodsOwnFileAndLineAreTheTestLocation()
    {
        var recorder = new RecordingLocations(Engines.Real);
        using var engine = Engines.Override(recorder);
        await Harness.Run(nameof(XunitSamples.Failing));
        var attribute = XunitSamples.Attribute(nameof(XunitSamples.Failing));
        var lines = File.ReadAllLines(attribute.SourceFile);
        Assert.Contains("[HegelTest(", lines[attribute.SourceLine - 1]);
        Assert.Contains("void Failing(TestCase tc)", lines[attribute.SourceLine]);
        var expected = (CallerLocation.ProjectPath(attribute.SourceFile), attribute.SourceLine, typeof(XunitSamples).FullName!, "Failing");
        Assert.Equal([expected], recorder.Locations.Distinct());
    }

    [Fact]
    public async Task TheTestLocationIsTheDatabaseKey()
    {
        using var fromAttribute = new TempDir(XunitSamples.DbDir);
        Assert.Single((await Harness.Run(nameof(XunitSamples.InDatabase))).OfType<ITestFailed>());
        var keys = KeyDirs(fromAttribute.Path);
        Assert.NotEmpty(keys);
        var attribute = XunitSamples.Attribute(nameof(XunitSamples.InDatabase));
        var expectedKey = $"{CallerLocation.ProjectPath(attribute.SourceFile)}::{nameof(XunitSamples.InDatabase)}";
        using var sameKey = new TempDir();
        Property.Run(XunitSamples.FailAboveTen, Engines.Quiet with { Name = expectedKey, Database = Database.At(sameKey.Path) }, Reporter.Silent);
        Assert.Equal(keys, KeyDirs(sameKey.Path));
        using var otherKey = new TempDir();
        Property.Run(XunitSamples.FailAboveTen, Engines.Quiet with { Name = "another test", Database = Database.At(otherKey.Path) }, Reporter.Silent);
        Assert.NotEqual(keys, KeyDirs(otherKey.Path));
    }

    [Fact]
    public async Task AMethodOfTheWrongShapeIsAnExecutionError()
    {
        foreach (var (name, problem) in new[]
        {
            (nameof(XunitSamples.NoParameters), "[HegelTest] methods must take exactly one parameter, a TestCase."),
            (nameof(XunitSamples.TwoParameters), "[HegelTest] methods must take exactly one parameter, a TestCase."),
            (nameof(XunitSamples.WrongParameterType), "[HegelTest] methods must take exactly one parameter, a TestCase."),
            (nameof(XunitSamples.ReturnsInt), "[HegelTest] methods must return void: the engine runs the body synchronously."),
            (nameof(XunitSamples.Generic), "[HegelTest] methods are not allowed to be generic."),
        })
        {
            var failed = Assert.Single((await Harness.Run(name)).OfType<ITestFailed>());
            Assert.Equal(problem, failed.Messages[0]);
        }
    }

    [Fact]
    public void TheTestCaseSurvivesXunitsSerialization()
    {
        var testCase = Harness.Discover(nameof(XunitSamples.Passing));
        var roundTripped = SerializationHelper.Deserialize<IXunitTestCase>(SerializationHelper.Serialize(testCase));
        var hegelTestCase = Assert.IsType<HegelTestCase>(roundTripped);
        Assert.Equal(testCase.UniqueID, hegelTestCase.UniqueID);
        Assert.Equal(testCase.DisplayName, hegelTestCase.DisplayName);
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
            : this(Directory.CreateTempSubdirectory("hegel-xunit-").FullName)
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
        internal static IXunitTestCase Discover(string methodName)
        {
            var type = typeof(XunitSamples);
            var method = type.GetMethod(methodName)!;
            var testClass = new TestClass(new TestCollection(new TestAssembly(Reflector.Wrap(type.Assembly)), null, "xunit samples"), Reflector.Wrap(type));
            var testMethod = new TestMethod(testClass, Reflector.Wrap(method));
            var attribute = Reflector.Wrap(method.GetCustomAttributesData().Single(a => a.AttributeType == typeof(HegelTestAttribute)));
            return Assert.Single(new HegelTestDiscoverer(new NullMessageSink()).Discover(new DiscoveryOptions(), testMethod, attribute));
        }

        internal static async Task<List<IMessageSinkMessage>> Run(string methodName)
        {
            var bus = new RecordingBus();
            await Discover(methodName).RunAsync(new NullMessageSink(), bus, [], new ExceptionAggregator(), new CancellationTokenSource());
            return bus.Messages;
        }

        private sealed class DiscoveryOptions : ITestFrameworkDiscoveryOptions
        {
            public TValue GetValue<TValue>(string name) => default!;

            public void SetValue<TValue>(string name, TValue value)
            {
            }
        }

        private sealed class RecordingBus : IMessageBus
        {
            internal List<IMessageSinkMessage> Messages { get; } = [];

            public bool QueueMessage(IMessageSinkMessage message)
            {
                Messages.Add(message);
                return true;
            }

            public void Dispose()
            {
            }
        }
    }

#pragma warning disable xUnit1000
    private sealed class XunitSamples
#pragma warning restore xUnit1000
    {
        internal const string Quiet = "hegel-xunit-quiet";
        internal const string Reporting = "hegel-xunit-reporting";
        internal const string WithDatabase = "hegel-xunit-database";
        internal static readonly string DbDir = Path.Combine(Path.GetTempPath(), $"hegel-xunit-db-{Environment.ProcessId}");
        private static readonly Lazy<bool> Registered = new(() =>
        {
            Settings.RegisterProfile(Quiet, Engines.Quiet);
            Settings.RegisterProfile(Reporting, Engines.Quiet with { Verbosity = Verbosity.Normal });
            Settings.RegisterProfile(WithDatabase, Engines.Quiet with { Database = Database.At(DbDir) });
            return true;
        });

        internal static int Runs;

        internal static void RegisterProfiles() => _ = Registered.Value;

        internal static HegelTestAttribute Attribute(string methodName) => typeof(XunitSamples).GetMethod(methodName)!.GetCustomAttribute<HegelTestAttribute>()!;

        internal static void FailAboveTen(TestCase tc)
        {
            var x = tc.Draw(Generators.Integers().Min(0), "x");
            Assert.True(x < 10);
        }

        [HegelTest(Profile = Quiet)]
        public void Passing(TestCase tc) => Assert.True(tc.Draw(Generators.Integers().Min(0)) >= 0);

        [HegelTest(Profile = Reporting)]
        public void Failing(TestCase tc) => FailAboveTen(tc);

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

        [HegelTest(Profile = "hegel-xunit-no-such-profile")]
        public void UnknownProfile(TestCase tc)
        {
        }

        [HegelTest]
        public void DefaultSettings(TestCase tc) => tc.Draw(Generators.Booleans());

        [HegelTest(Profile = Quiet, Skip = "not today", DisplayName = "a custom name")]
        public void Skipped(TestCase tc) => Assert.Fail("skipped tests do not run");

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
        public void Generic<T>(TestCase tc)
        {
        }
    }
}
