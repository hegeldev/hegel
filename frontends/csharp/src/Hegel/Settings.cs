using Hegel.Native;

namespace Hegel;

/// <summary>
/// How a test runs. A new <c>Settings</c> holds the engine's resolved defaults (the <c>hegel.toml</c>
/// profile and the <c>HEGEL_*</c> environment variables), so what you read back is what the run
/// uses; set properties with an object initializer or a <c>with</c> expression:
/// <code>
/// var settings = new Settings { TestCases = 500, Seed = 42 };
/// var ci = Settings.FromProfile("ci") with { Verbosity = Verbosity.Verbose };
/// </code>
/// </summary>
public sealed record Settings
{
    private readonly long _testCases;
    private readonly IReadOnlyList<string> _reproduceFailure = [];
    private readonly IReadOnlyList<string> _infrastructureNamespaces = [];
    private readonly IReadOnlyList<Example> _examples = [];
    private readonly Database _database = Database.DefaultLocation;

    /// <summary>The engine's resolved default settings.</summary>
    public Settings()
        : this(Resolve(Engine.Get(), null))
    {
    }

    private Settings(ILibhegel lib, string profile)
        : this(Resolve(lib, profile))
    {
    }

    private Settings(Resolved r)
    {
        _testCases = r.TestCases;
        Seed = r.Seed;
        Derandomize = r.Derandomize;
        _database = r.Database;
        SuppressHealthCheck = r.SuppressHealthCheck;
        Phases = r.Phases;
        Verbosity = r.Verbosity;
        Backend = r.Backend;
        ReportMultipleFailures = r.ReportMultipleFailures;
        StopOnFirstFailure = r.StopOnFirstFailure;
        ShowStatistics = r.ShowStatistics;
        UnboundedChoices = r.UnboundedChoices;
        PrintBlob = r.PrintBlob;
        NondeterminismStrictness = r.NondeterminismStrictness;
    }

    private readonly record struct Resolved(
        long TestCases, ulong? Seed, bool Derandomize, Database Database, HealthCheck SuppressHealthCheck, Phase Phases,
        Verbosity Verbosity, Backend Backend, bool ReportMultipleFailures, bool StopOnFirstFailure, bool ShowStatistics,
        bool UnboundedChoices, bool PrintBlob, NondeterminismStrictness NondeterminismStrictness);

    private static Resolved Resolve(ILibhegel lib, string? profile)
    {
        var s = Runner.NewSettings(lib, profile);
        try
        {
            var (seed, hasSeed) = lib.SettingsGetSeed(s);
            return new Resolved(
                checked((long)lib.SettingsGetTestCases(s)),
                hasSeed ? seed : null,
                lib.SettingsGetDerandomize(s),
                Database.FromEngine(lib.SettingsGetDatabase(s)),
                (HealthCheck)lib.SettingsGetSuppressHealthCheck(s),
                (Phase)lib.SettingsGetPhases(s),
                EnumCodes.VerbosityFrom(lib.SettingsGetVerbosity(s)),
                EnumCodes.BackendFrom(lib.SettingsGetBackend(s)),
                lib.SettingsGetReportMultipleFailures(s),
                lib.SettingsGetStopOnFirstFailure(s),
                lib.SettingsGetShowStatistics(s),
                lib.SettingsGetUnboundedChoices(s),
                lib.SettingsGetPrintBlob(s),
                EnumCodes.StrictnessFrom(lib.SettingsGetNondeterminismStrictness(s)));
        }
        finally
        {
            lib.SettingsFree(s);
        }
    }

    /// <summary>The settings of the named <c>hegel.toml</c> profile, resolved as <see cref="Settings()"/> resolves the default.</summary>
    public static Settings FromProfile(string name)
    {
        ArgumentNullException.ThrowIfNull(name);
        return new(Engine.Get(), name);
    }

    /// <summary>Register <paramref name="settings"/> as the profile <paramref name="name"/>, for <see cref="FromProfile"/> and <see cref="SetDefaultProfile"/>.</summary>
    public static void RegisterProfile(string name, Settings settings)
    {
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(settings);
        Runner.RegisterProfile(Engine.Get(), name, settings);
    }

    /// <summary>Make the profile <paramref name="name"/> the one new <see cref="Settings"/> resolve from.</summary>
    public static void SetDefaultProfile(string name)
    {
        ArgumentNullException.ThrowIfNull(name);
        Runner.SetDefaultProfile(Engine.Get(), name);
    }

    /// <summary>How many test cases to generate. Must be positive.</summary>
    public long TestCases
    {
        get => _testCases;
        init => _testCases = value > 0 ? value : throw new ArgumentOutOfRangeException(nameof(TestCases), value, "TestCases must be positive");
    }

    /// <summary>The random seed, or <c>null</c> to let the engine choose one.</summary>
    public ulong? Seed { get; init; }

    /// <summary>Whether to derive the seed from the test's identity, so every run draws the same way.</summary>
    public bool Derandomize { get; init; }

    /// <summary>Where failures are saved and replayed from.</summary>
    public Database Database
    {
        get => _database;
        init => _database = value ?? throw new ArgumentNullException(nameof(Database));
    }

    /// <summary>The health checks not to run.</summary>
    public HealthCheck SuppressHealthCheck { get; init; }

    /// <summary>The phases to run.</summary>
    public Phase Phases { get; init; }

    /// <summary>How much to report.</summary>
    public Verbosity Verbosity { get; init; }

    /// <summary>Where randomness comes from.</summary>
    public Backend Backend { get; init; }

    /// <summary>Whether to keep looking for distinct failures after the first.</summary>
    public bool ReportMultipleFailures { get; init; }

    /// <summary>Whether to stop at the first failure without shrinking it.</summary>
    public bool StopOnFirstFailure { get; init; }

    /// <summary>Whether to print the run's statistics when it ends.</summary>
    public bool ShowStatistics { get; init; }

    /// <summary>Whether a test case may draw without the engine's usual size limit.</summary>
    public bool UnboundedChoices { get; init; }

    /// <summary>Whether a failure's report ends with the blob that reproduces it.</summary>
    public bool PrintBlob { get; init; }

    /// <summary>What to do when the test turns out to be nondeterministic.</summary>
    public NondeterminismStrictness NondeterminismStrictness { get; init; }

    /// <summary>
    /// Failure blobs to replay instead of generating test cases. When none of them reproduces a
    /// failure the run throws <see cref="DidNotReproduceException"/>.
    /// </summary>
    public IReadOnlyList<string> ReproduceFailure
    {
        get => _reproduceFailure;
        init => _reproduceFailure = Copy(value, nameof(ReproduceFailure));
    }

    /// <summary>The test's name: its database key, and what its seed derives from under <see cref="Derandomize"/>. Defaults to the calling file and method.</summary>
    public string? Name { get; init; }

    /// <summary>Where the test is defined. Defaults to the caller of <see cref="Property.Test"/>.</summary>
    public TestLocation? TestLocation { get; init; }

    /// <summary>
    /// Namespace prefixes whose frames are not the test's own, on top of this library's and the
    /// runtime's: a failure's origin is the first frame outside them.
    /// </summary>
    public IReadOnlyList<string> InfrastructureNamespaces
    {
        get => _infrastructureNamespaces;
        init => _infrastructureNamespaces = Copy(value, nameof(InfrastructureNamespaces));
    }

    /// <summary>Explicit test cases to run before the generated ones.</summary>
    public IReadOnlyList<Example> Examples
    {
        get => _examples;
        init => _examples = Copy(value, nameof(Examples));
    }

    internal IReadOnlyList<KeyValuePair<string, string>> StringSettings { get; private init; } = [];

    /// <summary>
    /// These settings with the <c>hegel.toml</c> key <paramref name="key"/> set to <paramref name="value"/>,
    /// as the engine parses it. An unknown key or a malformed value throws <see cref="ArgumentException"/> here.
    /// </summary>
    public Settings Set(string key, string value)
    {
        ArgumentNullException.ThrowIfNull(key);
        ArgumentNullException.ThrowIfNull(value);
        Runner.CheckStringSetting(Engine.Get(), key, value);
        return this with { StringSettings = [.. StringSettings, new KeyValuePair<string, string>(key, value)] };
    }

    internal Settings Effective(long testCases, bool printBlob, NondeterminismStrictness strictness, Verbosity verbosity) =>
        this with { TestCases = testCases, PrintBlob = printBlob, NondeterminismStrictness = strictness, Verbosity = verbosity };

    private static IReadOnlyList<T> Copy<T>(IReadOnlyList<T> value, string name) =>
        value == null ? throw new ArgumentNullException(name) : Array.AsReadOnly(value.ToArray());
}
