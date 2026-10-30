namespace Hegel;

/// <summary>
/// Where a test is defined, as reported to the engine (and through it to Antithesis): the source
/// file, the line it begins on, the enclosing class and the method.
/// </summary>
/// <param name="File">The source file, relative to the project root where that can be found.</param>
/// <param name="BeginLine">The line the test begins on (1-based; 0 when unknown).</param>
/// <param name="ClassName">The full name of the enclosing class.</param>
/// <param name="Function">The method's name.</param>
public sealed record TestLocation(string File, int BeginLine, string ClassName, string Function);
