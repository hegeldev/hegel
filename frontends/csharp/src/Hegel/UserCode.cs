using System.Runtime.CompilerServices;

namespace Hegel;

/// <summary>
/// Every call from the library into code it was given. A failure's origin is the innermost frame
/// that is not the library's, and the optimising JIT can inline a callee into a library frame,
/// which removes the callee's frame from the stack trace and so changes the origin part-way
/// through a run. These methods are never optimised, so nothing is inlined into them.
/// </summary>
internal static class UserCode
{
    private const MethodImplOptions Unoptimised = MethodImplOptions.NoOptimization | MethodImplOptions.NoInlining;

    [MethodImpl(Unoptimised)]
    internal static void Call(Action f) => f();

    [MethodImpl(Unoptimised)]
    internal static void Call<T>(Action<T> f, T a) => f(a);

    [MethodImpl(Unoptimised)]
    internal static void Call<T1, T2>(Action<T1, T2> f, T1 a, T2 b) => f(a, b);

    [MethodImpl(Unoptimised)]
    internal static TResult Call<TResult>(Func<TResult> f) => f();

    [MethodImpl(Unoptimised)]
    internal static TResult Call<T, TResult>(Func<T, TResult> f, T a) => f(a);

    [MethodImpl(Unoptimised)]
    internal static TResult Call<T1, T2, TResult>(Func<T1, T2, TResult> f, T1 a, T2 b) => f(a, b);

    [MethodImpl(Unoptimised)]
    internal static TResult Call<T1, T2, T3, TResult>(Func<T1, T2, T3, TResult> f, T1 a, T2 b, T3 c) => f(a, b, c);

    [MethodImpl(Unoptimised)]
    internal static T Draw<T>(Generator<T> generator, TestCase tc) => generator.DoDraw(tc);

    [MethodImpl(Unoptimised)]
    internal static T DrawAndPrint<T>(PrintableGenerator<T> generator, TestCase tc, Printer printer) => generator.DoDrawAndPrint(tc, printer);
}
