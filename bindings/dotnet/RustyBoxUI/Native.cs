using System.Runtime.InteropServices;
using System.Text;

namespace RustyBoxUI;

// The layout and numeric status codes match include/rustybox.h (ABI 1, Windows x64).
internal static unsafe class Native
{
    private const string Dll = "rustybox_ui";
    internal const int Closed = 1, Empty = 2, Pending = 3, BufferTooSmall = -5;

    [StructLayout(LayoutKind.Sequential)]
    private struct Config
    {
        public uint Size, Abi;
        public nint Parent;
        public uint Width, Height, Flags, Reserved;
        public byte* Title;
        public nuint TitleLength;
        public byte* Assets;
        public nuint AssetsLength;
        public byte* Profile;
        public nuint ProfileLength;
    }

    [DllImport(Dll)] private static extern uint rb_abi_version();
    [DllImport(Dll)] private static extern int rb_create_begin(in Config config, uint timeoutMs, out ulong ticket);
    [DllImport(Dll)] internal static extern int rb_create_status(ulong ticket);
    [DllImport(Dll)] internal static extern int rb_create_cancel(ulong ticket);
    [DllImport(Dll)] internal static extern int rb_collect_startups();
    [DllImport(Dll)] internal static extern int rb_destroy(ulong view);
    [DllImport(Dll)] internal static extern int rb_tick(ViewHandle view);
    [DllImport(Dll)] internal static extern int rb_resize(ViewHandle view, uint width, uint height);
    [DllImport(Dll)] internal static extern int rb_post_json(ViewHandle view, byte[] json, nuint size);
    [DllImport(Dll)] internal static extern int rb_next_event(ViewHandle view, [Out] byte[]? data, nuint size, out nuint written);
    [DllImport(Dll)] private static extern nuint rb_last_error([Out] byte[]? data, nuint size);

    internal static ulong Begin(nint parent, string title, string assets, string profile, uint width, uint height)
    {
        if (RuntimeInformation.ProcessArchitecture != Architecture.X64 || rb_abi_version() != 1)
            throw new PlatformNotSupportedException("RustyBoxUI requires Windows x64 and ABI 1.");
        var t = Encoding.UTF8.GetBytes(title);
        var a = Encoding.UTF8.GetBytes(Path.GetFullPath(assets));
        var p = Encoding.UTF8.GetBytes(Path.GetFullPath(profile));
        fixed (byte* titlePtr = t, assetsPtr = a, profilePtr = p)
        {
            var config = new Config {
                Size = (uint)sizeof(Config), Abi = 1, Parent = parent, Width = width, Height = height,
                Flags = 16, Title = titlePtr, TitleLength = (nuint)t.Length, Assets = assetsPtr,
                AssetsLength = (nuint)a.Length, Profile = profilePtr, ProfileLength = (nuint)p.Length
            };
            Check(rb_create_begin(in config, 30000, out var ticket));
            return ticket;
        }
    }

    internal static void Check(int status)
    {
        if (status == 0) return;
        var size = rb_last_error(null, 0);
        var bytes = new byte[checked((int)size + 1)];
        rb_last_error(bytes, (nuint)bytes.Length);
        throw new InvalidOperationException($"RustyBoxUI ({status}): {Encoding.UTF8.GetString(bytes, 0, (int)size)}");
    }
}
