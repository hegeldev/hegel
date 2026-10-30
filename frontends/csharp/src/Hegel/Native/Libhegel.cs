using System.Runtime.InteropServices;
using System.Text;

namespace Hegel.Native;

/// <summary>
/// The real binding: <see cref="ILibhegel"/> over the P/Invoke layer, with one engine error
/// context per thread and the string marshalling in one place.
/// </summary>
internal sealed unsafe class Libhegel : ILibhegel
{
    private readonly ThreadLocal<Context> _contexts = new(() => new Context());
    private readonly Dictionary<nint, GCHandle> _outputs = new();

    private Libhegel() { }

    /// <summary>Load the engine and bind to it, warning on <paramref name="err"/> if it is not the expected version.</summary>
    internal static Libhegel Load(TextWriter err)
    {
        LibraryLoader.Load();
        var lib = new Libhegel();
        LibraryLoader.WarnOnVersionMismatch(lib.Version(), BuildInfo.EngineVersion, err);
        return lib;
    }

    /// <summary>A thread's engine error context, freed with the thread.</summary>
    private sealed class Context
    {
        public readonly nint Handle = NativeMethods.hegel_context_new();
        private bool _freed;

        public void Free()
        {
            if (!_freed)
            {
                _freed = true;
                NativeMethods.hegel_context_free(Handle);
            }
        }

        ~Context()
        {
            Free();
        }
    }

    private nint Ctx => _contexts.Value!.Handle;

    public void ReleaseThreadContext()
    {
        if (_contexts.IsValueCreated)
        {
            _contexts.Value!.Free();
            _contexts.Value = new Context();
        }
    }

    public string LastErrorMessage() => Borrowed(NativeMethods.hegel_context_last_error(Ctx));

    public string Version()
    {
        HegelString v;
        Require(NativeMethods.hegel_version(Ctx, &v), "hegel_version");
        return Borrowed(v);
    }

    private static string Borrowed(HegelString s) =>
        s.Data == null || s.Len == 0 ? "" : Encoding.UTF8.GetString(s.Data, checked((int)s.Len));

    private static string? Borrowed(HegelString* s) => s == null ? null : Borrowed(*s);

    private string Owned(HegelString s)
    {
        try
        {
            return Borrowed(s);
        }
        finally
        {
            NativeMethods.hegel_owned_string_free(Ctx, &s);
        }
    }

    private void Require(int rc, string op)
    {
        if (rc != Abi.Ok)
        {
            throw Failure(rc, op);
        }
    }

    private HegelException Failure(int rc, string op) => new($"{op} failed (rc={rc}): {LastErrorMessage()}");


    /// <summary>Unmanaged copies of strings for the duration of one call.</summary>
    private ref struct Arena
    {
        private List<nint>? _blocks;

        private byte* Alloc(nuint size)
        {
            var p = (byte*)NativeMemory.Alloc(size);
            (_blocks ??= new List<nint>()).Add((nint)p);
            return p;
        }

        public HegelString Str(string s)
        {
            var count = Encoding.UTF8.GetByteCount(s);
            var p = Alloc((nuint)Math.Max(count, 1));
            fixed (char* c = s)
            {
                Encoding.UTF8.GetBytes(c, s.Length, p, count);
            }
            return new HegelString { Data = p, Len = (nuint)count };
        }

        public HegelString* StrPtr(string? s)
        {
            if (s == null)
            {
                return null;
            }
            var p = (HegelString*)Alloc((nuint)sizeof(HegelString));
            *p = Str(s);
            return p;
        }

        public HegelString* Strs(IReadOnlyList<string>? strings)
        {
            if (strings == null)
            {
                return null;
            }
            var p = (HegelString*)Alloc((nuint)(sizeof(HegelString) * Math.Max(strings.Count, 1)));
            for (var i = 0; i < strings.Count; i++)
            {
                p[i] = Str(strings[i]);
            }
            return p;
        }

        public void Dispose()
        {
            if (_blocks != null)
            {
                foreach (var block in _blocks)
                {
                    NativeMemory.Free((void*)block);
                }
            }
        }
    }

    public int SettingsNew(out nint settings)
    {
        nint s;
        var rc = NativeMethods.hegel_settings_new(Ctx, &s);
        settings = s;
        return rc;
    }

    public int SettingsNewForProfile(string name, out nint settings)
    {
        using var a = new Arena();
        nint s;
        var rc = NativeMethods.hegel_settings_new_for_profile(Ctx, a.Str(name), &s);
        settings = s;
        return rc;
    }

    public int SettingsRegisterProfile(string name, nint settings)
    {
        using var a = new Arena();
        return NativeMethods.hegel_settings_register_profile(Ctx, a.Str(name), settings);
    }

    public int SettingsSetFromString(nint settings, string key, string value)
    {
        using var a = new Arena();
        return NativeMethods.hegel_settings_set_from_string(Ctx, settings, a.Str(key), a.Str(value));
    }

    public int SetDefaultProfile(string name)
    {
        using var a = new Arena();
        return NativeMethods.hegel_set_default_profile(Ctx, a.Str(name));
    }

    public void SettingsFree(nint settings) => NativeMethods.hegel_settings_free(Ctx, settings);

    public ulong SettingsGetTestCases(nint settings)
    {
        ulong o;
        Require(NativeMethods.hegel_settings_get_test_cases(Ctx, settings, &o), "hegel_settings_get_test_cases");
        return o;
    }

    private delegate int BoolGetter(nint ctx, nint s, byte* o);

    private bool GetBool(nint settings, BoolGetter getter, string op)
    {
        byte o;
        Require(getter(Ctx, settings, &o), op);
        return o != 0;
    }

    public bool SettingsGetPrintBlob(nint settings) => GetBool(settings, NativeMethods.hegel_settings_get_print_blob, "hegel_settings_get_print_blob");

    public int SettingsGetNondeterminismStrictness(nint settings)
    {
        int o;
        Require(NativeMethods.hegel_settings_get_nondeterminism_strictness(Ctx, settings, &o), "hegel_settings_get_nondeterminism_strictness");
        return o;
    }

    public int SettingsGetVerbosity(nint settings)
    {
        int o;
        Require(NativeMethods.hegel_settings_get_verbosity(Ctx, settings, &o), "hegel_settings_get_verbosity");
        return o;
    }

    public int SettingsGetPhases(nint settings)
    {
        uint o;
        Require(NativeMethods.hegel_settings_get_phases(Ctx, settings, &o), "hegel_settings_get_phases");
        return (int)o;
    }

    public bool SettingsGetReportMultipleFailures(nint settings) => GetBool(settings, NativeMethods.hegel_settings_get_report_multiple_failures, "hegel_settings_get_report_multiple_failures");

    public (ulong Seed, bool HasSeed) SettingsGetSeed(nint settings)
    {
        ulong seed;
        byte has;
        Require(NativeMethods.hegel_settings_get_seed(Ctx, settings, &seed, &has), "hegel_settings_get_seed");
        return (seed, has != 0);
    }

    public bool SettingsGetDerandomize(nint settings) => GetBool(settings, NativeMethods.hegel_settings_get_derandomize, "hegel_settings_get_derandomize");

    public string? SettingsGetDatabase(nint settings)
    {
        HegelString* o;
        Require(NativeMethods.hegel_settings_get_database(Ctx, settings, &o), "hegel_settings_get_database");
        return Borrowed(o);
    }

    public int SettingsGetSuppressHealthCheck(nint settings)
    {
        uint o;
        Require(NativeMethods.hegel_settings_get_suppress_health_check(Ctx, settings, &o), "hegel_settings_get_suppress_health_check");
        return (int)o;
    }

    public bool SettingsGetStopOnFirstFailure(nint settings) => GetBool(settings, NativeMethods.hegel_settings_get_stop_on_first_failure, "hegel_settings_get_stop_on_first_failure");

    public bool SettingsGetShowStatistics(nint settings) => GetBool(settings, NativeMethods.hegel_settings_get_show_statistics, "hegel_settings_get_show_statistics");

    public bool SettingsGetUnboundedChoices(nint settings) => GetBool(settings, NativeMethods.hegel_settings_get_unbounded_choices, "hegel_settings_get_unbounded_choices");

    public int SettingsGetBackend(nint settings)
    {
        int o;
        Require(NativeMethods.hegel_settings_get_backend(Ctx, settings, &o), "hegel_settings_get_backend");
        return o;
    }

    public void SettingsBackend(nint settings, int backend) => Require(NativeMethods.hegel_settings_set_backend(Ctx, settings, (uint)backend), "hegel_settings_set_backend");

    public void SettingsTestCases(nint settings, ulong n) => Require(NativeMethods.hegel_settings_set_test_cases(Ctx, settings, n), "hegel_settings_set_test_cases");

    public void SettingsVerbosity(nint settings, int verbosity) => Require(NativeMethods.hegel_settings_set_verbosity(Ctx, settings, (uint)verbosity), "hegel_settings_set_verbosity");

    public void SettingsSeed(nint settings, ulong seed, bool hasSeed) => Require(NativeMethods.hegel_settings_set_seed(Ctx, settings, seed, hasSeed), "hegel_settings_set_seed");

    public void SettingsDerandomize(nint settings, bool yes) => Require(NativeMethods.hegel_settings_set_derandomize(Ctx, settings, yes), "hegel_settings_set_derandomize");

    public void SettingsReportMultipleFailures(nint settings, bool yes) => Require(NativeMethods.hegel_settings_set_report_multiple_failures(Ctx, settings, yes), "hegel_settings_set_report_multiple_failures");

    public void SettingsStopOnFirstFailure(nint settings, bool yes) => Require(NativeMethods.hegel_settings_set_stop_on_first_failure(Ctx, settings, yes), "hegel_settings_set_stop_on_first_failure");

    public void SettingsShowStatistics(nint settings, bool yes) => Require(NativeMethods.hegel_settings_set_show_statistics(Ctx, settings, yes), "hegel_settings_set_show_statistics");

    public void SettingsUnboundedChoices(nint settings, bool yes) => Require(NativeMethods.hegel_settings_set_unbounded_choices(Ctx, settings, yes), "hegel_settings_set_unbounded_choices");

    public void SettingsTestLocation(nint settings, string file, int beginLine, string className, string function)
    {
        using var a = new Arena();
        Require(NativeMethods.hegel_settings_set_test_location(Ctx, settings, a.Str(file), (uint)beginLine, a.Str(className), a.Str(function)), "hegel_settings_set_test_location");
    }

    public void SettingsDatabase(nint settings, string? path)
    {
        using var a = new Arena();
        Require(NativeMethods.hegel_settings_set_database(Ctx, settings, a.StrPtr(path)), "hegel_settings_set_database");
    }

    public void SettingsDatabaseKey(nint settings, string key)
    {
        using var a = new Arena();
        Require(NativeMethods.hegel_settings_set_database_key(Ctx, settings, a.StrPtr(key)), "hegel_settings_set_database_key");
    }

    public void SettingsPhases(nint settings, int mask) => Require(NativeMethods.hegel_settings_set_phases(Ctx, settings, (uint)mask), "hegel_settings_set_phases");

    public void SettingsSuppressHealthCheck(nint settings, int mask) => Require(NativeMethods.hegel_settings_set_suppress_health_check(Ctx, settings, (uint)mask), "hegel_settings_set_suppress_health_check");

    public void SettingsPrintBlob(nint settings, bool yes) => Require(NativeMethods.hegel_settings_set_print_blob(Ctx, settings, yes), "hegel_settings_set_print_blob");

    public void SettingsNondeterminismStrictness(nint settings, int strictness) => Require(NativeMethods.hegel_settings_set_nondeterminism_strictness(Ctx, settings, (uint)strictness), "hegel_settings_set_nondeterminism_strictness");

    private sealed class OutputSink
    {
        public OutputSink(Action<string> output)
        {
            Output = output;
        }

        public Action<string> Output { get; }
        public Exception? Pending { get; set; }
    }

    [UnmanagedCallersOnly]
    private static void OnOutput(nint userData, byte* line, nuint len)
    {
        var sink = (OutputSink)GCHandle.FromIntPtr(userData).Target!;
        try
        {
            sink.Output(Encoding.UTF8.GetString(line, checked((int)len)));
        }
        catch (Exception e)
        {
            sink.Pending ??= e;
        }
    }

    private nint RegisterRun(nint run, GCHandle sink)
    {
        lock (_outputs)
        {
            _outputs[run] = sink;
        }
        return run;
    }

    private void RethrowPending(nint run)
    {
        OutputSink? sink;
        lock (_outputs)
        {
            sink = _outputs.TryGetValue(run, out var handle) ? (OutputSink)handle.Target! : null;
        }
        if (sink?.Pending != null)
        {
            var pending = sink.Pending;
            sink.Pending = null;
            throw new HegelException("the run's output callback threw", pending);
        }
    }

    public nint RunStart(nint settings, Action<string> output)
    {
        var sink = GCHandle.Alloc(new OutputSink(output));
        nint run;
        var rc = NativeMethods.hegel_run_start(Ctx, settings, &OnOutput, GCHandle.ToIntPtr(sink), &run);
        if (rc != Abi.Ok)
        {
            sink.Free();
            throw Failure(rc, "hegel_run_start");
        }
        return RegisterRun(run, sink);
    }

    public nint RunStartBlob(nint settings, string blob, Action<string> output)
    {
        using var a = new Arena();
        var sink = GCHandle.Alloc(new OutputSink(output));
        nint run;
        var rc = NativeMethods.hegel_run_start_blob(Ctx, settings, a.Str(blob), &OnOutput, GCHandle.ToIntPtr(sink), &run);
        if (rc != Abi.Ok)
        {
            sink.Free();
            throw Failure(rc, "hegel_run_start_blob");
        }
        return RegisterRun(run, sink);
    }

    public nint NextTestCase(nint run)
    {
        nint tc;
        Require(NativeMethods.hegel_next_test_case(Ctx, run, &tc), "hegel_next_test_case");
        RethrowPending(run);
        return tc;
    }

    public nint RunResult(nint run)
    {
        nint result;
        Require(NativeMethods.hegel_run_result(Ctx, run, &result), "hegel_run_result");
        RethrowPending(run);
        return result;
    }

    public void RunResultFree(nint result) => NativeMethods.hegel_run_result_free(Ctx, result);

    public void RunFree(nint run)
    {
        NativeMethods.hegel_run_free(Ctx, run);
        lock (_outputs)
        {
            if (_outputs.Remove(run, out var sink))
            {
                sink.Free();
            }
        }
    }

    public void TestCaseFree(nint testCase) => NativeMethods.hegel_test_case_free(Ctx, testCase);

    public bool TestCaseShouldCapture(nint testCase)
    {
        byte o;
        Require(NativeMethods.hegel_test_case_should_capture(Ctx, testCase, &o), "hegel_test_case_should_capture");
        return o != 0;
    }

    public int TestCaseClone(nint testCase, out nint clone)
    {
        nint o;
        var rc = NativeMethods.hegel_test_case_clone(Ctx, testCase, &o);
        clone = o;
        return rc;
    }

    public void TestCaseSetWorker(nint testCase, long workerIndex) => Require(NativeMethods.hegel_test_case_set_worker(Ctx, testCase, workerIndex), "hegel_test_case_set_worker");

    public int TestCaseBlock(nint testCase, ulong indent, out nint block)
    {
        nint o;
        var rc = NativeMethods.hegel_test_case_block(Ctx, testCase, indent, &o);
        block = o;
        return rc;
    }

    public int Note(nint testCase, string text)
    {
        using var a = new Arena();
        return NativeMethods.hegel_note(Ctx, testCase, a.Str(text));
    }

    public int TestCasePrinter(nint testCase, out nint printer)
    {
        nint o;
        var rc = NativeMethods.hegel_test_case_printer(Ctx, testCase, 0, &o);
        printer = o;
        return rc;
    }

    public nint PrinterNew(nint options)
    {
        nint o;
        Require(NativeMethods.hegel_printer_new(Ctx, options, &o), "hegel_printer_new");
        return o;
    }

    public nint PrinterOptionsNew()
    {
        nint o;
        Require(NativeMethods.hegel_printer_options_new(Ctx, &o), "hegel_printer_options_new");
        return o;
    }

    public int PrinterOptionsSetMaxWidth(nint options, ulong maxWidth) => NativeMethods.hegel_printer_options_set_max_width(Ctx, options, maxWidth);

    public void PrinterOptionsFree(nint options) => NativeMethods.hegel_printer_options_free(Ctx, options);

    public void PrinterFree(nint printer) => NativeMethods.hegel_printer_free(Ctx, printer);

    public int PrinterText(nint printer, string text)
    {
        using var a = new Arena();
        return NativeMethods.hegel_printer_text(Ctx, printer, a.Str(text));
    }

    public int PrinterBreakable(nint printer, string separator)
    {
        using var a = new Arena();
        return NativeMethods.hegel_printer_breakable(Ctx, printer, a.Str(separator));
    }

    public int PrinterHardBreak(nint printer) => NativeMethods.hegel_printer_hard_break(Ctx, printer);

    public int PrinterIfBreak(nint printer, string text)
    {
        using var a = new Arena();
        return NativeMethods.hegel_printer_if_break(Ctx, printer, a.Str(text));
    }

    public int PrinterComment(nint printer, string text)
    {
        using var a = new Arena();
        return NativeMethods.hegel_printer_comment(Ctx, printer, a.Str(text));
    }

    public int PrinterShiftIndent(nint printer, long delta) => NativeMethods.hegel_printer_shift_indent(Ctx, printer, delta);

    public int PrinterBeginGroup(nint printer, ulong indent, string open)
    {
        using var a = new Arena();
        return NativeMethods.hegel_printer_begin_group(Ctx, printer, indent, a.Str(open));
    }

    public int PrinterEndGroup(nint printer, string close)
    {
        using var a = new Arena();
        return NativeMethods.hegel_printer_end_group(Ctx, printer, a.Str(close));
    }

    public int PrinterReflow(nint printer, string text)
    {
        using var a = new Arena();
        return NativeMethods.hegel_printer_reflow(Ctx, printer, 0, a.Str(text));
    }

    public int PrinterBeginSpeculative(nint printer) => NativeMethods.hegel_printer_begin_speculative(Ctx, printer);

    public int PrinterCommitSpeculative(nint printer) => NativeMethods.hegel_printer_commit_speculative(Ctx, printer);

    public int PrinterAbortSpeculative(nint printer) => NativeMethods.hegel_printer_abort_speculative(Ctx, printer);

    public int PrinterDeferred(nint printer, out nint child)
    {
        nint o;
        var rc = NativeMethods.hegel_printer_deferred(Ctx, printer, &o);
        child = o;
        return rc;
    }

    public int PrinterIsLive(nint printer, out bool live)
    {
        byte o;
        var rc = NativeMethods.hegel_printer_is_live(Ctx, printer, &o);
        live = o != 0;
        return rc;
    }

    public int PrinterResolve(nint printer) => NativeMethods.hegel_printer_resolve(Ctx, printer);

    public int PrinterValue(nint printer, out string value)
    {
        HegelString o;
        var rc = NativeMethods.hegel_printer_value(Ctx, printer, &o);
        if (rc != Abi.Ok)
        {
            value = "";
            return rc;
        }
        try
        {
            value = Borrowed(o);
        }
        finally
        {
            NativeMethods.hegel_printer_value_result_free(Ctx, &o);
        }
        return rc;
    }

    public int GenerateBoolean(nint testCase, double p, out bool value)
    {
        byte o;
        var rc = NativeMethods.hegel_generate_boolean(Ctx, testCase, p, false, false, &o);
        value = o != 0;
        return rc;
    }

    public int GenerateInteger(nint testCase, long min, long max, out long value)
    {
        long o;
        var rc = NativeMethods.hegel_generate_integer(Ctx, testCase, min, max, &o);
        value = o;
        return rc;
    }

    public int GenerateIntegerBig(nint testCase, byte[] minLittleEndian, byte[] maxLittleEndian, out byte[] valueLittleEndian)
    {
        var buffer = new byte[Math.Max(minLittleEndian.Length, maxLittleEndian.Length) + 1];
        nuint len;
        int rc;
        fixed (byte* min = minLittleEndian)
        fixed (byte* max = maxLittleEndian)
        fixed (byte* o = buffer)
        {
            rc = NativeMethods.hegel_generate_integer_big(Ctx, testCase, min, (nuint)minLittleEndian.Length, max, (nuint)maxLittleEndian.Length, o, (nuint)buffer.Length, &len);
        }
        valueLittleEndian = rc == Abi.Ok ? buffer[..checked((int)len)] : [];
        return rc;
    }

    public int GenerateFloat(nint testCase, int width, double min, double max, bool allowNan, bool allowInfinity, bool excludeMin, bool excludeMax, double smallestNonzeroMagnitude, out double value)
    {
        double o;
        var rc = NativeMethods.hegel_generate_float(Ctx, testCase, (uint)width, min, max, allowNan, allowInfinity, excludeMin, excludeMax, smallestNonzeroMagnitude, &o);
        value = o;
        return rc;
    }

    public int GenerateBytes(nint testCase, ulong minSize, ulong maxSize, out byte[] value)
    {
        HegelBytes o;
        var rc = NativeMethods.hegel_generate_bytes(Ctx, testCase, minSize, maxSize, &o);
        if (rc != Abi.Ok)
        {
            value = [];
            return rc;
        }
        try
        {
            value = new byte[checked((int)o.Len)];
            if (o.Len > 0)
            {
                Marshal.Copy((nint)o.Data, value, 0, value.Length);
            }
        }
        finally
        {
            NativeMethods.hegel_generate_bytes_result_free(Ctx, &o);
        }
        return rc;
    }

    public int GenerateString(nint testCase, nint generator, out string value)
    {
        HegelString o;
        var rc = NativeMethods.hegel_generate_string(Ctx, testCase, generator, &o);
        value = rc == Abi.Ok ? Owned(o) : "";
        return rc;
    }

    public int GenerateDate(nint testCase, HegelDate min, HegelDate max, out HegelDate value)
    {
        HegelDate o;
        var rc = NativeMethods.hegel_generate_date(Ctx, testCase, min, max, &o);
        value = o;
        return rc;
    }

    public int GenerateTime(nint testCase, HegelTime min, HegelTime max, out HegelTime value)
    {
        HegelTime o;
        var rc = NativeMethods.hegel_generate_time(Ctx, testCase, min, max, &o);
        value = o;
        return rc;
    }

    public int GenerateDatetime(nint testCase, HegelDateTime min, HegelDateTime max, out HegelDateTime value)
    {
        HegelDateTime o;
        var rc = NativeMethods.hegel_generate_datetime(Ctx, testCase, min, max, &o);
        value = o;
        return rc;
    }

    public int GenerateUuid(nint testCase, int version, bool hasVersion, byte[] out16)
    {
        fixed (byte* o = out16)
        {
            return NativeMethods.hegel_generate_uuid(Ctx, testCase, (byte)version, hasVersion, o);
        }
    }

    public int GenerateIpv4(nint testCase, byte[] out4)
    {
        fixed (byte* o = out4)
        {
            return NativeMethods.hegel_generate_ipv4(Ctx, testCase, o);
        }
    }

    public int GenerateIpv6(nint testCase, byte[] out16)
    {
        fixed (byte* o = out16)
        {
            return NativeMethods.hegel_generate_ipv6(Ctx, testCase, o);
        }
    }

    public int StringGeneratorText(ulong minSize, ulong maxSize, string? codec, uint minCodepoint, uint maxCodepoint, IReadOnlyList<string>? categories, IReadOnlyList<string>? excludeCategories, string? includeCharacters, string? excludeCharacters, out nint generator)
    {
        using var a = new Arena();
        nint o;
        var rc = NativeMethods.hegel_string_generator_text(
            Ctx, minSize, maxSize, a.StrPtr(codec), minCodepoint, maxCodepoint,
            a.Strs(categories), (nuint)(categories?.Count ?? 0),
            a.Strs(excludeCategories), (nuint)(excludeCategories?.Count ?? 0),
            a.Str(includeCharacters ?? ""), a.Str(excludeCharacters ?? ""), &o);
        generator = o;
        return rc;
    }

    public int StringGeneratorRegex(string pattern, bool fullmatch, nint alphabet, out nint generator)
    {
        using var a = new Arena();
        nint o;
        var rc = NativeMethods.hegel_string_generator_regex(Ctx, a.Str(pattern), fullmatch, alphabet, &o);
        generator = o;
        return rc;
    }

    public int StringGeneratorEmail(out nint generator)
    {
        nint o;
        var rc = NativeMethods.hegel_string_generator_email(Ctx, &o);
        generator = o;
        return rc;
    }

    public int StringGeneratorUrl(out nint generator)
    {
        nint o;
        var rc = NativeMethods.hegel_string_generator_url(Ctx, &o);
        generator = o;
        return rc;
    }

    public int StringGeneratorDomain(ulong maxLength, out nint generator)
    {
        nint o;
        var rc = NativeMethods.hegel_string_generator_domain(Ctx, maxLength, &o);
        generator = o;
        return rc;
    }

    public void StringGeneratorFree(nint generator) => NativeMethods.hegel_string_generator_free(Ctx, generator);

    public int GrammarBuilderNew(string text, out nint builder)
    {
        using var a = new Arena();
        nint o;
        var rc = NativeMethods.hegel_grammar_builder_new(Ctx, a.Str(text), &o);
        builder = o;
        return rc;
    }

    public int GrammarBuilderFile(nint builder, ulong index, out string path)
    {
        HegelString o;
        var rc = NativeMethods.hegel_grammar_builder_file(Ctx, builder, (nuint)index, &o);
        path = rc == Abi.Ok ? Borrowed(o) : "";
        return rc;
    }

    public int GrammarBuilderProvide(nint builder, ulong index, string? text)
    {
        using var a = new Arena();
        return NativeMethods.hegel_grammar_builder_provide(Ctx, builder, (nuint)index, a.StrPtr(text));
    }

    public void GrammarBuilderFree(nint builder) => NativeMethods.hegel_grammar_builder_free(Ctx, builder);

    public int GrammarNew(nint builder, string? start, ulong maxSize, uint maxDepth, nint alphabet, IReadOnlyList<string> explicitNames, out nint grammar)
    {
        using var a = new Arena();
        nint o;
        var rc = NativeMethods.hegel_grammar_new(Ctx, builder, a.StrPtr(start), maxSize, maxDepth, alphabet, a.Strs(explicitNames), (nuint)explicitNames.Count, &o);
        grammar = o;
        return rc;
    }

    public void GrammarFree(nint grammar) => NativeMethods.hegel_grammar_free(Ctx, grammar);

    public nint GrammarDrawNew(nint grammar, ulong maxSize)
    {
        nint o;
        Require(NativeMethods.hegel_grammar_draw_new(Ctx, grammar, maxSize, &o), "hegel_grammar_draw_new");
        return o;
    }

    public int GrammarDrawNext(nint testCase, nint draw, out ulong terminal)
    {
        nuint o;
        var rc = NativeMethods.hegel_grammar_draw_next(Ctx, testCase, draw, &o);
        terminal = o;
        return rc;
    }

    public int GrammarDrawProvide(nint draw, string value)
    {
        using var a = new Arena();
        return NativeMethods.hegel_grammar_draw_provide(Ctx, draw, a.Str(value));
    }

    public int GrammarDrawResult(nint draw, out string value)
    {
        HegelString o;
        var rc = NativeMethods.hegel_grammar_draw_result(Ctx, draw, &o);
        value = rc == Abi.Ok ? Owned(o) : "";
        return rc;
    }

    public void GrammarDrawFree(nint draw) => NativeMethods.hegel_grammar_draw_free(Ctx, draw);

    public int StartSpan(nint testCase, ulong label) => NativeMethods.hegel_start_span(Ctx, testCase, label);

    public int StopSpan(nint testCase, bool discard) => NativeMethods.hegel_stop_span(Ctx, testCase, discard);

    public int NewCollection(nint testCase, ulong minSize, ulong maxSize, out nint collection)
    {
        nint o;
        var rc = NativeMethods.hegel_new_collection(Ctx, testCase, minSize, maxSize, &o);
        collection = o;
        return rc;
    }

    public int CollectionMore(nint testCase, nint collection, out bool more)
    {
        byte o;
        var rc = NativeMethods.hegel_collection_more(Ctx, testCase, collection, &o);
        more = o != 0;
        return rc;
    }

    public int CollectionReject(nint testCase, nint collection, string? why)
    {
        using var a = new Arena();
        return NativeMethods.hegel_collection_reject(Ctx, testCase, collection, a.StrPtr(why));
    }

    public void CollectionFree(nint collection) => NativeMethods.hegel_collection_free(Ctx, collection);

    public int NewRepeat(nint testCase, out nint repeat)
    {
        nint o;
        var rc = NativeMethods.hegel_new_repeat(Ctx, testCase, &o);
        repeat = o;
        return rc;
    }

    public int RepeatMore(nint testCase, nint repeat, out bool more)
    {
        byte o;
        var rc = NativeMethods.hegel_repeat_more(Ctx, testCase, repeat, &o);
        more = o != 0;
        return rc;
    }

    public int RepeatFailed(nint testCase, nint repeat) => NativeMethods.hegel_repeat_failed(Ctx, testCase, repeat);

    public void RepeatFree(nint repeat) => NativeMethods.hegel_repeat_free(Ctx, repeat);

    public int NewPool(nint testCase, out nint pool)
    {
        nint o;
        var rc = NativeMethods.hegel_new_pool(Ctx, testCase, &o);
        pool = o;
        return rc;
    }

    public int PoolAdd(nint testCase, nint pool, out long variableId)
    {
        long o;
        var rc = NativeMethods.hegel_pool_add(Ctx, testCase, pool, &o);
        variableId = o;
        return rc;
    }

    public int PoolGenerate(nint testCase, nint pool, bool consume, out long variableId)
    {
        long o;
        var rc = NativeMethods.hegel_pool_generate(Ctx, testCase, pool, consume, &o);
        variableId = o;
        return rc;
    }

    public int PoolRestore(nint testCase, nint pool, long variableId) => NativeMethods.hegel_pool_restore(Ctx, testCase, pool, variableId);

    public void PoolFree(nint pool) => NativeMethods.hegel_pool_free(Ctx, pool);

    public int NewStateMachine(nint testCase, IReadOnlyList<string> ruleNames, long[] ruleGroups, double[]? ruleWeights, IReadOnlyList<string> invariantNames, bool[] invariantAlwaysCheck, long minConcurrency, long maxConcurrency, long stepCount, out nint stateMachine, out long concurrency)
    {
        using var a = new Arena();
        var always = new byte[invariantAlwaysCheck.Length];
        for (var i = 0; i < always.Length; i++)
        {
            always[i] = invariantAlwaysCheck[i] ? (byte)1 : (byte)0;
        }
        nint sm;
        long c;
        int rc;
        fixed (long* groups = ruleGroups)
        fixed (double* weights = ruleWeights)
        fixed (byte* alwaysPtr = always)
        {
            rc = NativeMethods.hegel_new_state_machine(
                Ctx, testCase, a.Strs(ruleNames), groups, weights, (nuint)ruleNames.Count,
                a.Strs(invariantNames), alwaysPtr, (nuint)invariantNames.Count,
                minConcurrency, maxConcurrency, stepCount, &sm, &c);
        }
        stateMachine = sm;
        concurrency = c;
        return rc;
    }

    public int StateMachineNextGroup(nint testCase, nint stateMachine, out long group)
    {
        long o;
        var rc = NativeMethods.hegel_state_machine_next_group(Ctx, testCase, stateMachine, &o);
        group = o;
        return rc;
    }

    public int StateMachineNextRule(nint testCase, nint stateMachine, long workerIndex, out long rule)
    {
        long o;
        var rc = NativeMethods.hegel_state_machine_next_rule(Ctx, testCase, stateMachine, workerIndex, &o);
        rule = o;
        return rc;
    }

    public int StateMachineRuleRejected(nint testCase, nint stateMachine, long workerIndex) => NativeMethods.hegel_state_machine_rule_rejected(Ctx, testCase, stateMachine, workerIndex);

    public int StateMachineShouldCheckInvariant(nint testCase, nint stateMachine, long invariantIndex, out bool check)
    {
        byte o;
        var rc = NativeMethods.hegel_state_machine_should_check_invariant(Ctx, testCase, stateMachine, invariantIndex, &o);
        check = o != 0;
        return rc;
    }

    public void StateMachineFree(nint stateMachine) => NativeMethods.hegel_state_machine_free(Ctx, stateMachine);

    public int NewRecursion(nint testCase, ulong maxDepth, ulong maxLeaves, out nint recursion)
    {
        nint o;
        var rc = NativeMethods.hegel_new_recursion(Ctx, testCase, maxDepth, maxLeaves, &o);
        recursion = o;
        return rc;
    }

    public int RecursionBranch(nint testCase, nint recursion, ulong depth, out bool branch)
    {
        byte o;
        var rc = NativeMethods.hegel_recursion_branch(Ctx, testCase, recursion, depth, &o);
        branch = o != 0;
        return rc;
    }

    public int RecursionLeaf(nint testCase, nint recursion) => NativeMethods.hegel_recursion_leaf(Ctx, testCase, recursion);

    public int RecursionRetry(nint testCase, nint recursion) => NativeMethods.hegel_recursion_retry(Ctx, testCase, recursion);

    public int RecursionFinish(nint testCase, nint recursion) => NativeMethods.hegel_recursion_finish(Ctx, testCase, recursion);

    public void RecursionFree(nint recursion) => NativeMethods.hegel_recursion_free(Ctx, recursion);

    public int Target(nint testCase, double value, string label)
    {
        using var a = new Arena();
        return NativeMethods.hegel_target(Ctx, testCase, value, a.Str(label));
    }

    public int Event(nint testCase, string label)
    {
        using var a = new Arena();
        return NativeMethods.hegel_event(Ctx, testCase, a.Str(label));
    }

    public int EventValue(nint testCase, double value, string label)
    {
        using var a = new Arena();
        return NativeMethods.hegel_event_value(Ctx, testCase, value, a.Str(label));
    }

    public int MarkComplete(nint testCase, int status, string? origin)
    {
        using var a = new Arena();
        return NativeMethods.hegel_mark_complete(Ctx, testCase, (uint)status, a.StrPtr(origin));
    }

    public int RunResultStatus(nint result)
    {
        int o;
        Require(NativeMethods.hegel_run_result_status(Ctx, result, &o), "hegel_run_result_status");
        return o;
    }

    public string RunResultError(nint result)
    {
        HegelString* o;
        Require(NativeMethods.hegel_run_result_error(Ctx, result, &o), "hegel_run_result_error");
        return Borrowed(o) ?? "";
    }

    public int RunResultErrorKind(nint result, out int healthCheck)
    {
        int kind;
        uint hc;
        Require(NativeMethods.hegel_run_result_error_kind(Ctx, result, &kind, &hc), "hegel_run_result_error_kind");
        healthCheck = (int)hc;
        return kind;
    }

    public ulong RunResultFailureCount(nint result)
    {
        nuint o;
        Require(NativeMethods.hegel_run_result_failure_count(Ctx, result, &o), "hegel_run_result_failure_count");
        return o;
    }

    private T WithFailure<T>(nint result, ulong index, Func<nint, T> read)
    {
        nint f;
        Require(NativeMethods.hegel_run_result_failure(Ctx, result, (nuint)index, &f), "hegel_run_result_failure");
        try
        {
            return read(f);
        }
        finally
        {
            NativeMethods.hegel_failure_free(Ctx, f);
        }
    }

    public string? FailureBlob(nint result, ulong index) => WithFailure(result, index, f =>
    {
        HegelString* o;
        Require(NativeMethods.hegel_failure_reproduction_blob(Ctx, f, &o), "hegel_failure_reproduction_blob");
        return Borrowed(o);
    });

    public string FailureOrigin(nint result, ulong index) => WithFailure(result, index, f =>
    {
        HegelString o;
        Require(NativeMethods.hegel_failure_origin(Ctx, f, &o), "hegel_failure_origin");
        return Borrowed(o);
    });

    public string? FailureCaveat(nint result, ulong index) => WithFailure(result, index, f =>
    {
        HegelString* o;
        Require(NativeMethods.hegel_failure_caveat(Ctx, f, &o), "hegel_failure_caveat");
        return Borrowed(o);
    });

    public ulong LabelFromName(string name)
    {
        using var a = new Arena();
        ulong o;
        Require(NativeMethods.hegel_label_from_name(Ctx, a.Str(name), &o), "hegel_label_from_name");
        return o;
    }

    public ulong LabelCombine(ulong[] labels)
    {
        ulong o;
        fixed (ulong* p = labels)
        {
            Require(NativeMethods.hegel_label_combine(Ctx, p, (nuint)labels.Length, &o), "hegel_label_combine");
        }
        return o;
    }
}
