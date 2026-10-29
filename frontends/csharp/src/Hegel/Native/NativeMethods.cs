using System.Runtime.InteropServices;

namespace Hegel.Native;

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct HegelString
{
    public byte* Data;
    public nuint Len;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct HegelBytes
{
    public byte* Data;
    public nuint Len;
}

[StructLayout(LayoutKind.Sequential)]
internal struct HegelDate
{
    public int Year;
    public byte Month;
    public byte Day;
}

[StructLayout(LayoutKind.Sequential)]
internal struct HegelTime
{
    public byte Hour;
    public byte Minute;
    public byte Second;
    public uint Nanosecond;
}

[StructLayout(LayoutKind.Sequential)]
internal struct HegelDateTime
{
    public HegelDate Date;
    public HegelTime Time;
}

/// <summary>The raw C ABI of libhegel, one source-generated P/Invoke per exported function.</summary>
internal static unsafe partial class NativeMethods
{
    internal const string Library = "libhegel";

    [LibraryImport(Library)] internal static partial nint hegel_context_new();
    [LibraryImport(Library)] internal static partial int hegel_context_free(nint ctx);
    [LibraryImport(Library)] internal static partial HegelString hegel_context_last_error(nint ctx);

    [LibraryImport(Library)] internal static partial int hegel_settings_new(nint ctx, nint* outSettings);
    [LibraryImport(Library)] internal static partial int hegel_settings_new_for_profile(nint ctx, HegelString name, nint* outSettings);
    [LibraryImport(Library)] internal static partial int hegel_settings_free(nint ctx, nint s);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_backend(nint ctx, nint s, uint backend);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_test_cases(nint ctx, nint s, ulong n);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_verbosity(nint ctx, nint s, uint v);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_nondeterminism_strictness(nint ctx, nint s, uint strictness);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_seed(nint ctx, nint s, ulong seed, [MarshalAs(UnmanagedType.U1)] bool hasSeed);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_derandomize(nint ctx, nint s, [MarshalAs(UnmanagedType.U1)] bool yes);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_report_multiple_failures(nint ctx, nint s, [MarshalAs(UnmanagedType.U1)] bool yes);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_stop_on_first_failure(nint ctx, nint s, [MarshalAs(UnmanagedType.U1)] bool yes);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_show_statistics(nint ctx, nint s, [MarshalAs(UnmanagedType.U1)] bool yes);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_unbounded_choices(nint ctx, nint s, [MarshalAs(UnmanagedType.U1)] bool yes);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_database(nint ctx, nint s, HegelString* database);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_database_key(nint ctx, nint s, HegelString* key);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_test_location(nint ctx, nint s, HegelString file, uint beginLine, HegelString className, HegelString function);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_phases(nint ctx, nint s, uint phases);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_suppress_health_check(nint ctx, nint s, uint checks);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_print_blob(nint ctx, nint s, [MarshalAs(UnmanagedType.U1)] bool yes);
    [LibraryImport(Library)] internal static partial int hegel_settings_set_from_string(nint ctx, nint s, HegelString key, HegelString value);
    [LibraryImport(Library)] internal static partial int hegel_settings_register_profile(nint ctx, HegelString name, nint settings);
    [LibraryImport(Library)] internal static partial int hegel_set_unicode_version(nint ctx, HegelString version);
    [LibraryImport(Library)] internal static partial int hegel_set_default_profile(nint ctx, HegelString name);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_test_cases(nint ctx, nint s, ulong* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_verbosity(nint ctx, nint s, int* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_seed(nint ctx, nint s, ulong* outSeed, byte* outHasSeed);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_derandomize(nint ctx, nint s, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_database(nint ctx, nint s, HegelString** o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_phases(nint ctx, nint s, uint* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_suppress_health_check(nint ctx, nint s, uint* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_report_multiple_failures(nint ctx, nint s, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_stop_on_first_failure(nint ctx, nint s, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_show_statistics(nint ctx, nint s, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_unbounded_choices(nint ctx, nint s, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_print_blob(nint ctx, nint s, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_backend(nint ctx, nint s, int* o);
    [LibraryImport(Library)] internal static partial int hegel_settings_get_nondeterminism_strictness(nint ctx, nint s, int* o);

    [LibraryImport(Library)] internal static partial int hegel_run_start(nint ctx, nint settings, delegate* unmanaged<nint, byte*, nuint, void> callback, nint userData, nint* outRun);
    [LibraryImport(Library)] internal static partial int hegel_run_start_blob(nint ctx, nint settings, HegelString blob, delegate* unmanaged<nint, byte*, nuint, void> callback, nint userData, nint* outRun);
    [LibraryImport(Library)] internal static partial int hegel_next_test_case(nint ctx, nint run, nint* outTestCase);
    [LibraryImport(Library)] internal static partial int hegel_run_result(nint ctx, nint run, nint* outResult);
    [LibraryImport(Library)] internal static partial int hegel_run_result_free(nint ctx, nint r);
    [LibraryImport(Library)] internal static partial int hegel_run_free(nint ctx, nint run);
    [LibraryImport(Library)] internal static partial int hegel_test_case_free(nint ctx, nint tc);
    [LibraryImport(Library)] internal static partial int hegel_test_case_should_capture(nint ctx, nint tc, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_test_case_clone(nint ctx, nint tc, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_test_case_block(nint ctx, nint tc, ulong indent, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_test_case_set_worker(nint ctx, nint tc, long workerIndex);
    [LibraryImport(Library)] internal static partial int hegel_start_span(nint ctx, nint tc, ulong label);
    [LibraryImport(Library)] internal static partial int hegel_stop_span(nint ctx, nint tc, [MarshalAs(UnmanagedType.U1)] bool discard);
    [LibraryImport(Library)] internal static partial int hegel_label_from_name(nint ctx, HegelString name, ulong* o);
    [LibraryImport(Library)] internal static partial int hegel_label_combine(nint ctx, ulong* labels, nuint len, ulong* o);

    [LibraryImport(Library)] internal static partial int hegel_new_collection(nint ctx, nint tc, ulong minSize, ulong maxSize, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_collection_more(nint ctx, nint tc, nint collection, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_collection_reject(nint ctx, nint tc, nint collection, HegelString* why);
    [LibraryImport(Library)] internal static partial int hegel_collection_free(nint ctx, nint collection);
    [LibraryImport(Library)] internal static partial int hegel_new_repeat(nint ctx, nint tc, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_repeat_more(nint ctx, nint tc, nint repeat, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_repeat_failed(nint ctx, nint tc, nint repeat);
    [LibraryImport(Library)] internal static partial int hegel_repeat_free(nint ctx, nint repeat);
    [LibraryImport(Library)] internal static partial int hegel_new_recursion(nint ctx, nint tc, ulong maxDepth, ulong maxLeaves, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_recursion_branch(nint ctx, nint tc, nint recursion, ulong depth, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_recursion_leaf(nint ctx, nint tc, nint recursion);
    [LibraryImport(Library)] internal static partial int hegel_recursion_retry(nint ctx, nint tc, nint recursion);
    [LibraryImport(Library)] internal static partial int hegel_recursion_finish(nint ctx, nint tc, nint recursion);
    [LibraryImport(Library)] internal static partial int hegel_recursion_free(nint ctx, nint recursion);
    [LibraryImport(Library)] internal static partial int hegel_new_pool(nint ctx, nint tc, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_pool_add(nint ctx, nint tc, nint pool, long* o);
    [LibraryImport(Library)] internal static partial int hegel_pool_generate(nint ctx, nint tc, nint pool, [MarshalAs(UnmanagedType.U1)] bool consume, long* o);
    [LibraryImport(Library)] internal static partial int hegel_pool_restore(nint ctx, nint tc, nint pool, long variableId);
    [LibraryImport(Library)] internal static partial int hegel_pool_free(nint ctx, nint pool);
    [LibraryImport(Library)] internal static partial int hegel_new_state_machine(nint ctx, nint tc, HegelString* ruleNames, long* ruleGroups, double* ruleWeights, nuint numRules, HegelString* invariantNames, byte* invariantAlwaysCheck, nuint numInvariants, long minConcurrency, long maxConcurrency, long stepCount, nint* outStateMachine, long* outConcurrency);
    [LibraryImport(Library)] internal static partial int hegel_state_machine_next_group(nint ctx, nint tc, nint sm, long* o);
    [LibraryImport(Library)] internal static partial int hegel_state_machine_next_rule(nint ctx, nint tc, nint sm, long workerIndex, long* o);
    [LibraryImport(Library)] internal static partial int hegel_state_machine_rule_rejected(nint ctx, nint tc, nint sm, long workerIndex);
    [LibraryImport(Library)] internal static partial int hegel_state_machine_should_check_invariant(nint ctx, nint tc, nint sm, long invariantIndex, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_state_machine_free(nint ctx, nint sm);

    [LibraryImport(Library)] internal static partial int hegel_generate_boolean(nint ctx, nint tc, double p, [MarshalAs(UnmanagedType.U1)] bool forced, [MarshalAs(UnmanagedType.U1)] bool hasForced, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_generate_integer(nint ctx, nint tc, long min, long max, long* o);
    [LibraryImport(Library)] internal static partial int hegel_generate_integer_big(nint ctx, nint tc, byte* min, nuint minLen, byte* max, nuint maxLen, byte* o, nuint cap, nuint* outLen);
    [LibraryImport(Library)] internal static partial int hegel_generate_float(nint ctx, nint tc, uint width, double min, double max, [MarshalAs(UnmanagedType.U1)] bool allowNan, [MarshalAs(UnmanagedType.U1)] bool allowInfinity, [MarshalAs(UnmanagedType.U1)] bool excludeMin, [MarshalAs(UnmanagedType.U1)] bool excludeMax, double smallestNonzeroMagnitude, double* o);
    [LibraryImport(Library)] internal static partial int hegel_generate_bytes(nint ctx, nint tc, ulong minSize, ulong maxSize, HegelBytes* o);
    [LibraryImport(Library)] internal static partial int hegel_generate_bytes_result_free(nint ctx, HegelBytes* r);
    [LibraryImport(Library)] internal static partial int hegel_string_generator_text(nint ctx, ulong minSize, ulong maxSize, HegelString* codec, uint minCodepoint, uint maxCodepoint, HegelString* categories, nuint categoriesLen, HegelString* excludeCategories, nuint excludeCategoriesLen, HegelString includeCharacters, HegelString excludeCharacters, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_string_generator_regex(nint ctx, HegelString pattern, [MarshalAs(UnmanagedType.U1)] bool fullmatch, nint alphabet, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_string_generator_email(nint ctx, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_string_generator_url(nint ctx, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_string_generator_domain(nint ctx, ulong maxLength, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_string_generator_free(nint ctx, nint generator);
    [LibraryImport(Library)] internal static partial int hegel_generate_string(nint ctx, nint tc, nint generator, HegelString* o);
    [LibraryImport(Library)] internal static partial int hegel_owned_string_free(nint ctx, HegelString* r);
    [LibraryImport(Library)] internal static partial int hegel_grammar_builder_new(nint ctx, HegelString text, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_grammar_builder_file(nint ctx, nint builder, nuint index, HegelString* outPath);
    [LibraryImport(Library)] internal static partial int hegel_grammar_builder_provide(nint ctx, nint builder, nuint index, HegelString* text);
    [LibraryImport(Library)] internal static partial int hegel_grammar_builder_free(nint ctx, nint builder);
    [LibraryImport(Library)] internal static partial int hegel_grammar_new(nint ctx, nint builder, HegelString* start, ulong maxSize, uint maxDepth, nint alphabet, HegelString* explicitNames, nuint explicitLen, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_grammar_free(nint ctx, nint grammar);
    [LibraryImport(Library)] internal static partial int hegel_grammar_draw_new(nint ctx, nint grammar, ulong maxSize, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_grammar_draw_next(nint ctx, nint tc, nint draw, nuint* outTerminal);
    [LibraryImport(Library)] internal static partial int hegel_grammar_draw_provide(nint ctx, nint draw, HegelString value);
    [LibraryImport(Library)] internal static partial int hegel_grammar_draw_result(nint ctx, nint draw, HegelString* o);
    [LibraryImport(Library)] internal static partial int hegel_grammar_draw_free(nint ctx, nint draw);
    [LibraryImport(Library)] internal static partial int hegel_generate_date(nint ctx, nint tc, HegelDate min, HegelDate max, HegelDate* o);
    [LibraryImport(Library)] internal static partial int hegel_generate_time(nint ctx, nint tc, HegelTime min, HegelTime max, HegelTime* o);
    [LibraryImport(Library)] internal static partial int hegel_generate_datetime(nint ctx, nint tc, HegelDateTime min, HegelDateTime max, HegelDateTime* o);
    [LibraryImport(Library)] internal static partial int hegel_generate_uuid(nint ctx, nint tc, byte version, [MarshalAs(UnmanagedType.U1)] bool hasVersion, byte* outBytes);
    [LibraryImport(Library)] internal static partial int hegel_generate_ipv4(nint ctx, nint tc, byte* outBytes);
    [LibraryImport(Library)] internal static partial int hegel_generate_ipv6(nint ctx, nint tc, byte* outBytes);
    [LibraryImport(Library)] internal static partial int hegel_target(nint ctx, nint tc, double value, HegelString label);
    [LibraryImport(Library)] internal static partial int hegel_event(nint ctx, nint tc, HegelString label);
    [LibraryImport(Library)] internal static partial int hegel_event_value(nint ctx, nint tc, double value, HegelString label);

    [LibraryImport(Library)] internal static partial int hegel_printer_options_new(nint ctx, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_printer_options_free(nint ctx, nint options);
    [LibraryImport(Library)] internal static partial int hegel_printer_options_set_max_width(nint ctx, nint options, ulong maxWidth);
    [LibraryImport(Library)] internal static partial int hegel_printer_new(nint ctx, nint options, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_printer_free(nint ctx, nint printer);
    [LibraryImport(Library)] internal static partial int hegel_printer_if_break(nint ctx, nint printer, HegelString text);
    [LibraryImport(Library)] internal static partial int hegel_printer_text(nint ctx, nint printer, HegelString text);
    [LibraryImport(Library)] internal static partial int hegel_printer_breakable(nint ctx, nint printer, HegelString sep);
    [LibraryImport(Library)] internal static partial int hegel_printer_comment(nint ctx, nint printer, HegelString text);
    [LibraryImport(Library)] internal static partial int hegel_printer_hard_break(nint ctx, nint printer);
    [LibraryImport(Library)] internal static partial int hegel_printer_begin_group(nint ctx, nint printer, ulong indent, HegelString open);
    [LibraryImport(Library)] internal static partial int hegel_printer_end_group(nint ctx, nint printer, HegelString close);
    [LibraryImport(Library)] internal static partial int hegel_printer_shift_indent(nint ctx, nint printer, long delta);
    [LibraryImport(Library)] internal static partial int hegel_reflow_options_new(nint ctx, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_reflow_options_free(nint ctx, nint options);
    [LibraryImport(Library)] internal static partial int hegel_printer_reflow(nint ctx, nint printer, nint options, HegelString text);
    [LibraryImport(Library)] internal static partial int hegel_printer_deferred(nint ctx, nint printer, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_printer_begin_speculative(nint ctx, nint printer);
    [LibraryImport(Library)] internal static partial int hegel_printer_commit_speculative(nint ctx, nint printer);
    [LibraryImport(Library)] internal static partial int hegel_printer_abort_speculative(nint ctx, nint printer);
    [LibraryImport(Library)] internal static partial int hegel_printer_resolve(nint ctx, nint printer);
    [LibraryImport(Library)] internal static partial int hegel_printer_is_live(nint ctx, nint printer, byte* o);
    [LibraryImport(Library)] internal static partial int hegel_printer_value(nint ctx, nint printer, HegelString* o);
    [LibraryImport(Library)] internal static partial int hegel_printer_value_result_free(nint ctx, HegelString* r);
    [LibraryImport(Library)] internal static partial int hegel_test_case_printer(nint ctx, nint tc, nint options, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_note(nint ctx, nint tc, HegelString text);
    [LibraryImport(Library)] internal static partial int hegel_mark_complete(nint ctx, nint tc, uint status, HegelString* origin);

    [LibraryImport(Library)] internal static partial int hegel_run_result_status(nint ctx, nint r, int* o);
    [LibraryImport(Library)] internal static partial int hegel_run_result_error(nint ctx, nint r, HegelString** o);
    [LibraryImport(Library)] internal static partial int hegel_run_result_error_kind(nint ctx, nint r, int* outKind, uint* outHealthCheck);
    [LibraryImport(Library)] internal static partial int hegel_run_result_failure_count(nint ctx, nint r, nuint* o);
    [LibraryImport(Library)] internal static partial int hegel_run_result_failure(nint ctx, nint r, nuint index, nint* o);
    [LibraryImport(Library)] internal static partial int hegel_failure_free(nint ctx, nint f);
    [LibraryImport(Library)] internal static partial int hegel_failure_origin(nint ctx, nint f, HegelString* o);
    [LibraryImport(Library)] internal static partial int hegel_failure_reproduction_blob(nint ctx, nint f, HegelString** o);
    [LibraryImport(Library)] internal static partial int hegel_failure_caveat(nint ctx, nint f, HegelString** o);
    [LibraryImport(Library)] internal static partial int hegel_version(nint ctx, HegelString* o);
}
