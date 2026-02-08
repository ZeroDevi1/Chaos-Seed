# C#（WinUI3/Console）P/Invoke 示例

这是一个最小示例，展示如何在 .NET 7+ 中调用 `chaos_ffi.dll`。

注意事项：
- 所有字符串均为 UTF-8。
- 凡是以字符串形式返回的 `IntPtr`，都必须用 `chaos_ffi_string_free` 释放。
- 弹幕回调在后台线程触发（不是 UI 线程）。

## 互操作定义

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

## 字幕搜索

```csharp
var p = ChaosFfi.chaos_subtitle_search_json("三体", 20, -1.0, null, 20000);
var json = ChaosFfi.TakeString(p) ?? throw new Exception("search failed: " + ChaosFfi.TakeString(ChaosFfi.chaos_ffi_last_error_json()));

var items = JsonDocument.Parse(json).RootElement;
Console.WriteLine("items=" + items.GetArrayLength());
```

## 弹幕（callback + poll）

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

在 WinUI 3 应用中，请把 `Console.WriteLine` 这类 UI/日志更新逻辑 marshal 到 `DispatcherQueue`/UI 线程。
