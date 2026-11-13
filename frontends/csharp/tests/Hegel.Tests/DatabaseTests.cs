using Xunit;

namespace Hegel.Tests;

public class DatabaseTests
{
    [Fact]
    public void DefaultLocationHasNoPathAndRoundTripsThroughTheEngineForm()
    {
        Assert.Null(Database.DefaultLocation.Path);
        Assert.Null(Database.DefaultLocation.ForEngine());
        Assert.Same(Database.DefaultLocation, Database.FromEngine(null));
        Assert.Equal("Database.DefaultLocation", Database.DefaultLocation.ToString());
    }

    [Fact]
    public void DisabledIsTheEmptyStringToTheEngine()
    {
        Assert.Null(Database.Disabled.Path);
        Assert.Equal("", Database.Disabled.ForEngine());
        Assert.Same(Database.Disabled, Database.FromEngine(""));
        Assert.Equal("Database.Disabled", Database.Disabled.ToString());
    }

    [Fact]
    public void AtKeepsThePath()
    {
        var db = Database.At("/tmp/x");
        Assert.Equal("/tmp/x", db.Path);
        Assert.Equal("/tmp/x", db.ForEngine());
        Assert.Equal(db, Database.FromEngine("/tmp/x"));
        Assert.Equal("Database.At(\"/tmp/x\")", db.ToString());
    }

    [Fact]
    public void AtRejectsNull()
    {
        Assert.Throws<ArgumentNullException>(() => Database.At(null!));
    }

    [Fact]
    public void EqualityComparesKindAndPath()
    {
        Assert.Equal(Database.At("a"), Database.At("a"));
        Assert.NotEqual(Database.At("a"), Database.At("b"));
        Assert.NotEqual(Database.Disabled, Database.DefaultLocation);
        Assert.False(Database.Disabled.Equals(null));
        Assert.False(Database.Disabled.Equals("Database.Disabled"));
        Assert.True(Database.At("a").Equals((object)Database.At("a")));
        Assert.Equal(Database.At("a").GetHashCode(), Database.At("a").GetHashCode());
    }
}
