using System.Reflection;
using System.Runtime.CompilerServices;

namespace Hegel;

internal static class AsyncBodies
{
    internal static bool IsAsync(MethodInfo method) => method.IsDefined(typeof(AsyncStateMachineAttribute), false);

    internal static string Message(string what) =>
        $"{what} is async, but Hegel runs it synchronously: it would return at its first await and the rest would run after the test case had ended. Make it synchronous, blocking on any task it needs with task.GetAwaiter().GetResult().";
}
