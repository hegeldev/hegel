using Hegel.Native;

namespace Hegel.Temporal;

/// <summary>
/// Draws dates, shrinking toward 2000-01-01. See <see cref="Generators.Temporal.Dates"/> for how bad
/// arguments are reported.
/// </summary>
public sealed class DateGenerator : ValueGenerator<DateOnly>
{
    private readonly DateOnly? _min;
    private readonly DateOnly? _max;
    private readonly uint? _weekdays;
    private readonly uint? _months;
    private readonly PerEngine<EngineHandle> _options = new();

    internal DateGenerator()
    {
    }

    private DateGenerator(DateOnly? min, DateOnly? max, uint? weekdays, uint? months)
    {
        _min = min;
        _max = max;
        _weekdays = weekdays;
        _months = months;
        var calls = EngineCalls.Building();
        var options = Options(calls);
        calls.Check(calls.Lib.ValidateDate(TemporalValues.Date(Lower), TemporalValues.Date(Upper), options?.Handle ?? 0), "validate_date");
        GC.KeepAlive(options);
    }

    private DateOnly Lower => _min ?? DateOnly.MinValue;

    private DateOnly Upper => _max ?? DateOnly.MaxValue;

    private EngineHandle? Options(EngineCalls calls) => _weekdays == null && _months == null ? null : _options.Get(calls.Lib, () => calls.Create(
        "date_options",
        static (ILibhegel lib, out nint handle) => lib.DateOptionsNew(out handle),
        static (lib, handle) => lib.DateOptionsFree(handle),
        options =>
        {
            if (_weekdays is { } weekdays)
            {
                calls.Check(calls.Lib.DateOptionsSetWeekdays(options, weekdays), "date_options_set_weekdays");
            }
            if (_months is { } months)
            {
                calls.Check(calls.Lib.DateOptionsSetMonths(options, months), "date_options_set_months");
            }
        }));

    /// <summary>No earlier than <paramref name="min"/>.</summary>
    /// <exception cref="ArgumentException">The settings given so far allow no date.</exception>
    public DateGenerator Min(DateOnly min) => new(min, _max, _weekdays, _months);

    /// <summary>No later than <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The settings given so far allow no date.</exception>
    public DateGenerator Max(DateOnly max) => new(_min, max, _weekdays, _months);

    /// <summary>Only dates falling on one of <paramref name="days"/>.</summary>
    /// <exception cref="ArgumentException"><paramref name="days"/> is empty or holds a value that is no day, or the settings given so far allow no date.</exception>
    public DateGenerator Weekdays(params DayOfWeek[] days)
    {
        ArgumentNullException.ThrowIfNull(days);
        return new(_min, _max, TemporalValues.Weekdays(days), _months);
    }

    /// <summary>Only dates in one of <paramref name="months"/>, numbered from 1 for January.</summary>
    /// <exception cref="ArgumentException"><paramref name="months"/> is empty or holds a number that is no month, or the settings given so far allow no date.</exception>
    public DateGenerator Months(params int[] months)
    {
        ArgumentNullException.ThrowIfNull(months);
        return new(_min, _max, _weekdays, TemporalValues.Months(months));
    }

    /// <inheritdoc />
    protected internal override DateOnly DoDraw(TestCase tc) =>
        tc.GenerateDate(Lower, Upper, _weekdays == null && _months == null ? null : Options(EngineCalls.Drawing(tc.Binding())));
}

/// <summary>
/// Draws times of day, shrinking toward midnight. See <see cref="Generators.Temporal.Dates"/> for how bad
/// arguments are reported.
/// </summary>
public sealed class TimeGenerator : ValueGenerator<TimeOnly>
{
    private readonly TimeOnly? _min;
    private readonly TimeOnly? _max;
    private readonly Precision? _precision;

    internal TimeGenerator()
    {
    }

    private TimeGenerator(TimeOnly? min, TimeOnly? max, Precision? precision)
    {
        _min = min;
        _max = max;
        _precision = precision;
        var calls = EngineCalls.Building();
        calls.Check(calls.Lib.ValidateTime(TemporalValues.Time(Lower), TemporalValues.LastTime(Upper), (uint)Unit), "validate_time");
    }

    private TimeOnly Lower => _min ?? TimeOnly.MinValue;

    private TimeOnly Upper => _max ?? TimeOnly.MaxValue;

    private Precision Unit => _precision ?? Hegel.Temporal.Precision.Ticks;

    /// <summary>No earlier than <paramref name="min"/>.</summary>
    /// <exception cref="ArgumentException">The settings given so far allow no time.</exception>
    public TimeGenerator Min(TimeOnly min) => new(min, _max, _precision);

    /// <summary>No later than <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The settings given so far allow no time.</exception>
    public TimeGenerator Max(TimeOnly max) => new(_min, max, _precision);

    /// <summary>Only whole numbers of <paramref name="precision"/> since midnight, rather than of ticks.</summary>
    /// <exception cref="ArgumentException"><paramref name="precision"/> is not a precision, or the settings given so far allow no time.</exception>
    public TimeGenerator Precision(Precision precision) => new(_min, _max, precision);

    /// <inheritdoc />
    protected internal override TimeOnly DoDraw(TestCase tc) => tc.GenerateTime(Lower, Upper, Unit);
}

/// <summary>
/// Draws date-times without a time zone (<see cref="DateTimeKind.Unspecified"/>), shrinking toward
/// 2000-01-01T00:00. See <see cref="Generators.Temporal.Dates"/> for how bad arguments are reported.
/// </summary>
public sealed class DateTimeGenerator : ValueGenerator<DateTime>
{
    private readonly DateTime? _min;
    private readonly DateTime? _max;
    private readonly Precision? _precision;

    internal DateTimeGenerator()
    {
    }

    private DateTimeGenerator(DateTime? min, DateTime? max, Precision? precision)
    {
        _min = min;
        _max = max;
        _precision = precision;
        var calls = EngineCalls.Building();
        calls.Check(calls.Lib.ValidateDatetime(TemporalValues.DateTime(Lower), TemporalValues.LastDateTime(Upper), (uint)Unit), "validate_datetime");
    }

    private DateTime Lower => _min ?? DateTime.MinValue;

    private DateTime Upper => _max ?? DateTime.MaxValue;

    private Precision Unit => _precision ?? Hegel.Temporal.Precision.Ticks;

    /// <summary>No earlier than <paramref name="min"/>.</summary>
    /// <exception cref="ArgumentException">The settings given so far allow no date-time.</exception>
    public DateTimeGenerator Min(DateTime min) => new(min, _max, _precision);

    /// <summary>No later than <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The settings given so far allow no date-time.</exception>
    public DateTimeGenerator Max(DateTime max) => new(_min, max, _precision);

    /// <summary>Only times of day that are whole numbers of <paramref name="precision"/> since midnight, rather than of ticks.</summary>
    /// <exception cref="ArgumentException"><paramref name="precision"/> is not a precision, or the settings given so far allow no date-time.</exception>
    public DateTimeGenerator Precision(Precision precision) => new(_min, _max, precision);

    /// <inheritdoc />
    protected internal override DateTime DoDraw(TestCase tc) => tc.GenerateDatetime(Lower, Upper, Unit);

    /// <summary>These date-times paired with offsets from <paramref name="offsets"/>, as <see cref="DateTimeOffset"/>s.</summary>
    public PrintableGenerator<DateTimeOffset> WithOffsets(Generator<TimeSpan> offsets) =>
        Generators.Tuples(this, offsets).Map(parts => Combine(parts.Item1, parts.Item2));

    /// <summary>
    /// These date-times read as local times in zones from <paramref name="zones"/>, as
    /// <see cref="DateTimeOffset"/>s (a zone's historical offset with seconds is truncated to minutes).
    /// </summary>
    [Obsolete("Use Generators.Temporal.ZonedDateTimes().Zones(zones), whose values say which zone they are in and whether the wall-clock time was skipped or repeated; ZonedDateTime.ToDateTimeOffset() gives the DateTimeOffset.")]
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

/// <summary>
/// Draws instants, as <see cref="DateTimeOffset"/>s with a zero offset, shrinking toward
/// 2000-01-01T00:00Z. See <see cref="Generators.Temporal.Dates"/> for how bad arguments are reported.
/// </summary>
public sealed class InstantGenerator : ValueGenerator<DateTimeOffset>
{
    private readonly DateTimeOffset? _min;
    private readonly DateTimeOffset? _max;
    private readonly Precision? _precision;

    internal InstantGenerator()
    {
    }

    private InstantGenerator(DateTimeOffset? min, DateTimeOffset? max, Precision? precision)
    {
        _min = min;
        _max = max;
        _precision = precision;
        var calls = EngineCalls.Building();
        calls.Check(calls.Lib.ValidateTimestamp(TemporalValues.Timestamp(Lower), TemporalValues.LastTimestamp(Upper), (uint)Unit), "validate_timestamp");
    }

    private DateTimeOffset Lower => _min ?? DateTimeOffset.MinValue;

    private DateTimeOffset Upper => _max ?? DateTimeOffset.MaxValue;

    private Precision Unit => _precision ?? Hegel.Temporal.Precision.Ticks;

    /// <summary>No earlier than <paramref name="min"/>.</summary>
    /// <exception cref="ArgumentException">The settings given so far allow no instant.</exception>
    public InstantGenerator Min(DateTimeOffset min) => new(min, _max, _precision);

    /// <summary>No later than <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The settings given so far allow no instant.</exception>
    public InstantGenerator Max(DateTimeOffset max) => new(_min, max, _precision);

    /// <summary>Only whole numbers of <paramref name="precision"/> since the Unix epoch, rather than of ticks.</summary>
    /// <exception cref="ArgumentException"><paramref name="precision"/> is not a precision, or the settings given so far allow no instant.</exception>
    public InstantGenerator Precision(Precision precision) => new(_min, _max, precision);

    /// <inheritdoc />
    protected internal override DateTimeOffset DoDraw(TestCase tc) => tc.GenerateInstant(Lower, Upper, Unit);
}

/// <summary>
/// Draws UTC offsets, as <see cref="TimeSpan"/>s east of UTC, shrinking toward zero: whole minutes
/// between -14:00 and +14:00 by default, which is what a <see cref="DateTimeOffset"/> can hold.
/// See <see cref="Generators.Temporal.Dates"/> for how bad arguments are reported.
/// </summary>
public sealed class OffsetGenerator : ValueGenerator<TimeSpan>
{
    private const int DefaultLimitSeconds = 14 * 60 * 60;

    private readonly int? _min;
    private readonly int? _max;
    private readonly OffsetGranularity? _granularity;

    internal OffsetGenerator()
    {
    }

    private OffsetGenerator(int? min, int? max, OffsetGranularity? granularity)
    {
        _min = min;
        _max = max;
        _granularity = granularity;
        var calls = EngineCalls.Building();
        calls.Check(calls.Lib.ValidateOffset(min ?? -Abi.OffsetLimitSeconds, max ?? Abi.OffsetLimitSeconds, (uint)(granularity ?? OffsetGranularity.Seconds)), "validate_offset");
    }

    private static int Seconds(TimeSpan bound, string name) => bound.Ticks % TimeSpan.TicksPerSecond == 0
        ? TemporalValues.OffsetSeconds(bound)
        : throw new ArgumentException("Offsets: a bound must be a whole number of seconds", name);

    /// <summary>
    /// At least <paramref name="min"/>, which may be as low as -25:59:59. The upper bound stays
    /// +14:00 unless <see cref="Max"/> changes it.
    /// </summary>
    /// <exception cref="ArgumentException"><paramref name="min"/> is not a whole number of seconds, or the settings given so far allow no offset.</exception>
    public OffsetGenerator Min(TimeSpan min) => new(Seconds(min, nameof(min)), _max, _granularity);

    /// <summary>
    /// At most <paramref name="max"/>, which may be as high as +25:59:59. The lower bound stays
    /// -14:00 unless <see cref="Min"/> changes it.
    /// </summary>
    /// <exception cref="ArgumentException"><paramref name="max"/> is not a whole number of seconds, or the settings given so far allow no offset.</exception>
    public OffsetGenerator Max(TimeSpan max) => new(_min, Seconds(max, nameof(max)), _granularity);

    /// <summary>Only whole numbers of <paramref name="granularity"/>, rather than of minutes.</summary>
    /// <exception cref="ArgumentException"><paramref name="granularity"/> is not a granularity, or the settings given so far allow no offset.</exception>
    public OffsetGenerator Granularity(OffsetGranularity granularity) => new(_min, _max, granularity);

    /// <inheritdoc />
    protected internal override TimeSpan DoDraw(TestCase tc) =>
        tc.GenerateOffset(_min ?? -DefaultLimitSeconds, _max ?? DefaultLimitSeconds, _granularity ?? OffsetGranularity.Minutes);
}

/// <summary>
/// Draws exact lengths of time, as <see cref="TimeSpan"/>s of either sign, shrinking toward zero.
/// See <see cref="Generators.Temporal.Dates"/> for how bad arguments are reported.
/// </summary>
public sealed class DurationGenerator : ValueGenerator<TimeSpan>
{
    private readonly TimeSpan? _min;
    private readonly TimeSpan? _max;
    private readonly Precision? _precision;

    internal DurationGenerator()
    {
    }

    /// <summary>The durations from zero up, which need no check.</summary>
    internal static DurationGenerator NonNegative() => new(TimeSpan.Zero);

    private DurationGenerator(TimeSpan min)
    {
        _min = min;
    }

    private DurationGenerator(TimeSpan? min, TimeSpan? max, Precision? precision)
    {
        _min = min;
        _max = max;
        _precision = precision;
        var calls = EngineCalls.Building();
        calls.Check(calls.Lib.ValidateDuration(TemporalValues.Duration(Lower), TemporalValues.LastDuration(Upper), (uint)Unit), "validate_duration");
    }

    private TimeSpan Lower => _min ?? TimeSpan.MinValue;

    private TimeSpan Upper => _max ?? TimeSpan.MaxValue;

    private Precision Unit => _precision ?? Hegel.Temporal.Precision.Ticks;

    /// <summary>At least <paramref name="min"/>.</summary>
    /// <exception cref="ArgumentException">The settings given so far allow no duration.</exception>
    public DurationGenerator Min(TimeSpan min) => new(min, _max, _precision);

    /// <summary>At most <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The settings given so far allow no duration.</exception>
    public DurationGenerator Max(TimeSpan max) => new(_min, max, _precision);

    /// <summary>Only whole numbers of <paramref name="precision"/>, rather than of ticks.</summary>
    /// <exception cref="ArgumentException"><paramref name="precision"/> is not a precision, or the settings given so far allow no duration.</exception>
    public DurationGenerator Precision(Precision precision) => new(_min, _max, precision);

    /// <inheritdoc />
    protected internal override TimeSpan DoDraw(TestCase tc) => tc.GenerateDuration(Lower, Upper, Unit);
}

/// <summary>
/// Draws calendar spans, shrinking toward the zero span. Every unit is drawn until one of the unit
/// methods is called; from then on only the units given bounds are, and the others are zero. See
/// <see cref="Generators.Temporal.Dates"/> for how bad arguments are reported.
/// </summary>
public sealed class SpanGenerator : ValueGenerator<CalendarSpan>
{
    private readonly IReadOnlyDictionary<uint, (long Min, long Max)> _units;
    private readonly SpanSign? _sign;
    private readonly PerEngine<EngineHandle> _spec = new();

    internal SpanGenerator()
    {
        _units = new Dictionary<uint, (long, long)>();
    }

    private SpanGenerator(IReadOnlyDictionary<uint, (long Min, long Max)> units, SpanSign? sign)
    {
        _units = units;
        _sign = sign;
        Spec(EngineCalls.Building(), sign ?? SpanSign.Mixed).Dispose();
    }

    private EngineHandle Spec(EngineCalls calls, SpanSign sign) => calls.Create(
        "span_spec",
        static (ILibhegel lib, out nint handle) => lib.SpanSpecNew(out handle),
        static (lib, handle) => lib.SpanSpecFree(handle),
        spec =>
        {
            var lib = calls.Lib;
            var lowest = sign == SpanSign.NonNegative ? 0 : -long.MaxValue;
            for (var unit = 0u; unit < Abi.SpanUnitCount; unit++)
            {
                if (_units.TryGetValue(unit, out var bounds))
                {
                    calls.Check(lib.SpanSpecSetUnitBounds(spec, unit, bounds.Min, bounds.Max), "span_spec_set_unit_bounds");
                }
                else if (_units.Count == 0)
                {
                    calls.Check(lib.SpanSpecSetUnitBounds(spec, unit, lowest, long.MaxValue), "span_spec_set_unit_bounds");
                }
            }
            calls.Check(lib.SpanSpecSetSign(spec, (uint)sign), "span_spec_set_sign");
            calls.Check(lib.SpanSpecValidate(spec), "span_spec_validate");
        });

    private SpanGenerator Unit(uint unit, long min, long max) =>
        new(new Dictionary<uint, (long, long)>(_units) { [unit] = (min, max) }, _sign);

    /// <summary>Draw years, between <paramref name="min"/> and <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The bounds are out of order, or conflict with the sign policy given.</exception>
    public SpanGenerator Years(long min, long max) => Unit(Abi.SpanUnitYears, min, max);

    /// <summary>Draw months, between <paramref name="min"/> and <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The bounds are out of order, or conflict with the sign policy given.</exception>
    public SpanGenerator Months(long min, long max) => Unit(Abi.SpanUnitMonths, min, max);

    /// <summary>Draw weeks, between <paramref name="min"/> and <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The bounds are out of order, or conflict with the sign policy given.</exception>
    public SpanGenerator Weeks(long min, long max) => Unit(Abi.SpanUnitWeeks, min, max);

    /// <summary>Draw days, between <paramref name="min"/> and <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The bounds are out of order, or conflict with the sign policy given.</exception>
    public SpanGenerator Days(long min, long max) => Unit(Abi.SpanUnitDays, min, max);

    /// <summary>Draw hours, between <paramref name="min"/> and <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The bounds are out of order, or conflict with the sign policy given.</exception>
    public SpanGenerator Hours(long min, long max) => Unit(Abi.SpanUnitHours, min, max);

    /// <summary>Draw minutes, between <paramref name="min"/> and <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The bounds are out of order, or conflict with the sign policy given.</exception>
    public SpanGenerator Minutes(long min, long max) => Unit(Abi.SpanUnitMinutes, min, max);

    /// <summary>Draw seconds, between <paramref name="min"/> and <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The bounds are out of order, or conflict with the sign policy given.</exception>
    public SpanGenerator Seconds(long min, long max) => Unit(Abi.SpanUnitSeconds, min, max);

    /// <summary>Draw milliseconds, between <paramref name="min"/> and <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The bounds are out of order, or conflict with the sign policy given.</exception>
    public SpanGenerator Milliseconds(long min, long max) => Unit(Abi.SpanUnitMilliseconds, min, max);

    /// <summary>Draw microseconds, between <paramref name="min"/> and <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The bounds are out of order, or conflict with the sign policy given.</exception>
    public SpanGenerator Microseconds(long min, long max) => Unit(Abi.SpanUnitMicroseconds, min, max);

    /// <summary>Draw nanoseconds, between <paramref name="min"/> and <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The bounds are out of order, or conflict with the sign policy given.</exception>
    public SpanGenerator Nanoseconds(long min, long max) => Unit(Abi.SpanUnitNanoseconds, min, max);

    /// <summary>
    /// How the units' signs are drawn: by default <see cref="SpanSign.Uniform"/>, one sign for the
    /// whole span. Unit bounds that the default policy does not admit (a unit that must be positive
    /// beside one that must be negative) are reported by the first draw, since a later call of this
    /// method could still allow them.
    /// </summary>
    /// <exception cref="ArgumentException"><paramref name="sign"/> is not a sign policy, or does not admit the unit bounds given.</exception>
    public SpanGenerator Sign(SpanSign sign) => new(_units, sign);

    /// <inheritdoc />
    protected internal override CalendarSpan DoDraw(TestCase tc)
    {
        var calls = EngineCalls.Drawing(tc.Binding());
        return tc.GenerateSpan(_spec.Get(calls.Lib, () => Spec(calls, _sign ?? SpanSign.Uniform)));
    }
}

/// <summary>
/// Draws time zones, shrinking toward UTC: the zones of a <see cref="ZoneSource"/> that this system
/// has a <see cref="TimeZoneInfo"/> for. The source is the one <see cref="Source"/> names, or else
/// <see cref="ZoneSource.Default"/> as it was when the generator was created. See
/// <see cref="Generators.Temporal.Zones"/> for how bad arguments are reported.
/// </summary>
public sealed class ZoneGenerator : ValueGenerator<TimeZoneInfo>
{
    private readonly IReadOnlyList<string>? _names;
    private readonly bool? _links;
    private readonly bool? _legacy;
    private readonly ZoneSource? _source;
    private readonly ZoneSource _default;
    private readonly PerEngine<EngineHandle> _spec = new();

    internal ZoneGenerator(ZoneSource defaultSource)
    {
        _default = defaultSource;
    }

    private ZoneGenerator(IReadOnlyList<string>? names, bool? links, bool? legacy, ZoneSource? source, ZoneSource defaultSource)
    {
        _names = names;
        _links = links;
        _legacy = legacy;
        _source = source;
        _default = defaultSource;
        BuildSpec(EngineCalls.Building(), source, drawing: false).Dispose();
    }

    /// <summary>The zone source drawn from.</summary>
    internal ZoneSource DrawnFrom => _source ?? _default;

    /// <summary>
    /// The engine's spec for these settings. Without a source it is only filled in, which checks
    /// each setting on its own; with one it is checked against the source too. The link and legacy
    /// settings narrow the zones only when <paramref name="drawing"/>, since until then a list of
    /// names, which they do not apply to, may still be given.
    /// </summary>
    private EngineHandle BuildSpec(EngineCalls calls, ZoneSource? source, bool drawing) => calls.Create(
        "timezone_spec",
        static (ILibhegel lib, out nint handle) => lib.TimezoneSpecNew(out handle),
        static (lib, handle) => lib.TimezoneSpecFree(handle),
        spec =>
        {
            var lib = calls.Lib;
            var links = !drawing || (_links ?? true);
            var legacy = !drawing || (_legacy ?? false);
            calls.Check(lib.TimezoneSpecSetLinks(spec, links), "timezone_spec_set_links");
            calls.Check(lib.TimezoneSpecSetLegacy(spec, legacy), "timezone_spec_set_legacy");
            if (_names != null)
            {
                calls.Check(lib.TimezoneSpecSetNames(spec, _names), "timezone_spec_set_names");
            }
            if (source == null)
            {
                return;
            }
            var handle = source.Handle(calls);
            if (!source.IsHost)
            {
                var held = Held(calls, source, handle);
                if (_names == null)
                {
                    calls.Check(lib.TimezoneSpecSetNames(spec, Admitted(calls, held, links, legacy)), "timezone_spec_set_names");
                }
            }
            calls.Check(lib.TimezoneSpecSetSource(spec, handle.Handle), "timezone_spec_set_source");
            calls.Check(lib.TimezoneSpecValidate(spec), "timezone_spec_validate");
            GC.KeepAlive(handle);
            if (_names?.FirstOrDefault(name => source.Table.Resolve(name) == null) is { } missing)
            {
                throw calls.Invalid($"Zones: this system has no TimeZoneInfo for \"{missing}\", so it cannot be drawn");
            }
        });

    /// <summary>
    /// The names of <paramref name="source"/> that have a <see cref="TimeZoneInfo"/>, each with
    /// what the engine says the name is. Only whether .NET has the zone is decided here.
    /// </summary>
    private static List<(string Name, ZoneNameInfo Info)> Held(EngineCalls calls, ZoneSource source, EngineHandle handle)
    {
        var held = new List<(string Name, ZoneNameInfo Info)>();
        foreach (var name in ZoneSource.ReadNames(calls, handle))
        {
            calls.Check(calls.Lib.TimezoneInfoIn(handle.Handle, name, out var info), "timezone_info_in");
            if (info.Known && source.Table.Resolve(name) != null)
            {
                held.Add((name, info));
            }
        }
        GC.KeepAlive(handle);
        if (held.Count == 0)
        {
            throw calls.Invalid("Zones: the zone source has no zone that this system has a TimeZoneInfo for");
        }
        return held;
    }

    /// <summary>The names among <paramref name="held"/> that the link and legacy settings admit.</summary>
    private static List<string> Admitted(EngineCalls calls, List<(string Name, ZoneNameInfo Info)> held, bool links, bool legacy)
    {
        var admitted = held
            .Where(zone => (links || !zone.Info.IsLink) && (legacy || (zone.Info.Tags & Abi.TimezoneTagLegacy) == 0))
            .Select(zone => zone.Name)
            .ToList();
        if (admitted.Count == 0)
        {
            throw calls.Invalid("Zones: the AllowLinks and AllowLegacy settings admit none of the zone source's zones that this system has a TimeZoneInfo for");
        }
        return admitted;
    }

    internal EngineHandle Spec(EngineCalls calls) => _spec.Get(calls.Lib, () => BuildSpec(calls, DrawnFrom, drawing: true));

    /// <summary>
    /// Draw only these names, each exactly as the zone source spells it. <see cref="AllowLinks"/>
    /// and <see cref="AllowLegacy"/> do not apply to an explicit list.
    /// </summary>
    /// <exception cref="ArgumentException">
    /// <paramref name="names"/> is empty; or, when <see cref="Source"/> has named the zone source,
    /// it lacks one of the names or this system has no <see cref="TimeZoneInfo"/> for one.
    /// </exception>
    public ZoneGenerator Names(params string[] names)
    {
        ArgumentNullException.ThrowIfNull(names);
        return new(Array.AsReadOnly((string[])names.Clone()), _links, _legacy, _source, _default);
    }

    /// <summary>Whether link names such as <c>Asia/Calcutta</c>, another name for <c>Asia/Kolkata</c>, are drawn (they are by default).</summary>
    public ZoneGenerator AllowLinks(bool allow) => new(_names, allow, _legacy, _source, _default);

    /// <summary>Whether legacy names such as <c>EST5EDT</c> and <c>US/Eastern</c> are drawn (they are not by default).</summary>
    public ZoneGenerator AllowLegacy(bool allow) => new(_names, _links, allow, _source, _default);

    /// <summary>Draw the zones of <paramref name="source"/> rather than of the default zone source.</summary>
    /// <exception cref="ArgumentException">
    /// <paramref name="source"/> lacks a name given to <see cref="Names"/>, this system has no
    /// <see cref="TimeZoneInfo"/> for one or for any zone of <paramref name="source"/>.
    /// </exception>
    public ZoneGenerator Source(ZoneSource source)
    {
        ArgumentNullException.ThrowIfNull(source);
        return new(_names, _links, _legacy, source, _default);
    }

    /// <inheritdoc />
    protected internal override TimeZoneInfo DoDraw(TestCase tc) =>
        DrawnFrom.Table.Resolve(tc.GenerateTimezone(Spec(EngineCalls.Drawing(tc.Binding()))))!.Value.Zone;
}

/// <summary>
/// Draws date-times in time zones, shrinking toward 2000-01-01T00:00 in UTC. The engine draws the
/// zone, then a wall-clock time in it, and reads that time against the zone source: one draw in
/// twenty on average lands on or beside one of the zone's transitions, which is how skipped and
/// repeated times are reached. See <see cref="Generators.Temporal.ZonedDateTimes"/> for how bad arguments
/// are reported.
/// </summary>
public sealed class ZonedDateTimeGenerator : ValueGenerator<ZonedDateTime>
{
    private sealed record Options(
        Generator<TimeZoneInfo>? Zones,
        ZoneSource? Source,
        ZoneSource Default,
        DateTime? CivilMin,
        DateTime? CivilMax,
        DateTimeOffset? InstantMin,
        DateTimeOffset? InstantMax,
        Precision? Precision,
        Ambiguous? Ambiguous,
        bool? AllowImaginary);

    private readonly Options _settings;
    private readonly ZoneGenerator? _zones;
    private readonly PerEngine<EngineHandle> _spec = new();

    internal ZonedDateTimeGenerator(ZoneSource defaultSource)
    {
        _settings = new Options(null, null, defaultSource, null, null, null, null, null, null, null);
        _zones = new ZoneGenerator(defaultSource);
    }

    private ZonedDateTimeGenerator(Options settings, ZoneGenerator? zones)
    {
        _settings = settings;
        _zones = zones;
        BuildSpec(EngineCalls.Building()).Dispose();
    }

    private ZonedDateTimeGenerator With(Options settings) => new(settings, _zones);

    /// <summary>
    /// The zone generator the engine draws the zone through, or null when the zones come from a
    /// generator the engine cannot see into.
    /// </summary>
    private static ZoneGenerator? EngineZones(Options settings)
    {
        if (settings.Zones is ZoneGenerator given)
        {
            return settings.Source == null
                ? given
                : throw new ArgumentException("ZonedDateTimes: the generator given to Zones came from Generators.Temporal.Zones() and carries its own zone source, so Source cannot name another: call Source on that generator instead");
        }
        if (settings.Zones != null)
        {
            return null;
        }
        var zones = new ZoneGenerator(settings.Default);
        return settings.Source == null ? zones : zones.Source(settings.Source);
    }

    private EngineHandle BuildSpec(EngineCalls calls) => calls.Create(
        "zoned_spec",
        static (ILibhegel lib, out nint handle) => lib.ZonedSpecNew(out handle),
        static (lib, handle) => lib.ZonedSpecFree(handle),
        spec =>
        {
            var lib = calls.Lib;
            var s = _settings;
            var instantBounds = s.InstantMin != null || s.InstantMax != null;
            if (!instantBounds || s.CivilMin != null || s.CivilMax != null)
            {
                calls.Check(lib.ZonedSpecSetCivilBounds(spec, TemporalValues.DateTime(s.CivilMin ?? DateTime.MinValue), TemporalValues.LastDateTime(s.CivilMax ?? DateTime.MaxValue)), "zoned_spec_set_civil_bounds");
            }
            if (instantBounds)
            {
                calls.Check(lib.ZonedSpecSetInstantBounds(spec, TemporalValues.Timestamp(s.InstantMin ?? DateTimeOffset.MinValue), TemporalValues.LastTimestamp(s.InstantMax ?? DateTimeOffset.MaxValue)), "zoned_spec_set_instant_bounds");
            }
            calls.Check(lib.ZonedSpecSetPrecision(spec, (uint)(s.Precision ?? Hegel.Temporal.Precision.Ticks)), "zoned_spec_set_precision");
            calls.Check(lib.ZonedSpecSetImaginary(spec, s.AllowImaginary ?? false ? Abi.ImaginaryAllow : Abi.ImaginaryAvoid), "zoned_spec_set_imaginary");
            calls.Check(lib.ZonedSpecSetAmbiguous(spec, (uint)(s.Ambiguous ?? Hegel.Temporal.Ambiguous.Both)), "zoned_spec_set_ambiguous");
            calls.Check(lib.ZonedSpecValidate(spec), "zoned_spec_validate");
        });

    private static DateTime WallClock(DateTime bound, string name) => bound.Kind == DateTimeKind.Unspecified
        ? bound
        : throw new ArgumentException("ZonedDateTimes: a DateTime bound is a wall-clock time in whichever zone is drawn, so its Kind must be Unspecified; to bound the instant, pass a DateTimeOffset", name);

    /// <summary>
    /// The zones to draw: any generator of them, such as <c>Generators.Temporal.Zones().Names(...)</c> or
    /// <c>Generators.Just(zone)</c>. A generator from <see cref="Generators.Temporal.Zones"/> brings its
    /// zone source with it. A zone from any other generator is looked up in this generator's zone
    /// source by its <see cref="TimeZoneInfo.Id"/> (or, for a Windows id the source has no zone
    /// under, by the IANA name .NET maps it to), and one the source does not have fails the draw
    /// with a <see cref="HegelUsageException"/> naming it.
    /// </summary>
    /// <exception cref="ArgumentException"><paramref name="zones"/> came from <see cref="Generators.Temporal.Zones"/> and <see cref="Source"/> was also called.</exception>
    public ZonedDateTimeGenerator Zones(Generator<TimeZoneInfo> zones)
    {
        ArgumentNullException.ThrowIfNull(zones);
        var settings = _settings with { Zones = zones };
        return new(settings, EngineZones(settings));
    }

    /// <summary>
    /// Read wall-clock times against <paramref name="source"/> rather than the default zone source,
    /// and draw its zones unless <see cref="Zones"/> gives others.
    /// </summary>
    /// <exception cref="ArgumentException">
    /// <see cref="Zones"/> was given a generator from <see cref="Generators.Temporal.Zones"/>, which carries
    /// its own source; or <paramref name="source"/> has no zone to draw.
    /// </exception>
    public ZonedDateTimeGenerator Source(ZoneSource source)
    {
        ArgumentNullException.ThrowIfNull(source);
        var settings = _settings with { Source = source };
        return new(settings, EngineZones(settings));
    }

    /// <summary>A wall-clock time no earlier than <paramref name="min"/>, in whichever zone is drawn.</summary>
    /// <exception cref="ArgumentException">
    /// <paramref name="min"/> is not of <see cref="DateTimeKind.Unspecified"/>; the instant is
    /// bounded too, and only one of the two can be; or the settings given so far allow no value.
    /// </exception>
    public ZonedDateTimeGenerator Min(DateTime min) => With(_settings with { CivilMin = WallClock(min, nameof(min)) });

    /// <summary>A wall-clock time no later than <paramref name="max"/>, in whichever zone is drawn.</summary>
    /// <exception cref="ArgumentException">
    /// <paramref name="max"/> is not of <see cref="DateTimeKind.Unspecified"/>; the instant is
    /// bounded too, and only one of the two can be; or the settings given so far allow no value.
    /// </exception>
    public ZonedDateTimeGenerator Max(DateTime max) => With(_settings with { CivilMax = WallClock(max, nameof(max)) });

    /// <summary>An instant no earlier than <paramref name="min"/>.</summary>
    /// <exception cref="ArgumentException">The wall-clock time is bounded too, and only one of the two can be; or the settings given so far allow no value.</exception>
    public ZonedDateTimeGenerator Min(DateTimeOffset min) => With(_settings with { InstantMin = min });

    /// <summary>An instant no later than <paramref name="max"/>.</summary>
    /// <exception cref="ArgumentException">The wall-clock time is bounded too, and only one of the two can be; or the settings given so far allow no value.</exception>
    public ZonedDateTimeGenerator Max(DateTimeOffset max) => With(_settings with { InstantMax = max });

    /// <summary>Only wall-clock times of day that are whole numbers of <paramref name="precision"/> since midnight, rather than of ticks.</summary>
    /// <exception cref="ArgumentException"><paramref name="precision"/> is not a precision, or the settings given so far allow no value.</exception>
    public ZonedDateTimeGenerator Precision(Precision precision) => With(_settings with { Precision = precision });

    /// <summary>Which instant a repeated wall-clock time means: by default either, shrinking to the earlier.</summary>
    /// <exception cref="ArgumentException"><paramref name="ambiguous"/> is not a policy.</exception>
    public ZonedDateTimeGenerator Ambiguous(Ambiguous ambiguous) => With(_settings with { Ambiguous = ambiguous });

    /// <summary>
    /// Whether wall-clock times the clocks skipped are drawn (they are not by default). One is
    /// reported with <see cref="WallTimeKind.Skipped"/>, the offset in force after the clocks
    /// changed, and the instant its wall clock reads with the offset before.
    /// </summary>
    public ZonedDateTimeGenerator AllowImaginary(bool allow) => With(_settings with { AllowImaginary = allow });

    /// <inheritdoc />
    protected internal override ZonedDateTime DoDraw(TestCase tc)
    {
        var calls = EngineCalls.Drawing(tc.Binding());
        var spec = _spec.Get(calls.Lib, () => BuildSpec(calls));
        HegelZonedDateTime drawn;
        TimeZoneInfo zone;
        bool probed;
        if (_zones != null)
        {
            drawn = tc.GenerateZonedDatetime(_zones.Spec(calls), spec, out var name);
            (zone, probed) = _zones.DrawnFrom.Table.Resolve(name)!.Value;
        }
        else
        {
            zone = tc.DrawSilent(_settings.Zones!);
            var source = _settings.Source ?? _settings.Default;
            var name = source.Table.NameOf(zone);
            drawn = tc.GenerateZonedDatetimeForZone(source.Handle(calls), name, spec);
            probed = source.Table.Resolve(name) is { Probed: true };
        }
        if (!TemporalValues.Representable(drawn))
        {
            tc.Reject();
        }
        var value = TemporalValues.Zoned(drawn, zone);
        if (probed && zone.GetUtcOffset(value.Instant) != value.Offset)
        {
            tc.Reject();
        }
        return value;
    }
}
