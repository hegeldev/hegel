namespace Hegel;

/// <summary>
/// Span labels: how the engine tells one kind of draw from another when it shrinks. A generator's
/// <see cref="Generator{T}.Label"/> is one of these, <see cref="Of"/> a name, or a <see cref="Combine"/>d label.
/// </summary>
public static class Label
{
    /// <summary>A list.</summary>
    public const ulong List = 0x4ccf178a8c755675;
    /// <summary>A set.</summary>
    public const ulong Set = 0x8c7bf6e4c7f30665;
    /// <summary>A map.</summary>
    public const ulong Map = 0x68e71e450ea5be6d;
    /// <summary>A tuple.</summary>
    public const ulong Tuple = 0xd8fa223d51dda37b;
    /// <summary>A choice between generators.</summary>
    public const ulong OneOf = 0xf96229f21f87bc20;
    /// <summary>An optional value.</summary>
    public const ulong Optional = 0x46ac6fb1a1222720;
    /// <summary>A fixed set of keys.</summary>
    public const ulong FixedDict = 0x4588f998f6874808;
    /// <summary>A generator chosen from a drawn value.</summary>
    public const ulong FlatMap = 0xe85be60d862e687a;
    /// <summary>A filtered draw.</summary>
    public const ulong Filter = 0x954d8384f16a15fe;
    /// <summary>One attempt of a filtered draw.</summary>
    public const ulong FilterAttempt = 0x8b6a04585682cd19;
    /// <summary>A mapped draw.</summary>
    public const ulong Mapped = 0x46ef1386be65f262;
    /// <summary>A value sampled from a sequence.</summary>
    public const ulong SampledFrom = 0xf9573be7fe320c77;
    /// <summary>A stateful rule.</summary>
    public const ulong StatefulRule = 0xbd20e256d6f482f8;
    /// <summary>A recursive draw.</summary>
    public const ulong Recursive = 0xbbdcc6e73a04d5a8;
    /// <summary>A permutation.</summary>
    public const ulong Permutation = 0xb027cceaf4155d40;
    /// <summary>A subsequence.</summary>
    public const ulong Subsequence = 0xd25dd251af3d131f;
    /// <summary>A sample.</summary>
    public const ulong Sample = 0x7689a4323364eda6;
    /// <summary>An array.</summary>
    public const ulong Array = 0x5a6e9dc8e669fff1;
    /// <summary>A composite draw.</summary>
    public const ulong Composite = 0x28e766540c9ffb4f;
    /// <summary>A deferred generator.</summary>
    public const ulong Deferred = 0xb96dc18ba2ffa9f7;
    /// <summary>A Lark grammar draw.</summary>
    public const ulong FromLark = 0x8fc2df1e0331ed41;
    /// <summary>A reusable pool value.</summary>
    public const ulong PoolValuesReusable = 0x7f9558f7787b8b82;
    /// <summary>A consumed pool value.</summary>
    public const ulong PoolValuesConsumed = 0x06d5135fb19072d1;
    /// <summary>A generator that gives no label of its own.</summary>
    public const ulong Generator = 0xb86174e143a64e31;

    /// <summary>The label the engine derives from <paramref name="name"/>.</summary>
    public static ulong Of(string name) => Engine.Real().LabelFromName(name);

    /// <summary>The label the engine derives from <paramref name="labels"/> together.</summary>
    public static ulong Combine(params ulong[] labels) => Engine.Real().LabelCombine(labels);
}
