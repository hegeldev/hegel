using System.Runtime.CompilerServices;
using Hegel.Native;
using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class NativeTests
{
    [Fact]
    public void LibraryFileNameMatchesThePlatform()
    {
        var name = LibraryLoader.LibraryFileName;
        if (OperatingSystem.IsWindows())
        {
            Assert.Equal("libhegel.dll", name);
        }
        else if (OperatingSystem.IsMacOS())
        {
            Assert.Equal("libhegel.dylib", name);
        }
        else
        {
            Assert.Equal("libhegel.so", name);
        }
        Assert.Equal(Path.Combine("base", "runtimes", "rid-x", "native", name), LibraryLoader.BundledPath("base", "rid-x"));
    }

    [Fact]
    public void CandidatesListTheOverrideFirst()
    {
        var bundled = LibraryLoader.BundledPath("base", "rid");
        Assert.Equal([bundled], LibraryLoader.Candidates(null, "base", "rid"));
        Assert.Equal([bundled], LibraryLoader.Candidates("", "base", "rid"));
        Assert.Equal(["/over/ride", bundled], LibraryLoader.Candidates("/over/ride", "base", "rid"));
    }

    [Fact]
    public void ResolvePrefersTheOverride()
    {
        var file = Path.GetTempFileName();
        try
        {
            var loaded = new List<string>();
            Assert.Equal(7, LibraryLoader.Resolve(file, "base", "rid", path =>
            {
                loaded.Add(path);
                return 7;
            }, () => 9));
            Assert.Equal([file], loaded);
            var unloadable = Assert.Throws<HegelException>(() => LibraryLoader.Resolve(file, "base", "rid", _ => 0, () => 9));
            Assert.Equal($"HEGEL_LIBHEGEL_PATH is set to '{file}' but the library there could not be loaded.", unloadable.Message);
        }
        finally
        {
            File.Delete(file);
        }
        var missing = Assert.Throws<HegelException>(() => LibraryLoader.Resolve("/nonexistent/libhegel", "base", "rid", _ => 1, () => 1));
        Assert.Equal("HEGEL_LIBHEGEL_PATH is set to '/nonexistent/libhegel' but no file exists there.", missing.Message);
    }

    [Fact]
    public void ResolveFallsBackFromTheBundledLibraryToTheHost()
    {
        var baseDirectory = Directory.CreateTempSubdirectory("hegel-loader-").FullName;
        try
        {
            var bundled = LibraryLoader.BundledPath(baseDirectory, "rid");
            Assert.Equal(3, LibraryLoader.Resolve(null, baseDirectory, "rid", _ => 0, () => 3));
            var error = Assert.Throws<HegelException>(() => LibraryLoader.Resolve("", baseDirectory, "rid", _ => 0, () => 0));
            Assert.StartsWith($"Could not find libhegel: no library at {bundled} (the Hegel package bundles one for", error.Message);
            Assert.Contains("this process is rid)", error.Message);
            Assert.EndsWith("Set HEGEL_LIBHEGEL_PATH to the path of a libhegel built for this platform.", error.Message);
            Directory.CreateDirectory(Path.GetDirectoryName(bundled)!);
            File.WriteAllText(bundled, "");
            var loaded = new List<string>();
            Assert.Equal(5, LibraryLoader.Resolve(null, baseDirectory, "rid", path =>
            {
                loaded.Add(path);
                return 5;
            }, () => 9));
            Assert.Equal([bundled], loaded);
            Assert.Equal(9, LibraryLoader.Resolve(null, baseDirectory, "rid", _ => 0, () => 9));
        }
        finally
        {
            Directory.Delete(baseDirectory, true);
        }
    }

    [Fact]
    public void LoadIsIdempotent()
    {
        var handle = LibraryLoader.Load();
        Assert.NotEqual(0, handle);
        Assert.Equal(handle, LibraryLoader.Load());
    }

    [Fact]
    public void VersionMismatchesAreWarnedAbout()
    {
        var err = new StringWriter();
        LibraryLoader.WarnOnVersionMismatch("1.2.3", "1.2.3", err);
        Assert.Equal("", err.ToString());
        LibraryLoader.WarnOnVersionMismatch("1.2.3", "1.2.4", err);
        Assert.Equal("[hegel] warning: libhegel 1.2.3 loaded, but these bindings were built against 1.2.4; set HEGEL_LIBHEGEL_PATH to a matching engine.\n", err.ToString().ReplaceLineEndings("\n"));
    }

    [Fact]
    public void LoadingAgainGivesAWorkingBinding()
    {
        var err = new StringWriter();
        var lib = Libhegel.Load(err);
        Assert.Equal(BuildInfo.EngineVersion, lib.Version());
        Assert.Equal(Engines.Real.Version(), lib.Version());
        Assert.Equal("", err.ToString());
    }

    [Fact]
    public void FailedCallsNameTheOperation()
    {
        var error = Assert.Throws<HegelException>(() => Engines.Real.SettingsGetTestCases(0));
        Assert.StartsWith("hegel_settings_get_test_cases failed (rc=-4): ", error.Message);
        Assert.Throws<HegelException>(() => Engines.Real.RunStart(0, _ => { }));
        Assert.Throws<HegelException>(() => Engines.Real.RunStartBlob(0, "blob", _ => { }));
        Assert.Throws<HegelException>(() => Engines.Real.GrammarDrawNew(0, 0));
        Assert.Throws<HegelException>(() => Engines.Real.TestCaseShouldCapture(0));
    }

    [Fact]
    public void FailedOutParametersAreEmpty()
    {
        var lib = Engines.Real;
        Assert.Equal(Abi.EInvalidHandle, lib.GenerateIntegerBig(0, [1], [2], out var big));
        Assert.Empty(big);
        Assert.Equal(Abi.EInvalidHandle, lib.GenerateBytes(0, 0, 1, out var bytes));
        Assert.Empty(bytes);
        Assert.Equal(Abi.EInvalidHandle, lib.GenerateString(0, 0, out var text));
        Assert.Equal("", text);
        Assert.Equal(Abi.EInvalidHandle, lib.GrammarBuilderFile(0, 0, out var path));
        Assert.Equal("", path);
        Assert.Equal(Abi.EInvalidHandle, lib.GrammarDrawResult(0, out var result));
        Assert.Equal("", result);
        Assert.Equal(Abi.EInvalidHandle, lib.PrinterValue(0, out var value));
        Assert.Equal("", value);
    }

    [Fact]
    public void DatabaseSettingsDistinguishDefaultAndDisabled()
    {
        var lib = Engines.Real;
        Assert.Equal(Abi.Ok, lib.SettingsNew(out var s));
        try
        {
            lib.SettingsDatabase(s, null);
            Assert.Null(lib.SettingsGetDatabase(s));
            lib.SettingsDatabase(s, "");
            Assert.Equal("", lib.SettingsGetDatabase(s));
            lib.SettingsDatabase(s, "/some/where");
            Assert.Equal("/some/where", lib.SettingsGetDatabase(s));
            Assert.Equal(Abi.Ok, lib.SettingsSetFromString(s, "test_cases", "17"));
            Assert.Equal(17UL, lib.SettingsGetTestCases(s));
            Assert.Equal(Abi.EInvalidArg, lib.SettingsSetFromString(s, "test_cases", "many"));
            Assert.NotEqual("", lib.LastErrorMessage());
        }
        finally
        {
            lib.SettingsFree(s);
        }
    }

    [Fact]
    public void ProfilesCanBeRegisteredAndLoaded()
    {
        var lib = Engines.Real;
        Assert.Equal(Abi.Ok, lib.SettingsNew(out var s));
        try
        {
            lib.SettingsTestCases(s, 23);
            Assert.Equal(Abi.Ok, lib.SettingsRegisterProfile("native-tests-profile", s));
        }
        finally
        {
            lib.SettingsFree(s);
        }
        Assert.Equal(Abi.Ok, lib.SettingsNewForProfile("native-tests-profile", out var loaded));
        try
        {
            Assert.Equal(23UL, lib.SettingsGetTestCases(loaded));
        }
        finally
        {
            lib.SettingsFree(loaded);
        }
        Assert.Equal(Abi.EInvalidArg, lib.SettingsNewForProfile("native-tests-missing", out _));
        Assert.Equal(Abi.EInvalidArg, lib.SetDefaultProfile("default"));
    }

    [Fact]
    public void LabelsCombine()
    {
        var lib = Engines.Real;
        Assert.Equal(Label.Of("x"), lib.LabelFromName("x"));
        Assert.Equal(Label.Combine(1, 2), lib.LabelCombine([1, 2]));
        Assert.NotEqual(0UL, lib.LabelCombine([]));
    }

    private sealed class ThrowingReporter : Reporter
    {
        public override void EngineOutput(string line) => throw new InvalidOperationException($"reporter rejected: {line}");
    }

    [Fact]
    public void OutputCallbackExceptionsSurfaceAsEngineErrors()
    {
        var error = Assert.Throws<HegelException>(() => Property.Run(tc => tc.Draw(Generators.Integers()), Engines.Quiet with { ShowStatistics = true }, new ThrowingReporter()));
        Assert.Equal("the run's output callback threw", error.Message);
        var inner = Assert.IsType<InvalidOperationException>(error.InnerException);
        Assert.StartsWith("reporter rejected: ", inner.Message);
    }

    private sealed class CollectingReporter : Reporter
    {
        internal List<string> Lines { get; } = [];

        public override void EngineOutput(string line) => Lines.Add(line);
    }

    [Fact]
    public void EngineOutputReachesTheReporter()
    {
        var reporter = new CollectingReporter();
        var report = Property.Run(tc => tc.Draw(Generators.Integers()), Engines.Quiet with { ShowStatistics = true, TestCases = 5 }, reporter);
        Assert.True(report.Passed);
        Assert.NotEmpty(reporter.Lines);
    }

    [Fact]
    public void ReproductionBlobsReplayTheFailure()
    {
        var first = Runs.Failing(tc => tc.Draw(Generators.Integers().Min(1).Max(9), "n"));
        Assert.NotNull(first.ReproduceBlob);
        var replayed = Runs.Failing(tc => tc.Draw(Generators.Integers().Min(1).Max(9), "n"), Engines.Quiet with { ReproduceFailure = [first.ReproduceBlob!] });
        Assert.Equal(first.Output, replayed.Output);
    }

    [Fact]
    public void ErrorRunsCarryTheirKind()
    {
        var report = Property.Run(tc => tc.Reject(), Engines.Quiet, Reporter.Silent);
        Assert.Equal(RunStatus.Error, report.Status);
        Assert.NotNull(report.ErrorKind);
        Assert.False(string.IsNullOrEmpty(report.Error));
    }

    [Fact]
    public void MarkCompleteRejectsFinishedCases()
    {
        var lib = Engines.Real;
        using var run = new RawRun();
        Assert.Equal(Abi.Ok, lib.MarkComplete(run.Tc, Abi.StatusInteresting, "Boom at here:1"));
        Assert.Equal(Abi.EAlreadyComplete, lib.MarkComplete(run.Tc, Abi.StatusValid, null));
    }

    [Fact]
    public void ThreadContextsAreReleasedAndRecreated()
    {
        var lib = Engines.Real;
        string? version = null;
        var thread = new Thread(() =>
        {
            lib.ReleaseThreadContext();
            version = lib.Version();
            lib.ReleaseThreadContext();
            Assert.Equal(version, lib.Version());
        });
        thread.Start();
        thread.Join();
        Assert.Equal(lib.Version(), version);
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static void MakeGarbageHandles()
    {
        var lib = Engines.Real;
        Assert.Equal(Abi.Ok, lib.StringGeneratorEmail(out var email));
        _ = new StringGeneratorHandle(lib, email);
        Assert.Equal(Abi.Ok, lib.GrammarBuilderNew("start: \"a\"", out var builder));
        Assert.Equal(Abi.Ok, lib.GrammarNew(builder, null, 10, 10, 0, [], out var grammar));
        lib.GrammarBuilderFree(builder);
        _ = new GrammarHandle(lib, grammar);
        var thread = new Thread(() => lib.Version());
        thread.Start();
        thread.Join();
    }

    [Fact]
    public void FinalizersFreeEngineHandles()
    {
        MakeGarbageHandles();
        for (var i = 0; i < 3; i++)
        {
            GC.Collect();
            GC.WaitForPendingFinalizers();
        }
    }
}
