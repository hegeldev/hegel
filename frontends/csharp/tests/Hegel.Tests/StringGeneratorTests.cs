using System.Text;
using System.Text.RegularExpressions;
using Xunit;

namespace Hegel.Tests;

public class StringGeneratorTests
{
    private static IEnumerable<Rune> Runes(string s) => s.EnumerateRunes();

    [Fact]
    public void TextRespectsSizes()
    {
        Gen.All(Generators.Text().MinSize(2).MaxSize(3), s => Runes(s).Count() is >= 2 and <= 3, 100);
        Gen.All(Generators.Text().MaxSize(0), s => s.Length == 0, 10);
        Gen.All(Generators.Text(), _ => true, 20);
        Gen.All(Generators.Characters(), s => Runes(s).Count() == 1, 50);
        Assert.Equal(Label.Of("hegel.text"), Generators.Text().Label);
    }

    [Fact]
    public void TextAlphabetRestrictsCharacters()
    {
        Gen.All(Generators.Text().Alphabet("ab").MinSize(1), s => s.All(c => c is 'a' or 'b'), 100);
    }

    [Fact]
    public void TextAlphabetMustNotBeNull()
    {
        Assert.Throws<ArgumentNullException>(() => Generators.Text().Alphabet(null!));
    }

    [Fact]
    public void TextAlphabetCannotBeCombinedWithRestrictions()
    {
        var e = Assert.Throws<ArgumentException>(() => Generators.Text().Alphabet("ab").Codec("ascii"));
        Assert.Contains("Text: an alphabet cannot be combined with character restrictions", e.Message);
        Assert.Throws<ArgumentException>(() => Generators.Text().Codepoints(0, 10).Alphabet("ab"));
    }

    [Fact]
    public void TextCodecRestrictsToAscii()
    {
        Gen.All(Generators.Text().Codec("ascii").MinSize(1), s => s.All(c => c < 128), 100);
        Assert.Throws<ArgumentNullException>(() => Generators.Text().Codec(null!));
    }

    [Fact]
    public void TextCodepointsRestrictTheRange()
    {
        Gen.All(Generators.Text().Codepoints(0x61, 0x63).MinSize(1), s => s.All(c => c is >= 'a' and <= 'c'), 100);
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Text().Codepoints(5, 4));
        Assert.Contains("Text: Codepoints needs 0 <= min <= max", e.Message);
        Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Text().Codepoints(-1, 4));
    }

    [Fact]
    public void TextCategoriesRestrictAndExclude()
    {
        Gen.All(Generators.Text().Codepoints(0x41, 0x7A).Categories("Lu").MinSize(1), s => s.All(char.IsUpper), 100);
        Gen.All(Generators.Text().Codepoints(0x41, 0x7A).ExcludeCategories("Lu").MinSize(1), s => !s.Any(char.IsUpper), 100);
    }

    [Fact]
    public void TextIncludesAndExcludesCharacters()
    {
        var withX = Gen.Sample(Generators.Text().Codepoints(0x30, 0x39).IncludeCharacters("x").MinSize(1), 200);
        Assert.All(withX, s => Assert.True(s.All(c => char.IsAsciiDigit(c) || c == 'x')));
        Assert.Contains(withX, s => s.Contains('x'));
        Gen.All(Generators.Text().Codepoints(0x30, 0x39).ExcludeCharacters("0").MinSize(1), s => !s.Contains('0'), 100);
        Assert.Throws<ArgumentNullException>(() => Generators.Text().IncludeCharacters(null!));
        Assert.Throws<ArgumentNullException>(() => Generators.Text().ExcludeCharacters(null!));
    }

    [Fact]
    public void TextReusesItsHandleAcrossRuns()
    {
        var g = Generators.Text().Codepoints(0x7A, 0x7A).MinSize(1).MaxSize(1);
        Gen.All(g, s => s == "z", 5);
        Gen.All(g, s => s == "z", 5);
    }

    [Fact]
    public void TextPrintsAsAQuotedString()
    {
        Assert.Equal("var x_1 = \"\";", Gen.MinimalOutput(Generators.Text()));
        Assert.Equal("var x_1 = \"aa\";", Gen.MinimalOutput(Generators.Text().Codepoints(0x61, 0x62).MinSize(2).MaxSize(2)));
    }

    [Fact]
    public void RegexMatchesThePattern()
    {
        Gen.All(Generators.FromRegex("[a-c]{2}"), s => Regex.IsMatch(s, "^[a-c]{2}$"), 100);
        Gen.All(Generators.FromRegex("^ab$").Fullmatch(false), s => s.Contains("ab"), 50);
        Assert.Equal(Label.Of("hegel.from_regex"), Generators.FromRegex("a").Label);
        Assert.Throws<ArgumentNullException>(() => Generators.FromRegex(null!));
    }

    [Fact]
    public void RegexWildcardsDrawFromTheAlphabet()
    {
        Gen.All(Generators.FromRegex("a.b").Alphabet(Generators.Text().Alphabet("abxy")), s => Regex.IsMatch(s, "^a[abxy]b$"), 100);
    }

    [Fact]
    public void EmailsAndUrlsAndDomainsAreWellFormed()
    {
        Gen.All(Generators.Emails(), s => s.Contains('@'), 50);
        Gen.All(Generators.Urls(), s => Uri.TryCreate(s, UriKind.Absolute, out _), 50);
        Gen.All(Generators.Domains().MaxLength(10), s => s.Length is >= 1 and <= 10, 50);
        Gen.All(Generators.Domains(), s => s.Length <= 255, 20);
        Assert.Equal(Label.Of("hegel.emails"), Generators.Emails().Label);
        Assert.Equal(Label.Of("hegel.urls"), Generators.Urls().Label);
        Assert.Equal(Label.Of("hegel.domains"), Generators.Domains().Label);
    }

    [Fact]
    public void DomainsRejectBadLengths()
    {
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Domains().MaxLength(3));
        Assert.Contains("Domains: MaxLength must be in [4, 255]", e.Message);
        Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Domains().MaxLength(256));
    }

    [Fact]
    public void UuidsHaveTheRequestedVersion()
    {
        Gen.All(Generators.Uuids(), g => g != Guid.Empty || true, 20);
        for (var version = 1; version <= 5; version++)
        {
            var v = version;
            Gen.All(Generators.Uuids().Version(v), g => g.ToString("N")[12] == (char)('0' + v), 20);
        }
        Assert.Equal(Label.Of("hegel.uuids"), Generators.Uuids().Label);
        var e = Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Uuids().Version(6));
        Assert.Contains("Uuids: Version must be in [1, 5]", e.Message);
        Assert.Throws<ArgumentOutOfRangeException>(() => Generators.Uuids().Version(0));
    }

    [Fact]
    public void UuidsPrintAsGuidParse()
    {
        Assert.Matches("^var x_1 = Guid\\.Parse\\(\"[0-9a-f-]{36}\"\\);$", Gen.MinimalOutput(Generators.Uuids()));
    }

    [Fact]
    public void IpAddressesComeInBothVersions()
    {
        var mixed = Gen.Sample(Generators.IpAddresses(), 100);
        Assert.Contains(mixed, s => s.Contains('.'));
        Assert.Contains(mixed, s => s.Contains(':'));
        Gen.All(Generators.IpAddresses().V4(), s => System.Net.IPAddress.TryParse(s, out var a) && a.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork, 50);
        Gen.All(Generators.IpAddresses().V6(), s => System.Net.IPAddress.TryParse(s, out var a) && a.AddressFamily == System.Net.Sockets.AddressFamily.InterNetworkV6, 50);
        Assert.Equal(Label.Of("hegel.ip_addresses"), Generators.IpAddresses().Label);
    }

    [Theory]
    [InlineData("00000000000000000000000000000000", "::")]
    [InlineData("00000000000000000000000000000001", "::1")]
    [InlineData("00010000000000000000000000000000", "1::")]
    [InlineData("00010000000100000000000000000001", "1:0:1::1")]
    [InlineData("00010000000000010000000000000001", "1:0:0:1::1")]
    [InlineData("00010000000000010000000000000000", "1:0:0:1::")]
    [InlineData("00010000000000020000000000030004", "1::2:0:0:3:4")]
    [InlineData("20010db8000000000000000000000001", "2001:db8::1")]
    [InlineData("00010002000300040005000600070008", "1:2:3:4:5:6:7:8")]
    [InlineData("00010000000200000003000000040000", "1:0:2:0:3:0:4:0")]
    [InlineData("0000fffe00000000ffff000000000000", "0:fffe:0:0:ffff::")]
    public void Ipv6FormattingFollowsRfc5952(string hex, string expected)
    {
        Assert.Equal(expected, IpAddressGenerator.FormatV6(Convert.FromHexString(hex)));
    }

    [Fact]
    public void LarkDrawsFromTheGrammar()
    {
        Gen.All(Generators.FromLark("start: \"a\"+\n"), s => s.Length > 0 && s.All(c => c == 'a'), 50);
        Gen.All(Generators.FromLark("a: \"a\"+\nb: \"b\"+\n").Start("b"), s => s.All(c => c == 'b'), 50);
        Gen.All(Generators.FromLark("start: \"a\"+\n").MaxSize(1).MaxDepth(1), s => s.Length > 0, 20);
        Assert.Equal(Label.FromLark, Generators.FromLark("start: \"a\"\n").Label);
        Assert.Throws<ArgumentNullException>(() => Generators.FromLark(null!));
    }

    [Fact]
    public void LarkWildcardsDrawFromTheAlphabet()
    {
        Gen.All(Generators.FromLark("start: /./\n").Alphabet(Generators.Text().Alphabet("xy")), s => s is "x" or "y", 50);
    }

    [Fact]
    public void LarkExplicitTerminalsComeFromGenerators()
    {
        var g = Generators.FromLark("start: WORD \"!\" NUM\n%declare WORD NUM\n")
            .Explicit("WORD", Generators.FromRegex("[a-z]{3}"))
            .Explicit("NUM", Generators.FromRegex("[0-9]+"));
        Gen.All(g, s => Regex.IsMatch(s, "^[a-z]{3}![0-9]+$"), 50);
        var e = Assert.Throws<ArgumentException>(() => g.Explicit("NUM", Generators.Just("1")));
        Assert.Contains("FromLark: explicit terminal 'NUM' is given more than one generator", e.Message);
    }

    [Fact]
    public void LarkRejectsBadLimits()
    {
        Assert.Contains("FromLark: MaxSize must be at least 1", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.FromLark("start: \"a\"\n").MaxSize(0)).Message);
        Assert.Contains("FromLark: MaxDepth must be at least 1", Assert.Throws<ArgumentOutOfRangeException>(() => Generators.FromLark("start: \"a\"\n").MaxDepth(0)).Message);
    }

    [Fact]
    public void LarkResolvesImportsFromTheImportPath()
    {
        var dir = Directory.CreateTempSubdirectory("hegel-lark").FullName;
        try
        {
            File.WriteAllText(Path.Combine(dir, "lib.lark"), "LETTERS: /[a-c]+/\n");
            Gen.All(Generators.FromLark("%import lib.LETTERS\nstart: LETTERS\n").ImportPath(dir), s => s.Length > 0 && s.All(c => c is >= 'a' and <= 'c'), 50);
            Gen.All(Generators.FromLark("%import common.DIGIT\nstart: DIGIT\n").ImportPath(dir), s => s.Length == 1 && char.IsAsciiDigit(s[0]), 20);
        }
        finally
        {
            Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void LarkFallsBackToBundledGrammarsWithoutAnImportPath()
    {
        Gen.All(Generators.FromLark("%import common.DIGIT\nstart: DIGIT\n"), s => s.Length == 1 && char.IsAsciiDigit(s[0]), 20);
    }

    [Fact]
    public void LarkMissingImportIsAUsageError()
    {
        var g = Generators.FromLark("%import nowhere.X\nstart: X\n");
        Assert.ThrowsAny<ArgumentException>(() => Property.Test(tc => tc.Draw(g), Gen.Quiet(5), Reporter.Silent));
    }

    [Fact]
    public void LarkReusesItsCompiledGrammarAcrossRuns()
    {
        var g = Generators.FromLark("start: \"a\"\n");
        Gen.All(g, s => s == "a", 5);
        Gen.All(g, s => s == "a", 5);
    }
}
