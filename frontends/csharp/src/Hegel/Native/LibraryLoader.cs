using System.Runtime.InteropServices;

namespace Hegel.Native;

/// <summary>
/// Locates and loads the libhegel shared library.
///
/// <para>Resolution order (first hit wins):</para>
/// <list type="number">
/// <item><c>HEGEL_LIBHEGEL_PATH</c>: an explicit override (e.g. for local engine development); if
/// set it must point at an existing file, otherwise resolution fails.</item>
/// <item>The library bundled for this platform next to the assembly, at
/// <c>runtimes/&lt;rid&gt;/native/libhegel.&lt;ext&gt;</c>: what a build of this repository
/// copies there and what the NuGet package carries.</item>
/// <item>The .NET host's own native-library probing (the application directory, the package's
/// <c>runtimes</c> assets as <c>deps.json</c> lists them, and the OS search path).</item>
/// </list>
/// </summary>
internal static class LibraryLoader
{
    internal const string PathVariable = "HEGEL_LIBHEGEL_PATH";

    private static readonly object Gate = new();
    private static nint _handle;
    private static bool _registered;

    internal static string LibraryFileName =>
        OperatingSystem.IsWindows() ? "libhegel.dll" : OperatingSystem.IsMacOS() ? "libhegel.dylib" : "libhegel.so";

    internal static string BundledPath(string baseDirectory, string rid) =>
        Path.Combine(baseDirectory, "runtimes", rid, "native", LibraryFileName);

    /// <summary>
    /// The candidate locations, in order, for the given environment: the override, then the
    /// bundled library. The host's own probing comes after these and has no path to list.
    /// </summary>
    internal static List<string> Candidates(string? overridePath, string baseDirectory, string rid)
    {
        var candidates = new List<string>();
        if (!string.IsNullOrEmpty(overridePath))
        {
            candidates.Add(overridePath);
        }
        candidates.Add(BundledPath(baseDirectory, rid));
        return candidates;
    }

    /// <summary>Load libhegel once and route the bindings' imports to it.</summary>
    internal static nint Load()
    {
        lock (Gate)
        {
            if (_handle == 0)
            {
                _handle = Resolve(
                    Environment.GetEnvironmentVariable(PathVariable),
                    AppContext.BaseDirectory,
                    RuntimeInformation.RuntimeIdentifier,
                    name => NativeLibrary.TryLoad(name, out var h) ? h : 0,
                    () => NativeLibrary.TryLoad(NativeMethods.Library, typeof(LibraryLoader).Assembly, null, out var h) ? h : 0);
            }
            if (!_registered)
            {
                NativeLibrary.SetDllImportResolver(typeof(LibraryLoader).Assembly, (name, _, _) => name == NativeMethods.Library ? _handle : 0);
                _registered = true;
            }
            return _handle;
        }
    }

    internal static nint Resolve(string? overridePath, string baseDirectory, string rid, Func<string, nint> loadPath, Func<nint> loadDefault)
    {
        if (!string.IsNullOrEmpty(overridePath))
        {
            if (!File.Exists(overridePath))
            {
                throw new HegelException($"{PathVariable} is set to '{overridePath}' but no file exists there.");
            }
            var overridden = loadPath(overridePath);
            if (overridden != 0)
            {
                return overridden;
            }
            throw new HegelException($"{PathVariable} is set to '{overridePath}' but the library there could not be loaded.");
        }
        var bundled = BundledPath(baseDirectory, rid);
        if (File.Exists(bundled))
        {
            var handle = loadPath(bundled);
            if (handle != 0)
            {
                return handle;
            }
        }
        var probed = loadDefault();
        if (probed != 0)
        {
            return probed;
        }
        throw new HegelException(
            $"Could not find libhegel: no library at {bundled} (the Hegel package bundles one for osx-arm64, linux-x64, linux-arm64, win-x64 and win-arm64; this process is {rid}), and the .NET host found none. Set {PathVariable} to the path of a libhegel built for this platform.");
    }

    /// <summary>Warn once when the loaded engine is not the version these bindings were built against.</summary>
    internal static void WarnOnVersionMismatch(string actual, string expected, TextWriter err)
    {
        if (actual != expected)
        {
            err.WriteLine($"[hegel] warning: libhegel {actual} loaded, but these bindings were built against {expected}; set {PathVariable} to a matching engine.");
        }
    }
}
