using System.Text;

namespace ChaosSeed.WinUI3.Cli;

/// <summary>
/// 命令行参数解析器
/// </summary>
public static class CliParser
{
    /// <summary>
    /// 解析命令行参数
    /// </summary>
    public static CliOptions Parse(string[] args)
    {
        var options = new CliOptions();
        var i = 0;

        // 解析全局选项
        while (i < args.Length)
        {
            var arg = args[i];

            if (arg is "-h" or "--help" or "-?")
            {
                options.ShowHelp = true;
                i++;
                continue;
            }

            if (arg is "--json" or "-j")
            {
                options.JsonOutput = true;
                i++;
                continue;
            }

            if (arg is "--tui" or "-t")
            {
                options.TuiMode = true;
                i++;
                continue;
            }

            if (arg is "-i" or "--interactive")
            {
                options.Interactive = true;
                i++;
                continue;
            }

            if (arg == "--backend" && i + 1 < args.Length)
            {
                options.Backend = args[i + 1].ToLowerInvariant();
                i += 2;
                continue;
            }

            // 遇到非选项参数，认为是命令开始
            break;
        }

        // 解析命令
        if (i < args.Length)
        {
            options.Command = args[i].ToLowerInvariant();
            i++;

            // 解析子命令
            if (i < args.Length && !args[i].StartsWith('-'))
            {
                options.SubCommand = args[i].ToLowerInvariant();
                i++;
            }
        }

        // 解析剩余参数（位置参数和命名参数）
        while (i < args.Length)
        {
            var arg = args[i];

            if (arg.StartsWith("--"))
            {
                var key = arg[2..];
                string? value = null;

                if (key.Contains('='))
                {
                    var parts = key.Split('=', 2);
                    key = parts[0];
                    value = parts[1];
                }
                else if (i + 1 < args.Length && !args[i + 1].StartsWith('-'))
                {
                    value = args[i + 1];
                    i++;
                }

                options.NamedArgs[key] = value ?? "true";
            }
            else if (arg.StartsWith('-') && arg.Length > 1)
            {
                // 短选项
                var key = arg[1..];
                string? value = null;

                if (i + 1 < args.Length && !args[i + 1].StartsWith('-'))
                {
                    value = args[i + 1];
                    i++;
                }

                options.NamedArgs[key] = value ?? "true";
            }
            else
            {
                options.Arguments.Add(arg);
            }

            i++;
        }

        return options;
    }

    /// <summary>
    /// 检查是否为 CLI 模式（有命令行参数或 CLI 特定选项）
    /// </summary>
    public static bool IsCliMode(string[] args)
    {
        if (args == null || args.Length == 0) return false;

        // 常见的 CLI 选项，遇到这些直接进入 CLI 模式
        var cliOptions = new[] { "--help", "-h", "-?", "--version", "--json", "--tui", "-t", "--interactive", "-i", "--backend" };
        if (args.Any(a => cliOptions.Any(opt => a.Equals(opt, StringComparison.OrdinalIgnoreCase))))
        {
            return true;
        }

        // 排除 Windows App SDK 自动注入的参数
        var filtered = args.Where(a => !a.StartsWith("-", StringComparison.Ordinal)).ToList();

        // 如果有任何非选项参数，认为是 CLI 模式
        return filtered.Count > 0;
    }

    /// <summary>
    /// 生成帮助文本
    /// </summary>
    public static string GetHelpText()
    {
        var sb = new StringBuilder();
        sb.AppendLine("ChaosSeed.WinUI3 - 直播工具 CLI");
        sb.AppendLine();
        sb.AppendLine("用法:");
        sb.AppendLine("  ChaosSeed.WinUI3.exe [全局选项] <命令> [子命令] [选项] [参数]");
        sb.AppendLine();
        sb.AppendLine("全局选项:");
        sb.AppendLine("  -h, --help           显示帮助信息");
        sb.AppendLine("  -j, --json           输出 JSON 格式");
        sb.AppendLine("  -t, --tui            使用 TUI 交互模式");
        sb.AppendLine("  -i, --interactive    交互式模式");
        sb.AppendLine("  --backend <模式>     后端模式: ffi, daemon, auto (默认: auto)");
        sb.AppendLine();
        sb.AppendLine("命令:");
        sb.AppendLine("  stream               直播源相关操作");
        sb.AppendLine("    resolve <URL>      解析直播源");
        sb.AppendLine("      --variant <ID>   指定清晰度 ID");
        sb.AppendLine("    variants <URL>     列出可用清晰度");
        sb.AppendLine();
        sb.AppendLine("  danmaku              弹幕相关操作");
        sb.AppendLine("    connect <URL>      连接弹幕服务器");
        sb.AppendLine("      --duration <秒>  连接持续时间");
        sb.AppendLine("      --filter <关键词> 过滤关键词 (可多次使用)");
        sb.AppendLine();
        sb.AppendLine("示例:");
        sb.AppendLine("  # 解析直播源");
        sb.AppendLine("  ChaosSeed.WinUI3.exe stream resolve https://live.bilibili.com/123");
        sb.AppendLine();
        sb.AppendLine("  # 获取指定清晰度的播放地址 (JSON 输出)");
        sb.AppendLine("  ChaosSeed.WinUI3.exe stream resolve https://live.bilibili.com/123 --variant \"bili_live:10000:原画\" --json");
        sb.AppendLine();
        sb.AppendLine("  # 列出所有可用清晰度");
        sb.AppendLine("  ChaosSeed.WinUI3.exe stream variants https://live.bilibili.com/123");
        sb.AppendLine();
        sb.AppendLine("  # 连接弹幕 60 秒");
        sb.AppendLine("  ChaosSeed.WinUI3.exe danmaku connect https://live.bilibili.com/123 --duration 60");
        sb.AppendLine();
        sb.AppendLine("  # TUI 交互模式");
        sb.AppendLine("  ChaosSeed.WinUI3.exe --tui");
        sb.AppendLine();

        return sb.ToString();
    }
}
