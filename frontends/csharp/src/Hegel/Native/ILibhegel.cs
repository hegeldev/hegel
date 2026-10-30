namespace Hegel.Native;

/// <summary>
/// The engine as the frontend drives it: every C ABI function the frontend uses, with managed
/// arguments and raw handles. Return codes are the ABI's; the <c>Free</c> functions and those
/// that cannot fail for a valid handle return nothing. The real binding is <see cref="Libhegel"/>;
/// tests substitute a fake for the error paths the real engine never takes.
/// </summary>
internal interface ILibhegel
{
    string LastErrorMessage();
    void ReleaseThreadContext();
    string Version();

    int SettingsNew(out nint settings);
    int SettingsNewForProfile(string name, out nint settings);
    int SettingsRegisterProfile(string name, nint settings);
    int SettingsSetFromString(nint settings, string key, string value);
    int SetDefaultProfile(string name);
    void SettingsFree(nint settings);
    ulong SettingsGetTestCases(nint settings);
    bool SettingsGetPrintBlob(nint settings);
    int SettingsGetNondeterminismStrictness(nint settings);
    int SettingsGetVerbosity(nint settings);
    int SettingsGetPhases(nint settings);
    bool SettingsGetReportMultipleFailures(nint settings);
    (ulong Seed, bool HasSeed) SettingsGetSeed(nint settings);
    bool SettingsGetDerandomize(nint settings);
    string? SettingsGetDatabase(nint settings);
    int SettingsGetSuppressHealthCheck(nint settings);
    bool SettingsGetStopOnFirstFailure(nint settings);
    bool SettingsGetShowStatistics(nint settings);
    bool SettingsGetUnboundedChoices(nint settings);
    int SettingsGetBackend(nint settings);
    void SettingsBackend(nint settings, int backend);
    void SettingsTestCases(nint settings, ulong n);
    void SettingsVerbosity(nint settings, int verbosity);
    void SettingsSeed(nint settings, ulong seed, bool hasSeed);
    void SettingsDerandomize(nint settings, bool yes);
    void SettingsReportMultipleFailures(nint settings, bool yes);
    void SettingsStopOnFirstFailure(nint settings, bool yes);
    void SettingsShowStatistics(nint settings, bool yes);
    void SettingsUnboundedChoices(nint settings, bool yes);
    void SettingsTestLocation(nint settings, string file, int beginLine, string className, string function);
    void SettingsDatabase(nint settings, string? path);
    void SettingsDatabaseKey(nint settings, string key);
    void SettingsPhases(nint settings, int mask);
    void SettingsSuppressHealthCheck(nint settings, int mask);
    void SettingsPrintBlob(nint settings, bool yes);
    void SettingsNondeterminismStrictness(nint settings, int strictness);

    nint RunStart(nint settings, Action<string> output);
    nint RunStartBlob(nint settings, string blob, Action<string> output);
    nint NextTestCase(nint run);
    nint RunResult(nint run);
    void RunResultFree(nint result);
    void RunFree(nint run);
    void TestCaseFree(nint testCase);
    bool TestCaseShouldCapture(nint testCase);
    int TestCaseClone(nint testCase, out nint clone);
    void TestCaseSetWorker(nint testCase, long workerIndex);
    int TestCaseBlock(nint testCase, ulong indent, out nint block);
    int Note(nint testCase, string text);
    int TestCasePrinter(nint testCase, out nint printer);

    nint PrinterNew(nint options);
    nint PrinterOptionsNew();
    int PrinterOptionsSetMaxWidth(nint options, ulong maxWidth);
    void PrinterOptionsFree(nint options);
    void PrinterFree(nint printer);
    int PrinterText(nint printer, string text);
    int PrinterBreakable(nint printer, string separator);
    int PrinterHardBreak(nint printer);
    int PrinterIfBreak(nint printer, string text);
    int PrinterComment(nint printer, string text);
    int PrinterShiftIndent(nint printer, long delta);
    int PrinterBeginGroup(nint printer, ulong indent, string open);
    int PrinterEndGroup(nint printer, string close);
    int PrinterReflow(nint printer, string text);
    int PrinterBeginSpeculative(nint printer);
    int PrinterCommitSpeculative(nint printer);
    int PrinterAbortSpeculative(nint printer);
    int PrinterDeferred(nint printer, out nint child);
    int PrinterIsLive(nint printer, out bool live);
    int PrinterResolve(nint printer);
    int PrinterValue(nint printer, out string value);

    int GenerateBoolean(nint testCase, double p, out bool value);
    int GenerateInteger(nint testCase, long min, long max, out long value);
    int GenerateIntegerBig(nint testCase, byte[] minLittleEndian, byte[] maxLittleEndian, out byte[] valueLittleEndian);
    int GenerateFloat(nint testCase, int width, double min, double max, bool allowNan, bool allowInfinity, bool excludeMin, bool excludeMax, double smallestNonzeroMagnitude, out double value);
    int GenerateBytes(nint testCase, ulong minSize, ulong maxSize, out byte[] value);
    int GenerateString(nint testCase, nint generator, out string value);
    int GenerateDate(nint testCase, HegelDate min, HegelDate max, out HegelDate value);
    int GenerateTime(nint testCase, HegelTime min, HegelTime max, out HegelTime value);
    int GenerateDatetime(nint testCase, HegelDateTime min, HegelDateTime max, out HegelDateTime value);
    int GenerateUuid(nint testCase, int version, bool hasVersion, byte[] out16);
    int GenerateIpv4(nint testCase, byte[] out4);
    int GenerateIpv6(nint testCase, byte[] out16);

    int StringGeneratorText(ulong minSize, ulong maxSize, string? codec, uint minCodepoint, uint maxCodepoint, IReadOnlyList<string>? categories, IReadOnlyList<string>? excludeCategories, string? includeCharacters, string? excludeCharacters, out nint generator);
    int StringGeneratorRegex(string pattern, bool fullmatch, nint alphabet, out nint generator);
    int StringGeneratorEmail(out nint generator);
    int StringGeneratorUrl(out nint generator);
    int StringGeneratorDomain(ulong maxLength, out nint generator);
    void StringGeneratorFree(nint generator);

    int GrammarBuilderNew(string text, out nint builder);
    int GrammarBuilderFile(nint builder, ulong index, out string path);
    int GrammarBuilderProvide(nint builder, ulong index, string? text);
    void GrammarBuilderFree(nint builder);
    int GrammarNew(nint builder, string? start, ulong maxSize, uint maxDepth, nint alphabet, IReadOnlyList<string> explicitNames, out nint grammar);
    void GrammarFree(nint grammar);
    nint GrammarDrawNew(nint grammar, ulong maxSize);
    int GrammarDrawNext(nint testCase, nint draw, out ulong terminal);
    int GrammarDrawProvide(nint draw, string value);
    int GrammarDrawResult(nint draw, out string value);
    void GrammarDrawFree(nint draw);

    int StartSpan(nint testCase, ulong label);
    int StopSpan(nint testCase, bool discard);
    int NewCollection(nint testCase, ulong minSize, ulong maxSize, out nint collection);
    int CollectionMore(nint testCase, nint collection, out bool more);
    int CollectionReject(nint testCase, nint collection, string? why);
    void CollectionFree(nint collection);
    int NewRepeat(nint testCase, out nint repeat);
    int RepeatMore(nint testCase, nint repeat, out bool more);
    int RepeatFailed(nint testCase, nint repeat);
    void RepeatFree(nint repeat);
    int NewPool(nint testCase, out nint pool);
    int PoolAdd(nint testCase, nint pool, out long variableId);
    int PoolGenerate(nint testCase, nint pool, bool consume, out long variableId);
    int PoolRestore(nint testCase, nint pool, long variableId);
    void PoolFree(nint pool);
    int NewStateMachine(nint testCase, IReadOnlyList<string> ruleNames, long[] ruleGroups, double[]? ruleWeights, IReadOnlyList<string> invariantNames, bool[] invariantAlwaysCheck, long minConcurrency, long maxConcurrency, long stepCount, out nint stateMachine, out long concurrency);
    int StateMachineNextGroup(nint testCase, nint stateMachine, out long group);
    int StateMachineNextRule(nint testCase, nint stateMachine, long workerIndex, out long rule);
    int StateMachineRuleRejected(nint testCase, nint stateMachine, long workerIndex);
    int StateMachineShouldCheckInvariant(nint testCase, nint stateMachine, long invariantIndex, out bool check);
    void StateMachineFree(nint stateMachine);
    int NewRecursion(nint testCase, ulong maxDepth, ulong maxLeaves, out nint recursion);
    int RecursionBranch(nint testCase, nint recursion, ulong depth, out bool branch);
    int RecursionLeaf(nint testCase, nint recursion);
    int RecursionRetry(nint testCase, nint recursion);
    int RecursionFinish(nint testCase, nint recursion);
    void RecursionFree(nint recursion);

    int Target(nint testCase, double value, string label);
    int Event(nint testCase, string label);
    int EventValue(nint testCase, double value, string label);
    int MarkComplete(nint testCase, int status, string? origin);

    int RunResultStatus(nint result);
    string RunResultError(nint result);
    int RunResultErrorKind(nint result, out int healthCheck);
    ulong RunResultFailureCount(nint result);
    string? FailureBlob(nint result, ulong index);
    string FailureOrigin(nint result, ulong index);
    string? FailureCaveat(nint result, ulong index);

    ulong LabelFromName(string name);
    ulong LabelCombine(ulong[] labels);
}
