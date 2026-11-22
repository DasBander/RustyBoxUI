using System.Text.Json;

namespace RustyBoxUI;

/// <summary>A WebView2 view owned by the creating Windows UI thread.</summary>
public sealed class View : IDisposable
{
    private readonly Thread owner = Thread.CurrentThread;
    private readonly ViewHandle handle;
    private readonly Dictionary<string, Func<JsonElement, object?>> handlers = new();
    private static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);
    public event Action<JsonElement>? RuntimeEvent;
    private View(ulong handle) => this.handle = new(handle);

    /// <summary>Open from a running WinForms or WPF dispatcher. Startup yields to its STA message loop.</summary>
    public static async Task<View> OpenAsync(nint parent, string title, string assets, string profile,
        uint width = 800, uint height = 500, CancellationToken cancellation = default)
    {
        if (Thread.CurrentThread.GetApartmentState() != ApartmentState.STA ||
            SynchronizationContext.Current is null || SynchronizationContext.Current.GetType() == typeof(SynchronizationContext))
            throw new InvalidOperationException("Open the view on a running STA UI dispatcher.");
        cancellation.ThrowIfCancellationRequested();
        var ticket = Native.Begin(parent, title, assets, profile, width, height);
        while (true)
        {
            if (cancellation.IsCancellationRequested)
            {
                Native.Check(Native.rb_create_cancel(ticket));
                while (Native.rb_collect_startups() == Native.Pending) await Task.Delay(10);
                cancellation.ThrowIfCancellationRequested();
            }
            int status = Native.rb_create_status(ticket);
            if (status == 0) return new View(ticket);
            if (status != Native.Pending) Native.Check(status);
            await Task.Delay(10);
        }
    }

    public void Register<TRequest, TReply>(string method, Func<TRequest, TReply> handler)
    {
        RequireOwner();
        if (!method.StartsWith("host.", StringComparison.Ordinal))
            throw new ArgumentException("Host commands must begin with host.", nameof(method));
        handlers.Add(method, value => handler(value.Deserialize<TRequest>(Json)!));
    }

    /// <summary>Call regularly from the native UI loop. Returns false after the view closes.</summary>
    public bool Pump()
    {
        RequireOwner();
        ViewHandle.Collect();
        int status = Native.rb_tick(handle);
        if (status == Native.Closed) return false;
        Native.Check(status);
        for (var i = 0; i < 64 && !handle.IsClosed; ++i)
        {
            status = Native.rb_next_event(handle, null, 0, out var size);
            if (status == Native.Empty) break;
            if (status != Native.BufferTooSmall) Native.Check(status);
            var bytes = new byte[checked((int)size)];
            Native.Check(Native.rb_next_event(handle, bytes, size, out _));
            using var doc = JsonDocument.Parse(bytes);
            var message = doc.RootElement;
            if (message.GetProperty("event").GetString() == "invoke") Reply(message.GetProperty("request"));
            else RuntimeEvent?.Invoke(message.Clone());
        }
        return !handle.IsClosed;
    }

    private void Reply(JsonElement request)
    {
        var id = request.GetProperty("id").GetUInt64();
        var session = request.GetProperty("session").GetString();
        var method = request.GetProperty("method").GetString()!;
        byte[] bytes;
        try
        {
            if (!handlers.TryGetValue(method, out var handler)) throw new KeyNotFoundException($"Unknown command {method}");
            var result = handler(request.GetProperty("params"));
            bytes = JsonSerializer.SerializeToUtf8Bytes(new { v = 1, id, session, ok = true, result }, Json);
        }
        catch (Exception ex)
        {
            var error = new { code = "COMMAND_FAILED", message = ex.Message };
            bytes = JsonSerializer.SerializeToUtf8Bytes(new { v = 1, id, session, ok = false, error }, Json);
        }
        if (!handle.IsClosed) Native.Check(Native.rb_post_json(handle, bytes, (nuint)bytes.Length));
    }

    public void Resize(uint width, uint height)
    {
        RequireOwner();
        if (width > 0 && height > 0) Native.Check(Native.rb_resize(handle, width, height));
    }

    private void RequireOwner()
    {
        ObjectDisposedException.ThrowIf(handle.IsClosed, this);
        if (Thread.CurrentThread != owner) throw new InvalidOperationException("Use the view on its creating UI thread.");
    }

    public void Dispose()
    {
        if (handle.IsClosed) return;
        RequireOwner();
        handle.Dispose();
        ViewHandle.Collect();
    }
}
