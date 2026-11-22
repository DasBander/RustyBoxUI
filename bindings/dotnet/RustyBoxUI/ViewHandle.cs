using System.Collections.Concurrent;
using System.Runtime.InteropServices;

namespace RustyBoxUI;

internal sealed class ViewHandle : SafeHandle
{
    [ThreadStatic] private static ConcurrentQueue<ulong>? pending;
    private readonly ConcurrentQueue<ulong> ownerQueue = pending ??= new();
    private readonly Thread owner = Thread.CurrentThread;

    internal ViewHandle(ulong value) : base(IntPtr.Zero, true) => SetHandle((nint)value);
    public override bool IsInvalid => handle == 0;

    // SafeHandle finalization can run on a worker. Native destruction belongs to the creating STA.
    protected override bool ReleaseHandle()
    {
        if (Thread.CurrentThread == owner) return Native.rb_destroy((ulong)handle) == 0;
        ownerQueue.Enqueue((ulong)handle);
        return true;
    }

    internal static void Collect()
    {
        while (pending?.TryDequeue(out var value) == true) Native.Check(Native.rb_destroy(value));
        Native.rb_collect_startups();
    }
}
