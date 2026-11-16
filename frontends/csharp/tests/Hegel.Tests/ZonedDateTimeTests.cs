using Hegel.Native;
using Hegel.Temporal;
using Hegel.Tests.Support;
using Xunit;
using static Hegel.Tests.Support.TestZones;

namespace Hegel.Tests;

public class ZonedDateTimeTests
{
    private static readonly TimeZoneInfo Home = Springfield();
    private static readonly ZoneSource HomeOnly = ZoneSource.FromTimeZones(Home);

    private static ZonedDateTimeGenerator AtHome() => Generators.Temporal.ZonedDateTimes().Source(HomeOnly);

    private static bool Consistent(ZonedDateTime value) =>
        value.LocalDateTime.Kind == DateTimeKind.Unspecified
        && value.Instant.Offset == TimeSpan.Zero
        && value.Zone.GetUtcOffset(value.Instant) == value.Offset
        && (value.Kind == WallTimeKind.Skipped || value.Instant.UtcDateTime + value.Offset == value.LocalDateTime);

    [Fact]
    public void ZonedDateTimesAreInTheHostsZonesByDefault()
    {
        var host = ZoneSource.Host.Names.ToHashSet();
        var values = Gen.Sample(Generators.Temporal.ZonedDateTimes(), 200);
        Assert.All(values, value => Assert.Contains(value.Zone.Id, host));
        Assert.All(values, value => Assert.True(Consistent(value), value.ToString()));
        Assert.DoesNotContain(values, value => value.Kind == WallTimeKind.Skipped);
        var minimal = Gen.MinimalOutput(Generators.Temporal.ZonedDateTimes());
        Assert.Equal(
            """
            var x_1 = new ZonedDateTime(DateTime.Parse("2000-01-01T00:00:00"),
                              TimeSpan.Parse("00:00:00"),
                              TimeZoneInfo.FindSystemTimeZoneById("UTC"),
                              DateTimeOffset.Parse("2000-01-01T00:00:00+00:00"),
                              WallTimeKind.Unique);
            """.ReplaceLineEndings("\n"),
            minimal);
    }

    [Fact]
    public void SkippedTimesAreDrawnOnlyWhenAskedFor()
    {
        bool Skipped(ZonedDateTime value) => value.LocalDateTime >= GapStart && value.LocalDateTime < GapEnd;
        var around = AtHome().Min(GapStart.AddMinutes(-30)).Max(GapEnd.AddMinutes(30));
        Gen.All(around, value => Consistent(value) && value.Kind == WallTimeKind.Unique && !Skipped(value), 200);
        Gen.All(around.AllowImaginary(true), value => Consistent(value) && Skipped(value) == (value.Kind == WallTimeKind.Skipped), 200);
        Gen.All(around.Max(GapStart.AddTicks(-1)), value => value.Offset == Standard, 20);
        Gen.All(around.Min(GapEnd), value => value.Offset == Daylight, 20);

        var within = AtHome().Min(GapStart).Max(GapEnd.AddTicks(-1));
        var skipped = Gen.Sample(within.AllowImaginary(true), 50);
        Assert.All(skipped, value =>
        {
            Assert.True(Consistent(value) && Skipped(value), value.ToString());
            Assert.Equal(WallTimeKind.Skipped, value.Kind);
            Assert.Equal(Daylight, value.Offset);
            Assert.Equal(value.LocalDateTime - Standard, value.Instant.UtcDateTime);
            Assert.EndsWith("-04:00[Test/Springfield][gap]", value.ToString());
        });
        foreach (var real in new[] { within, within.AllowImaginary(true).AllowImaginary(false) })
        {
            Assert.StartsWith(
                "hegel_generate_zoned_datetime: a zoned datetime draw has no value at precision nanoseconds in [2024-03-10T02:00:00, 2024-03-10T02:59:59.999999999] in any allowed zone",
                Assert.Throws<HegelUsageException>(() => Gen.Sample(real, 1)).Message);
        }
    }

    [Fact]
    public void RepeatedTimesAreDrawnAsTheAmbiguousPolicySays()
    {
        bool Repeated(ZonedDateTime value) => value.LocalDateTime >= FoldStart && value.LocalDateTime < FoldEnd;
        var around = AtHome().Min(FoldStart.AddMinutes(-30)).Max(FoldEnd.AddMinutes(30));
        Gen.All(around, value => Consistent(value) && Repeated(value) == (value.Kind is WallTimeKind.Earlier or WallTimeKind.Later), 200);
        Gen.All(around.Ambiguous(Ambiguous.Earlier), value => Consistent(value) && Repeated(value) == (value.Kind == WallTimeKind.Earlier), 200);
        Gen.All(around.Ambiguous(Ambiguous.Later), value => Consistent(value) && Repeated(value) == (value.Kind == WallTimeKind.Later), 200);

        var within = AtHome().Min(FoldStart).Max(FoldEnd.AddTicks(-1));
        var both = Gen.Sample(within, 100);
        Assert.All(both, value => Assert.True(Consistent(value) && Repeated(value), value.ToString()));
        var earlier = both.Where(value => value.Kind == WallTimeKind.Earlier).ToList();
        var later = both.Where(value => value.Kind == WallTimeKind.Later).ToList();
        Assert.NotEmpty(earlier);
        Assert.NotEmpty(later);
        Assert.Equal(both.Count, earlier.Count + later.Count);
        Assert.All(earlier, value => Assert.EndsWith("-04:00[Test/Springfield][fold=0]", value.ToString()));
        Assert.All(later, value => Assert.EndsWith("-05:00[Test/Springfield][fold=1]", value.ToString()));
        Gen.All(within.Ambiguous(Ambiguous.Earlier), value => value.Kind == WallTimeKind.Earlier && value.Offset == Daylight, 50);
        Gen.All(within.Ambiguous(Ambiguous.Later), value => value.Kind == WallTimeKind.Later && value.Offset == Standard, 50);
        Assert.Contains(Gen.Sample(within.Ambiguous(Ambiguous.Later).Ambiguous(Ambiguous.Both), 100), value => value.Kind == WallTimeKind.Earlier);
        Assert.EndsWith("WallTimeKind.Earlier);", Gen.MinimalOutput(within));
        Assert.Equal("hegel_zoned_spec_set_ambiguous: unknown policy 9", Assert.Throws<ArgumentException>(() => around.Ambiguous((Ambiguous)9)).Message);
    }

    [Fact]
    public void WallClockBoundsHoldInWhicheverZoneIsDrawn()
    {
        var lo = new DateTime(2020, 6, 1, 9, 0, 0);
        var hi = new DateTime(2020, 6, 1, 17, 0, 0);
        var source = ZoneSource.FromTimeZones(Home, Fixed("Test/East", TimeSpan.FromHours(9)));
        var values = Gen.Sample(Generators.Temporal.ZonedDateTimes().Source(source).Min(lo).Max(hi), 100);
        Assert.All(values, value => Assert.InRange(value.LocalDateTime, lo, hi));
        Assert.Contains(values, value => value.Zone.Id == "Test/East");
        Assert.Contains(values, value => value.Zone.Id == "Test/Springfield");
        Gen.All(AtHome().Min(lo), value => value.LocalDateTime >= lo, 50);
        Gen.All(AtHome().Max(hi), value => value.LocalDateTime <= hi, 50);
        var odd = lo.AddTicks(1234567);
        Assert.Equal(new[] { odd, odd.AddTicks(1) }, Gen.Sample(AtHome().Min(odd).Max(odd.AddTicks(1)), 200).Select(value => value.LocalDateTime).Distinct().Order());
        Assert.StartsWith("var x_1 = new ZonedDateTime(DateTime.Parse(\"2020-06-01T09:00:00\"),", Gen.MinimalOutput(AtHome().Min(lo).Max(hi)));
    }

    [Fact]
    public void InstantBoundsHoldInWhicheverZoneIsDrawn()
    {
        var lo = new DateTimeOffset(2020, 6, 1, 9, 0, 0, TimeSpan.Zero);
        var hi = new DateTimeOffset(2020, 6, 3, 17, 0, 0, TimeSpan.FromHours(2));
        var source = ZoneSource.FromTimeZones(Home, Fixed("Test/East", TimeSpan.FromHours(9)));
        var values = Gen.Sample(Generators.Temporal.ZonedDateTimes().Source(source).Min(lo).Max(hi), 100);
        Assert.All(values, value => Assert.InRange(value.Instant, lo, hi));
        Assert.All(values, value => Assert.True(Consistent(value), value.ToString()));
        Gen.All(AtHome().Min(lo), value => value.Instant >= lo, 50);
        Gen.All(AtHome().Max(hi), value => value.Instant <= hi, 50);
    }

    [Fact]
    public void ZonedDateTimesFollowTheirPrecision()
    {
        Gen.All(AtHome().Precision(Precision.Hours), value => value.LocalDateTime.TimeOfDay.Ticks % TimeSpan.TicksPerHour == 0, 50);
        Gen.All(AtHome().Precision(Precision.Seconds), value => value.LocalDateTime.TimeOfDay.Ticks % TimeSpan.TicksPerSecond == 0, 50);
        Gen.All(AtHome().Precision(Precision.Seconds).Precision(Precision.Ticks), Consistent, 50);
    }

    [Fact]
    public void ZonesFromAnyGeneratorAreLookedUpInTheSource()
    {
        var east = Fixed("Test/East", TimeSpan.FromHours(9));
        var source = ZoneSource.FromTimeZones(Home, east);
        var chosen = Generators.Temporal.ZonedDateTimes().Zones(Generators.SampledFrom([Home, east])).Source(source);
        var values = Gen.Sample(chosen, 100);
        Assert.All(values, value => Assert.True(Consistent(value) && (ReferenceEquals(value.Zone, Home) || ReferenceEquals(value.Zone, east)), value.ToString()));
        Assert.Contains(values, value => ReferenceEquals(value.Zone, east));
        Gen.All(Generators.Temporal.ZonedDateTimes().Zones(Generators.Just(TimeZoneInfo.Utc)), value => value.Zone.Equals(TimeZoneInfo.Utc) && value.Offset == TimeSpan.Zero, 20);
        Gen.All(Generators.Temporal.ZonedDateTimes().Zones(Generators.Temporal.Zones().Source(source).Names("Test/East")), value => ReferenceEquals(value.Zone, east), 20);
        Gen.All(Generators.Temporal.ZonedDateTimes().Zones(Generators.Temporal.Zones().Source(source)).Min(GapStart).Max(GapEnd), Consistent, 50);

        var tzif = ZoneSource.CreateBuilder().AddTzif("UTC", SpringfieldTzif()).Build();
        var unchecked_ = Gen.Sample(Generators.Temporal.ZonedDateTimes().Zones(Generators.Just(TimeZoneInfo.Utc)).Source(tzif), 100);
        Assert.All(unchecked_, value => Assert.True(value.Offset == Standard || value.Offset == Daylight));

        var stranger = Fixed("Nowhere/Land", TimeSpan.Zero);
        var unknown = Assert.Throws<HegelUsageException>(() => Gen.Sample(Generators.Temporal.ZonedDateTimes().Zones(Generators.Just(stranger)).Source(source), 1));
        Assert.StartsWith("hegel_generate_zoned_datetime_for_zone: unknown time zone \"Nowhere/Land\"", unknown.Message);
        Assert.Throws<HegelUsageException>(() => Gen.Sample(Generators.Temporal.ZonedDateTimes().Zones(Generators.Just(stranger)), 1));
    }

    [Fact]
    public void AWindowsZoneIdIsLookedUpByItsIanaName()
    {
        var table = new ZoneTable(false);
        Assert.Equal("UTC", table.NameOf(Fixed("UTC", TimeSpan.Zero)));
        Assert.Equal("Test/East", table.NameOf(Fixed("Test/East", TimeSpan.FromHours(9))));
        var windows = Fixed("Pacific Standard Time", TimeSpan.FromHours(-8));
        var expected = TimeZoneInfo.TryConvertWindowsIdToIanaId(windows.Id, out var iana) ? iana : windows.Id;
        Assert.Equal(expected, table.NameOf(windows));
        var byIana = ZoneSource.FromTimeZones(Fixed(expected, TimeSpan.FromHours(-8)));
        Gen.All(Generators.Temporal.ZonedDateTimes().Zones(Generators.Just(windows)).Source(byIana), value => ReferenceEquals(value.Zone, windows) && value.Offset == TimeSpan.FromHours(-8), 20);
        var byWindowsId = ZoneSource.FromTimeZones(windows);
        Assert.Equal(windows.Id, byWindowsId.Table.NameOf(windows));
        Gen.All(Generators.Temporal.ZonedDateTimes().Zones(Generators.Just(windows)).Source(byWindowsId), value => ReferenceEquals(value.Zone, windows) && value.Offset == TimeSpan.FromHours(-8), 20);
        Gen.All(Generators.Temporal.ZonedDateTimes().Source(byWindowsId), value => ReferenceEquals(value.Zone, windows), 20);
    }

    [Fact]
    public void AValueTheZoneObjectDisagreesWithIsRejected()
    {
        var source = ZoneSource.FromTimeZones(Fixed("Test/Fixed", TimeSpan.FromHours(2)));
        var impostor = Fixed("Test/Fixed", TimeSpan.FromHours(3));
        var generator = Generators.Temporal.ZonedDateTimes().Zones(Generators.Just(impostor)).Source(source);
        using var run = new RawRun();
        Assert.Throws<AssumeRejected>(() => run.Case(captured: false).DrawSilent(generator));
    }

    [Fact]
    public void AValueDotNetCannotHoldIsRejected()
    {
        var east = ZoneSource.FromTimeZones(Fixed("Test/East", TimeSpan.FromHours(9)));
        var west = ZoneSource.FromTimeZones(Fixed("Test/West", TimeSpan.FromHours(-9)));
        using (var run = new RawRun())
        {
            Assert.Throws<AssumeRejected>(() => run.Case(captured: false).DrawSilent(Generators.Temporal.ZonedDateTimes().Source(east).Max(DateTime.MinValue)));
        }
        using (var run = new RawRun())
        {
            Assert.Throws<AssumeRejected>(() => run.Case(captured: false).DrawSilent(Generators.Temporal.ZonedDateTimes().Source(west).Min(DateTime.MaxValue)));
        }
        using (var run = new RawRun())
        {
            Assert.Throws<AssumeRejected>(() => run.Case(captured: false).DrawSilent(Generators.Temporal.ZonedDateTimes().Source(east).Min(DateTimeOffset.MaxValue)));
        }
        using (var run = new RawRun())
        {
            Assert.Throws<AssumeRejected>(() => run.Case(captured: false).DrawSilent(Generators.Temporal.ZonedDateTimes().Source(west).Max(DateTimeOffset.MinValue)));
        }
        Gen.All(Generators.Temporal.ZonedDateTimes().Source(west).Max(DateTime.MinValue), value => value.Instant.UtcDateTime == DateTime.MinValue.AddHours(9), 3);
    }

    [Fact]
    public void ZonedDateTimesRejectBadArgumentsWhenBuiltInAnyOrder()
    {
        var lo = new DateTime(2020, 1, 1, 9, 10, 0);
        var hi = new DateTime(2020, 1, 1, 9, 50, 0);
        Assert.StartsWith(
            "hegel_zoned_spec_set_civil_bounds",
            RejectedWhenBuilt(Generators.Temporal.ZonedDateTimes(), g => g.Min(hi), g => g.Max(lo)));
        Assert.StartsWith(
            "hegel_zoned_spec_set_instant_bounds",
            RejectedWhenBuilt(Generators.Temporal.ZonedDateTimes(), g => g.Min(DateTimeOffset.UnixEpoch.AddDays(1)), g => g.Max(DateTimeOffset.UnixEpoch)));
        Assert.Equal(
            "hegel_zoned_spec_validate: a zoned datetime draw is bounded on the wall clock or on the instant, not both: civil bounds and instant bounds were both set",
            RejectedWhenBuilt(Generators.Temporal.ZonedDateTimes(), g => g.Min(lo), g => g.Max(DateTimeOffset.UnixEpoch)));
        Assert.StartsWith(
            "hegel_zoned_spec_validate",
            RejectedWhenBuilt(Generators.Temporal.ZonedDateTimes(), g => g.Min(lo), g => g.Max(hi), g => g.Precision(Precision.Hours)));
        Assert.StartsWith(
            "hegel_zoned_spec_validate",
            RejectedWhenBuilt(Generators.Temporal.ZonedDateTimes(), g => g.Min(DateTimeOffset.UnixEpoch.AddTicks(1)), g => g.Max(DateTimeOffset.UnixEpoch.AddTicks(2)), g => g.Precision(Precision.Seconds)));
        Assert.Equal("hegel_zoned_spec_set_precision: unknown precision 99", Assert.Throws<ArgumentException>(() => Generators.Temporal.ZonedDateTimes().Precision((Precision)99)).Message);
        Assert.StartsWith(
            "ZonedDateTimes: the generator given to Zones came from Generators.Temporal.Zones() and carries its own zone source",
            RejectedWhenBuilt(Generators.Temporal.ZonedDateTimes(), g => g.Zones(Generators.Temporal.Zones()), g => g.Source(HomeOnly)));
        foreach (var kind in new[] { DateTimeKind.Utc, DateTimeKind.Local })
        {
            var min = Assert.Throws<ArgumentException>(() => Generators.Temporal.ZonedDateTimes().Min(DateTime.SpecifyKind(lo, kind)));
            Assert.StartsWith("ZonedDateTimes: a DateTime bound is a wall-clock time in whichever zone is drawn, so its Kind must be Unspecified; to bound the instant, pass a DateTimeOffset", min.Message);
            Assert.Equal("min", min.ParamName);
            Assert.Equal("max", Assert.Throws<ArgumentException>(() => Generators.Temporal.ZonedDateTimes().Max(DateTime.SpecifyKind(lo, kind))).ParamName);
        }
        Assert.Throws<ArgumentNullException>(() => Generators.Temporal.ZonedDateTimes().Zones(null!));
        Assert.Throws<ArgumentNullException>(() => Generators.Temporal.ZonedDateTimes().Source(null!));
        Assert.Equal(
            "Zones: the zone source has no zone that this system has a TimeZoneInfo for",
            Assert.Throws<ArgumentException>(() => Generators.Temporal.ZonedDateTimes().Source(ZoneSource.CreateBuilder().Build())).Message);
    }

    [Fact]
    public void AZonedDateTimeIsAValue()
    {
        var local = new DateTime(2024, 3, 10, 1, 30, 0);
        var instant = new DateTimeOffset(2024, 3, 10, 6, 30, 0, TimeSpan.Zero);
        var value = new ZonedDateTime(DateTime.SpecifyKind(local, DateTimeKind.Utc), Standard, Home, instant.ToOffset(TimeSpan.FromHours(3)), WallTimeKind.Unique);
        Assert.Equal(local, value.LocalDateTime);
        Assert.Equal(DateTimeKind.Unspecified, value.LocalDateTime.Kind);
        Assert.Equal(Standard, value.Offset);
        Assert.Same(Home, value.Zone);
        Assert.Equal(instant, value.Instant);
        Assert.Equal(TimeSpan.Zero, value.Instant.Offset);
        Assert.Equal(WallTimeKind.Unique, value.Kind);
        Assert.Equal(new ZonedDateTime(local, Standard, Springfield(), instant, WallTimeKind.Unique), value);
        Assert.NotEqual(new ZonedDateTime(local, Standard, Home, instant, WallTimeKind.Earlier), value);
        Assert.Equal("2024-03-10T01:30:00-05:00[Test/Springfield]", value.ToString());
        Assert.Equal(new DateTimeOffset(local, Standard), value.ToDateTimeOffset());
        Assert.Equal(Standard, value.ToDateTimeOffset().Offset);
        var seconds = new ZonedDateTime(local, new TimeSpan(-4, -56, -2), Home, instant, WallTimeKind.Unique);
        Assert.Equal(new TimeSpan(-4, -56, 0), seconds.ToDateTimeOffset().Offset);
        Assert.Equal(instant, seconds.ToDateTimeOffset());
        Assert.Equal("2024-03-10T01:30:00-04:56:02[Test/Springfield]", seconds.ToString());
        Assert.Throws<ArgumentNullException>(() => new ZonedDateTime(local, Standard, null!, instant, WallTimeKind.Unique));
        Assert.Equal(
            "new ZonedDateTime(DateTime.Parse(\"2024-03-10T01:30:00\"), TimeSpan.Parse(\"-05:00:00\"), TimeZoneInfo.FindSystemTimeZoneById(\"Test/Springfield\"), DateTimeOffset.Parse(\"2024-03-10T06:30:00+00:00\"), WallTimeKind.Unique)",
            Runs.Render(value, 300));
        Assert.All(Gen.Printed(AtHome()), drawn => Assert.StartsWith("new ZonedDateTime(DateTime.Parse(", drawn.Text));
    }

    private sealed class Failing : DelegatingLibhegel
    {
        public Failing(ILibhegel inner)
            : base(inner)
        {
        }

        public override int FormatZonedDatetime(HegelZonedDateTime value, string zone, out string text)
        {
            text = "";
            return Abi.EInternal;
        }
    }

    [Fact]
    public void AFormattingFailureIsAnEngineFailure()
    {
        var value = new ZonedDateTime(new DateTime(2024, 1, 1), TimeSpan.Zero, TimeZoneInfo.Utc, new DateTimeOffset(2024, 1, 1, 0, 0, 0, TimeSpan.Zero), WallTimeKind.Unique);
        using var engine = Engines.Override(new Failing(Engines.Real));
        Assert.StartsWith("hegel_format_zoned_datetime failed (rc=-8)", Assert.Throws<HegelException>(() => value.ToString()).Message);
    }
}
