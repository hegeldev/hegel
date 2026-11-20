using System.Runtime.CompilerServices;
using Hegel.Native;
using Hegel.Temporal;
using Hegel.Tests.Support;
using Xunit;
using static Hegel.Tests.Support.TestZones;

namespace Hegel.Tests;

public class ZoneSourceTests
{
    private static readonly DateTimeOffset Y2K = new(2000, 1, 1, 0, 0, 0, TimeSpan.Zero);

    private static ZoneLocalType OneThenTwo(DateTimeOffset instant) =>
        instant < Y2K ? new ZoneLocalType(TimeSpan.FromHours(1), false, "ONE") : new ZoneLocalType(TimeSpan.FromHours(2), true, "TWO");

    private static ZonedDateTimeGenerator In(ZoneSource source) => Generators.Temporal.ZonedDateTimes().Source(source);

    [Fact]
    public void TheHostSourceIsTheBundledNamesThisSystemResolves()
    {
        var host = ZoneSource.Host.Names;
        Assert.Contains("UTC", host);
        Assert.Subset(ZoneSource.Bundled.Names.ToHashSet(), host.ToHashSet());
        Assert.All(host, name => Assert.True(TimeZoneInfo.TryFindSystemTimeZoneById(name, out _)));
        Assert.Equal(host.Order(StringComparer.Ordinal), host);
        Assert.Equal("", ZoneSource.Host.Version);
        Assert.Matches("^20[0-9]{2}[a-z]$", ZoneSource.Bundled.Version);
        Assert.Same(ZoneSource.Host, ZoneSource.Default);
    }

    [Fact]
    public void ZonesDrawTheHostsZonesByDefault()
    {
        var host = ZoneSource.Host.Names.ToHashSet();
        Gen.All(Generators.Temporal.Zones(), zone => host.Contains(zone.Id), 100);
        Assert.Equal("var x_1 = TimeZoneInfo.FindSystemTimeZoneById(\"UTC\");", Gen.MinimalOutput(Generators.Temporal.Zones()));
        Gen.All(Generators.Temporal.Zones().Names("UTC"), zone => zone.Id == "UTC", 5);
        Gen.All(Generators.Temporal.Zones().AllowLinks(false).AllowLegacy(true), zone => host.Contains(zone.Id), 50);
    }

    [Fact]
    public void TheBundledSourceDrawsOnlyZonesThisSystemHas()
    {
        var bundled = ZoneSource.Bundled.Names.ToHashSet();
        Gen.All(Generators.Temporal.Zones().Source(ZoneSource.Bundled), zone => bundled.Contains(zone.Id) && TimeZoneInfo.TryFindSystemTimeZoneById(zone.Id, out _), 100);
        Gen.All(In(ZoneSource.Bundled), value => bundled.Contains(value.Zone.Id), 50);
        Gen.All(Generators.Temporal.Zones().Source(ZoneSource.Bundled).Names("UTC"), zone => zone.Id == "UTC", 5);
    }

    [Fact]
    public void ASourceFromTimeZonesDrawsThoseObjects()
    {
        var springfield = Springfield();
        var fixedZone = Fixed("Test/Fixed", TimeSpan.FromHours(2));
        var source = ZoneSource.FromTimeZones(springfield, fixedZone);
        Assert.Equal(new[] { "Test/Fixed", "Test/Springfield" }, source.Names);
        Assert.Equal("", source.Version);
        var zones = Gen.Sample(Generators.Temporal.Zones().Source(source), 50);
        Assert.All(zones, zone => Assert.True(ReferenceEquals(zone, springfield) || ReferenceEquals(zone, fixedZone)));
        Assert.Contains(springfield, zones);
        Assert.Contains(fixedZone, zones);
        Gen.All(Generators.Temporal.Zones().Source(source).Names("Test/Fixed"), zone => ReferenceEquals(zone, fixedZone), 5);
        Gen.All(In(source), value => value.Zone.GetUtcOffset(value.Instant) == value.Offset, 100);
        Assert.Throws<ArgumentNullException>(() => ZoneSource.FromTimeZones((IEnumerable<TimeZoneInfo>)null!));
        Assert.Throws<ArgumentNullException>(() => ZoneSource.FromTimeZones([null!]));
    }

    [Fact]
    public void ASourceFromTzifBytesIsReadByTheEngine()
    {
        var source = ZoneSource.CreateBuilder().AddTzif("UTC", SpringfieldTzif()).Version("test-1").Build();
        Assert.Equal(new[] { "UTC" }, source.Names);
        Assert.Equal("test-1", source.Version);
        Gen.All(Generators.Temporal.Zones().Source(source), zone => zone.Id == "UTC", 5);
        var values = Gen.Sample(In(source), 200);
        Assert.All(values, value => Assert.True(value.Offset == Standard || value.Offset == Daylight));
        Assert.Contains(values, value => value.Offset == Daylight);
        Assert.All(values, value => Assert.Equal(TimeZoneInfo.Utc, value.Zone));
        var gap = In(source).Min(GapStart.AddMinutes(-30)).Max(GapEnd.AddMinutes(30));
        Gen.All(gap, value => value.LocalDateTime < GapStart || value.LocalDateTime >= GapEnd, 100);
    }

    [Fact]
    public void ASourceFromAnOracleIsProbedByTheEngine()
    {
        var sampled = ZoneSource.CreateBuilder().AddOracle("UTC", new ZoneOracle(OneThenTwo, 1990, 2010)).Build();
        var asked = ZoneSource.CreateBuilder()
            .AddOracle("UTC", new ZoneOracle(OneThenTwo, 1990, 2010) { NextTransition = instant => instant < Y2K ? Y2K : null, Beyond = ZoneBeyond.Infer })
            .Build();
        foreach (var source in new[] { sampled, asked })
        {
            Gen.All(In(source), value => value.Offset == TimeSpan.FromHours(value.Instant < Y2K ? 1 : 2), 100);
        }
        var oracle = new ZoneOracle(OneThenTwo, 1990, 2010);
        Assert.Equal(1990, oracle.FirstYear);
        Assert.Equal(2010, oracle.LastYear);
        Assert.Null(oracle.NextTransition);
        Assert.Equal(ZoneBeyond.Hold, oracle.Beyond);
        Assert.Equal("ONE", oracle.TypeAt(DateTimeOffset.UnixEpoch).Abbreviation);
        Assert.Throws<ArgumentNullException>(() => new ZoneOracle(null!, 1990, 2010));
    }

    [Fact]
    public void ALinkIsAnotherNameForItsTarget()
    {
        var springfield = Springfield();
        var source = ZoneSource.CreateBuilder().AddTimeZone(springfield).AddLink("Test/Shelbyville", "Test/Springfield").Build();
        Assert.Equal(new[] { "Test/Shelbyville", "Test/Springfield" }, source.Names);
        Gen.All(Generators.Temporal.Zones().Source(source), zone => ReferenceEquals(zone, springfield), 50);
        Gen.All(Generators.Temporal.Zones().Source(source).Names("Test/Shelbyville"), zone => ReferenceEquals(zone, springfield), 5);
        Gen.All(Generators.Temporal.Zones().Source(source).AllowLinks(false), zone => ReferenceEquals(zone, springfield), 20);
        var linked = Generators.Temporal.ZonedDateTimes().Zones(Generators.Temporal.Zones().Source(source).Names("Test/Shelbyville"));
        Gen.All(linked, value => ReferenceEquals(value.Zone, springfield) && springfield.GetUtcOffset(value.Instant) == value.Offset, 50);
        var dangling = ZoneSource.CreateBuilder().AddTimeZone(springfield).AddLink("Test/Dangling", "Test/Nowhere").Build();
        Assert.Equal(
            "hegel_timezone_info_in: the zone source's name \"Test/Dangling\" links to \"Test/Nowhere\", which it does not know",
            RejectedWhenBuilt(Generators.Temporal.Zones(), g => g.Names("Test/Springfield"), g => g.Source(dangling)));
    }

    [Fact]
    public void LinkAndLegacyNamesFollowTheirSettings()
    {
        var kolkata = Fixed("Asia/Kolkata", new TimeSpan(5, 30, 0));
        var calcutta = Fixed("Asia/Calcutta", new TimeSpan(5, 30, 0));
        var eastern = Fixed("EST5EDT", TimeSpan.FromHours(-5));
        var source = ZoneSource.FromTimeZones(kolkata, calcutta, eastern);
        string[] Ids(ZoneGenerator zones) => [.. Gen.Sample(zones, 100).Select(zone => zone.Id).Distinct().Order()];
        Assert.Equal(new[] { "Asia/Calcutta", "Asia/Kolkata" }, Ids(Generators.Temporal.Zones().Source(source)));
        Assert.Equal(new[] { "Asia/Kolkata" }, Ids(Generators.Temporal.Zones().Source(source).AllowLinks(false)));
        Assert.Equal(new[] { "Asia/Calcutta", "Asia/Kolkata", "EST5EDT" }, Ids(Generators.Temporal.Zones().Source(source).AllowLegacy(true)));
        Assert.Equal(new[] { "Asia/Kolkata", "EST5EDT" }, Ids(Generators.Temporal.Zones().AllowLegacy(true).AllowLinks(false).Source(source)));
        Assert.Equal(new[] { "EST5EDT" }, Ids(Generators.Temporal.Zones().Source(source).Names("EST5EDT")));
    }

    [Fact]
    public void ALoaderSuppliesDeclaredZonesOnDemand()
    {
        var lazy = Springfield("Test/Lazy");
        var asked = new List<string>();
        var source = ZoneSource.CreateBuilder()
            .Declare("Test/Lazy", "Test/Bytes", "Test/Alias", "Test/Probed")
            .Loader((name, data) =>
            {
                lock (asked)
                {
                    asked.Add(name);
                }
                switch (name)
                {
                    case "Test/Lazy":
                        data.AddTimeZone(lazy);
                        break;
                    case "Test/Bytes":
                        data.AddTzif(name, SpringfieldTzif());
                        break;
                    case "Test/Alias":
                        data.AddLink(name, "Test/Lazy");
                        break;
                    case "Test/Probed":
                        data.AddOracle(name, new ZoneOracle(OneThenTwo, 1990, 2010));
                        break;
                }
            })
            .Build();
        Assert.Empty(asked);
        Assert.Equal(new[] { "Test/Alias", "Test/Bytes", "Test/Lazy", "Test/Probed" }, source.Names);
        Gen.All(Generators.Temporal.Zones().Source(source), zone => ReferenceEquals(zone, lazy), 30);
        Assert.Equal(new[] { "Test/Alias", "Test/Bytes", "Test/Lazy", "Test/Probed" }, asked.Order());
        Gen.All(In(source), value => ReferenceEquals(value.Zone, lazy) && lazy.GetUtcOffset(value.Instant) == value.Offset, 50);
        var forgetful = ZoneSource.CreateBuilder().Declare("Test/Lazy", "Test/Missing").Loader((name, data) =>
        {
            if (name == "Test/Lazy")
            {
                data.AddTimeZone(lazy);
            }
        }).Build();
        Assert.Equal(
            "hegel_timezone_info_in: the zone source's loader supplied no data for \"Test/Missing\"",
            RejectedWhenBuilt(Generators.Temporal.Zones(), g => g.Names("Test/Lazy"), g => g.Source(forgetful)));
    }

    [Fact]
    public void AFailingLoaderFailsWhateverNeededTheData()
    {
        var source = ZoneSource.CreateBuilder().Declare("UTC", "Test/Lazy").Loader((name, _) => throw new InvalidOperationException($"no data for {name} today")).Build();
        var built = Assert.Throws<ArgumentException>(() => Generators.Temporal.Zones().Source(source));
        Assert.Contains("no data for Test/Lazy today", built.Message);
        var drawn = Assert.Throws<HegelUsageException>(() => Gen.Sample(Generators.Temporal.ZonedDateTimes().Zones(Generators.Just(TimeZoneInfo.Utc)).Source(source), 1));
        Assert.Contains("no data for UTC today", drawn.Message);
    }

    [Fact]
    public void BadZoneDataFailsTheBuild()
    {
        var malformed = Assert.Throws<ArgumentException>(() => ZoneSource.CreateBuilder().AddTzif("UTC", new byte[3]).Build());
        Assert.Contains("TZif", malformed.Message);
        var throwing = Assert.Throws<ArgumentException>(() => ZoneSource.CreateBuilder().AddOracle("UTC", new ZoneOracle(_ => throw new InvalidOperationException("the oracle is silent"), 1990, 2010)).Build());
        Assert.Contains("the oracle is silent", throwing.Message);
        var lost = new ZoneOracle(OneThenTwo, 1990, 2010) { NextTransition = _ => throw new InvalidOperationException("the oracle lost its place") };
        Assert.Contains("the oracle lost its place", Assert.Throws<ArgumentException>(() => ZoneSource.CreateBuilder().AddOracle("UTC", lost).Build()).Message);
        Assert.Throws<ArgumentException>(() => ZoneSource.CreateBuilder().AddOracle("UTC", new ZoneOracle(OneThenTwo, 2010, 1990)).Build());
        Assert.Throws<ArgumentException>(() => ZoneSource.CreateBuilder().AddLink("UTC", "UTC").Build());
        Assert.Throws<ArgumentException>(() => ZoneSource.CreateBuilder().Declare("").Build());
        Assert.Throws<ArgumentException>(() => ZoneSource.CreateBuilder().Version("a\0b").Build());
    }

    [Fact]
    public void TheBuilderRejectsNulls()
    {
        var builder = ZoneSource.CreateBuilder();
        Assert.Throws<ArgumentNullException>(() => builder.AddTzif(null!, []));
        Assert.Throws<ArgumentNullException>(() => builder.AddTzif("UTC", null!));
        Assert.Throws<ArgumentNullException>(() => builder.AddTimeZone(null!));
        Assert.Throws<ArgumentNullException>(() => builder.AddOracle(null!, new ZoneOracle(OneThenTwo, 1990, 2010)));
        Assert.Throws<ArgumentNullException>(() => builder.AddOracle("UTC", null!));
        Assert.Throws<ArgumentNullException>(() => builder.AddLink(null!, "UTC"));
        Assert.Throws<ArgumentNullException>(() => builder.AddLink("UTC", null!));
        Assert.Throws<ArgumentNullException>(() => builder.Declare(null!));
        Assert.Throws<ArgumentNullException>(() => builder.Loader(null!));
        Assert.Throws<ArgumentNullException>(() => builder.Version(null!));
        Assert.Throws<ArgumentNullException>(() => ZoneSource.SetDefault(null!));
        Assert.Throws<ArgumentNullException>(() => Generators.Temporal.Zones().Source(null!));
        Assert.Throws<ArgumentNullException>(() => Generators.Temporal.Zones().Names(null!));
        var nulls = new List<Exception>();
        var source = ZoneSource.CreateBuilder().Declare("Test/Lazy").Loader((name, data) =>
        {
            nulls.Add(Assert.Throws<ArgumentNullException>(() => data.AddTzif(null!, [])));
            nulls.Add(Assert.Throws<ArgumentNullException>(() => data.AddTzif(name, null!)));
            nulls.Add(Assert.Throws<ArgumentNullException>(() => data.AddTimeZone(null!)));
            nulls.Add(Assert.Throws<ArgumentNullException>(() => data.AddOracle(null!, new ZoneOracle(OneThenTwo, 1990, 2010))));
            nulls.Add(Assert.Throws<ArgumentNullException>(() => data.AddOracle(name, null!)));
            nulls.Add(Assert.Throws<ArgumentNullException>(() => data.AddLink(null!, name)));
            nulls.Add(Assert.Throws<ArgumentNullException>(() => data.AddLink(name, null!)));
        }).Build();
        Assert.Equal("hegel_timezone_info_in: the zone source's loader supplied no data for \"Test/Lazy\"", Assert.Throws<ArgumentException>(() => Generators.Temporal.Zones().Source(source)).Message);
        Assert.Equal(7, nulls.Count);
    }

    [Fact]
    public void ABuiltSourceDoesNotFollowItsBuilder()
    {
        var builder = ZoneSource.CreateBuilder().AddTimeZone(Fixed("Test/One", TimeSpan.FromHours(1)));
        var first = builder.Build();
        var second = builder.AddTimeZone(Fixed("Test/Two", TimeSpan.FromHours(2))).Build();
        Assert.Equal(new[] { "Test/One" }, first.Names);
        Assert.Equal(new[] { "Test/One", "Test/Two" }, second.Names);
    }

    [Fact]
    public void TheDefaultSourceIsReadWhenAGeneratorIsCreated()
    {
        var fixedZone = Fixed("Test/Fixed", TimeSpan.FromHours(2));
        var custom = ZoneSource.FromTimeZones(fixedZone);
        var before = Generators.Temporal.Zones();
        var previous = ZoneSource.SetDefault(custom);
        try
        {
            Assert.Same(ZoneSource.Host, previous);
            Assert.Same(custom, ZoneSource.Default);
            Gen.All(Generators.Temporal.Zones(), zone => ReferenceEquals(zone, fixedZone), 10);
            Gen.All(Generators.Temporal.ZonedDateTimes(), value => ReferenceEquals(value.Zone, fixedZone) && value.Offset == TimeSpan.FromHours(2), 10);
            Gen.All(Generators.Temporal.ZonedDateTimes().Zones(Generators.Just(fixedZone)), value => value.Offset == TimeSpan.FromHours(2), 10);
            Gen.All(Generators.ForType<TimeZoneInfo>(), zone => ReferenceEquals(zone, fixedZone), 5);
            Gen.All(before, zone => !ReferenceEquals(zone, fixedZone), 10);
            Gen.All(Generators.Temporal.Zones().Source(ZoneSource.Host), zone => !ReferenceEquals(zone, fixedZone), 10);
        }
        finally
        {
            Assert.Same(custom, ZoneSource.SetDefault(previous));
        }
        Assert.Same(ZoneSource.Host, ZoneSource.Default);
        Gen.All(Generators.Temporal.Zones(), zone => !ReferenceEquals(zone, fixedZone), 10);
    }

    [Fact]
    public void ZonesCheckWhatDependsOnTheSourceOnceItIsNamed()
    {
        Assert.StartsWith(
            "hegel_timezone_spec_validate: unknown time zone \"Nowhere/Land\"",
            RejectedWhenBuilt(Generators.Temporal.Zones(), g => g.Names("Nowhere/Land"), g => g.Source(ZoneSource.Bundled)));
        Assert.StartsWith(
            "hegel_timezone_spec_validate: unknown time zone \"Nowhere/Land\"",
            RejectedWhenBuilt(Generators.Temporal.Zones(), g => g.Names("Nowhere/Land"), g => g.Source(ZoneSource.Host), g => g.AllowLinks(false)));
        var custom = ZoneSource.FromTimeZones(Fixed("Test/Fixed", TimeSpan.FromHours(2)));
        Assert.StartsWith(
            "hegel_timezone_spec_validate: unknown time zone \"UTC\"",
            RejectedWhenBuilt(Generators.Temporal.Zones(), g => g.Names("UTC"), g => g.Source(custom)));
        Assert.Equal(
            "Zones: the zone source has no zone that this system has a TimeZoneInfo for",
            Assert.Throws<ArgumentException>(() => Generators.Temporal.Zones().Source(ZoneSource.CreateBuilder().Build())).Message);
        var unresolved = ZoneSource.CreateBuilder().AddTzif("Test/Bytes", SpringfieldTzif()).Build();
        Assert.Equal(
            "Zones: the zone source has no zone that this system has a TimeZoneInfo for",
            RejectedWhenBuilt(Generators.Temporal.Zones(), g => g.Names("Test/Bytes"), g => g.Source(unresolved)));
        var partly = ZoneSource.CreateBuilder().AddTzif("Test/Bytes", SpringfieldTzif()).AddTimeZone(Fixed("Test/Fixed", TimeSpan.Zero)).Build();
        Assert.Equal(
            "Zones: this system has no TimeZoneInfo for \"Test/Bytes\", so it cannot be drawn",
            RejectedWhenBuilt(Generators.Temporal.Zones(), g => g.Names("Test/Fixed", "Test/Bytes"), g => g.Source(partly)));
        Gen.All(Generators.Temporal.Zones().Source(partly), zone => zone.Id == "Test/Fixed", 20);
        Assert.Equal(
            "hegel_timezone_spec_set_names: a time zone draw needs at least one allowed name",
            Assert.Throws<ArgumentException>(() => Generators.Temporal.Zones().Names()).Message);
    }

    [Fact]
    public void ZonesLeaveWhatDependsOnADefaultToTheFirstDraw()
    {
        var eastern = ZoneSource.FromTimeZones(Fixed("EST5EDT", TimeSpan.FromHours(-5)));
        var calcutta = ZoneSource.FromTimeZones(Fixed("Asia/Calcutta", new TimeSpan(5, 30, 0)));
        const string NoneAdmitted = "Zones: the AllowLinks and AllowLegacy settings admit none of the zone source's zones that this system has a TimeZoneInfo for";
        Assert.StartsWith(
            "hegel_timezone_spec_validate: unknown time zone \"Nowhere/Land\"",
            RejectedWhenDrawn(Generators.Temporal.Zones(), g => Gen.Sample(g, 1), g => g.Names("Nowhere/Land"), g => g.AllowLinks(false)));
        Assert.StartsWith(
            "hegel_timezone_spec_validate: unknown time zone \"Nowhere/Land\"",
            RejectedWhenDrawn(Generators.Temporal.ZonedDateTimes(), g => Gen.Sample(g, 1), g => g.Zones(Generators.Temporal.Zones().Names("Nowhere/Land")), g => g.Precision(Precision.Seconds)));
        Assert.Equal(NoneAdmitted, RejectedWhenDrawn(Generators.Temporal.Zones(), g => Gen.Sample(g, 1), g => g.Source(eastern)));
        Assert.Equal(NoneAdmitted, RejectedWhenDrawn(Generators.Temporal.Zones(), g => Gen.Sample(g, 1), g => g.Source(eastern), g => g.AllowLegacy(false)));
        Assert.Equal(NoneAdmitted, RejectedWhenDrawn(Generators.Temporal.Zones(), g => Gen.Sample(g, 1), g => g.Source(calcutta), g => g.AllowLinks(false)));
        Assert.Equal(NoneAdmitted, RejectedWhenDrawn(Generators.Temporal.ZonedDateTimes(), g => Gen.Sample(g, 1), g => g.Source(eastern), g => g.Precision(Precision.Seconds)));
        AcceptedInAnyOrder(
            Generators.Temporal.Zones(),
            g => Gen.All(g, zone => zone.Id == "EST5EDT", 5),
            g => g.Source(eastern),
            g => g.AllowLegacy(true));
        AcceptedInAnyOrder(
            Generators.Temporal.Zones(),
            g => Gen.All(g, zone => zone.Id == "EST5EDT", 5),
            g => g.Source(eastern),
            g => g.AllowLegacy(false),
            g => g.Names("EST5EDT"));
        AcceptedInAnyOrder(
            Generators.Temporal.Zones(),
            g => Gen.All(g, zone => zone.Id == "Asia/Calcutta", 5),
            g => g.Source(calcutta),
            g => g.AllowLinks(false),
            g => g.Names("Asia/Calcutta"));
        var custom = ZoneSource.FromTimeZones(Fixed("Test/Fixed", TimeSpan.FromHours(2)));
        AcceptedInAnyOrder(
            Generators.Temporal.Zones(),
            g => Gen.All(g, zone => zone.Id == "Test/Fixed", 5),
            g => g.Names("Test/Fixed"),
            g => g.Source(custom));
        var previous = ZoneSource.SetDefault(ZoneSource.CreateBuilder().AddTzif("Test/Bytes", SpringfieldTzif()).AddTimeZone(Fixed("EST5EDT", TimeSpan.FromHours(-5))).Build());
        try
        {
            Assert.Equal(NoneAdmitted, RejectedWhenDrawn(Generators.Temporal.Zones(), g => Gen.Sample(g, 1), g => g.AllowLinks(true)));
            Assert.Equal(
                "Zones: this system has no TimeZoneInfo for \"Test/Bytes\", so it cannot be drawn",
                RejectedWhenDrawn(Generators.Temporal.Zones(), g => Gen.Sample(g, 1), g => g.Names("Test/Bytes", "EST5EDT"), g => g.AllowLegacy(true)));
        }
        finally
        {
            ZoneSource.SetDefault(previous);
        }
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static WeakReference UseAndDropASource()
    {
        var held = new object();
        var source = ZoneSource.CreateBuilder().Declare("Test/Lazy").Loader((_, data) =>
        {
            GC.KeepAlive(held);
            data.AddTimeZone(Fixed("Test/Lazy", TimeSpan.FromHours(1)));
        }).Build();
        Gen.All(Generators.Temporal.ZonedDateTimes().Source(source), value => value.Offset == TimeSpan.FromHours(1), 5);
        return new WeakReference(held);
    }

    [Fact]
    public void ASourceNothingRefersToReleasesItsLoader()
    {
        var held = UseAndDropASource();
        for (var attempt = 0; attempt < 500 && held.IsAlive; attempt++)
        {
            GC.Collect();
            GC.WaitForPendingFinalizers();
        }
        Assert.False(held.IsAlive);
    }

    private sealed class Failing : DelegatingLibhegel
    {
        public Failing(ILibhegel inner)
            : base(inner)
        {
        }

        public override int ZoneSourceNew(out nint source)
        {
            source = 0;
            return Abi.EInternal;
        }
    }

    [Fact]
    public void ASourceIsBuiltAgainForAnotherBinding()
    {
        var fixedZone = Fixed("Test/Fixed", TimeSpan.FromHours(2));
        var source = ZoneSource.FromTimeZones(fixedZone);
        using (var run = new RawRun())
        {
            var tc = run.Case(wantsOutput: false, lib: new DelegatingLibhegel(Engines.Real));
            Assert.Same(fixedZone, tc.DrawSilent(Generators.Temporal.Zones().Source(source)));
            Assert.Same(fixedZone, tc.DrawSilent(Generators.Temporal.ZonedDateTimes().Source(source)).Zone);
        }
        using var engine = Engines.Override(new Failing(Engines.Real));
        Assert.StartsWith("hegel_zone_source_new failed (rc=-8)", Assert.Throws<HegelException>(() => ZoneSource.FromTimeZones(fixedZone)).Message);
    }
}
