# C#（IPC / StreamJsonRpc）调用示例

本示例展示如何从 .NET（WinUI3/Console）通过 **NamedPipe + JSON-RPC（LSP framing）** 调用 `chaos-daemon` 的全部公开方法，并接收 `danmaku.message` 通知。

依赖：
- NuGet：`StreamJsonRpc`

注意：
- 目前 daemon 对所有请求都要求 `params` 存在；即使没有参数也请传 `new { }`。
- 通知（notification）是没有 `id` 的 server push；用 `AddLocalRpcTarget` 接收。

## 最小客户端（启动 + 连接 + 鉴权 + 调用 + 通知）

```csharp
using System;
using System.Diagnostics;
using System.IO.Pipes;
using System.Text;
using System.Threading.Tasks;
using StreamJsonRpc;
using StreamJsonRpc.Protocol;

static async Task Main()
{
    var authToken = Guid.NewGuid().ToString("N");
    var pipeName = $"chaos-seed-{Guid.NewGuid():N}";

    var daemonExe = Path.Combine(AppContext.BaseDirectory, "chaos-daemon.exe");
    _ = Process.Start(new ProcessStartInfo
    {
        FileName = daemonExe,
        Arguments = $"--pipe-name {pipeName} --auth-token {authToken}",
        UseShellExecute = false,
        CreateNoWindow = true,
        RedirectStandardOutput = true,
        RedirectStandardError = true,
        StandardOutputEncoding = Encoding.UTF8,
        StandardErrorEncoding = Encoding.UTF8,
    }) ?? throw new Exception("failed to start daemon");

    using var pipe = new NamedPipeClientStream(".", pipeName, PipeDirection.InOut, PipeOptions.Asynchronous);
    await pipe.ConnectAsync(TimeSpan.FromSeconds(10));

    var formatter = new JsonMessageFormatter(); // JSON.NET
    var handler = new HeaderDelimitedMessageHandler(pipe, pipe, formatter); // LSP framing
    using var rpc = new JsonRpc(handler);

    // 0) Subscribe to notifications (must be registered before StartListening)
    var sink = new DanmakuSink();
    sink.OnMessage += m => Console.WriteLine($"[danmaku] sid={m.SessionId} user={m.User} text={m.Text}");
    rpc.AddLocalRpcTarget(sink);

    rpc.StartListening();

    // 1) Authenticate (required)
    var ping = await rpc.InvokeWithParameterObjectAsync<PingResult>(
        "daemon.ping",
        new { authToken }
    );
    Console.WriteLine("daemon version=" + ping.Version);

    // 2) nowPlaying.snapshot
    var nowPlaying = await rpc.InvokeWithParameterObjectAsync<NowPlayingSnapshot>(
        "nowPlaying.snapshot",
        new { includeThumbnail = false, maxThumbnailBytes = 262_144, maxSessions = 32 }
    );
    Console.WriteLine("nowPlaying supported=" + nowPlaying.Supported);

    // 3) lyrics.search
    var lyrics = await rpc.InvokeWithParameterObjectAsync<LyricsSearchResult[]>(
        "lyrics.search",
        new
        {
            title = "Hello",
            album = (string?)null,
            artist = "Adele",
            durationMs = 296_000,
            limit = 5,
            strictMatch = true,
            services = new[] { "netease", "qq", "kugou" },
            timeoutMs = 10_000
        }
    );
    Console.WriteLine("lyrics results=" + lyrics.Length);

    // 4) livestream.decodeManifest
    var input = "https://live.bilibili.com/1";
    var manifest = await rpc.InvokeWithParameterObjectAsync<LivestreamDecodeManifestResult>(
        "livestream.decodeManifest",
        new { input }
    );
    Console.WriteLine("manifest variants=" + (manifest.Variants?.Length ?? 0));

    // 5) liveDir.*
    var site = "bili_live";
    var categories = await rpc.InvokeWithParameterObjectAsync<LiveDirCategory[]>(
        "liveDir.categories",
        new { site }
    );
    Console.WriteLine("categories=" + categories.Length);

    var rec = await rpc.InvokeWithParameterObjectAsync<LiveDirRoomListResult>(
        "liveDir.recommendRooms",
        new { site, page = 1 }
    );
    Console.WriteLine("recommend items=" + (rec.Items?.Length ?? 0));

    // 6) live.open (will also start danmaku.message notifications)
    var live = await rpc.InvokeWithParameterObjectAsync<LiveOpenResult>(
        "live.open",
        new
        {
            input,
            preferredQuality = "highest", // or "lowest"; optional
            variantId = (string?)null      // optional, pick a concrete variants[i].id if you want
        }
    );
    Console.WriteLine("live url=" + live.Url);

    // 7) Optionally fetch image for image danmaku
    // if (someMessage.ImageUrl != null) { ... }
    // var img = await rpc.InvokeWithParameterObjectAsync<DanmakuFetchImageResult>(
    //     "danmaku.fetchImage",
    //     new { sessionId = live.SessionId, url = someMessage.ImageUrl }
    // );

    await Task.Delay(10_000);

    // 8) Close live session
    var ok = await rpc.InvokeWithParameterObjectAsync<OkReply>(
        "live.close",
        new { sessionId = live.SessionId }
    );
    Console.WriteLine("closed ok=" + ok.Ok);
}

public sealed class DanmakuSink
{
    public event Action<DanmakuMessage>? OnMessage;

    [JsonRpcMethod("danmaku.message")]
    public void OnDanmakuMessage(DanmakuMessage msg) => OnMessage?.Invoke(msg);
}

public sealed class PingResult { public string Version { get; set; } = ""; }
public sealed class OkReply { public bool Ok { get; set; } }

public sealed class LiveOpenResult
{
    public string SessionId { get; set; } = "";
    public string Site { get; set; } = "";
    public string RoomId { get; set; } = "";
    public string Title { get; set; } = "";
    public string VariantId { get; set; } = "";
    public string VariantLabel { get; set; } = "";
    public string Url { get; set; } = "";
    public string[] BackupUrls { get; set; } = Array.Empty<string>();
    public string? Referer { get; set; }
    public string? UserAgent { get; set; }
}

public sealed class LivestreamDecodeManifestResult
{
    public string Site { get; set; } = "";
    public string RoomId { get; set; } = "";
    public string RawInput { get; set; } = "";
    public LivestreamInfo Info { get; set; } = new();
    public LivestreamPlaybackHints Playback { get; set; } = new();
    public LivestreamVariant[] Variants { get; set; } = Array.Empty<LivestreamVariant>();
}

public sealed class LivestreamInfo
{
    public string Title { get; set; } = "";
    public string? Name { get; set; }
    public string? Avatar { get; set; }
    public string? Cover { get; set; }
    public bool IsLiving { get; set; }
}

public sealed class LivestreamPlaybackHints
{
    public string? Referer { get; set; }
    public string? UserAgent { get; set; }
}

public sealed class LivestreamVariant
{
    public string Id { get; set; } = "";
    public string Label { get; set; } = "";
    public int Quality { get; set; }
    public int? Rate { get; set; }
    public string? Url { get; set; }
    public string[] BackupUrls { get; set; } = Array.Empty<string>();
}

public sealed class NowPlayingSnapshot
{
    public bool Supported { get; set; }
    public NowPlayingSession? NowPlaying { get; set; }
    public NowPlayingSession[] Sessions { get; set; } = Array.Empty<NowPlayingSession>();
    public string? PickedAppId { get; set; }
    public ulong RetrievedAtUnixMs { get; set; }
}

public sealed class NowPlayingSession
{
    public string AppId { get; set; } = "";
    public bool IsCurrent { get; set; }
    public string PlaybackStatus { get; set; } = "";
    public string? Title { get; set; }
    public string? Artist { get; set; }
    public string? AlbumTitle { get; set; }
    public ulong? PositionMs { get; set; }
    public ulong? DurationMs { get; set; }
    public string[] Genres { get; set; } = Array.Empty<string>();
    public string? SongId { get; set; }
    public NowPlayingThumbnail? Thumbnail { get; set; }
    public string? Error { get; set; }
}

public sealed class NowPlayingThumbnail
{
    public string Mime { get; set; } = "";
    public string Base64 { get; set; } = "";
}

public sealed class LyricsSearchResult
{
    public string Service { get; set; } = "";
    public string ServiceToken { get; set; } = "";
    public string? Title { get; set; }
    public string? Artist { get; set; }
    public string? Album { get; set; }
    public ulong? DurationMs { get; set; }
    public byte MatchPercentage { get; set; }
    public double Quality { get; set; }
    public bool Matched { get; set; }
    public bool HasTranslation { get; set; }
    public bool HasInlineTimetags { get; set; }
    public string LyricsOriginal { get; set; } = "";
    public string? LyricsTranslation { get; set; }
}

public sealed class DanmakuMessage
{
    public string SessionId { get; set; } = "";
    public long ReceivedAtMs { get; set; }
    public string User { get; set; } = "";
    public string Text { get; set; } = "";
    public string? ImageUrl { get; set; }
    public uint? ImageWidth { get; set; }
}

public sealed class DanmakuFetchImageResult
{
    public string Mime { get; set; } = "";
    public string Base64 { get; set; } = "";
    public uint? Width { get; set; }
}

public sealed class LiveDirCategory
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public LiveDirSubCategory[] Children { get; set; } = Array.Empty<LiveDirSubCategory>();
}

public sealed class LiveDirSubCategory
{
    public string Id { get; set; } = "";
    public string ParentId { get; set; } = "";
    public string Name { get; set; } = "";
    public string? Pic { get; set; }
}

public sealed class LiveDirRoomListResult
{
    public bool HasMore { get; set; }
    public LiveDirRoomCard[] Items { get; set; } = Array.Empty<LiveDirRoomCard>();
}

public sealed class LiveDirRoomCard
{
    public string Site { get; set; } = "";
    public string RoomId { get; set; } = "";
    public string Input { get; set; } = "";
    public string Title { get; set; } = "";
    public string? Cover { get; set; }
    public string? UserName { get; set; }
    public long? Online { get; set; }
}
```

说明：
- daemon 传输层是 LSP framing（`HeaderDelimitedMessageHandler`），不是普通换行 JSON。
- `preferredQuality` 建议传 `"highest"`（默认）或 `"lowest"`；只有在 `variantId` 为空时生效。
- WinUI3 实际项目可参考 `ChaosSeed.WinUI3/Services/DaemonClient.cs` 的完整实现（包含重连/日志/通知订阅）。
