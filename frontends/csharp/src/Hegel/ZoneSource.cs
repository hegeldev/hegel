using System.Collections.Concurrent;
using Hegel.Native;

namespace Hegel;

/// <summary>The offset, daylight-saving flag and abbreviation a <see cref="ZoneOracle"/> reports for an instant.</summary>
/// <param name="Offset">The UTC offset in force, a whole number of seconds less than two days in size.</param>
/// <param name="IsDaylightSaving">Whether daylight saving time is in force. Informational only.</param>
/// <param name="Abbreviation">The abbreviation in force, such as <c>"EST"</c>. Informational only.</param>
public readonly record struct ZoneLocalType(TimeSpan Offset, bool IsDaylightSaving, string Abbreviation);

/// <summary>
/// A time zone described by a function from an instant to the offset in force, for a library that
/// answers lookups but does not expose its zone data. The engine probes the function over a range
/// of years to find the zone's transitions, when the zone is added to a <see cref="ZoneSource"/>.
/// </summary>
public sealed class ZoneOracle
{
    /// <summary>An oracle probed from the start of <paramref name="firstYear"/> to the end of <paramref name="lastYear"/>.</summary>
    /// <param name="typeAt">The offset in force at an instant. Before <paramref name="firstYear"/> the zone keeps the answer for that year's start.</param>
    /// <param name="firstYear">The first year probed.</param>
    /// <param name="lastYear">The last year probed.</param>
    public ZoneOracle(Func<DateTimeOffset, ZoneLocalType> typeAt, int firstYear, int lastYear)
    {
        ArgumentNullException.ThrowIfNull(typeAt);
        TypeAt = typeAt;
        FirstYear = firstYear;
        LastYear = lastYear;
    }

    /// <summary>The offset in force at an instant.</summary>
    public Func<DateTimeOffset, ZoneLocalType> TypeAt { get; }

    /// <summary>The first year probed.</summary>
    public int FirstYear { get; }

    /// <summary>The last year probed.</summary>
    public int LastYear { get; }

    /// <summary>
    /// The first instant after the one given at which the answer of <see cref="TypeAt"/> changes,
    /// or null when it never changes again. Without it the engine samples <see cref="TypeAt"/>
    /// once a day and bisects each change to the second, which misses two changes less than a day
    /// apart.
    /// </summary>
    public Func<DateTimeOffset, DateTimeOffset?>? NextTransition { get; init; }

    /// <summary>What the zone does after <see cref="LastYear"/>: by default it keeps its last offset.</summary>
    public ZoneBeyond Beyond { get; init; } = ZoneBeyond.Hold;
}

/// <summary>
/// Where a zone source's data goes: what <see cref="ZoneSourceBuilder.Loader"/>'s function is
/// given to supply the zone it was asked for.
/// </summary>
public interface IZoneData
{
    /// <summary>Add a zone from the contents of a TZif file (RFC 9636, version 2 or later), such as one under <c>/usr/share/zoneinfo</c>.</summary>
    /// <param name="name">The name the zone is drawn by.</param>
    /// <param name="tzif">The file's contents.</param>
    void AddTzif(string name, byte[] tzif);

    /// <summary>Add a zone that behaves as <paramref name="zone"/> does, under its <see cref="TimeZoneInfo.Id"/>, which is also the object a draw of that name yields.</summary>
    /// <param name="zone">The zone.</param>
    void AddTimeZone(TimeZoneInfo zone);

    /// <summary>Add a zone by probing <paramref name="oracle"/>.</summary>
    /// <param name="name">The name the zone is drawn by.</param>
    /// <param name="oracle">The function to probe and the years to probe it over.</param>
    void AddOracle(string name, ZoneOracle oracle);

    /// <summary>Make <paramref name="name"/> another name for the zone <paramref name="target"/>.</summary>
    /// <param name="name">The link name.</param>
    /// <param name="target">The name of the zone whose data applies.</param>
    void AddLink(string name, string target);
}

/// <summary>
/// The <see cref="TimeZoneInfo"/> behind each name of a zone source: the object a zone was added
/// from, or else the one this system has under that name.
/// </summary>
internal sealed class ZoneTable
{
    private readonly bool _host;
    private readonly ConcurrentDictionary<string, TimeZoneInfo> _added = new(StringComparer.Ordinal);
    private readonly ConcurrentDictionary<string, string> _links = new(StringComparer.Ordinal);

    internal ZoneTable(bool host)
    {
        _host = host;
    }

    /// <summary>A zone and whether the engine's data for its name was probed from that very object's rules.</summary>
    internal readonly record struct Entry(TimeZoneInfo Zone, bool Probed);

    internal void Add(TimeZoneInfo zone) => _added[zone.Id] = zone;

    internal void Link(string name, string target) => _links[name] = target;

    internal bool AnyAdded => !_added.IsEmpty;

    /// <summary>The zone whose rules supply the data of <paramref name="name"/> when the engine first asks for it, if any.</summary>
    internal TimeZoneInfo? Supplier(string name)
    {
        if (_added.TryGetValue(name, out var added))
        {
            return added;
        }
        return _host && TimeZoneInfo.TryFindSystemTimeZoneById(name, out var system) ? system : null;
    }

    internal Entry? Resolve(string name)
    {
        if (_added.TryGetValue(name, out var added))
        {
            return new Entry(added, true);
        }
        if (_links.TryGetValue(name, out var target) && _added.TryGetValue(target, out var linked))
        {
            return new Entry(linked, true);
        }
        return TimeZoneInfo.TryFindSystemTimeZoneById(name, out var system) ? new Entry(system, _host) : null;
    }

    /// <summary>
    /// The name the source knows <paramref name="zone"/> by: its <see cref="TimeZoneInfo.Id"/>, or
    /// for a Windows zone id that no zone was added under, the IANA name .NET maps it to.
    /// </summary>
    internal string NameOf(TimeZoneInfo zone) =>
        !_added.ContainsKey(zone.Id) && !zone.HasIanaId && TimeZoneInfo.TryConvertWindowsIdToIanaId(zone.Id, out var iana) ? iana : zone.Id;
}

/// <summary>Writes zone data into one engine zone source.</summary>
internal sealed class ZoneFiller : IZoneData
{
    private const int HostFirstYear = 1800;
    private const int HostLastYear = 2100;

    private readonly EngineCalls _calls;
    private readonly nint _source;
    private readonly ZoneTable _table;

    internal ZoneFiller(EngineCalls calls, nint source, ZoneTable table)
    {
        _calls = calls;
        _source = source;
        _table = table;
    }

    public void AddTzif(string name, byte[] tzif)
    {
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(tzif);
        _calls.Check(_calls.Lib.ZoneSourceAddTzif(_source, name, tzif), "zone_source_add_tzif");
    }

    public void AddTimeZone(TimeZoneInfo zone)
    {
        ArgumentNullException.ThrowIfNull(zone);
        _table.Add(zone);
        Probe(zone.Id, zone);
    }

    /// <summary>
    /// Add <paramref name="name"/> with the offsets <paramref name="zone"/> reports. .NET exposes a
    /// zone's rules only as lookups, so the engine probes them, over the years a
    /// <see cref="TimeZoneInfo"/>'s rules are worth asking about.
    /// </summary>
    internal void Probe(string name, TimeZoneInfo zone)
    {
        OracleType At(long second)
        {
            var instant = DateTimeOffset.FromUnixTimeSeconds(second);
            return new OracleType(TemporalValues.OffsetSeconds(zone.GetUtcOffset(instant)), zone.IsDaylightSavingTime(instant), "");
        }
        _calls.Check(_calls.Lib.ZoneSourceAddOracle(_source, name, At, null, HostFirstYear, HostLastYear, Abi.ZoneAfterInfer), "zone_source_add_oracle");
    }

    public void AddOracle(string name, ZoneOracle oracle)
    {
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(oracle);
        OracleType At(long second)
        {
            var type = oracle.TypeAt(DateTimeOffset.FromUnixTimeSeconds(second));
            return new OracleType(TemporalValues.OffsetSeconds(type.Offset), type.IsDaylightSaving, type.Abbreviation ?? "");
        }
        var next = oracle.NextTransition;
        Func<long, long?>? following = next == null ? null : second => next(DateTimeOffset.FromUnixTimeSeconds(second))?.ToUnixTimeSeconds();
        _calls.Check(_calls.Lib.ZoneSourceAddOracle(_source, name, At, following, oracle.FirstYear, oracle.LastYear, (uint)oracle.Beyond), "zone_source_add_oracle");
    }

    public void AddLink(string name, string target)
    {
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(target);
        _table.Link(name, target);
        _calls.Check(_calls.Lib.ZoneSourceAddLink(_source, name, target), "zone_source_add_link");
    }
}

/// <summary>
/// The time zone data that <see cref="Generators.Zones"/> and <see cref="Generators.ZonedDateTimes"/>
/// draw from: which zones exist, and when each one's offset changes. It should be the data the code
/// under test uses, so that a drawn wall-clock time is skipped or repeated for that code too.
/// </summary>
/// <remarks>
/// <para>
/// <see cref="Host"/>, the default, is this system's zones as <see cref="TimeZoneInfo"/> sees
/// them; <see cref="Bundled"/> is the IANA database compiled into the engine, the same on every
/// machine; <see cref="FromTimeZones(TimeZoneInfo[])"/> and <see cref="CreateBuilder"/> make a
/// source from zones of your own. Use one for a generator with <see cref="ZoneGenerator.Source"/>
/// or <see cref="ZonedDateTimeGenerator.Source"/>, or for every generator built from then on with
/// <see cref="SetDefault"/>.
/// </para>
/// <para>
/// A draw yields a <see cref="TimeZoneInfo"/>, so only names that have one are drawn: the object a
/// zone was added from (<see cref="FromTimeZones(TimeZoneInfo[])"/>, <see cref="IZoneData.AddTimeZone"/>),
/// or else the zone this system has under that name. For a zone added from a
/// <see cref="TimeZoneInfo"/>, which includes every zone of <see cref="Host"/>, a zoned date-time
/// whose offset the object does not agree with is rejected rather than drawn. For a zone added any
/// other way nothing compares the source's data with this system's zone of the same name: the
/// drawn <see cref="ZonedDateTime"/> reports the source's reading.
/// </para>
/// </remarks>
public sealed class ZoneSource
{
    private enum Kind
    {
        Bundled,
        Host,
        Custom,
    }

    private static readonly ZoneSource HostSource = new(Kind.Host, [], [], null, null, new ZoneTable(true));
    private static readonly ZoneSource BundledSource = new(Kind.Bundled, [], [], null, null, new ZoneTable(false));
    private static ZoneSource _default = HostSource;

    private readonly Kind _kind;
    private readonly IReadOnlyList<Action<IZoneData>> _steps;
    private readonly IReadOnlyList<string> _declared;
    private readonly Action<string, IZoneData>? _loader;
    private readonly string? _version;
    private readonly PerEngine<EngineHandle> _handles = new();

    private ZoneSource(Kind kind, IReadOnlyList<Action<IZoneData>> steps, IReadOnlyList<string> declared, Action<string, IZoneData>? loader, string? version, ZoneTable table)
    {
        _kind = kind;
        _steps = steps;
        _declared = declared;
        _loader = loader;
        _version = version;
        Table = table;
    }

    internal static ZoneSource Custom(IReadOnlyList<Action<IZoneData>> steps, IReadOnlyList<TimeZoneInfo> zones, IReadOnlyList<string> declared, Action<string, IZoneData>? loader, string? version)
    {
        var table = new ZoneTable(false);
        foreach (var zone in zones)
        {
            table.Add(zone);
        }
        var source = new ZoneSource(Kind.Custom, steps, [.. declared, .. zones.Select(zone => zone.Id)], loader, version, table);
        source.Handle(EngineCalls.Building());
        return source;
    }

    /// <summary>
    /// This system's time zones, as <see cref="TimeZoneInfo"/> reports them: every name of the
    /// IANA database that <see cref="TimeZoneInfo.TryFindSystemTimeZoneById(string, out TimeZoneInfo)"/>
    /// finds, each with the offsets that <see cref="TimeZoneInfo"/> gives for it. The default.
    /// </summary>
    public static ZoneSource Host => HostSource;

    /// <summary>
    /// The IANA time zone database compiled into the engine, the same on every machine. A name of
    /// it that this system has no <see cref="TimeZoneInfo"/> for is not drawn, and nothing checks
    /// that this system's zones agree with it.
    /// </summary>
    public static ZoneSource Bundled => BundledSource;

    /// <summary>The source a generator uses unless it is given one: <see cref="Host"/> until <see cref="SetDefault"/> says otherwise.</summary>
    public static ZoneSource Default => Volatile.Read(ref _default);

    /// <summary>
    /// Make <paramref name="source"/> the default for the whole process. A generator reads the
    /// default when it is created, so those that already exist keep the source they have.
    /// </summary>
    /// <param name="source">The new default.</param>
    /// <returns>The previous default, to restore afterwards.</returns>
    public static ZoneSource SetDefault(ZoneSource source)
    {
        ArgumentNullException.ThrowIfNull(source);
        return Interlocked.Exchange(ref _default, source);
    }

    /// <summary>A source of exactly <paramref name="zones"/>, each drawn under its <see cref="TimeZoneInfo.Id"/> and behaving as the object does.</summary>
    /// <param name="zones">The zones.</param>
    public static ZoneSource FromTimeZones(params TimeZoneInfo[] zones) => FromTimeZones((IEnumerable<TimeZoneInfo>)zones);

    /// <inheritdoc cref="FromTimeZones(TimeZoneInfo[])"/>
    public static ZoneSource FromTimeZones(IEnumerable<TimeZoneInfo> zones)
    {
        ArgumentNullException.ThrowIfNull(zones);
        var builder = new ZoneSourceBuilder();
        foreach (var zone in zones)
        {
            builder.AddTimeZone(zone);
        }
        return builder.Build();
    }

    /// <summary>A builder for a source of zones given as TZif files, <see cref="TimeZoneInfo"/> objects, oracles and links.</summary>
    public static ZoneSourceBuilder CreateBuilder() => new();

    /// <summary>
    /// Which release of its database the source holds, such as <c>2025b</c>: the engine's for
    /// <see cref="Bundled"/>, what <see cref="ZoneSourceBuilder.Version"/> recorded for a built
    /// source, and otherwise empty.
    /// </summary>
    public string Version
    {
        get
        {
            var calls = EngineCalls.Building();
            var handle = Handle(calls);
            calls.Check(calls.Lib.ZoneSourceVersion(handle.Handle, out var version), "zone_source_version");
            GC.KeepAlive(handle);
            return version;
        }
    }

    /// <summary>
    /// Every name the source knows, sorted, links and legacy names included, whether or not this
    /// system has a <see cref="TimeZoneInfo"/> for it.
    /// </summary>
    public IReadOnlyList<string> Names
    {
        get
        {
            var calls = EngineCalls.Building();
            return ReadNames(calls, Handle(calls));
        }
    }

    internal ZoneTable Table { get; }

    internal bool IsHost => _kind == Kind.Host;

    internal static IReadOnlyList<string> ReadNames(EngineCalls calls, EngineHandle source)
    {
        var lib = calls.Lib;
        calls.Check(lib.ZoneSourceNameCount(source.Handle, out var count), "zone_source_name_count");
        var names = new string[count];
        for (var index = 0UL; index < count; index++)
        {
            calls.Check(lib.ZoneSourceName(source.Handle, index, out names[index]), "zone_source_name");
        }
        GC.KeepAlive(source);
        return names;
    }

    /// <summary>The engine's source for <paramref name="calls"/>' binding, built on first use.</summary>
    internal EngineHandle Handle(EngineCalls calls) => _handles.Get(calls.Lib, () => Build(calls));

    private EngineHandle Build(EngineCalls calls)
    {
        var lib = calls.Lib;
        if (_kind == Kind.Bundled)
        {
            return calls.Create("zone_source", static (ILibhegel l, out nint h) => l.ZoneSourceBundled(out h), static (l, h) => l.ZoneSourceFree(h), _ => { });
        }
        return calls.Create("zone_source", static (ILibhegel l, out nint h) => l.ZoneSourceNew(out h), static (l, h) => l.ZoneSourceFree(h), source =>
        {
            var filler = new ZoneFiller(calls, source, Table);
            foreach (var step in _steps)
            {
                step(filler);
            }
            var declared = _kind == Kind.Host
                ? ReadNames(calls, BundledSource.Handle(calls)).Where(name => Table.Supplier(name) != null).ToArray()
                : _declared;
            if (declared.Count > 0)
            {
                calls.Check(lib.ZoneSourceDeclare(source, declared), "zone_source_declare");
            }
            if (_kind == Kind.Host || Table.AnyAdded || _loader != null)
            {
                calls.Check(lib.ZoneSourceSetLoader(source, Loader(lib, Table, _loader)), "zone_source_set_loader");
            }
            if (_version != null)
            {
                calls.Check(lib.ZoneSourceSetVersion(source, _version), "zone_source_set_version");
            }
        });
    }

    /// <summary>
    /// The function the engine calls for the data of a name it needs. The engine keeps it for as
    /// long as its source lives, so it holds the table and the user's loader but not this object,
    /// whose collection is what frees the engine's source.
    /// </summary>
    private static Action<nint, string> Loader(ILibhegel lib, ZoneTable table, Action<string, IZoneData>? loader) => (source, name) =>
    {
        var filler = new ZoneFiller(EngineCalls.Drawing(lib), source, table);
        if (table.Supplier(name) is { } zone)
        {
            filler.Probe(name, zone);
        }
        else
        {
            loader?.Invoke(name, filler);
        }
    };
}

/// <summary>
/// Collects the zones of a <see cref="ZoneSource"/>; <see cref="Build"/> makes the source. Each
/// method returns the builder.
/// </summary>
public sealed class ZoneSourceBuilder
{
    private readonly List<Action<IZoneData>> _steps = [];
    private readonly List<TimeZoneInfo> _zones = [];
    private readonly List<string> _declared = [];
    private Action<string, IZoneData>? _loader;
    private string? _version;

    internal ZoneSourceBuilder()
    {
    }

    /// <inheritdoc cref="IZoneData.AddTzif"/>
    public ZoneSourceBuilder AddTzif(string name, byte[] tzif)
    {
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(tzif);
        var copy = (byte[])tzif.Clone();
        _steps.Add(data => data.AddTzif(name, copy));
        return this;
    }

    /// <summary>
    /// Add a zone that behaves as <paramref name="zone"/> does, under its <see cref="TimeZoneInfo.Id"/>,
    /// which is also the object a draw of that name yields. Its offsets are read from the object
    /// the first time a draw needs them.
    /// </summary>
    /// <param name="zone">The zone.</param>
    public ZoneSourceBuilder AddTimeZone(TimeZoneInfo zone)
    {
        ArgumentNullException.ThrowIfNull(zone);
        _zones.Add(zone);
        return this;
    }

    /// <inheritdoc cref="IZoneData.AddOracle"/>
    public ZoneSourceBuilder AddOracle(string name, ZoneOracle oracle)
    {
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(oracle);
        _steps.Add(data => data.AddOracle(name, oracle));
        return this;
    }

    /// <inheritdoc cref="IZoneData.AddLink"/>
    public ZoneSourceBuilder AddLink(string name, string target)
    {
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(target);
        _steps.Add(data => data.AddLink(name, target));
        return this;
    }

    /// <summary>Name zones the source has whose data <see cref="Loader"/>'s function supplies when a draw first needs it.</summary>
    /// <param name="names">The names.</param>
    public ZoneSourceBuilder Declare(params string[] names)
    {
        ArgumentNullException.ThrowIfNull(names);
        _declared.AddRange(names);
        return this;
    }

    /// <summary>
    /// Supply zone data on demand: <paramref name="loader"/> is called with a name the source
    /// declared, or one a generator was told to draw, the first time its data is needed, and adds
    /// that zone to the <see cref="IZoneData"/> it is given, or adds nothing for a name it does not
    /// know. It may be called from any thread, and more than once for a name. An exception it
    /// throws fails the draw that needed the data, with the exception's message.
    /// </summary>
    /// <param name="loader">The function, given the name and where to add its data.</param>
    public ZoneSourceBuilder Loader(Action<string, IZoneData> loader)
    {
        ArgumentNullException.ThrowIfNull(loader);
        _loader = loader;
        return this;
    }

    /// <summary>Record which release of its database the source holds, such as <c>2025b</c>.</summary>
    /// <param name="version">The release.</param>
    public ZoneSourceBuilder Version(string version)
    {
        ArgumentNullException.ThrowIfNull(version);
        _version = version;
        return this;
    }

    /// <summary>The source. Later calls on the builder do not change it.</summary>
    /// <exception cref="ArgumentException">The engine rejected a zone: a malformed TZif file, a bad name, an oracle that failed.</exception>
    public ZoneSource Build() => ZoneSource.Custom([.. _steps], [.. _zones], [.. _declared], _loader, _version);
}
