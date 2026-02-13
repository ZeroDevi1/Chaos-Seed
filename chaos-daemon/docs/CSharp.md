# C#（IPC / StreamJsonRpc）调用示例

本示例展示如何从 .NET（WinUI3/Console）通过 **NamedPipe + JSON-RPC（LSP framing）** 调用 `chaos-daemon`，并使用新增的 `liveDir.*` 目录接口。

依赖：
- NuGet：`StreamJsonRpc`

## 最小客户端（含鉴权）

```csharp
using System;
using System.Diagnostics;
using System.IO.Pipes;
using System.Text;
using StreamJsonRpc;
using StreamJsonRpc.Protocol;

static async Task Main()
{
    var authToken = Guid.NewGuid().ToString("N");
    var pipeName = $"chaos-seed-{Guid.NewGuid():N}";

    var daemonExe = Path.Combine(AppContext.BaseDirectory, "chaos-daemon.exe");
    var proc = Process.Start(new ProcessStartInfo
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

    var formatter = new JsonMessageFormatter();
    var handler = new HeaderDelimitedMessageHandler(pipe, pipe, formatter);
    using var rpc = new JsonRpc(handler);
    rpc.StartListening();

    // 1) Authenticate (required)
    var ping = await rpc.InvokeWithParameterObjectAsync<PingResult>(
        "daemon.ping",
        new { authToken }
    );
    Console.WriteLine("daemon version=" + ping.Version);

    // 2) Live directory calls
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
}

public sealed class PingResult
{
    public string Version { get; set; } = "";
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
- WinUI3 实际项目可参考 `ChaosSeed.WinUI3/Services/DaemonClient.cs` 的完整实现（包含重连/日志/通知订阅）。

