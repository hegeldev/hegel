using Hegel.Native;
using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class SettingsTests
{
    [Fact]
    public void DefaultsComeFromTheEngine()
    {
        var s = new Settings();
        Assert.Equal(100, s.TestCases);
        Assert.Null(s.Seed);
        Assert.False(s.Derandomize);
        Assert.Equal(Database.DefaultLocation, s.Database);
        Assert.Equal(HealthCheck.None, s.SuppressHealthCheck);
        Assert.Equal(Phase.All, s.Phases);
        Assert.Equal(Verbosity.Normal, s.Verbosity);
        Assert.Equal(Backend.Default, s.Backend);
        Assert.Empty(s.ReproduceFailure);
        Assert.Empty(s.InfrastructureNamespaces);
        Assert.Empty(s.Examples);
        Assert.Empty(s.StringSettings);
        Assert.Null(s.Name);
        Assert.Null(s.TestLocation);
    }

    [Fact]
    public void TheSeedEnvironmentVariableIsResolved()
    {
        using var env = NativeEnv.Set("HEGEL_SEED", "123");
        Assert.Equal(123UL, new Settings().Seed);
    }

    [Fact]
    public void TestCasesMustBePositive()
    {
        Assert.Equal(1, new Settings { TestCases = 1 }.TestCases);
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => new Settings { TestCases = 0 });
        Assert.Equal("TestCases", e.ParamName);
    }

    [Fact]
    public void DatabaseMustNotBeNull()
    {
        Assert.Equal(Database.Disabled, new Settings { Database = Database.Disabled }.Database);
        Assert.Throws<ArgumentNullException>(() => new Settings { Database = null! });
    }

    [Fact]
    public void ListsAreCopiedAndMustNotBeNull()
    {
        var blobs = new List<string> { "a" };
        var namespaces = new List<string> { "N." };
        var examples = new List<Example> { Example.Of(1) };
        var s = new Settings { ReproduceFailure = blobs, InfrastructureNamespaces = namespaces, Examples = examples };
        blobs.Add("b");
        namespaces.Clear();
        examples.Clear();
        Assert.Equal(["a"], s.ReproduceFailure);
        Assert.Equal(["N."], s.InfrastructureNamespaces);
        Assert.Single(s.Examples);
        Assert.Throws<ArgumentNullException>(() => new Settings { ReproduceFailure = null! });
        Assert.Throws<ArgumentNullException>(() => new Settings { InfrastructureNamespaces = null! });
        Assert.Throws<ArgumentNullException>(() => new Settings { Examples = null! });
    }

    [Fact]
    public void PropertiesRoundTrip()
    {
        var location = new TestLocation("f.cs", 1, "C", "M");
        var s = new Settings
        {
            Seed = 7,
            Derandomize = true,
            SuppressHealthCheck = HealthCheck.TooSlow | HealthCheck.FilterTooMuch,
            Phases = Phase.Generate | Phase.Shrink,
            Verbosity = Verbosity.Debug,
            Backend = Backend.Urandom,
            ReportMultipleFailures = true,
            StopOnFirstFailure = true,
            ShowStatistics = true,
            UnboundedChoices = true,
            PrintBlob = false,
            NondeterminismStrictness = NondeterminismStrictness.Error,
            Name = "name",
            TestLocation = location,
        };
        Assert.Equal(7UL, s.Seed);
        Assert.True(s.Derandomize);
        Assert.Equal(HealthCheck.TooSlow | HealthCheck.FilterTooMuch, s.SuppressHealthCheck);
        Assert.Equal(Phase.Generate | Phase.Shrink, s.Phases);
        Assert.Equal(Verbosity.Debug, s.Verbosity);
        Assert.Equal(Backend.Urandom, s.Backend);
        Assert.True(s.ReportMultipleFailures);
        Assert.True(s.StopOnFirstFailure);
        Assert.True(s.ShowStatistics);
        Assert.True(s.UnboundedChoices);
        Assert.False(s.PrintBlob);
        Assert.Equal(NondeterminismStrictness.Error, s.NondeterminismStrictness);
        Assert.Equal("name", s.Name);
        Assert.Equal(location, s.TestLocation);
        Assert.Equal(s, s with { });
    }

    [Fact]
    public void SetValidatesTheKeyAndValueThroughTheEngine()
    {
        var s = Engines.Quiet.Set("test_cases", "7").Set("verbosity", "quiet");
        Assert.Equal([new("test_cases", "7"), new("verbosity", "quiet")], s.StringSettings);
        Assert.Empty(Engines.Quiet.StringSettings);
        var unknown = Assert.Throws<ArgumentException>(() => Engines.Quiet.Set("no_such_key", "1"));
        Assert.Contains("unknown setting `no_such_key`", unknown.Message);
        var malformed = Assert.Throws<ArgumentException>(() => Engines.Quiet.Set("test_cases", "abc"));
        Assert.Contains("`test_cases` expects an integer", malformed.Message);
    }

    [Fact]
    public void EffectiveOverridesWhatTheEngineResolved()
    {
        var s = Engines.Quiet.Effective(3, true, NondeterminismStrictness.Error, Verbosity.Verbose);
        Assert.Equal(3, s.TestCases);
        Assert.True(s.PrintBlob);
        Assert.Equal(NondeterminismStrictness.Error, s.NondeterminismStrictness);
        Assert.Equal(Verbosity.Verbose, s.Verbosity);
        Assert.Equal(Database.Disabled, s.Database);
    }

    [Fact]
    public void BuiltInProfilesCanBeLoaded()
    {
        var ci = Settings.FromProfile("ci");
        Assert.True(ci.TestCases > 0);
        Assert.Equal(100, Settings.FromProfile("base").TestCases);
        var e = Assert.Throws<ArgumentException>(() => Settings.FromProfile("no-such-profile-xyz"));
        Assert.Contains("unknown settings profile \"no-such-profile-xyz\"", e.Message);
    }

    [Fact]
    public void RegisteredProfilesResolveAndCanBecomeTheDefault()
    {
        Settings.RegisterProfile("settings-tests", Engines.Quiet with { TestCases = 7, Seed = 3 });
        var s = Settings.FromProfile("settings-tests");
        Assert.Equal(7, s.TestCases);
        Assert.Equal(3UL, s.Seed);
        Assert.Equal(Database.Disabled, s.Database);
        Assert.Equal(Verbosity.Quiet, s.Verbosity);
        Settings.SetDefaultProfile("settings-tests");
        try
        {
            Assert.Equal(7, new Settings().TestCases);
        }
        finally
        {
            Settings.SetDefaultProfile("");
        }
        Assert.Equal(100, new Settings().TestCases);
    }

    [Fact]
    public void TheDefaultAliasCannotBeTheDefaultProfile()
    {
        var e = Assert.Throws<ArgumentException>(() => Settings.SetDefaultProfile("default"));
        Assert.Contains("cannot name the \"default\" alias", e.Message);
    }

    [Fact]
    public void AnEngineFailureCreatingSettingsIsAHegelException()
    {
        using var engine = Engines.Override(new FailingSettingsNew(Engines.Real));
        var e = Assert.Throws<HegelException>(() => new Settings());
        Assert.StartsWith("hegel_settings_new failed (rc=-8): ", e.Message);
    }

    [Fact]
    public void AnUnknownVerbosityFromTheEngineIsAHegelException()
    {
        using var engine = Engines.Override(new UnknownVerbosity(Engines.Real));
        var e = Assert.Throws<HegelException>(() => new Settings());
        Assert.Equal("unknown hegel_verbosity_t value 42", e.Message);
    }

    private sealed class FailingSettingsNew(ILibhegel inner) : DelegatingLibhegel(inner)
    {
        public override int SettingsNew(out nint settings)
        {
            settings = 0;
            return Abi.EInternal;
        }
    }

    private sealed class UnknownVerbosity(ILibhegel inner) : DelegatingLibhegel(inner)
    {
        public override int SettingsGetVerbosity(nint settings) => 42;
    }
}
