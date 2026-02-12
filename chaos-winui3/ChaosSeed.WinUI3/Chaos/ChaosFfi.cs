using System.Runtime.InteropServices;

namespace ChaosSeed.WinUI3.Chaos;

internal static partial class ChaosFfi
{
    private const string Dll = "chaos_ffi";

    [LibraryImport(Dll)]
    internal static partial uint chaos_ffi_api_version();

    [LibraryImport(Dll)]
    internal static partial IntPtr chaos_ffi_last_error_json();

    [LibraryImport(Dll)]
    internal static partial void chaos_ffi_string_free(IntPtr s);

    [LibraryImport(Dll)]
    internal static partial IntPtr chaos_now_playing_snapshot_json(
        byte include_thumbnail,
        uint max_thumbnail_bytes,
        uint max_sessions
    );

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_livestream_decode_manifest_json(
        string input_utf8,
        byte drop_inaccessible_high_qualities
    );

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_livestream_resolve_variant2_json(
        string site_utf8,
        string room_id_utf8,
        string variant_id_utf8
    );

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate void chaos_danmaku_callback(IntPtr event_json_utf8, IntPtr user_data);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_danmaku_connect(string input_utf8);

    [LibraryImport(Dll)]
    internal static partial int chaos_danmaku_set_callback(
        IntPtr handle,
        chaos_danmaku_callback? cb,
        IntPtr user_data
    );

    [LibraryImport(Dll)]
    internal static partial int chaos_danmaku_disconnect(IntPtr handle);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_lyrics_search_json(
        string title_utf8,
        string? album_utf8_or_null,
        string? artist_utf8_or_null,
        uint duration_ms_or_0,
        uint limit,
        byte strict_match,
        string? services_csv_utf8_or_null,
        uint timeout_ms
    );

    internal static string? TakeString(IntPtr p)
    {
        if (p == IntPtr.Zero)
        {
            return null;
        }

        try
        {
            return Marshal.PtrToStringUTF8(p);
        }
        finally
        {
            chaos_ffi_string_free(p);
        }
    }

    internal static string? TakeLastErrorJson()
    {
        var p = chaos_ffi_last_error_json();
        return TakeString(p);
    }
}
