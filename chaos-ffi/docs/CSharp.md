# C# (WinUI3/Console) P/Invoke Example

This is a minimal example showing how to call `chaos_ffi.dll` from .NET 7+.

Notes:
- All strings are UTF-8.
- Any `IntPtr` returned as a string must be freed with `chaos_ffi_string_free`.
- Danmaku callbacks are invoked on a background thread.

## Interop definitions

```csharp
using System;
using System.Runtime.InteropServices;
using System.Text.Json;

internal static partial class ChaosFfi
{
    private const string Dll = "chaos_ffi";

    [LibraryImport(Dll)]
    internal static partial uint chaos_ffi_api_version();

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_ffi_version_json();

    [LibraryImport(Dll)]
    internal static partial IntPtr chaos_ffi_last_error_json();

    [LibraryImport(Dll)]
    internal static partial void chaos_ffi_string_free(IntPtr s);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_subtitle_search_json(
        string query_utf8,
        uint limit,
        double min_score_or_neg1,
        string? lang_utf8_or_null,
        uint timeout_ms);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_subtitle_download_item_json(
        string item_json_utf8,
        string out_dir_utf8,
        uint timeout_ms,
        uint retries,
        byte overwrite);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate void chaos_danmaku_callback(IntPtr event_json_utf8, IntPtr user_data);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_danmaku_connect(string input_utf8);

    [LibraryImport(Dll)]
    internal static partial int chaos_danmaku_set_callback(IntPtr handle, chaos_danmaku_callback? cb, IntPtr user_data);

    [LibraryImport(Dll)]
    internal static partial IntPtr chaos_danmaku_poll_json(IntPtr handle, uint max_events);

    [LibraryImport(Dll)]
    internal static partial int chaos_danmaku_disconnect(IntPtr handle);

    internal static string? TakeString(IntPtr p)
    {
        if (p == IntPtr.Zero) return null;
        try { return Marshal.PtrToStringUTF8(p); }
        finally { chaos_ffi_string_free(p); }
    }
}
```

## Subtitle search

```csharp
var p = ChaosFfi.chaos_subtitle_search_json("三体", 20, -1.0, null, 20000);
var json = ChaosFfi.TakeString(p) ?? throw new Exception("search failed: " + ChaosFfi.TakeString(ChaosFfi.chaos_ffi_last_error_json()));

var items = JsonDocument.Parse(json).RootElement;
Console.WriteLine("items=" + items.GetArrayLength());
```

## Danmaku (callback + poll)

```csharp
var handle = ChaosFfi.chaos_danmaku_connect("https://live.bilibili.com/1");
if (handle == IntPtr.Zero)
    throw new Exception("connect failed: " + ChaosFfi.TakeString(ChaosFfi.chaos_ffi_last_error_json()));

ChaosFfi.chaos_danmaku_callback cb = (p, ud) =>
{
    var s = Marshal.PtrToStringUTF8(p);
    Console.WriteLine("[cb] " + s);
};

ChaosFfi.chaos_danmaku_set_callback(handle, cb, IntPtr.Zero);

for (int i = 0; i < 20; i++)
{
    var pj = ChaosFfi.chaos_danmaku_poll_json(handle, 50);
    var arrJson = ChaosFfi.TakeString(pj);
    if (!string.IsNullOrEmpty(arrJson))
        Console.WriteLine("[poll] " + arrJson);
    await Task.Delay(500);
}

ChaosFfi.chaos_danmaku_disconnect(handle);
```

In a WinUI 3 app, marshal `Console.WriteLine` parts onto `DispatcherQueue`/UI thread.

