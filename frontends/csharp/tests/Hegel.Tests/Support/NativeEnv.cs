using System.Runtime.InteropServices;

namespace Hegel.Tests.Support;

internal static partial class NativeEnv
{
    [LibraryImport("libc", EntryPoint = "setenv", StringMarshalling = StringMarshalling.Utf8)]
    private static partial int Setenv(string name, string value, int overwrite);

    [LibraryImport("libc", EntryPoint = "unsetenv", StringMarshalling = StringMarshalling.Utf8)]
    private static partial int Unsetenv(string name);

    [LibraryImport("ucrtbase", EntryPoint = "_putenv_s", StringMarshalling = StringMarshalling.Utf8)]
    private static partial int PutenvS(string name, string value);

    internal static IDisposable Set(string name, string value)
    {
        var previous = Environment.GetEnvironmentVariable(name);
        Apply(name, value);
        return new Restore(name, previous);
    }

    private static void Apply(string name, string? value)
    {
        Environment.SetEnvironmentVariable(name, value);
        if (OperatingSystem.IsWindows())
        {
            PutenvS(name, value ?? "");
        }
        else if (value == null)
        {
            Unsetenv(name);
        }
        else
        {
            Setenv(name, value, 1);
        }
    }

    private sealed class Restore(string name, string? previous) : IDisposable
    {
        public void Dispose() => Apply(name, previous);
    }
}
