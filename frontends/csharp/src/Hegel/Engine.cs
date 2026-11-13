using Hegel.Native;

namespace Hegel;

/// <summary>
/// Process-wide holder for the loaded engine. It caches only the binding, which is immutable and
/// safe to share; every run gets its own engine state. Tests may install a fake binding and
/// restore the real one; <see cref="Real"/> always answers with the real binding, for
/// computations such as <see cref="Label"/>'s that must agree with the engine.
/// </summary>
internal static class Engine
{
    private static readonly object Gate = new();
    private static ILibhegel? _loaded;
    private static ILibhegel? _override;

    internal static ILibhegel Get()
    {
        lock (Gate)
        {
            return _override ?? Real();
        }
    }

    internal static ILibhegel Real()
    {
        lock (Gate)
        {
            return _loaded ??= Libhegel.Load(Console.Error);
        }
    }

    internal static void SetForTesting(ILibhegel? binding)
    {
        lock (Gate)
        {
            _override = binding;
        }
    }
}
