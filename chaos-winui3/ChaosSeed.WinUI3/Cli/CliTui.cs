using System.Text;

namespace ChaosSeed.WinUI3.Cli;

/// <summary>
/// TUI (Terminal User Interface) 实现
/// </summary>
public class CliTui
{
    public async Task<int> RunAsync(CancellationToken cancellationToken = default)
    {
        Console.Clear();
        Console.WriteLine();
        Console.WriteLine("╔════════════════════════════════════════════════════════╗");
        Console.WriteLine("║        ChaosSeed.WinUI3 - TUI 交互模式                  ║");
        Console.WriteLine("╠════════════════════════════════════════════════════════╣");
        Console.WriteLine("║  1. 解析直播源 (stream resolve)                        ║");
        Console.WriteLine("║  2. 查看清晰度 (stream variants)                       ║");
        Console.WriteLine("║  3. 连接弹幕 (danmaku connect)                         ║");
        Console.WriteLine("║  4. 查看帮助                                            ║");
        Console.WriteLine("║  5. 退出                                                ║");
        Console.WriteLine("╚════════════════════════════════════════════════════════╝");
        Console.WriteLine();

        while (!cancellationToken.IsCancellationRequested)
        {
            Console.Write("请选择操作 [1-5]: ");
            var key = Console.ReadKey(intercept: true);
            Console.WriteLine(key.KeyChar);

            switch (key.KeyChar)
            {
                case '1':
                    await RunStreamResolveAsync(cancellationToken);
                    break;
                case '2':
                    await RunStreamVariantsAsync(cancellationToken);
                    break;
                case '3':
                    await RunDanmakuConnectAsync(cancellationToken);
                    break;
                case '4':
                    Console.WriteLine();
                    Console.WriteLine(CliParser.GetHelpText());
                    break;
                case '5':
                case 'q':
                    Console.WriteLine();
                    Console.WriteLine("再见!");
                    return 0;
                default:
                    Console.WriteLine("无效选项，请重新选择");
                    break;
            }

            Console.WriteLine();
        }

        return 0;
    }

    private async Task RunStreamResolveAsync(CancellationToken ct)
    {
        Console.WriteLine();
        Console.WriteLine("--- 解析直播源 ---");
        Console.Write("请输入直播间 URL: ");
        var url = Console.ReadLine()?.Trim();

        if (string.IsNullOrEmpty(url))
        {
            Console.WriteLine("URL 不能为空");
            return;
        }

        Console.Write("是否指定清晰度? (y/N): ");
        var specifyVariant = Console.ReadLine()?.Trim().ToLowerInvariant() == "y";

        string? variantId = null;
        if (specifyVariant)
        {
            Console.Write("请输入清晰度 ID: ");
            variantId = Console.ReadLine()?.Trim();
        }

        Console.Write("是否输出 JSON? (y/N): ");
        var jsonOutput = Console.ReadLine()?.Trim().ToLowerInvariant() == "y";

        // 构建命令参数
        var args = new List<string> { "stream", "resolve", url };
        if (!string.IsNullOrEmpty(variantId))
        {
            args.Add("--variant");
            args.Add(variantId);
        }
        if (jsonOutput)
        {
            args.Add("--json");
        }

        var options = CliParser.Parse(args.ToArray());
        var command = new Commands.StreamCommand();
        await command.ExecuteAsync(options, ct);
    }

    private async Task RunStreamVariantsAsync(CancellationToken ct)
    {
        Console.WriteLine();
        Console.WriteLine("--- 查看清晰度列表 ---");
        Console.Write("请输入直播间 URL: ");
        var url = Console.ReadLine()?.Trim();

        if (string.IsNullOrEmpty(url))
        {
            Console.WriteLine("URL 不能为空");
            return;
        }

        var args = new[] { "stream", "variants", url };
        var options = CliParser.Parse(args);
        var command = new Commands.StreamCommand();
        await command.ExecuteAsync(options, ct);
    }

    private async Task RunDanmakuConnectAsync(CancellationToken ct)
    {
        Console.WriteLine();
        Console.WriteLine("--- 连接弹幕 ---");
        Console.Write("请输入直播间 URL: ");
        var url = Console.ReadLine()?.Trim();

        if (string.IsNullOrEmpty(url))
        {
            Console.WriteLine("URL 不能为空");
            return;
        }

        Console.Write("连接持续时间 (秒, 0=无限): ");
        var durationStr = Console.ReadLine()?.Trim();
        var duration = string.IsNullOrEmpty(durationStr) ? -1 : int.Parse(durationStr);

        Console.Write("是否输出 JSON? (y/N): ");
        var jsonOutput = Console.ReadLine()?.Trim().ToLowerInvariant() == "y";

        // 构建命令参数
        var args = new List<string> { "danmaku", "connect", url };
        if (duration > 0)
        {
            args.Add("--duration");
            args.Add(duration.ToString());
        }
        if (jsonOutput)
        {
            args.Add("--json");
        }

        var options = CliParser.Parse(args.ToArray());
        var command = new Commands.DanmakuCommand();

        Console.WriteLine();
        Console.WriteLine("按 Ctrl+C 停止接收弹幕...");
        Console.WriteLine();

        await command.ExecuteAsync(options, ct);
    }
}
