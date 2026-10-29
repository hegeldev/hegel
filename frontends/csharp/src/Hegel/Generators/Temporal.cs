namespace Hegel;

/// <summary>Draws dates.</summary>
public sealed class DateGenerator : ValueGenerator<DateOnly>
{
    private readonly DateOnly _min;
    private readonly DateOnly _max;

    internal DateGenerator(DateOnly min, DateOnly max)
    {
        if (min > max)
        {
            throw new ArgumentOutOfRangeException(nameof(min), min, $"Dates: Min ({min:yyyy-MM-dd}) > Max ({max:yyyy-MM-dd})");
        }
        _min = min;
        _max = max;
    }

    /// <summary>No earlier than <paramref name="min"/>.</summary>
    public DateGenerator Min(DateOnly min) => new(min, _max);

    /// <summary>No later than <paramref name="max"/>.</summary>
    public DateGenerator Max(DateOnly max) => new(_min, max);

    /// <inheritdoc />
    public override DateOnly DoDraw(TestCase tc) => tc.GenerateDate(_min, _max);
}

/// <summary>Draws times of day.</summary>
public sealed class TimeGenerator : ValueGenerator<TimeOnly>
{
    private readonly TimeOnly _min;
    private readonly TimeOnly _max;

    internal TimeGenerator(TimeOnly min, TimeOnly max)
    {
        if (min > max)
        {
            throw new ArgumentOutOfRangeException(nameof(min), min, $"Times: Min ({min:HH:mm:ss.FFFFFFF}) > Max ({max:HH:mm:ss.FFFFFFF})");
        }
        _min = min;
        _max = max;
    }

    /// <summary>No earlier than <paramref name="min"/>.</summary>
    public TimeGenerator Min(TimeOnly min) => new(min, _max);

    /// <summary>No later than <paramref name="max"/>.</summary>
    public TimeGenerator Max(TimeOnly max) => new(_min, max);

    /// <inheritdoc />
    public override TimeOnly DoDraw(TestCase tc) => tc.GenerateTime(_min, _max);
}

/// <summary>Draws date-times without a time zone (<see cref="DateTimeKind.Unspecified"/>).</summary>
public sealed class DateTimeGenerator : ValueGenerator<DateTime>
{
    private readonly DateTime _min;
    private readonly DateTime _max;

    internal DateTimeGenerator(DateTime min, DateTime max)
    {
        if (min > max)
        {
            throw new ArgumentOutOfRangeException(nameof(min), min, $"DateTimes: Min ({min:O}) > Max ({max:O})");
        }
        _min = min;
        _max = max;
    }

    /// <summary>No earlier than <paramref name="min"/>.</summary>
    public DateTimeGenerator Min(DateTime min) => new(min, _max);

    /// <summary>No later than <paramref name="max"/>.</summary>
    public DateTimeGenerator Max(DateTime max) => new(_min, max);

    /// <inheritdoc />
    public override DateTime DoDraw(TestCase tc) => tc.GenerateDatetime(_min, _max);

    /// <summary>These date-times paired with offsets from <paramref name="offsets"/>, as <see cref="DateTimeOffset"/>s.</summary>
    public PrintableGenerator<DateTimeOffset> WithOffsets(Generator<TimeSpan> offsets) =>
        Generators.Tuples(this, offsets).Map(parts => Combine(parts.Item1, parts.Item2));

    /// <summary>
    /// These date-times read as local times in zones from <paramref name="zones"/>, as
    /// <see cref="DateTimeOffset"/>s (a zone's historical offset with seconds is truncated to minutes).
    /// </summary>
    public PrintableGenerator<DateTimeOffset> WithTimeZones(Generator<TimeZoneInfo> zones) =>
        Generators.Tuples(this, zones).Map(parts =>
        {
            var offset = parts.Item2.GetUtcOffset(parts.Item1);
            return Combine(parts.Item1, TimeSpan.FromMinutes(Math.Truncate(offset.TotalMinutes)));
        });

    private static DateTimeOffset Combine(DateTime dateTime, TimeSpan offset)
    {
        var utc = Math.Clamp(dateTime.Ticks - offset.Ticks, 0, DateTime.MaxValue.Ticks);
        return new DateTimeOffset(new DateTime(utc + offset.Ticks, DateTimeKind.Unspecified), offset);
    }
}

/// <summary>Draws durations.</summary>
public sealed class TimeSpanGenerator : ValueGenerator<TimeSpan>
{
    private readonly TimeSpan _min;
    private readonly TimeSpan _max;

    internal TimeSpanGenerator(TimeSpan min, TimeSpan max)
    {
        if (min > max)
        {
            throw new ArgumentOutOfRangeException(nameof(min), min, $"TimeSpans: Min ({min:c}) > Max ({max:c})");
        }
        _min = min;
        _max = max;
    }

    /// <summary>At least <paramref name="min"/>.</summary>
    public TimeSpanGenerator Min(TimeSpan min) => new(min, _max);

    /// <summary>At most <paramref name="max"/>.</summary>
    public TimeSpanGenerator Max(TimeSpan max) => new(_min, max);

    /// <inheritdoc />
    public override TimeSpan DoDraw(TestCase tc) => TimeSpan.FromTicks(tc.GenerateInteger(_min.Ticks, _max.Ticks));
}

/// <summary>Draws UTC offsets: whole minutes between -14:00 and +14:00.</summary>
public sealed class UtcOffsetGenerator : ValueGenerator<TimeSpan>
{
    internal static readonly TimeSpan Limit = TimeSpan.FromHours(14);

    private readonly long _minMinutes;
    private readonly long _maxMinutes;

    internal UtcOffsetGenerator(TimeSpan min, TimeSpan max)
    {
        _minMinutes = Minutes(min, nameof(min));
        _maxMinutes = Minutes(max, nameof(max));
        if (_minMinutes > _maxMinutes)
        {
            throw new ArgumentOutOfRangeException(nameof(min), min, $"UtcOffsets: Min ({min:c}) > Max ({max:c})");
        }
    }

    private static long Minutes(TimeSpan offset, string name)
    {
        if (offset.Ticks % TimeSpan.TicksPerMinute != 0)
        {
            throw new ArgumentOutOfRangeException(name, offset, "UtcOffsets: offsets are whole minutes");
        }
        if (offset < -Limit || offset > Limit)
        {
            throw new ArgumentOutOfRangeException(name, offset, "UtcOffsets: offsets are between -14:00 and +14:00");
        }
        return offset.Ticks / TimeSpan.TicksPerMinute;
    }

    /// <summary>At least <paramref name="min"/>.</summary>
    public UtcOffsetGenerator Min(TimeSpan min) => new(min, TimeSpan.FromMinutes(_maxMinutes));

    /// <summary>At most <paramref name="max"/>.</summary>
    public UtcOffsetGenerator Max(TimeSpan max) => new(TimeSpan.FromMinutes(_minMinutes), max);

    /// <inheritdoc />
    public override TimeSpan DoDraw(TestCase tc) => TimeSpan.FromMinutes(tc.GenerateInteger(_minMinutes, _maxMinutes));
}
