using Hegel.Native;
using Xunit;

namespace Hegel.Tests;

public class EnumsTests
{
    [Fact]
    public void VerbosityCodesMapToTheEnum()
    {
        Assert.Equal(Verbosity.Normal, EnumCodes.VerbosityFrom(Abi.VerbosityNormal));
        Assert.Equal(Verbosity.Quiet, EnumCodes.VerbosityFrom(Abi.VerbosityQuiet));
        Assert.Equal(Verbosity.Verbose, EnumCodes.VerbosityFrom(Abi.VerbosityVerbose));
        Assert.Equal(Verbosity.Debug, EnumCodes.VerbosityFrom(Abi.VerbosityDebug));
        var e = Assert.Throws<HegelException>(() => EnumCodes.VerbosityFrom(42));
        Assert.Equal("unknown hegel_verbosity_t value 42", e.Message);
    }

    [Fact]
    public void BackendCodesMapToTheEnum()
    {
        Assert.Equal(Backend.Default, EnumCodes.BackendFrom(Abi.BackendDefault));
        Assert.Equal(Backend.Urandom, EnumCodes.BackendFrom(Abi.BackendUrandom));
        var e = Assert.Throws<HegelException>(() => EnumCodes.BackendFrom(0));
        Assert.Equal("unknown hegel_backend_t value 0", e.Message);
    }

    [Fact]
    public void StrictnessCodesMapToTheEnum()
    {
        Assert.Equal(NondeterminismStrictness.Quiet, EnumCodes.StrictnessFrom(Abi.NondeterminismQuiet));
        Assert.Equal(NondeterminismStrictness.Warn, EnumCodes.StrictnessFrom(Abi.NondeterminismWarn));
        Assert.Equal(NondeterminismStrictness.Error, EnumCodes.StrictnessFrom(Abi.NondeterminismError));
        var e = Assert.Throws<HegelException>(() => EnumCodes.StrictnessFrom(-1));
        Assert.Equal("unknown hegel_nondeterminism_strictness_t value -1", e.Message);
    }

    [Fact]
    public void ErrorKindCodesMapToTheEnumOrNull()
    {
        Assert.Equal(RunErrorKind.HealthCheck, EnumCodes.ErrorKindFrom(Abi.RunErrorHealthCheck));
        Assert.Equal(RunErrorKind.Internal, EnumCodes.ErrorKindFrom(Abi.RunErrorInternal));
        Assert.Null(EnumCodes.ErrorKindFrom(Abi.RunErrorNone));
        Assert.Null(EnumCodes.ErrorKindFrom(99));
    }

    [Fact]
    public void HealthCheckBitsMapToTheEnumOrNull()
    {
        Assert.Equal(HealthCheck.FilterTooMuch, EnumCodes.HealthCheckFrom(Abi.HcFilterTooMuch));
        Assert.Equal(HealthCheck.TooSlow, EnumCodes.HealthCheckFrom(Abi.HcTooSlow));
        Assert.Equal(HealthCheck.TestCasesTooLarge, EnumCodes.HealthCheckFrom(Abi.HcTestCasesTooLarge));
        Assert.Equal(HealthCheck.LargeInitialTestCase, EnumCodes.HealthCheckFrom(Abi.HcLargeInitialTestCase));
        Assert.Null(EnumCodes.HealthCheckFrom(0));
        Assert.Null(EnumCodes.HealthCheckFrom(1 << 7));
    }

    [Fact]
    public void PhaseAllIsEveryPhase()
    {
        Assert.Equal((Phase)Abi.PhaseAll, Phase.All);
        Assert.Equal(Phase.None, Phase.All & ~Phase.All);
    }
}
