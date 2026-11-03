using Hegel.Native;

namespace Hegel.Tests.Support;

internal static class Engines
{
    internal static ILibhegel Real => Engine.Real();

    internal static IDisposable Override(ILibhegel fake)
    {
        Engine.SetForTesting(fake);
        return new Reset();
    }

    private sealed class Reset : IDisposable
    {
        public void Dispose() => Engine.SetForTesting(null);
    }

    internal static Settings Quiet => new() { Database = Database.Disabled, Verbosity = Verbosity.Quiet, PrintBlob = false };
}
