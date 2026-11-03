using Hegel.Tests.Support;
using Xunit;

namespace Hegel.Tests;

public class NullArgumentTests
{
    public static TheoryData<string, Action> Calls => new()
    {
        { "Lists", () => Generators.Lists<int>(null!) },
        { "Arrays", () => Generators.Arrays<int>(null!) },
        { "Sets", () => Generators.Sets<int>(null!) },
        { "Maps keys", () => Generators.Maps<int, int>(null!, Generators.Integers()) },
        { "Maps values", () => Generators.Maps<int, int>(Generators.Integers(), null!) },
        { "OneOf", () => Generators.OneOf<int>(null!) },
        { "Optional", () => Generators.Optional<string>(null!) },
        { "Nullable", () => Generators.Nullable<int>(null!) },
        { "Permutations", () => Generators.Permutations<int>(null!) },
        { "Subsequences", () => Generators.Subsequences<int>(null!) },
        { "Samples", () => Generators.Samples<int>(null!) },
        { "Composite", () => Generators.Composite<int>((Func<TestCase, int>)null!) },
        { "Composite printing", () => Generators.Composite<int>((Func<TestCase, Printer, int>)null!) },
        { "Recursive base", () => Generators.Recursive<int>(null!, g => g) },
        { "Recursive extend", () => Generators.Recursive(Generators.Integers(), null!) },
        { "Tuples", () => Generators.Tuples(Generators.Integers(), (Generator<int>)null!) },
        { "FromRegex", () => Generators.FromRegex(null!) },
        { "FromLark", () => Generators.FromLark(null!) },
        { "Map", () => Generators.Integers().Map<int>(null!) },
        { "Filter", () => Generators.Integers().Filter(null!) },
        { "FlatMap", () => Generators.Integers().FlatMap<int>(null!) },
        { "PrintWith", () => Generators.Integers().PrintWith(null!) },
        { "Records.With name", () => Generators.Records<Box<int>>().With<int>(null!, Generators.Integers()) },
        { "Records.With generator", () => Generators.Records<Box<int>>().With<int>("Value", null!) },
        { "Example.Of", () => Example.Of(null!) },
        { "Example.With", () => Example.Of().With(null!, 1) },
        { "FromProfile", () => Settings.FromProfile(null!) },
        { "RegisterProfile name", () => Settings.RegisterProfile(null!, new Settings()) },
        { "RegisterProfile settings", () => Settings.RegisterProfile("p", null!) },
        { "SetDefaultProfile", () => Settings.SetDefaultProfile(null!) },
        { "Set key", () => new Settings().Set(null!, "1") },
        { "Set value", () => new Settings().Set("test_cases", null!) },
    };

    [Theory]
    [MemberData(nameof(Calls))]
    public void PublicEntryPointsRejectNull(string name, Action call)
    {
        Assert.NotNull(name);
        Assert.Throws<ArgumentNullException>(call);
    }

    [Fact]
    public void TestCaseEntryPointsRejectNull()
    {
        Property.Test(tc =>
        {
            Assert.Throws<ArgumentNullException>(() => tc.Draw<int>(null!));
            Assert.Throws<ArgumentNullException>(() => tc.DrawSilent<int>(null!));
            Assert.Throws<ArgumentNullException>(() => tc.DrawAndPrint(Generators.Integers(), null!));
            Assert.Throws<ArgumentNullException>(() => tc.Note(null!));
            Assert.Throws<ArgumentNullException>(() => tc.Repeat(null!));
        }, Engines.Quiet with { TestCases = 1 });
    }
}
