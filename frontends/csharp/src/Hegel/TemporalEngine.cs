using System.Runtime.CompilerServices;
using Hegel.Native;
using Hegel.Temporal;

namespace Hegel;

/// <summary>
/// An engine object the frontend owns (a spec, a set of options or a zone source), freed when
/// disposed or, failing that, when collected. Whoever passes <see cref="Handle"/> to the engine
/// keeps this object reachable until the call returns.
/// </summary>
internal sealed class EngineHandle : IDisposable
{
    private readonly Action<ILibhegel, nint> _free;

    internal EngineHandle(ILibhegel lib, nint handle, Action<ILibhegel, nint> free)
    {
        Lib = lib;
        Handle = handle;
        _free = free;
    }

    internal ILibhegel Lib { get; }

    internal nint Handle { get; }

    public void Dispose()
    {
        _free(Lib, Handle);
        GC.SuppressFinalize(this);
    }

    ~EngineHandle()
    {
        _free(Lib, Handle);
    }
}

/// <summary>One lazily built value per engine binding, so that a handle is only ever used with the binding that made it.</summary>
internal sealed class PerEngine<T>
    where T : class
{
    private readonly ConditionalWeakTable<ILibhegel, T> _values = new();

    internal T Get(ILibhegel lib, Func<T> build) => _values.GetValue(lib, _ => build());
}

/// <summary>
/// Engine calls made on a generator's behalf, either while it is being built or during a draw: the
/// engine judges the arguments, and what it rejects is an <see cref="ArgumentException"/> in the
/// first case and a <see cref="HegelUsageException"/> in the second, with the engine's message.
/// </summary>
internal readonly struct EngineCalls
{
    private readonly bool _drawing;

    private EngineCalls(ILibhegel lib, bool drawing)
    {
        Lib = lib;
        _drawing = drawing;
    }

    internal ILibhegel Lib { get; }

    internal static EngineCalls Building() => new(Engine.Get(), false);

    internal static EngineCalls Drawing(ILibhegel lib) => new(lib, true);

    internal Exception Invalid(string message) => _drawing ? new HegelUsageException(message) : new ArgumentException(message);

    internal void Check(int rc, string op)
    {
        if (rc == Abi.Ok)
        {
            return;
        }
        var message = Lib.LastErrorMessage();
        if (rc == Abi.EInvalidArg)
        {
            throw Invalid(message);
        }
        throw new HegelException($"hegel_{op} failed (rc={rc}): {message}");
    }

    internal delegate int Constructor(ILibhegel lib, out nint handle);

    /// <summary>Create an engine object and fill it in, freeing it again if filling it fails.</summary>
    internal EngineHandle Create(string kind, Constructor create, Action<ILibhegel, nint> free, Action<nint> fill)
    {
        Check(create(Lib, out var handle), kind + "_new");
        var owned = new EngineHandle(Lib, handle, free);
        try
        {
            fill(handle);
            return owned;
        }
        catch
        {
            owned.Dispose();
            throw;
        }
    }
}

/// <summary>
/// Conversions between .NET's temporal values and the engine's. .NET counts in ticks of 100
/// nanoseconds and the engine in nanoseconds: a lower bound becomes its tick's first nanosecond,
/// an upper bound its tick's last, and a drawn value is rounded down to the tick holding it.
/// </summary>
internal static class TemporalValues
{
    private const long TicksPerSecond = TimeSpan.TicksPerSecond;
    private const int NanosecondsPerTick = 100;
    private const int LastNanosecondOfTick = NanosecondsPerTick - 1;
    private const long EpochSeconds = 62135596800;
    private const long LastSecond = 315537897599 - EpochSeconds;

    internal static HegelDate Date(DateOnly date) => new() { Year = date.Year, Month = (byte)date.Month, Day = (byte)date.Day };

    internal static DateOnly Date(HegelDate date) => new(date.Year, date.Month, date.Day);

    internal static HegelTime Time(TimeOnly time) => new()
    {
        Hour = (byte)time.Hour,
        Minute = (byte)time.Minute,
        Second = (byte)time.Second,
        Nanosecond = (uint)(time.Ticks % TicksPerSecond * NanosecondsPerTick),
    };

    internal static HegelTime LastTime(TimeOnly time)
    {
        var native = Time(time);
        native.Nanosecond += LastNanosecondOfTick;
        return native;
    }

    internal static TimeOnly Time(HegelTime time) =>
        new(((time.Hour * 60L + time.Minute) * 60 + time.Second) * TicksPerSecond + time.Nanosecond / NanosecondsPerTick);

    internal static HegelDateTime DateTime(DateTime value) => new() { Date = Date(DateOnly.FromDateTime(value)), Time = Time(TimeOnly.FromDateTime(value)) };

    internal static HegelDateTime LastDateTime(DateTime value) => new() { Date = Date(DateOnly.FromDateTime(value)), Time = LastTime(TimeOnly.FromDateTime(value)) };

    internal static DateTime DateTime(HegelDateTime value) => Date(value.Date).ToDateTime(Time(value.Time), DateTimeKind.Unspecified);

    internal static HegelTimestamp Timestamp(DateTimeOffset instant) => new()
    {
        Second = instant.UtcTicks / TicksPerSecond - EpochSeconds,
        Nanosecond = (uint)(instant.UtcTicks % TicksPerSecond * NanosecondsPerTick),
    };

    internal static HegelTimestamp LastTimestamp(DateTimeOffset instant)
    {
        var native = Timestamp(instant);
        native.Nanosecond += LastNanosecondOfTick;
        return native;
    }

    internal static DateTimeOffset Instant(HegelTimestamp timestamp) =>
        new((timestamp.Second + EpochSeconds) * TicksPerSecond + timestamp.Nanosecond / NanosecondsPerTick, TimeSpan.Zero);

    internal static HegelDuration Duration(TimeSpan duration) => new()
    {
        Seconds = duration.Ticks / TicksPerSecond,
        Nanoseconds = (int)(duration.Ticks % TicksPerSecond) * NanosecondsPerTick,
    };

    internal static HegelDuration LastDuration(TimeSpan duration)
    {
        if (duration.Ticks >= 0)
        {
            var native = Duration(duration);
            native.Nanoseconds += LastNanosecondOfTick;
            return native;
        }
        var next = Duration(new TimeSpan(duration.Ticks + 1));
        next.Nanoseconds -= 1;
        return next;
    }

    internal static TimeSpan Duration(HegelDuration duration) =>
        new(duration.Seconds * TicksPerSecond + (long)Math.Floor(duration.Nanoseconds / (double)NanosecondsPerTick));

    internal static int OffsetSeconds(TimeSpan offset) => (int)Math.Clamp(offset.Ticks / TicksPerSecond, int.MinValue, int.MaxValue);

    internal static TimeSpan Offset(int seconds) => new(seconds * TicksPerSecond);

    internal static HegelSpan Span(CalendarSpan span) => new()
    {
        Years = span.Years,
        Months = span.Months,
        Weeks = span.Weeks,
        Days = span.Days,
        Hours = span.Hours,
        Minutes = span.Minutes,
        Seconds = span.Seconds,
        Milliseconds = span.Milliseconds,
        Microseconds = span.Microseconds,
        Nanoseconds = span.Nanoseconds,
    };

    internal static CalendarSpan Span(HegelSpan span) => new()
    {
        Years = span.Years,
        Months = span.Months,
        Weeks = span.Weeks,
        Days = span.Days,
        Hours = span.Hours,
        Minutes = span.Minutes,
        Seconds = span.Seconds,
        Milliseconds = span.Milliseconds,
        Microseconds = span.Microseconds,
        Nanoseconds = span.Nanoseconds,
    };

    internal static HegelZonedDateTime Zoned(ZonedDateTime value)
    {
        var offset = OffsetSeconds(value.Offset);
        return new HegelZonedDateTime
        {
            Civil = DateTime(value.LocalDateTime),
            OffsetSeconds = offset,
            OffsetKnown = 1,
            Fold = value.Kind == WallTimeKind.Later ? (byte)1 : (byte)0,
            Kind = value.Kind switch
            {
                WallTimeKind.Skipped => Abi.TimezoneKindGap,
                WallTimeKind.Earlier or WallTimeKind.Later => Abi.TimezoneKindFold,
                _ => Abi.TimezoneKindUnique,
            },
            OffsetBefore = offset,
            OffsetAfter = offset,
            Timestamp = Timestamp(value.Instant),
        };
    }

    /// <summary>Whether .NET can hold a drawn value: a known offset, and a wall-clock year and an instant both within the years 1 to 9999.</summary>
    internal static bool Representable(HegelZonedDateTime value) =>
        value.OffsetKnown != 0 && value.Civil.Date.Year is >= 1 and <= 9999 && value.Timestamp.Second is >= -EpochSeconds and <= LastSecond;

    internal static ZonedDateTime Zoned(HegelZonedDateTime value, TimeZoneInfo zone) => new(
        DateTime(value.Civil),
        Offset(value.OffsetSeconds),
        zone,
        Instant(value.Timestamp),
        value.Kind switch
        {
            Abi.TimezoneKindGap => WallTimeKind.Skipped,
            Abi.TimezoneKindFold => value.Fold != 0 ? WallTimeKind.Later : WallTimeKind.Earlier,
            _ => WallTimeKind.Unique,
        });

    /// <summary>The bit of a mask at <paramref name="index"/>, or one no mask of the engine's has when the index is out of range, for the engine to reject.</summary>
    private static uint Bit(int index) => (uint)index < 31 ? 1u << index : 1u << 31;

    /// <summary>The engine's weekday mask, whose bit 0 is Monday.</summary>
    internal static uint Weekdays(IEnumerable<DayOfWeek> days) =>
        days.Aggregate(0u, (mask, day) => mask | Bit(day == DayOfWeek.Sunday ? 6 : (int)day - 1));

    /// <summary>The engine's month mask, whose bit 0 is January.</summary>
    internal static uint Months(IEnumerable<int> months) =>
        months.Aggregate(0u, (mask, month) => mask | Bit(month - 1));
}
