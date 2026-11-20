using System.Buffers.Binary;
using System.Text;
using Xunit;

namespace Hegel.Tests.Support;

/// <summary>Time zones and zone data that are the same on every machine.</summary>
internal static class TestZones
{
    internal static readonly TimeSpan Standard = TimeSpan.FromHours(-5);
    internal static readonly TimeSpan Daylight = TimeSpan.FromHours(-4);

    /// <summary>The wall-clock hour of 2024 that Springfield's clocks skipped, from its start to its end.</summary>
    internal static readonly DateTime GapStart = new(2024, 3, 10, 2, 0, 0);
    internal static readonly DateTime GapEnd = new(2024, 3, 10, 3, 0, 0);

    /// <summary>The wall-clock hour of 2024 that Springfield's clocks repeated.</summary>
    internal static readonly DateTime FoldStart = new(2024, 11, 3, 1, 0, 0);
    internal static readonly DateTime FoldEnd = new(2024, 11, 3, 2, 0, 0);

    /// <summary>
    /// A zone five hours behind UTC that moves its clocks forward at 02:00 on the second Sunday of
    /// March and back at 02:00 on the first Sunday of November, every year.
    /// </summary>
    internal static TimeZoneInfo Springfield(string id = "Test/Springfield")
    {
        var rule = TimeZoneInfo.AdjustmentRule.CreateAdjustmentRule(
            DateTime.MinValue.Date,
            DateTime.MaxValue.Date,
            TimeSpan.FromHours(1),
            TimeZoneInfo.TransitionTime.CreateFloatingDateRule(new DateTime(1, 1, 1, 2, 0, 0), 3, 2, DayOfWeek.Sunday),
            TimeZoneInfo.TransitionTime.CreateFloatingDateRule(new DateTime(1, 1, 1, 2, 0, 0), 11, 1, DayOfWeek.Sunday));
        return TimeZoneInfo.CreateCustomTimeZone(id, Standard, id, "STD", "DST", [rule]);
    }

    internal static TimeZoneInfo Fixed(string id, TimeSpan offset) => TimeZoneInfo.CreateCustomTimeZone(id, offset, id, id);

    /// <summary>
    /// A TZif file (version 2) of a zone with no recorded transitions: <paramref name="offset"/>
    /// until its footer's POSIX rule <paramref name="rule"/> takes over.
    /// </summary>
    internal static byte[] Tzif(TimeSpan offset, string abbreviation, string rule)
    {
        var name = Encoding.ASCII.GetBytes(abbreviation + "\0");
        var block = new byte[44 + 6 + name.Length];
        Encoding.ASCII.GetBytes("TZif2").CopyTo(block, 0);
        BinaryPrimitives.WriteInt32BigEndian(block.AsSpan(36), 1);
        BinaryPrimitives.WriteInt32BigEndian(block.AsSpan(40), name.Length);
        BinaryPrimitives.WriteInt32BigEndian(block.AsSpan(44), (int)offset.TotalSeconds);
        name.CopyTo(block, 50);
        return [.. block, .. block, .. Encoding.ASCII.GetBytes($"\n{rule}\n")];
    }

    /// <summary>Springfield's rules as a TZif file.</summary>
    internal static byte[] SpringfieldTzif() => Tzif(Standard, "STD", "STD5DST,M3.2.0,M11.1.0");

    internal static IEnumerable<T[]> Orders<T>(T[] items)
    {
        if (items.Length <= 1)
        {
            yield return items;
            yield break;
        }
        for (var first = 0; first < items.Length; first++)
        {
            T[] rest = [.. items[..first], .. items[(first + 1)..]];
            foreach (var order in Orders(rest))
            {
                yield return [items[first], .. order];
            }
        }
    }

    /// <summary>
    /// Every order of <paramref name="steps"/> throws an <see cref="ArgumentException"/> while the
    /// generator is being built, with one message, which is returned.
    /// </summary>
    internal static string RejectedWhenBuilt<TGenerator>(TGenerator start, params Func<TGenerator, TGenerator>[] steps)
    {
        string? message = null;
        foreach (var order in Orders(steps))
        {
            var e = Assert.Throws<ArgumentException>(() => order.Aggregate(start, (generator, step) => step(generator)));
            message ??= e.Message;
            Assert.Equal(message, e.Message);
        }
        return message!;
    }

    /// <summary>
    /// Every order of <paramref name="steps"/> builds, and <paramref name="draw"/> then fails with a
    /// <see cref="HegelUsageException"/>, with one message, which is returned.
    /// </summary>
    internal static string RejectedWhenDrawn<TGenerator>(TGenerator start, Action<TGenerator> draw, params Func<TGenerator, TGenerator>[] steps)
    {
        string? message = null;
        foreach (var order in Orders(steps))
        {
            var generator = order.Aggregate(start, (g, step) => step(g));
            var e = Assert.Throws<HegelUsageException>(() => draw(generator));
            message ??= e.Message;
            Assert.Equal(message, e.Message);
        }
        return message!;
    }

    /// <summary>Every order of <paramref name="steps"/> builds a generator that passes <paramref name="check"/>.</summary>
    internal static void AcceptedInAnyOrder<TGenerator>(TGenerator start, Action<TGenerator> check, params Func<TGenerator, TGenerator>[] steps)
    {
        foreach (var order in Orders(steps))
        {
            check(order.Aggregate(start, (g, step) => step(g)));
        }
    }
}
