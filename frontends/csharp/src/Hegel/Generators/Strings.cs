using System.Globalization;
using System.Text;
using Hegel.Native;

namespace Hegel;

/// <summary>Draws strings: any Unicode text by default, narrowed by size, code point, category or alphabet.</summary>
public sealed class TextGenerator : ValueGenerator<string>
{
    private readonly ulong _minSize;
    private readonly ulong _maxSize;
    private readonly string? _codec;
    private readonly uint? _minCodepoint;
    private readonly uint? _maxCodepoint;
    private readonly IReadOnlyList<string>? _categories;
    private readonly IReadOnlyList<string>? _excludeCategories;
    private readonly string? _includeChars;
    private readonly string? _excludeChars;
    private readonly string? _alphabet;
    private readonly HandleCache _cache = new();

    internal TextGenerator(int minSize, ulong maxSize)
        : this(minSize, maxSize, null, null, null, null, null, null, null, null)
    {
    }

    private TextGenerator(int minSize, ulong maxSize, string? codec, uint? minCodepoint, uint? maxCodepoint, IReadOnlyList<string>? categories, IReadOnlyList<string>? excludeCategories, string? includeChars, string? excludeChars, string? alphabet)
    {
        _minSize = Sizes.Validate(minSize, maxSize, "Text");
        if (alphabet != null && (codec != null || minCodepoint != null || maxCodepoint != null || categories != null || excludeCategories != null || includeChars != null || excludeChars != null))
        {
            throw new ArgumentException("Text: an alphabet cannot be combined with character restrictions", nameof(alphabet));
        }
        _maxSize = maxSize;
        _codec = codec;
        _minCodepoint = minCodepoint;
        _maxCodepoint = maxCodepoint;
        _categories = categories;
        _excludeCategories = excludeCategories;
        _includeChars = includeChars;
        _excludeChars = excludeChars;
        _alphabet = alphabet;
    }

    private TextGenerator With(int? minSize = null, ulong? maxSize = null, string? codec = null, uint? minCodepoint = null, uint? maxCodepoint = null, IReadOnlyList<string>? categories = null, IReadOnlyList<string>? excludeCategories = null, string? includeChars = null, string? excludeChars = null, string? alphabet = null) =>
        new(minSize ?? (int)_minSize, maxSize ?? _maxSize, codec ?? _codec, minCodepoint ?? _minCodepoint, maxCodepoint ?? _maxCodepoint, categories ?? _categories, excludeCategories ?? _excludeCategories, includeChars ?? _includeChars, excludeChars ?? _excludeChars, alphabet ?? _alphabet);

    /// <summary>At least <paramref name="minSize"/> code points.</summary>
    public TextGenerator MinSize(int minSize) => With(minSize: minSize);

    /// <summary>At most <paramref name="maxSize"/> code points.</summary>
    public TextGenerator MaxSize(int maxSize) => With(maxSize: Sizes.Max(maxSize, "Text"));

    /// <summary>Only the characters of <paramref name="chars"/>. Cannot be combined with the other restrictions.</summary>
    public TextGenerator Alphabet(string chars) => With(alphabet: chars ?? throw new ArgumentNullException(nameof(chars)));

    /// <summary>Only text that <paramref name="codec"/> (<c>"ascii"</c>, <c>"utf-8"</c>, ...) can encode.</summary>
    public TextGenerator Codec(string codec) => With(codec: codec ?? throw new ArgumentNullException(nameof(codec)));

    /// <summary>Only code points in [<paramref name="min"/>, <paramref name="max"/>].</summary>
    public TextGenerator Codepoints(int min, int max)
    {
        if (min < 0 || max < min)
        {
            throw new ArgumentOutOfRangeException(nameof(min), min, "Text: Codepoints needs 0 <= min <= max");
        }
        return With(minCodepoint: (uint)min, maxCodepoint: (uint)max);
    }

    /// <summary>Only the Unicode general categories <paramref name="categories"/> (<c>"L"</c>, <c>"Nd"</c>, ...).</summary>
    public TextGenerator Categories(params string[] categories) => With(categories: Array.AsReadOnly((string[])categories.Clone()));

    /// <summary>None of the Unicode general categories <paramref name="categories"/>.</summary>
    public TextGenerator ExcludeCategories(params string[] categories) => With(excludeCategories: Array.AsReadOnly((string[])categories.Clone()));

    /// <summary>Also the characters of <paramref name="chars"/>, whatever the other restrictions say.</summary>
    public TextGenerator IncludeCharacters(string chars) => With(includeChars: chars ?? throw new ArgumentNullException(nameof(chars)));

    /// <summary>Never the characters of <paramref name="chars"/>.</summary>
    public TextGenerator ExcludeCharacters(string chars) => With(excludeChars: chars ?? throw new ArgumentNullException(nameof(chars)));

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.text");

    /// <inheritdoc />
    public override string DoDraw(TestCase tc) =>
        tc.GenerateString(_cache.Get(tc, t => BuildHandle(t, _minSize, Sizes.EngineMax(_maxSize))));

    internal StringGeneratorHandle AlphabetHandle(TestCase tc) => BuildHandle(tc, 0, 0);

    private StringGeneratorHandle BuildHandle(TestCase tc, ulong min, ulong max)
    {
        if (_alphabet != null)
        {
            return tc.TextGenerator(min, max, null, 0, Abi.NoMaxCodepoint, [], null, _alphabet, null);
        }
        return tc.TextGenerator(min, max, _codec, _minCodepoint ?? 0, _maxCodepoint ?? Abi.NoMaxCodepoint, _categories, _excludeCategories, _includeChars, _excludeChars);
    }
}

/// <summary>Draws strings matching a regular expression.</summary>
public sealed class RegexGenerator : ValueGenerator<string>
{
    private readonly string _pattern;
    private readonly bool _fullmatch;
    private readonly TextGenerator? _alphabet;
    private readonly HandleCache _cache = new();

    internal RegexGenerator(string pattern, bool fullmatch, TextGenerator? alphabet)
    {
        _pattern = pattern ?? throw new ArgumentNullException(nameof(pattern));
        _fullmatch = fullmatch;
        _alphabet = alphabet;
    }

    /// <summary>Whether the whole string must match (the default), or only a substring.</summary>
    public RegexGenerator Fullmatch(bool fullmatch) => new(_pattern, fullmatch, _alphabet);

    /// <summary>Draw the characters a wildcard matches from <paramref name="characters"/>.</summary>
    public RegexGenerator Alphabet(TextGenerator characters) => new(_pattern, _fullmatch, characters);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.from_regex");

    /// <inheritdoc />
    public override string DoDraw(TestCase tc) =>
        tc.GenerateString(_cache.Get(tc, t => t.RegexGenerator(_pattern, _fullmatch, _alphabet?.AlphabetHandle(t))));
}

internal sealed class EmailGenerator : ValueGenerator<string>
{
    private readonly HandleCache _cache = new();

    public override ulong Label => Hegel.Label.Of("hegel.emails");

    public override string DoDraw(TestCase tc) => tc.GenerateString(_cache.Get(tc, t => t.EmailGenerator()));
}

internal sealed class UrlGenerator : ValueGenerator<string>
{
    private readonly HandleCache _cache = new();

    public override ulong Label => Hegel.Label.Of("hegel.urls");

    public override string DoDraw(TestCase tc) => tc.GenerateString(_cache.Get(tc, t => t.UrlGenerator()));
}

/// <summary>Draws domain names.</summary>
public sealed class DomainGenerator : ValueGenerator<string>
{
    private readonly int _maxLength;
    private readonly HandleCache _cache = new();

    internal DomainGenerator(int maxLength)
    {
        if (maxLength < 4 || maxLength > 255)
        {
            throw new ArgumentOutOfRangeException(nameof(maxLength), maxLength, "Domains: MaxLength must be in [4, 255]");
        }
        _maxLength = maxLength;
    }

    /// <summary>At most <paramref name="maxLength"/> characters, between 4 and 255.</summary>
    public DomainGenerator MaxLength(int maxLength) => new(maxLength);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.domains");

    /// <inheritdoc />
    public override string DoDraw(TestCase tc) => tc.GenerateString(_cache.Get(tc, t => t.DomainGenerator((ulong)_maxLength)));
}

/// <summary>Draws UUIDs, of any version or one in particular.</summary>
public sealed class UuidGenerator : ValueGenerator<Guid>
{
    private readonly int? _version;

    internal UuidGenerator(int? version)
    {
        if (version is < 1 or > 5)
        {
            throw new ArgumentOutOfRangeException(nameof(version), version, "Uuids: Version must be in [1, 5]");
        }
        _version = version;
    }

    /// <summary>Only UUIDs of <paramref name="version"/> (1 to 5).</summary>
    public UuidGenerator Version(int version) => new(version);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.uuids");

    /// <inheritdoc />
    public override Guid DoDraw(TestCase tc) => tc.GenerateUuid(_version);
}

/// <summary>Draws IP addresses as text: a mix of IPv4 and IPv6 unless narrowed.</summary>
public sealed class IpAddressGenerator : ValueGenerator<string>
{
    private readonly int? _version;

    internal IpAddressGenerator(int? version)
    {
        _version = version;
    }

    /// <summary>Only IPv4 addresses.</summary>
    public IpAddressGenerator V4() => new(4);

    /// <summary>Only IPv6 addresses.</summary>
    public IpAddressGenerator V6() => new(6);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.Of("hegel.ip_addresses");

    /// <inheritdoc />
    public override string DoDraw(TestCase tc)
    {
        if (_version != null)
        {
            return _version == 4 ? FormatV4(tc.GenerateIpv4()) : FormatV6(tc.GenerateIpv6());
        }
        return tc.GenerateInteger(0, 1) == 0 ? FormatV4(tc.GenerateIpv4()) : FormatV6(tc.GenerateIpv6());
    }

    private static string FormatV4(byte[] b) => $"{b[0]}.{b[1]}.{b[2]}.{b[3]}";

    internal static string FormatV6(byte[] b)
    {
        var groups = new int[8];
        for (var i = 0; i < 8; i++)
        {
            groups[i] = (b[2 * i] << 8) | b[2 * i + 1];
        }
        var bestStart = -1;
        var bestLen = 1;
        for (var i = 0; i < 8;)
        {
            if (groups[i] != 0)
            {
                i++;
                continue;
            }
            var j = i;
            while (j < 8 && groups[j] == 0)
            {
                j++;
            }
            if (j - i > bestLen)
            {
                bestStart = i;
                bestLen = j - i;
            }
            i = j;
        }
        var sb = new StringBuilder();
        for (var i = 0; i < 8; i++)
        {
            if (i == bestStart)
            {
                sb.Append("::");
                i += bestLen - 1;
                continue;
            }
            if (i > 0 && sb[^1] != ':')
            {
                sb.Append(':');
            }
            sb.Append(groups[i].ToString("x", CultureInfo.InvariantCulture));
        }
        return sb.ToString();
    }
}

/// <summary>Draws strings from a grammar in Lark syntax.</summary>
public sealed class LarkGenerator : ValueGenerator<string>
{
    private readonly string _grammar;
    private readonly string? _start;
    private readonly IReadOnlyList<(string Name, Generator<string> Generator)> _explicit;
    private readonly TextGenerator? _alphabet;
    private readonly int _maxSize;
    private readonly int _maxDepth;
    private readonly string? _importPath;
    private volatile GrammarHandle? _handle;

    internal LarkGenerator(string grammar)
        : this(grammar ?? throw new ArgumentNullException(nameof(grammar)), null, [], null, 0, 0, null)
    {
    }

    private LarkGenerator(string grammar, string? start, IReadOnlyList<(string, Generator<string>)> explicitTerminals, TextGenerator? alphabet, int maxSize, int maxDepth, string? importPath)
    {
        _grammar = grammar;
        _start = start;
        _explicit = explicitTerminals;
        _alphabet = alphabet;
        _maxSize = maxSize;
        _maxDepth = maxDepth;
        _importPath = importPath;
    }

    /// <summary>Start from <paramref name="rule"/> instead of the grammar's first rule.</summary>
    public LarkGenerator Start(string rule) => new(_grammar, rule, _explicit, _alphabet, _maxSize, _maxDepth, _importPath);

    /// <summary>Draw the terminal <paramref name="name"/>, which the grammar declares but does not define, from <paramref name="generator"/>.</summary>
    public LarkGenerator Explicit(string name, Generator<string> generator)
    {
        if (_explicit.Any(e => e.Name == name))
        {
            throw new ArgumentException($"FromLark: explicit terminal '{name}' is given more than one generator", nameof(name));
        }
        return new LarkGenerator(_grammar, _start, [.. _explicit, (name, generator)], _alphabet, _maxSize, _maxDepth, _importPath);
    }

    /// <summary>Draw the characters a wildcard matches from <paramref name="characters"/>.</summary>
    public LarkGenerator Alphabet(TextGenerator characters) => new(_grammar, _start, _explicit, characters, _maxSize, _maxDepth, _importPath);

    /// <summary>Aim for at most <paramref name="maxSize"/> terminals.</summary>
    public LarkGenerator MaxSize(int maxSize)
    {
        if (maxSize < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(maxSize), maxSize, "FromLark: MaxSize must be at least 1");
        }
        return new LarkGenerator(_grammar, _start, _explicit, _alphabet, maxSize, _maxDepth, _importPath);
    }

    /// <summary>Nest rules at most <paramref name="maxDepth"/> deep.</summary>
    public LarkGenerator MaxDepth(int maxDepth)
    {
        if (maxDepth < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(maxDepth), maxDepth, "FromLark: MaxDepth must be at least 1");
        }
        return new LarkGenerator(_grammar, _start, _explicit, _alphabet, _maxSize, maxDepth, _importPath);
    }

    /// <summary>Resolve the grammar's <c>%import</c>s against <paramref name="directory"/>.</summary>
    public LarkGenerator ImportPath(string directory) => new(_grammar, _start, _explicit, _alphabet, _maxSize, _maxDepth, directory);

    /// <inheritdoc />
    public override ulong Label => Hegel.Label.FromLark;

    /// <inheritdoc />
    public override string DoDraw(TestCase tc) => tc.GenerateGrammar(Compiled(tc), i => tc.DrawSilent(_explicit[i].Generator));

    private GrammarHandle Compiled(TestCase tc)
    {
        var h = _handle;
        if (h == null || !tc.OwnsGrammar(h))
        {
            h = tc.Grammar(_grammar, _start, (ulong)_maxSize, (uint)_maxDepth, _alphabet?.AlphabetHandle(tc), _explicit.Select(e => e.Name).ToList(), ReadImport);
            _handle = h;
        }
        return h;
    }

    private string? ReadImport(string path)
    {
        if (_importPath == null)
        {
            return null;
        }
        var file = Path.Combine(_importPath, path);
        return File.Exists(file) ? File.ReadAllText(file) : null;
    }
}
