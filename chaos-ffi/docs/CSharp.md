# C#（WinUI3/Console）P/Invoke 示例

这是一个最小示例，展示如何在 .NET 7+ 中调用 `chaos_ffi.dll`。

注意事项：
- 所有字符串均为 UTF-8。
- 凡是以字符串形式返回的 `IntPtr`，都必须用 `chaos_ffi_string_free` 释放。
- 弹幕回调在后台线程触发（不是 UI 线程）。

## 互操作定义

```csharp
using System;
using System.IO;
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

    [LibraryImport(Dll)]
    internal static partial IntPtr chaos_now_playing_snapshot_json(
        byte include_thumbnail,
        uint max_thumbnail_bytes,
        uint max_sessions);

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

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_lyrics_search_json(
        string title_utf8,
        string? album_utf8_or_null,
        string? artist_utf8_or_null,
        uint duration_ms_or_0,
        uint limit,
        byte strict_match,
        string? services_csv_utf8_or_null,
        uint timeout_ms);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_live_dir_categories_json(string site_utf8);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_live_dir_recommend_rooms_json(string site_utf8, uint page);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_live_dir_category_rooms_json(
        string site_utf8,
        string? parent_id_utf8_or_null,
        string category_id_utf8,
        uint page);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_live_dir_search_rooms_json(string site_utf8, string keyword_utf8, uint page);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_livestream_decode_manifest_json(
        string input_utf8,
        byte drop_inaccessible_high_qualities);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_livestream_resolve_variant_json(
        string input_utf8,
        string variant_id_utf8);

    [LibraryImport(Dll, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr chaos_livestream_resolve_variant2_json(
        string site_utf8,
        string room_id_utf8,
        string variant_id_utf8);

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

## 系统媒体（Win11 Now Playing）

```csharp
static string TakeOrThrow(IntPtr p, string what)
{
    var s = ChaosFfi.TakeString(p);
    if (!string.IsNullOrEmpty(s)) return s;
    var err = ChaosFfi.TakeString(ChaosFfi.chaos_ffi_last_error_json());
    throw new Exception($"{what} failed: {err}");
}

// include_thumbnail=1, max_thumbnail_bytes=256KB, max_sessions=32
var json = TakeOrThrow(ChaosFfi.chaos_now_playing_snapshot_json(1, 262_144, 32), "now_playing_snapshot");
using var doc = JsonDocument.Parse(json);
var root = doc.RootElement;
Console.WriteLine("supported=" + root.GetProperty("supported").GetBoolean());
Console.WriteLine("sessions=" + root.GetProperty("sessions").GetArrayLength());

if (root.TryGetProperty("now_playing", out var np) && np.ValueKind != JsonValueKind.Null)
{
    Console.WriteLine("app_id=" + np.GetProperty("app_id").GetString());
    Console.WriteLine("title=" + np.GetProperty("title").GetString());
    Console.WriteLine("artist=" + np.GetProperty("artist").GetString());

    if (np.TryGetProperty("thumbnail", out var th) && th.ValueKind == JsonValueKind.Object)
    {
        var b64 = th.GetProperty("base64").GetString() ?? "";
        var bytes = Convert.FromBase64String(b64);
        Console.WriteLine("thumbnail_bytes=" + bytes.Length);
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

## 字幕下载

`chaos_subtitle_download_item_json` 的 `item_json_utf8` 建议直接传 `search` 返回数组中某个元素的 `GetRawText()`。

```csharp
static string TakeOrThrow(IntPtr p, string what)
{
    var s = ChaosFfi.TakeString(p);
    if (!string.IsNullOrEmpty(s)) return s;
    var err = ChaosFfi.TakeString(ChaosFfi.chaos_ffi_last_error_json());
    throw new Exception($"{what} failed: {err}");
}

var searchJson = TakeOrThrow(ChaosFfi.chaos_subtitle_search_json("三体", 10, -1.0, "zh", 20000), "subtitle_search");
using var doc = JsonDocument.Parse(searchJson);
var arr = doc.RootElement;
if (arr.GetArrayLength() == 0) throw new Exception("no subtitle items");

var itemJson = arr[0].GetRawText();
var outDir = Path.Combine(Path.GetTempPath(), "chaos-seed-subs");
Directory.CreateDirectory(outDir);

var dlJson = TakeOrThrow(
    ChaosFfi.chaos_subtitle_download_item_json(
        itemJson,
        outDir,
        20000, // timeout_ms
        2,     // retries
        1      // overwrite
    ),
    "subtitle_download"
);
Console.WriteLine(dlJson); // {"path":"...","bytes":12345}
```

## 歌词搜索

```csharp
static string TakeOrThrow(IntPtr p, string what)
{
    var s = ChaosFfi.TakeString(p);
    if (!string.IsNullOrEmpty(s)) return s;
    var err = ChaosFfi.TakeString(ChaosFfi.chaos_ffi_last_error_json());
    throw new Exception($"{what} failed: {err}");
}

var json = TakeOrThrow(
    ChaosFfi.chaos_lyrics_search_json(
        "Hello",
        "Hello",
        "Adele",
        296_000,
        5,
        1, // strict_match
        "netease,qq,kugou",
        10_000),
    "lyrics_search");

using var doc = JsonDocument.Parse(json);
var arr = doc.RootElement;
Console.WriteLine("results=" + arr.GetArrayLength());

for (int i = 0; i < Math.Min(3, arr.GetArrayLength()); i++)
{
    var it = arr[i];
    Console.WriteLine($"#{i} service={it.GetProperty(\"service\").GetString()} quality={it.GetProperty(\"quality\").GetDouble():0.0000}");
    Console.WriteLine($"    title={it.GetProperty(\"title\").GetString()} artist={it.GetProperty(\"artist\").GetString()}");
}

if (arr.GetArrayLength() > 0)
{
    var best = arr[0];
    var lyrics = best.GetProperty("lyrics_original").GetString() ?? "";
    Console.WriteLine("best_lyrics_prefix=" + lyrics.Substring(0, Math.Min(200, lyrics.Length)));
}
```

## 直播源解析（manifest + 二段解析）

```csharp
static string TakeOrThrow(IntPtr p, string what)
{
    var s = ChaosFfi.TakeString(p);
    if (!string.IsNullOrEmpty(s)) return s;
    var err = ChaosFfi.TakeString(ChaosFfi.chaos_ffi_last_error_json());
    throw new Exception($"{what} failed: {err}");
}

var input = "https://live.bilibili.com/1";

// 1) decode manifest
var manifestJson = TakeOrThrow(ChaosFfi.chaos_livestream_decode_manifest_json(input, 1), "decode_manifest");
Console.WriteLine(manifestJson);

// 2) pick a variant_id from manifest.variants[i].id, then resolve (optional, platform-dependent)
// Prefer resolve_variant2_json with manifest.site + manifest.room_id (canonical rid/long id).
// Note (BiliLive): some rooms may ignore qn in the first response; resolve_variant* will fallback
// to another endpoint to fetch the real URL for the requested qn, or fail if that qn is inaccessible.
var variantId = "bili_live:2000:原画";
using var manDoc = JsonDocument.Parse(manifestJson);
var site = manDoc.RootElement.GetProperty("site").GetString() ?? "";
var roomId = manDoc.RootElement.GetProperty("room_id").GetString() ?? "";
var variantJson = TakeOrThrow(ChaosFfi.chaos_livestream_resolve_variant2_json(site, roomId, variantId), "resolve_variant2");
Console.WriteLine(variantJson);

// Player-side hints:
// - Use manifest.playback.referer / user_agent as request headers.
// - Prefer variant.url; fallback to variant.backup_urls.
```

## 直播目录（首页/分类）

```csharp
static string TakeOrThrow(IntPtr p, string what)
{
    var s = ChaosFfi.TakeString(p);
    if (!string.IsNullOrEmpty(s)) return s;
    var err = ChaosFfi.TakeString(ChaosFfi.chaos_ffi_last_error_json());
    throw new Exception($"{what} failed: {err}");
}

var site = "bili_live";

// 1) categories (for 分类页)
var categoriesJson = TakeOrThrow(ChaosFfi.chaos_live_dir_categories_json(site), "live_dir_categories");
Console.WriteLine(categoriesJson);

// 2) recommend rooms (for 首页)
var recJson = TakeOrThrow(ChaosFfi.chaos_live_dir_recommend_rooms_json(site, 1), "live_dir_recommend_rooms");
using var recDoc = JsonDocument.Parse(recJson);
Console.WriteLine("items=" + recDoc.RootElement.GetProperty("items").GetArrayLength());

// 3) search (for 首页搜索)
var searchJson = TakeOrThrow(ChaosFfi.chaos_live_dir_search_rooms_json(site, "lol", 1), "live_dir_search_rooms");
Console.WriteLine(searchJson);
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

var rc = ChaosFfi.chaos_danmaku_disconnect(handle);
if (rc != 0)
    throw new Exception("disconnect failed: " + ChaosFfi.TakeString(ChaosFfi.chaos_ffi_last_error_json()));
```

在 WinUI 3 应用中，请把 `Console.WriteLine` 这类 UI/日志更新逻辑 marshal 到 `DispatcherQueue`/UI 线程。
