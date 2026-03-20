using System.Text;
using System.Text.Json;

namespace ChaosSeed.WinUI3.Cli.Commands;

/// <summary>
/// Danmaku 命令 - 弹幕相关操作
/// </summary>
public class DanmakuCommand : ICliCommand
{
    public string Name => "danmaku";
    public string Description => "弹幕连接和实时获取";

    public async Task<int> ExecuteAsync(CliOptions options, CancellationToken cancellationToken = default)
    {
        var backend = CliBackendFactory.Create(options.Backend);
        await backend.InitializeAsync(cancellationToken);

        try
        {
            return options.SubCommand?.ToLowerInvariant() switch
            {
                "connect" => await ExecuteConnectAsync(backend, options, cancellationToken),
                null or "" => await ExecuteConnectAsync(backend, options, cancellationToken),
                _ => throw new InvalidOperationException($"未知子命令: {options.SubCommand}")
            };
        }
        finally
        {
            await backend.DisposeAsync();
        }
    }

    private async Task<int> ExecuteConnectAsync(ICliBackend backend, CliOptions options, CancellationToken ct)
    {
        if (options.Arguments.Count == 0)
        {
            Console.Error.WriteLine("错误: 需要提供直播间 URL");
            Console.WriteLine("用法: danmaku connect <URL> [--duration <秒>] [--filter <关键词>]");
            return 1;
        }

        var input = options.Arguments[0];
        var durationStr = options.NamedArgs.GetValueOrDefault("duration") ?? options.NamedArgs.GetValueOrDefault("d");
        var filters = options.NamedArgs
            .Where(kv => kv.Key == "filter" || kv.Key == "f")
            .Select(kv => kv.Value)
            .ToList();

        var duration = string.IsNullOrEmpty(durationStr) ? -1 : int.Parse(durationStr);

        try
        {
            IDanmakuSession? session = null;

            try
            {
                session = await backend.ConnectDanmakuAsync(input, ct);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"连接弹幕失败: {ex.Message}");
                return 1;
            }

            if (session == null)
            {
                Console.Error.WriteLine("错误: 无法连接弹幕服务器");
                return 1;
            }

            await using var _ = session;

            // 打印连接信息
            if (!options.JsonOutput)
            {
                Console.WriteLine();
                Console.WriteLine("╔════════════════════════════════════════════════════════╗");
                Console.WriteLine("║                   弹幕连接已建立                        ║");
                Console.WriteLine("╠════════════════════════════════════════════════════════╣");
                Console.WriteLine($"║ 直播间: {Truncate(input, 44),-44} ║");

                if (duration > 0)
                {
                    Console.WriteLine($"║ 持续时间: {duration} 秒{' ',38} ║");
                }
                else
                {
                    Console.WriteLine($"║ 持续时间: 无限 (按 Ctrl+C 停止){' ',26} ║");
                }

                if (filters.Count > 0)
                {
                    Console.WriteLine($"║ 过滤器: {string.Join(", ", filters),-44} ║");
                }

                Console.WriteLine("╚════════════════════════════════════════════════════════╝");
                Console.WriteLine();
            }

            var startTime = DateTime.UtcNow;
            var eventCount = 0;

            while (!ct.IsCancellationRequested)
            {
                // 检查持续时间
                if (duration > 0 && (DateTime.UtcNow - startTime).TotalSeconds >= duration)
                {
                    break;
                }

                var events = await session.PollAsync(50, ct);

                foreach (var ev in events)
                {
                    // 应用过滤器
                    if (filters.Count > 0 && filters.Any(f => ev.Text.Contains(f, StringComparison.OrdinalIgnoreCase)))
                    {
                        continue;
                    }

                    eventCount++;

                    if (options.JsonOutput)
                    {
                        Console.WriteLine(JsonSerializer.Serialize(ev));
                    }
                    else
                    {
                        PrintDanmakuEvent(ev, eventCount);
                    }
                }

                // 短暂等待，避免 CPU 占用过高
                await Task.Delay(100, ct);
            }

            if (!options.JsonOutput)
            {
                Console.WriteLine();
                Console.WriteLine($"✓ 共接收 {eventCount} 条弹幕消息");
                Console.WriteLine();
            }

            return 0;
        }
        catch (OperationCanceledException)
        {
            if (!options.JsonOutput)
            {
                Console.WriteLine();
                Console.WriteLine("✓ 用户取消连接");
            }
            return 0;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"错误: {ex.Message}");
            return 1;
        }
    }

    private void PrintDanmakuEvent(DanmakuEvent ev, int index)
    {
        var time = DateTimeOffset.FromUnixTimeMilliseconds(ev.ReceivedAtMs).ToLocalTime();
        var timeStr = time.ToString("HH:mm:ss");

        // 解析图片表情
        var text = ev.Text;
        var imageInfo = "";

        if (ev.Dms?.Count > 0)
        {
            var images = ev.Dms.Where(d => !string.IsNullOrEmpty(d.ImageUrl)).ToList();
            if (images.Count > 0)
            {
                imageInfo = $" [图片x{images.Count}]";
            }
        }

        var sb = new StringBuilder();
        sb.Append($"[{timeStr}] ");

        // 根据用户类型使用不同颜色（通过字符区分）
        var user = string.IsNullOrEmpty(ev.User) ? "匿名" : ev.User;
        sb.Append($"<{user}>: ");
        sb.Append(text);
        sb.Append(imageInfo);

        Console.WriteLine(sb.ToString());
    }

    private static string Truncate(string? value, int maxLength)
    {
        if (string.IsNullOrEmpty(value)) return "";
        return value.Length <= maxLength ? value : value[..(maxLength - 3)] + "...";
    }
}
