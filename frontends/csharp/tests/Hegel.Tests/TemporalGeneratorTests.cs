using System.Globalization;
using Hegel.Native;
using Hegel.Temporal;
using Hegel.Tests.Support;
using Xunit;
using static Hegel.Tests.Support.TestZones;

namespace Hegel.Tests;

public class TemporalGeneratorTests
{
    private static readonly Precision[] CoarserThanTicks = [Precision.Microseconds, Precision.Milliseconds, Precision.Seconds, Precision.Minutes, Precision.Hours];

    private static long TicksOf(Precision precision) => precision switch
    {
        Precision.Microseconds => TimeSpan.TicksPerMicrosecond,
        Precision.Milliseconds => TimeSpan.TicksPerMillisecond,
        Precision.Seconds => TimeSpan.TicksPerSecond,
        Precision.Minutes => TimeSpan.TicksPerMinute,
        Precision.Hours => TimeSpan.TicksPerHour,
        _ => 1,
    };

    [Fact]
    public void DatesRespectBounds()
    {
        var lo = new DateOnly(2020, 1, 1);
        var hi = new DateOnly(2020, 12, 31);
        Gen.All(Generators.Temporal.Dates().Min(lo).Max(hi), d => d >= lo && d <= hi, 100);
        Gen.All(Generators.Temporal.Dates(), _ => true, 20);
        Gen.All(Generators.Temporal.Dates().Max(DateOnly.MinValue), d => d == DateOnly.MinValue, 3);
        Gen.All(Generators.Temporal.Dates().Min(DateOnly.MaxValue), d => d == DateOnly.MaxValue, 3);
        Assert.Equal("var x_1 = DateOnly.Parse(\"2020-01-01\");", Gen.MinimalOutput(Generators.Temporal.Dates().Min(lo).Max(hi)));
        Assert.Equal("var x_1 = DateOnly.Parse(\"2000-01-01\");", Gen.MinimalOutput(Generators.Temporal.Dates()));
    }

    [Fact]
    public void DatesCanBeLimitedToWeekdaysAndMonths()
    {
        Gen.All(Generators.Temporal.Dates().Weekdays(DayOfWeek.Sunday, DayOfWeek.Wednesday), d => d.DayOfWeek is DayOfWeek.Sunday or DayOfWeek.Wednesday, 100);
        foreach (var day in Enum.GetValues<DayOfWeek>())
        {
            Gen.All(Generators.Temporal.Dates().Weekdays(day), d => d.DayOfWeek == day, 10);
        }
        Gen.All(Generators.Temporal.Dates().Months(1, 12), d => d.Month is 1 or 12, 100);
        Gen.All(Generators.Temporal.Dates().Months(2).Weekdays(DayOfWeek.Monday), d => d.Month == 2 && d.DayOfWeek == DayOfWeek.Monday, 50);
        Assert.Equal("var x_1 = DateOnly.Parse(\"2000-02-07\");", Gen.MinimalOutput(Generators.Temporal.Dates().Months(2).Weekdays(DayOfWeek.Monday)));
    }

    [Fact]
    public void DatesRejectBadArgumentsWhenBuiltInAnyOrder()
    {
        var lo = new DateOnly(2024, 1, 1);
        var hi = new DateOnly(2024, 1, 2);
        Assert.Equal(
            "generate_date requires min_value <= max_value, got [2024-01-02, 2024-01-01]",
            RejectedWhenBuilt(Generators.Temporal.Dates(), g => g.Min(hi), g => g.Max(lo)));
        Assert.Contains(
            "no date in [2024-01-01, 2024-01-02] falls on an allowed weekday and month",
            RejectedWhenBuilt(Generators.Temporal.Dates(), g => g.Min(lo), g => g.Max(hi), g => g.Weekdays(DayOfWeek.Friday)));
        Assert.Contains("no date in [2024-01-01, 2024-01-02]", RejectedWhenBuilt(Generators.Temporal.Dates(), g => g.Min(lo), g => g.Max(hi), g => g.Months(2)));
        Assert.Contains("weekdays mask must name at least one weekday", Assert.Throws<ArgumentException>(() => Generators.Temporal.Dates().Weekdays()).Message);
        Assert.Contains("weekdays mask must name at least one weekday", Assert.Throws<ArgumentException>(() => Generators.Temporal.Dates().Weekdays((DayOfWeek)9)).Message);
        Assert.Contains("weekdays mask must name at least one weekday", Assert.Throws<ArgumentException>(() => Generators.Temporal.Dates().Weekdays((DayOfWeek)(-40))).Message);
        Assert.Contains("months mask must name at least one month", Assert.Throws<ArgumentException>(() => Generators.Temporal.Dates().Months()).Message);
        Assert.Contains("months mask must name at least one month", Assert.Throws<ArgumentException>(() => Generators.Temporal.Dates().Months(0)).Message);
        Assert.Contains("months mask must name at least one month", Assert.Throws<ArgumentException>(() => Generators.Temporal.Dates().Months(13)).Message);
        Assert.Throws<ArgumentNullException>(() => Generators.Temporal.Dates().Weekdays(null!));
        Assert.Throws<ArgumentNullException>(() => Generators.Temporal.Dates().Months(null!));
    }

    [Fact]
    public void BoundErrorsDoNotDependOnTheCurrentCulture()
    {
        var before = CultureInfo.CurrentCulture;
        CultureInfo.CurrentCulture = new CultureInfo("th-TH");
        try
        {
            var dates = Assert.Throws<ArgumentException>(() => Generators.Temporal.Dates().Min(new DateOnly(2024, 5, 1)).Max(new DateOnly(2024, 1, 1)));
            Assert.Equal("generate_date requires min_value <= max_value, got [2024-05-01, 2024-01-01]", dates.Message);
            var times = Assert.Throws<ArgumentException>(() => Generators.Temporal.DateTimes().Min(new DateTime(2024, 5, 1, 13, 0, 0)).Max(new DateTime(2024, 1, 1)));
            Assert.Contains("got [2024-05-01T13:00:00, 2024-01-01T00:00:00", times.Message);
            Assert.Equal("new CalendarSpan {Days = -1234567}", Runs.Render(new CalendarSpan { Days = -1234567 }));
        }
        finally
        {
            CultureInfo.CurrentCulture = before;
        }
    }

    [Fact]
    public void TimesRespectBounds()
    {
        var lo = new TimeOnly(9, 0);
        var hi = new TimeOnly(17, 0);
        Gen.All(Generators.Temporal.Times().Min(lo).Max(hi), t => t >= lo && t <= hi, 100);
        Gen.All(Generators.Temporal.Times(), _ => true, 20);
        Gen.All(Generators.Temporal.Times().Min(TimeOnly.MaxValue), t => t == TimeOnly.MaxValue, 3);
        Assert.Equal("var x_1 = TimeOnly.Parse(\"09:00:00\");", Gen.MinimalOutput(Generators.Temporal.Times().Min(lo).Max(hi)));
        Assert.Equal("var x_1 = TimeOnly.Parse(\"00:00:00\");", Gen.MinimalOutput(Generators.Temporal.Times()));
    }

    [Fact]
    public void TimesAreDrawnToTheTickUnlessAPrecisionIsGiven()
    {
        var odd = new TimeOnly(13, 4, 5).Add(TimeSpan.FromTicks(1234567));
        Gen.All(Generators.Temporal.Times().Min(odd).Max(odd), t => t == odd, 5);
        var next = odd.Add(TimeSpan.FromTicks(1));
        Assert.Equal(new[] { odd, next }, Gen.Sample(Generators.Temporal.Times().Min(odd).Max(next), 200).Distinct().Order());
        foreach (var precision in CoarserThanTicks)
        {
            Gen.All(Generators.Temporal.Times().Precision(precision), t => t.Ticks % TicksOf(precision) == 0, 30);
        }
        Gen.All(Generators.Temporal.Times().Precision(Precision.Ticks).Min(odd).Max(odd), t => t == odd, 3);
        Gen.All(Generators.Temporal.Times().Min(new TimeOnly(9, 10)).Max(new TimeOnly(11, 50)).Precision(Precision.Hours), t => t == new TimeOnly(10, 0) || t == new TimeOnly(11, 0), 30);
    }

    [Fact]
    public void TimesRejectBadArgumentsWhenBuiltInAnyOrder()
    {
        Assert.StartsWith(
            "generate_time requires min_value <= max_value, got [17:00:00, 09:00:00",
            RejectedWhenBuilt(Generators.Temporal.Times(), g => g.Min(new TimeOnly(17, 0)), g => g.Max(new TimeOnly(9, 0))));
        Assert.Equal(
            "generate_time: no value at precision hours lies in the bounds",
            RejectedWhenBuilt(Generators.Temporal.Times(), g => g.Min(new TimeOnly(9, 10)), g => g.Max(new TimeOnly(9, 50)), g => g.Precision(Precision.Hours)));
        Assert.Equal(
            "generate_time: no value at precision hours lies in the bounds",
            RejectedWhenBuilt(Generators.Temporal.Times(), g => g.Min(new TimeOnly(23, 10)), g => g.Precision(Precision.Hours)));
        Assert.Equal("hegel_validate_time: unknown precision 99", Assert.Throws<ArgumentException>(() => Generators.Temporal.Times().Precision((Precision)99)).Message);
    }

    [Fact]
    public void DateTimesRespectBounds()
    {
        var lo = new DateTime(2020, 1, 1, 0, 0, 0);
        var hi = new DateTime(2020, 1, 2, 0, 0, 0);
        Gen.All(Generators.Temporal.DateTimes().Min(lo).Max(hi), d => d >= lo && d <= hi && d.Kind == DateTimeKind.Unspecified, 100);
        Gen.All(Generators.Temporal.DateTimes(), d => d.Kind == DateTimeKind.Unspecified, 20);
        Gen.All(Generators.Temporal.DateTimes().Min(DateTime.MaxValue), d => d == DateTime.MaxValue, 3);
        Gen.All(Generators.Temporal.DateTimes().Max(DateTime.MinValue), d => d == DateTime.MinValue, 3);
        var odd = lo.AddTicks(1234567);
        Assert.Equal(new[] { odd, odd.AddTicks(1) }, Gen.Sample(Generators.Temporal.DateTimes().Min(odd).Max(odd.AddTicks(1)), 200).Distinct().Order());
        foreach (var precision in CoarserThanTicks)
        {
            Gen.All(Generators.Temporal.DateTimes().Precision(precision), d => d.TimeOfDay.Ticks % TicksOf(precision) == 0, 30);
        }
        Assert.Equal("var x_1 = DateTime.Parse(\"2020-01-01T00:00:00\");", Gen.MinimalOutput(Generators.Temporal.DateTimes().Min(lo).Max(hi)));
        Assert.Equal("var x_1 = DateTime.Parse(\"2000-01-01T00:00:00\");", Gen.MinimalOutput(Generators.Temporal.DateTimes()));
    }

    [Fact]
    public void DateTimesRejectBadArgumentsWhenBuiltInAnyOrder()
    {
        var lo = new DateTime(2020, 1, 1, 9, 10, 0);
        var hi = new DateTime(2020, 1, 1, 9, 50, 0);
        Assert.StartsWith(
            "generate_datetime requires min_value <= max_value, got [2020-01-01T09:50:00, 2020-01-01T09:10:00",
            RejectedWhenBuilt(Generators.Temporal.DateTimes(), g => g.Min(hi), g => g.Max(lo)));
        Assert.Equal(
            "generate_datetime: no value at precision hours lies in the bounds",
            RejectedWhenBuilt(Generators.Temporal.DateTimes(), g => g.Min(lo), g => g.Max(hi), g => g.Precision(Precision.Hours)));
        Assert.Equal("hegel_validate_datetime: unknown precision 99", Assert.Throws<ArgumentException>(() => Generators.Temporal.DateTimes().Precision((Precision)99)).Message);
    }

    [Fact]
    public void DateTimesPairWithOffsets()
    {
        var lo = new DateTime(2020, 1, 1);
        var hi = new DateTime(2020, 12, 31);
        var offsets = Generators.Temporal.Offsets().Min(TimeSpan.FromHours(-2)).Max(TimeSpan.FromHours(2));
        Gen.All(Generators.Temporal.DateTimes().Min(lo).Max(hi).WithOffsets(offsets), d => d.DateTime >= lo && d.DateTime <= hi && Math.Abs(d.Offset.TotalHours) <= 2, 100);
        Assert.Equal("var x_1 = DateTimeOffset.Parse(\"2020-01-01T00:00:00+00:00\");", Gen.MinimalOutput(Generators.Temporal.DateTimes().Min(lo).Max(hi).WithOffsets(offsets)));
        var earliest = Generators.Temporal.DateTimes().Max(DateTime.MinValue).WithOffsets(Generators.Just(TimeSpan.FromHours(1)));
        Gen.All(earliest, d => d.Offset == TimeSpan.FromHours(1), 3);
    }

#pragma warning disable CS0618
    [Fact]
    public void TheReleasedNamesStillWork()
    {
        var lo = new DateTime(1900, 1, 1);
        var hi = new DateTime(2100, 1, 1);
        var zones = Generators.SampledFrom([TimeZoneInfo.Utc, Springfield(), Fixed("Test/Seconds", new TimeSpan(5, 30, 0))]);
        Gen.All(Generators.Temporal.DateTimes().Min(lo).Max(hi).WithTimeZones(zones), d => d.Offset.Ticks % TimeSpan.TicksPerMinute == 0 && d.DateTime >= lo && d.DateTime <= hi, 100);
        Gen.All(Generators.Temporal.DateTimes().Min(lo).Max(hi).WithTimeZones(Generators.TimeZones()), d => d.Offset.Ticks % TimeSpan.TicksPerMinute == 0, 50);
        var latest = Generators.Temporal.DateTimes().Min(DateTime.MaxValue).WithTimeZones(Generators.Just(Fixed("Test/East", TimeSpan.FromHours(14))));
        Gen.All(latest, d => d.Offset == TimeSpan.FromHours(14), 3);

        Gen.All(Generators.TimeSpans(), t => t >= TimeSpan.Zero, 50);
        Gen.All(Generators.TimeSpans().Max(TimeSpan.FromSeconds(2)), t => t >= TimeSpan.Zero && t <= TimeSpan.FromSeconds(2), 50);
        var limit = TimeSpan.FromHours(14);
        Gen.All(Generators.UtcOffsets(), o => o.Ticks % TimeSpan.TicksPerMinute == 0 && o >= -limit && o <= limit, 100);
        Gen.All(Generators.TimeZones(), zone => ZoneSource.Host.Names.Contains(zone.Id), 20);
        Assert.Equal("var x_1 = DateOnly.Parse(\"2000-01-01\");", Gen.MinimalOutput(Generators.Dates()));
        Assert.Equal("var x_1 = TimeOnly.Parse(\"00:00:00\");", Gen.MinimalOutput(Generators.Times()));
        Assert.Equal("var x_1 = DateTime.Parse(\"2000-01-01T00:00:00\");", Gen.MinimalOutput(Generators.DateTimes()));
    }
#pragma warning restore CS0618

    [Fact]
    public void InstantsRespectBounds()
    {
        var lo = new DateTimeOffset(2020, 1, 1, 0, 0, 0, TimeSpan.Zero);
        var hi = new DateTimeOffset(2020, 1, 2, 0, 0, 0, TimeSpan.FromHours(-3));
        Gen.All(Generators.Temporal.Instants().Min(lo).Max(hi), i => i >= lo && i <= hi && i.Offset == TimeSpan.Zero, 100);
        Gen.All(Generators.Temporal.Instants(), i => i.Offset == TimeSpan.Zero, 50);
        Gen.All(Generators.Temporal.Instants().Min(DateTimeOffset.MaxValue), i => i == DateTimeOffset.MaxValue, 3);
        Gen.All(Generators.Temporal.Instants().Max(DateTimeOffset.MinValue), i => i == DateTimeOffset.MinValue, 3);
        var odd = new DateTimeOffset(1960, 2, 29, 13, 4, 5, TimeSpan.Zero).AddTicks(1234567);
        Assert.Equal(new[] { odd, odd.AddTicks(1) }, Gen.Sample(Generators.Temporal.Instants().Min(odd).Max(odd.AddTicks(1)), 200).Distinct().Order());
        foreach (var precision in CoarserThanTicks)
        {
            Gen.All(Generators.Temporal.Instants().Precision(precision), i => (i - DateTimeOffset.UnixEpoch).Ticks % TicksOf(precision) == 0, 30);
        }
        Assert.Equal("var x_1 = DateTimeOffset.Parse(\"2020-01-01T00:00:00+00:00\");", Gen.MinimalOutput(Generators.Temporal.Instants().Min(lo).Max(hi)));
        Assert.Equal("var x_1 = DateTimeOffset.Parse(\"2000-01-01T00:00:00+00:00\");", Gen.MinimalOutput(Generators.Temporal.Instants()));
    }

    [Fact]
    public void InstantsRejectBadArgumentsWhenBuiltInAnyOrder()
    {
        var lo = DateTimeOffset.UnixEpoch.AddMinutes(10);
        var hi = DateTimeOffset.UnixEpoch.AddMinutes(50);
        Assert.StartsWith(
            "generate_timestamp requires min_value <= max_value, got [1970-01-01T00:50:00Z, 1970-01-01T00:10:00",
            RejectedWhenBuilt(Generators.Temporal.Instants(), g => g.Min(hi), g => g.Max(lo)));
        Assert.Contains(
            "no value at precision hours",
            RejectedWhenBuilt(Generators.Temporal.Instants(), g => g.Min(lo), g => g.Max(hi), g => g.Precision(Precision.Hours)));
        Assert.Equal("hegel_validate_timestamp: unknown precision 99", Assert.Throws<ArgumentException>(() => Generators.Temporal.Instants().Precision((Precision)99)).Message);
    }

    [Fact]
    public void OffsetsAreWholeMinutesWithinFourteenHoursByDefault()
    {
        var limit = TimeSpan.FromHours(14);
        Gen.All(Generators.Temporal.Offsets(), o => o.Ticks % TimeSpan.TicksPerMinute == 0 && o >= -limit && o <= limit, 200);
        Gen.All(Generators.Temporal.Offsets().Min(TimeSpan.Zero).Max(TimeSpan.FromMinutes(30)), o => o >= TimeSpan.Zero && o <= TimeSpan.FromMinutes(30), 100);
        Gen.All(Generators.Temporal.Offsets().Max(TimeSpan.Zero), o => o <= TimeSpan.Zero && o >= -limit, 50);
        Gen.All(Generators.Temporal.Offsets().Min(TimeSpan.Zero), o => o >= TimeSpan.Zero && o <= limit, 50);
        Gen.All(Generators.Temporal.Offsets().Min(TimeSpan.FromHours(15)).Max(new TimeSpan(25, 59, 59)), o => o >= TimeSpan.FromHours(15) && o <= TimeSpan.FromHours(26), 50);
        Gen.All(Generators.Temporal.Offsets().Max(TimeSpan.FromHours(-15)).Min(new TimeSpan(-25, -59, -59)), o => o <= TimeSpan.FromHours(-15), 50);
        Assert.Equal("var x_1 = TimeSpan.Parse(\"00:00:00\");", Gen.MinimalOutput(Generators.Temporal.Offsets()));
        Assert.Equal("var x_1 = TimeSpan.Parse(\"01:00:00\");", Gen.MinimalOutput(Generators.Temporal.Offsets().Min(TimeSpan.FromHours(1))));
    }

    [Fact]
    public void OffsetsFollowTheirGranularity()
    {
        Gen.All(Generators.Temporal.Offsets().Granularity(OffsetGranularity.Hours), o => o.Ticks % TimeSpan.TicksPerHour == 0, 50);
        Gen.All(Generators.Temporal.Offsets().Granularity(OffsetGranularity.QuarterHours), o => o.Ticks % (15 * TimeSpan.TicksPerMinute) == 0, 50);
        Gen.All(Generators.Temporal.Offsets().Granularity(OffsetGranularity.Minutes), o => o.Ticks % TimeSpan.TicksPerMinute == 0, 50);
        var seconds = Gen.Sample(Generators.Temporal.Offsets().Granularity(OffsetGranularity.Seconds).Min(TimeSpan.FromSeconds(1)).Max(TimeSpan.FromSeconds(59)), 20);
        Assert.All(seconds, o => Assert.InRange(o, TimeSpan.FromSeconds(1), TimeSpan.FromSeconds(59)));
    }

    [Fact]
    public void OffsetsRejectBadArgumentsWhenBuiltInAnyOrder()
    {
        Assert.Equal(
            "generate_offset requires min_seconds <= max_seconds, got [7200, 3600]",
            RejectedWhenBuilt(Generators.Temporal.Offsets(), g => g.Min(TimeSpan.FromHours(2)), g => g.Max(TimeSpan.FromHours(1))));
        Assert.Equal(
            "generate_offset: no offset at granularity hours lies in [600, 1200]",
            RejectedWhenBuilt(Generators.Temporal.Offsets(), g => g.Min(TimeSpan.FromMinutes(10)), g => g.Max(TimeSpan.FromMinutes(20)), g => g.Granularity(OffsetGranularity.Hours)));
        Assert.Equal("min_seconds must be in [-93599, 93599], got 97200", Assert.Throws<ArgumentException>(() => Generators.Temporal.Offsets().Min(TimeSpan.FromHours(27))).Message);
        Assert.Contains("max_seconds must be in [-93599, 93599]", Assert.Throws<ArgumentException>(() => Generators.Temporal.Offsets().Max(TimeSpan.FromDays(-100000))).Message);
        Assert.Contains("hegel_validate_offset: unknown granularity 99", Assert.Throws<ArgumentException>(() => Generators.Temporal.Offsets().Granularity((OffsetGranularity)99)).Message);
        var min = Assert.Throws<ArgumentException>(() => Generators.Temporal.Offsets().Min(TimeSpan.FromMilliseconds(1500)));
        Assert.StartsWith("Offsets: a bound must be a whole number of seconds", min.Message);
        Assert.Equal("min", min.ParamName);
        Assert.Equal("max", Assert.Throws<ArgumentException>(() => Generators.Temporal.Offsets().Max(TimeSpan.FromTicks(1))).ParamName);
    }

    [Fact]
    public void OffsetsLeaveAConflictWithADefaultToTheFirstDraw()
    {
        Assert.Equal(
            "generate_offset requires min_seconds <= max_seconds, got [54000, 50400]",
            RejectedWhenDrawn(Generators.Temporal.Offsets(), g => Gen.Sample(g, 1), g => g.Min(TimeSpan.FromHours(15))));
        Assert.Equal(
            "generate_offset: no offset at granularity minutes lies in [10, 20]",
            RejectedWhenDrawn(Generators.Temporal.Offsets(), g => Gen.Sample(g, 1), g => g.Min(TimeSpan.FromSeconds(10)), g => g.Max(TimeSpan.FromSeconds(20))));
        AcceptedInAnyOrder(
            Generators.Temporal.Offsets(),
            g => Gen.All(g, o => o >= TimeSpan.FromSeconds(10) && o <= TimeSpan.FromSeconds(20), 10),
            g => g.Min(TimeSpan.FromSeconds(10)),
            g => g.Max(TimeSpan.FromSeconds(20)),
            g => g.Granularity(OffsetGranularity.Seconds));
        AcceptedInAnyOrder(
            Generators.Temporal.Offsets(),
            g => Gen.All(g, o => o >= TimeSpan.FromHours(15) && o <= TimeSpan.FromHours(16), 10),
            g => g.Min(TimeSpan.FromHours(15)),
            g => g.Max(TimeSpan.FromHours(16)));
    }

    [Fact]
    public void DurationsRespectBounds()
    {
        var lo = TimeSpan.FromSeconds(1);
        var hi = TimeSpan.FromSeconds(2);
        Gen.All(Generators.Temporal.Durations().Min(lo).Max(hi), t => t >= lo && t <= hi, 100);
        Gen.All(Generators.Temporal.Durations(), _ => true, 50);
        Gen.All(Generators.Temporal.Durations().Max(TimeSpan.FromTicks(-1)), t => t < TimeSpan.Zero, 50);
        Gen.All(Generators.Temporal.Durations().Min(TimeSpan.MaxValue), t => t == TimeSpan.MaxValue, 3);
        Gen.All(Generators.Temporal.Durations().Max(TimeSpan.MinValue), t => t == TimeSpan.MinValue, 3);
        foreach (var odd in new[] { TimeSpan.FromTicks(-1), TimeSpan.FromTicks(-TimeSpan.TicksPerSecond), TimeSpan.FromTicks(-12345678901), TimeSpan.FromTicks(12345678901), TimeSpan.Zero })
        {
            Gen.All(Generators.Temporal.Durations().Min(odd).Max(odd), t => t == odd, 3);
            var next = odd + TimeSpan.FromTicks(1);
            Assert.Equal(new[] { odd, next }, Gen.Sample(Generators.Temporal.Durations().Min(odd).Max(next), 200).Distinct().Order());
        }
        foreach (var precision in CoarserThanTicks)
        {
            Gen.All(Generators.Temporal.Durations().Precision(precision), t => t.Ticks % TicksOf(precision) == 0, 30);
        }
        Assert.Equal("var x_1 = TimeSpan.Parse(\"00:00:01\");", Gen.MinimalOutput(Generators.Temporal.Durations().Min(lo).Max(hi)));
        Assert.Equal("var x_1 = TimeSpan.Parse(\"00:00:00\");", Gen.MinimalOutput(Generators.Temporal.Durations()));
    }

    [Fact]
    public void DurationsRejectBadArgumentsWhenBuiltInAnyOrder()
    {
        Assert.StartsWith(
            "generate_duration requires min_value <= max_value, got [PT2S, PT1",
            RejectedWhenBuilt(Generators.Temporal.Durations(), g => g.Min(TimeSpan.FromSeconds(2)), g => g.Max(TimeSpan.FromSeconds(1))));
        Assert.Contains(
            "no value at precision hours",
            RejectedWhenBuilt(Generators.Temporal.Durations(), g => g.Min(TimeSpan.FromMinutes(10)), g => g.Max(TimeSpan.FromMinutes(50)), g => g.Precision(Precision.Hours)));
        Assert.Equal("hegel_validate_duration: unknown precision 99", Assert.Throws<ArgumentException>(() => Generators.Temporal.Durations().Precision((Precision)99)).Message);
    }

    private static readonly (string Name, Func<SpanGenerator, long, long, SpanGenerator> Bound)[] SpanUnits =
    [
        ("Years", (g, min, max) => g.Years(min, max)),
        ("Months", (g, min, max) => g.Months(min, max)),
        ("Weeks", (g, min, max) => g.Weeks(min, max)),
        ("Days", (g, min, max) => g.Days(min, max)),
        ("Hours", (g, min, max) => g.Hours(min, max)),
        ("Minutes", (g, min, max) => g.Minutes(min, max)),
        ("Seconds", (g, min, max) => g.Seconds(min, max)),
        ("Milliseconds", (g, min, max) => g.Milliseconds(min, max)),
        ("Microseconds", (g, min, max) => g.Microseconds(min, max)),
        ("Nanoseconds", (g, min, max) => g.Nanoseconds(min, max)),
    ];

    private static long[] Values(CalendarSpan span) => [.. span.Units().Select(unit => unit.Value)];

    [Fact]
    public void SpansDrawEveryUnitWithOneSignByDefault()
    {
        var spans = Gen.Sample(Generators.Temporal.Spans(), 200);
        Assert.All(spans, span => Assert.True(Values(span).All(v => v >= 0) || Values(span).All(v => v <= 0)));
        for (var unit = 0; unit < SpanUnits.Length; unit++)
        {
            Assert.Contains(spans, span => Values(span)[unit] != 0);
        }
        Assert.Contains(spans, span => Values(span).Any(v => v < 0));
        Assert.Contains(spans, span => Values(span).Any(v => v > 0));
        Assert.Equal("var x_1 = new CalendarSpan();", Gen.MinimalOutput(Generators.Temporal.Spans()));
        Assert.Equal(Enumerable.Range(0, 10).Select(i => SpanUnits[i].Name), new CalendarSpan().Units().Select(unit => unit.Name));
    }

    [Fact]
    public void SpansDrawOnlyTheUnitsGivenBounds()
    {
        for (var unit = 0; unit < SpanUnits.Length; unit++)
        {
            var index = unit;
            Gen.All(SpanUnits[unit].Bound(Generators.Temporal.Spans(), 3, 7), span => Values(span)[index] is >= 3 and <= 7 && Values(span).Count(v => v != 0) == 1, 20);
        }
        Gen.All(Generators.Temporal.Spans().Days(1, 5).Hours(0, 23), span => span.Days is >= 1 and <= 5 && span.Hours is >= 0 and <= 23 && span.Years == 0, 50);
        Gen.All(Generators.Temporal.Spans().Days(1, 5).Days(-5, -1), span => span.Days is >= -5 and <= -1, 20);
        Assert.Equal("var x_1 = new CalendarSpan {Days = 1};", Gen.MinimalOutput(Generators.Temporal.Spans().Days(1, 5).Hours(0, 23)));
    }

    [Fact]
    public void SpansFollowTheirSignPolicy()
    {
        Gen.All(Generators.Temporal.Spans().Sign(SpanSign.NonNegative), span => Values(span).All(v => v >= 0), 100);
        Gen.All(Generators.Temporal.Spans().Sign(SpanSign.Uniform), span => Values(span).All(v => v >= 0) || Values(span).All(v => v <= 0), 50);
        var mixed = Gen.Sample(Generators.Temporal.Spans().Sign(SpanSign.Mixed), 200);
        Assert.Contains(mixed, span => Values(span).Any(v => v < 0) && Values(span).Any(v => v > 0));
        AcceptedInAnyOrder(
            Generators.Temporal.Spans(),
            g => Gen.All(g, span => span.Years is 1 or 2 && span.Days is -1 or -2, 10),
            g => g.Years(1, 2),
            g => g.Days(-2, -1),
            g => g.Sign(SpanSign.Mixed));
    }

    [Fact]
    public void SpansRejectBadArgumentsWhenBuiltInAnyOrder()
    {
        Assert.Equal(
            "hegel_span_spec_set_unit_bounds: min_value <= max_value is required for days, got [2, 1]",
            Assert.Throws<ArgumentException>(() => Generators.Temporal.Spans().Days(2, 1)).Message);
        Assert.Equal(
            "hegel_span_spec_validate: a span draw under the nonnegative sign policy requires min >= 0 for days, got -2",
            RejectedWhenBuilt(Generators.Temporal.Spans(), g => g.Days(-2, -1), g => g.Sign(SpanSign.NonNegative)));
        Assert.Contains(
            "under the uniform sign policy needs every unit to admit one sign",
            RejectedWhenBuilt(Generators.Temporal.Spans(), g => g.Days(-2, -1), g => g.Years(1, 2), g => g.Sign(SpanSign.Uniform)));
        Assert.Equal("hegel_span_spec_set_sign: unknown sign policy 9", Assert.Throws<ArgumentException>(() => Generators.Temporal.Spans().Sign((SpanSign)9)).Message);
    }

    [Fact]
    public void SpansLeaveAConflictWithTheDefaultSignToTheFirstDraw()
    {
        Assert.Contains(
            "under the uniform sign policy needs every unit to admit one sign, but years [1, 2], days [-2, -1] admit none in common",
            RejectedWhenDrawn(Generators.Temporal.Spans(), g => Gen.Sample(g, 1), g => g.Days(-2, -1), g => g.Years(1, 2)));
    }

    [Fact]
    public void SpansPrintAsTheEngineWritesThem()
    {
        Assert.Equal("+1y -3d +5ns", new CalendarSpan { Years = 1, Days = -3, Nanoseconds = 5 }.ToString());
        Assert.Equal("0s", new CalendarSpan().ToString());
        var all = new CalendarSpan { Years = 1, Months = 2, Weeks = 3, Days = 4, Hours = 5, Minutes = 6, Seconds = 7, Milliseconds = 8, Microseconds = 9, Nanoseconds = 10 };
        Assert.Equal("1y 2mo 3w 4d 5h 6m 7s 8ms 9µs 10ns", all.ToString());
        Assert.Equal(all, all with { });
        Assert.NotEqual(all, all with { Weeks = 0 });
        Assert.Equal("new CalendarSpan()", Runs.Render(new CalendarSpan()));
        Assert.Equal(
            "new CalendarSpan {Years = 1, Months = 2, Weeks = 3, Days = 4, Hours = 5, Minutes = 6, Seconds = 7, Milliseconds = 8, Microseconds = 9, Nanoseconds = 10}",
            Runs.Render(all, 200));
        Assert.All(Gen.Printed(Generators.Temporal.Spans().Days(1, 5)), drawn => Assert.Equal($"new CalendarSpan {{Days = {drawn.Value.Days}}}", drawn.Text));
    }

    private sealed class Failing : DelegatingLibhegel
    {
        public Failing(ILibhegel inner)
            : base(inner)
        {
        }

        public override int ValidateDate(HegelDate min, HegelDate max, nint options) => Abi.EInternal;

        public override int FormatSpan(HegelSpan value, out string text)
        {
            text = "";
            return Abi.EInvalidHandle;
        }
    }

    [Fact]
    public void AnEngineFailureThatIsNotABadArgumentIsNotReportedAsOne()
    {
        using var engine = Engines.Override(new Failing(Engines.Real));
        Assert.StartsWith("hegel_validate_date failed (rc=-8)", Assert.Throws<HegelException>(() => Generators.Temporal.Dates().Min(DateOnly.MinValue)).Message);
        Assert.StartsWith("hegel_format_span failed (rc=-4)", Assert.Throws<HegelException>(() => new CalendarSpan().ToString()).Message);
    }

    [Fact]
    public void AGeneratorBuiltAgainstOneBindingDrawsThroughAnother()
    {
        var dates = Generators.Temporal.Dates().Weekdays(DayOfWeek.Monday);
        var spans = Generators.Temporal.Spans().Days(1, 2);
        using var run = new RawRun();
        var tc = run.Case(wantsOutput: false, lib: new DelegatingLibhegel(Engines.Real));
        Assert.Equal(DayOfWeek.Monday, tc.DrawSilent(dates).DayOfWeek);
        Assert.InRange(tc.DrawSilent(spans).Days, 1, 2);
    }
}
