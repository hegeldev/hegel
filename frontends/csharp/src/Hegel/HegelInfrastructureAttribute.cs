using System.Reflection;

namespace Hegel;

/// <summary>
/// Marks an assembly as part of Hegel rather than of the test under it: a failure's origin skips
/// its frames, as it skips the test framework's. Hegel's own frontends carry it; a library of
/// generators or a test-framework integration built on Hegel may too.
/// </summary>
[AttributeUsage(AttributeTargets.Assembly)]
public sealed class HegelInfrastructureAttribute : Attribute
{
    internal static bool Marks(Assembly assembly) =>
        assembly == typeof(HegelInfrastructureAttribute).Assembly || assembly.IsDefined(typeof(HegelInfrastructureAttribute), false);
}
