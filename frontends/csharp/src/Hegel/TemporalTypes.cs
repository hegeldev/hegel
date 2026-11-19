using Hegel.Native;

namespace Hegel.Temporal;

/// <summary>The unit a drawn time is a whole number of.</summary>
public enum Precision
{
    /// <summary>
    /// .NET's own resolution, 100 nanoseconds: the engine draws to the nanosecond within the
    /// bounding ticks and the result is rounded down to a tick.
    /// </summary>
    Ticks = Abi.PrecisionNanoseconds,
    /// <summary>Whole microseconds.</summary>
    Microseconds = Abi.PrecisionMicroseconds,
    /// <summary>Whole milliseconds.</summary>
    Milliseconds = Abi.PrecisionMilliseconds,
    /// <summary>Whole seconds.</summary>
    Seconds = Abi.PrecisionSeconds,
    /// <summary>Whole minutes.</summary>
    Minutes = Abi.PrecisionMinutes,
    /// <summary>Whole hours.</summary>
    Hours = Abi.PrecisionHours,
}

/// <summary>The unit a drawn UTC offset is a whole number of.</summary>
public enum OffsetGranularity
{
    /// <summary>Whole hours.</summary>
    Hours = Abi.OffsetGranularityHours,
    /// <summary>Whole quarter-hours.</summary>
    QuarterHours = Abi.OffsetGranularityQuarterHours,
    /// <summary>Whole minutes, all a <see cref="DateTimeOffset"/> can hold.</summary>
    Minutes = Abi.OffsetGranularityMinutes,
    /// <summary>Whole seconds.</summary>
    Seconds = Abi.OffsetGranularitySeconds,
}

/// <summary>Which instant a zoned date-time draw means by a wall-clock time the clocks repeated.</summary>
public enum Ambiguous
{
    /// <summary>Either: the repeat is drawn, shrinking to the earlier instant.</summary>
    Both = Abi.AmbiguousBoth,
    /// <summary>Always the earlier instant.</summary>
    Earlier = Abi.AmbiguousEarlier,
    /// <summary>Always the later instant.</summary>
    Later = Abi.AmbiguousLater,
}

/// <summary>How the signs of a calendar span's units are drawn.</summary>
public enum SpanSign
{
    /// <summary>Every unit has the one drawn sign.</summary>
    Uniform = Abi.SpanSignUniform,
    /// <summary>Each unit is signed on its own.</summary>
    Mixed = Abi.SpanSignMixed,
    /// <summary>No unit is negative.</summary>
    NonNegative = Abi.SpanSignNonNegative,
}

/// <summary>What a wall-clock time is in its zone.</summary>
public enum WallTimeKind
{
    /// <summary>The wall clock showed it once.</summary>
    Unique,
    /// <summary>The clocks skipped it: it names no instant of its own.</summary>
    Skipped,
    /// <summary>The clocks showed it twice, and this is the first time.</summary>
    Earlier,
    /// <summary>The clocks showed it twice, and this is the second time.</summary>
    Later,
}

/// <summary>What a <see cref="ZoneOracle"/>'s zone does after the last year it was probed over.</summary>
public enum ZoneBeyond
{
    /// <summary>Keeps the last offset for ever.</summary>
    Hold = Abi.ZoneAfterHold,
    /// <summary>Repeats the yearly rule inferred from the last two years probed.</summary>
    Infer = Abi.ZoneAfterInfer,
}

/// <summary>
/// A date and time in a time zone, as <see cref="Generators.Temporal.ZonedDateTimes"/> draws it: the
/// wall-clock time, the zone, and what the generator's <see cref="ZoneSource"/> says that wall
/// clock means there. .NET has no such type of its own: a <see cref="DateTimeOffset"/> keeps the
/// offset but not the zone, and cannot say that a wall-clock time was skipped or repeated.
/// </summary>
public readonly record struct ZonedDateTime
{
    /// <summary>A zoned date-time from its parts, which are taken as given: nothing checks them against <paramref name="zone"/>.</summary>
    /// <param name="localDateTime">The wall-clock time; its <see cref="DateTime.Kind"/> is ignored.</param>
    /// <param name="offset">The UTC offset in force.</param>
    /// <param name="zone">The time zone.</param>
    /// <param name="instant">The instant the wall-clock time means.</param>
    /// <param name="kind">What the wall-clock time is in the zone.</param>
    public ZonedDateTime(DateTime localDateTime, TimeSpan offset, TimeZoneInfo zone, DateTimeOffset instant, WallTimeKind kind)
    {
        ArgumentNullException.ThrowIfNull(zone);
        LocalDateTime = DateTime.SpecifyKind(localDateTime, DateTimeKind.Unspecified);
        Offset = offset;
        Zone = zone;
        Instant = instant.ToUniversalTime();
        Kind = kind;
    }

    /// <summary>The wall-clock time in <see cref="Zone"/>, of <see cref="DateTimeKind.Unspecified"/>.</summary>
    public DateTime LocalDateTime { get; }

    /// <summary>
    /// The UTC offset in force at <see cref="Instant"/>. It need not be a whole number of minutes:
    /// a zone's offset before standard time usually is not.
    /// </summary>
    public TimeSpan Offset { get; }

    /// <summary>The time zone.</summary>
    public TimeZoneInfo Zone { get; }

    /// <summary>
    /// The instant, in UTC: the one <see cref="LocalDateTime"/> names, the one <see cref="Kind"/>
    /// picks of the two a repeated time names, or for a skipped time the wall-clock time read with
    /// the offset in force before the clocks changed.
    /// </summary>
    public DateTimeOffset Instant { get; }

    /// <summary>Whether the wall clock showed <see cref="LocalDateTime"/> once, twice or never.</summary>
    public WallTimeKind Kind { get; }

    /// <summary>
    /// <see cref="Instant"/> at <see cref="Offset"/>, with the offset's seconds dropped because a
    /// <see cref="DateTimeOffset"/> holds whole minutes only.
    /// </summary>
    /// <exception cref="ArgumentOutOfRangeException">The result lies outside the years 1 to 9999.</exception>
    public DateTimeOffset ToDateTimeOffset() =>
        Instant.ToOffset(new TimeSpan(Offset.Ticks - Offset.Ticks % TimeSpan.TicksPerMinute));

    /// <summary>The engine's text for the value, such as <c>2024-03-10T01:30:00-05:00[America/New_York]</c>.</summary>
    public override string ToString()
    {
        var calls = EngineCalls.Building();
        calls.Check(calls.Lib.FormatZonedDatetime(TemporalValues.Zoned(this), Zone.Id, out var text), "format_zoned_datetime");
        return text;
    }
}

/// <summary>
/// A span of calendar and clock units, as <see cref="Generators.Temporal.Spans"/> draws it: one count per
/// unit, each kept as drawn rather than folded into the next (13 months stay 13 months). .NET has
/// no such type of its own: a <see cref="TimeSpan"/> is an exact length of time and has no months
/// or years.
/// </summary>
public readonly record struct CalendarSpan
{
    /// <summary>The years.</summary>
    public long Years { get; init; }

    /// <summary>The months.</summary>
    public long Months { get; init; }

    /// <summary>The weeks.</summary>
    public long Weeks { get; init; }

    /// <summary>The days.</summary>
    public long Days { get; init; }

    /// <summary>The hours.</summary>
    public long Hours { get; init; }

    /// <summary>The minutes.</summary>
    public long Minutes { get; init; }

    /// <summary>The seconds.</summary>
    public long Seconds { get; init; }

    /// <summary>The milliseconds.</summary>
    public long Milliseconds { get; init; }

    /// <summary>The microseconds.</summary>
    public long Microseconds { get; init; }

    /// <summary>The nanoseconds.</summary>
    public long Nanoseconds { get; init; }

    /// <summary>The units in order, largest first, each with the name of its property.</summary>
    internal (string Name, long Value)[] Units() =>
    [
        (nameof(Years), Years),
        (nameof(Months), Months),
        (nameof(Weeks), Weeks),
        (nameof(Days), Days),
        (nameof(Hours), Hours),
        (nameof(Minutes), Minutes),
        (nameof(Seconds), Seconds),
        (nameof(Milliseconds), Milliseconds),
        (nameof(Microseconds), Microseconds),
        (nameof(Nanoseconds), Nanoseconds),
    ];

    /// <summary>The engine's text for the value: its units that are not zero, such as <c>1y 2mo 3d</c>, each signed when their signs differ (<c>+1y -3d</c>), and <c>0s</c> for the zero span.</summary>
    public override string ToString()
    {
        var calls = EngineCalls.Building();
        calls.Check(calls.Lib.FormatSpan(TemporalValues.Span(this), out var text), "format_span");
        return text;
    }
}
