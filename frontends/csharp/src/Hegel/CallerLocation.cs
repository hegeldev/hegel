using System.Reflection;
using System.Runtime.CompilerServices;

namespace Hegel;

internal static class CallerLocation
{
    internal static TestLocation Of(Delegate body, string file, int line, string member) =>
        new(ProjectPath(file), line, ClassName(body), member);

    internal static string DatabaseKey(TestLocation location) => $"{location.File}::{location.Function}";

    internal static string ClassName(Delegate body)
    {
        var type = UserType(body);
        while (type != null && (type.IsDefined(typeof(CompilerGeneratedAttribute), false) || type.Name.Contains('<') || type.Name.Contains('@')))
        {
            type = type.DeclaringType;
        }
        return type?.FullName ?? "";
    }

    private static Type? UserType(Delegate body)
    {
        var declaring = body.Method.DeclaringType;
        if (declaring == null || body.Target == null || !HegelInfrastructureAttribute.Marks(declaring.Assembly))
        {
            return declaring;
        }
        foreach (var field in body.Target.GetType().GetFields(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
        {
            var captured = field.GetValue(body.Target) switch
            {
                Delegate inner => UserType(inner),
                { } value => value.GetType(),
                null => null,
            };
            if (captured != null && Runner.IsUserFrame(captured, []))
            {
                return captured;
            }
        }
        return declaring;
    }

    internal static string ProjectPath(string file)
    {
        if (file.Length == 0)
        {
            return "";
        }
        var root = ProjectRoot(Path.GetDirectoryName(file));
        var relative = root == null ? Path.GetFileName(file) : Path.GetRelativePath(root, file);
        return relative.Replace('\\', '/');
    }

    internal static string? ProjectRoot(string? directory)
    {
        for (var dir = directory; dir != null; dir = Path.GetDirectoryName(dir))
        {
            if (!Directory.Exists(dir))
            {
                continue;
            }
            if (Directory.Exists(Path.Combine(dir, ".git")) || File.Exists(Path.Combine(dir, ".git"))
                || Directory.EnumerateFiles(dir).Any(f => f.EndsWith(".sln", StringComparison.OrdinalIgnoreCase) || f.EndsWith(".slnx", StringComparison.OrdinalIgnoreCase)))
            {
                return dir;
            }
        }
        return null;
    }
}
