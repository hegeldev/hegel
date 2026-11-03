using Xunit;

namespace Hegel.Tests;

public class TemporalGeneratorTests
{
    [Fact]
    public void DatesRespectBounds()
    {
        var lo = new DateOnly(2020, 1, 1);
        var hi = new DateOnly(2020, 12, 31);
        Gen.All(Generators.Dates().Min(lo).Max(hi), d => d >= lo && d <= hi, 100);
        Gen.All(Generators.Dates(), _ => true, 20);
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Dates().Max(lo).Min(hi));
        Assert.Contains("Dates: Min (2020-12-31) > Max (2020-01-01)", e.Message);
        Assert.Equal("var x_1 = DateOnly.Parse(\"2020-01-01\");", Gen.MinimalOutput(Generators.Dates().Min(lo).Max(hi)));
    }

    [Fact]
    public void TimesRespectBounds()
    {
        var lo = new TimeOnly(9, 0);
        var hi = new TimeOnly(17, 0);
        Gen.All(Generators.Times().Min(lo).Max(hi), t => t >= lo && t <= hi, 100);
        Gen.All(Generators.Times(), _ => true, 20);
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Times().Max(lo).Min(hi));
        Assert.Contains("Times: Min (17:00:00) > Max (09:00:00)", e.Message);
        Assert.Equal("var x_1 = TimeOnly.Parse(\"09:00:00\");", Gen.MinimalOutput(Generators.Times().Min(lo).Max(hi)));
    }

    [Fact]
    public void DateTimesRespectBounds()
    {
        var lo = new DateTime(2020, 1, 1, 0, 0, 0);
        var hi = new DateTime(2020, 1, 2, 0, 0, 0);
        Gen.All(Generators.DateTimes().Min(lo).Max(hi), d => d >= lo && d <= hi && d.Kind == DateTimeKind.Unspecified, 100);
        Gen.All(Generators.DateTimes(), _ => true, 20);
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.DateTimes().Max(lo).Min(hi));
        Assert.Contains("DateTimes: Min (2020-01-02T00:00:00.0000000) > Max (2020-01-01T00:00:00.0000000)", e.Message);
        Assert.Equal("var x_1 = DateTime.Parse(\"2020-01-01T00:00:00\");", Gen.MinimalOutput(Generators.DateTimes().Min(lo).Max(hi)));
    }

    [Fact]
    public void DateTimesPairWithOffsets()
    {
        var lo = new DateTime(2020, 1, 1);
        var hi = new DateTime(2020, 12, 31);
        var offsets = Generators.UtcOffsets().Min(TimeSpan.FromHours(-2)).Max(TimeSpan.FromHours(2));
        Gen.All(Generators.DateTimes().Min(lo).Max(hi).WithOffsets(offsets), d => d.DateTime >= lo && d.DateTime <= hi && Math.Abs(d.Offset.TotalHours) <= 2, 100);
        Assert.Equal("var x_1 = DateTimeOffset.Parse(\"2020-01-01T00:00:00+00:00\");", Gen.MinimalOutput(Generators.DateTimes().Min(lo).Max(hi).WithOffsets(offsets)));
    }

    [Fact]
    public void DateTimesPairWithTimeZones()
    {
        var lo = new DateTime(1900, 1, 1);
        var hi = new DateTime(2100, 1, 1);
        var zones = Generators.SampledFrom([TimeZoneInfo.Utc, TimeZoneInfo.FindSystemTimeZoneById("Europe/London"), TimeZoneInfo.FindSystemTimeZoneById("Asia/Kolkata")]);
        Gen.All(Generators.DateTimes().Min(lo).Max(hi).WithTimeZones(zones), d => d.Offset.Ticks % TimeSpan.TicksPerMinute == 0 && d.DateTime >= lo && d.DateTime <= hi, 100);
        Gen.All(Generators.DateTimes().Min(lo).Max(hi).WithTimeZones(Generators.TimeZones()), d => d.Offset.Ticks % TimeSpan.TicksPerMinute == 0, 50);
    }

    [Fact]
    public void DateTimesAtTheExtremesStillPairWithOffsets()
    {
        var earliest = Generators.DateTimes().Max(DateTime.MinValue).WithOffsets(Generators.Just(TimeSpan.FromHours(1)));
        Gen.All(earliest, d => d.Offset == TimeSpan.FromHours(1), 3);
        var latest = Generators.DateTimes().Min(DateTime.MaxValue).WithTimeZones(Generators.Just(TimeZoneInfo.FindSystemTimeZoneById("Pacific/Kiritimati")));
        Gen.All(latest, d => d.Offset == TimeSpan.FromHours(14), 3);
    }

    [Fact]
    public void TimeSpansRespectBounds()
    {
        var lo = TimeSpan.FromSeconds(1);
        var hi = TimeSpan.FromSeconds(2);
        Gen.All(Generators.TimeSpans().Min(lo).Max(hi), t => t >= lo && t <= hi, 100);
        Gen.All(Generators.TimeSpans(), t => t >= TimeSpan.Zero, 20);
        Gen.All(Generators.TimeSpans().Min(TimeSpan.MinValue), _ => true, 20);
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.TimeSpans().Max(lo).Min(hi));
        Assert.Contains("TimeSpans: Min (00:00:02) > Max (00:00:01)", e.Message);
        Assert.Equal("var x_1 = TimeSpan.Parse(\"00:00:01\");", Gen.MinimalOutput(Generators.TimeSpans().Min(lo).Max(hi)));
    }

    [Fact]
    public void UtcOffsetsAreWholeMinutesWithinFourteenHours()
    {
        var limit = TimeSpan.FromHours(14);
        Gen.All(Generators.UtcOffsets(), o => o.Ticks % TimeSpan.TicksPerMinute == 0 && o >= -limit && o <= limit, 200);
        Gen.All(Generators.UtcOffsets().Min(TimeSpan.Zero).Max(TimeSpan.FromMinutes(30)), o => o >= TimeSpan.Zero && o <= TimeSpan.FromMinutes(30), 100);
        Gen.All(Generators.UtcOffsets().Max(TimeSpan.Zero), o => o <= TimeSpan.Zero, 50);
        Assert.Equal("var x_1 = TimeSpan.Parse(\"00:00:00\");", Gen.MinimalOutput(Generators.UtcOffsets()));
    }

    [Fact]
    public void UtcOffsetsRejectBadBounds()
    {
        Assert.Contains("UtcOffsets: offsets are whole minutes", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.UtcOffsets().Min(TimeSpan.FromSeconds(30))).Message);
        Assert.Contains("UtcOffsets: offsets are between -14:00 and +14:00", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.UtcOffsets().Max(TimeSpan.FromHours(15))).Message);
        Assert.Throws<ArgumentOutOfRangeException>(() => Generators.UtcOffsets().Min(TimeSpan.FromHours(-15)));
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.UtcOffsets().Min(TimeSpan.FromHours(1)).Max(TimeSpan.Zero));
        Assert.Contains("UtcOffsets: Min (01:00:00) > Max (00:00:00)", e.Message);
    }

    [Fact]
    public void TimeZonesComeFromTheSystem()
    {
        var zones = TimeZoneInfo.GetSystemTimeZones();
        Gen.All(Generators.TimeZones(), z => zones.Contains(z), 20);
    }
}
