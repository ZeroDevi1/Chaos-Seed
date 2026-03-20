using System.Text;
using ChaosSeed.WinUI3.Cli.Commands;

namespace ChaosSeed.WinUI3.Cli;

/// <summary>
/// CLI 命令执行器
/// </summary>
public class CliRunner
{
    private readonly Dictionary<string, ICliCommand> _commands;

    public CliRunner()
    {
        _commands = new Dictionary<string, ICliCommand>(StringComparer.OrdinalIgnoreCase)
        {
            ["stream"] = new StreamCommand(),
            ["danmaku"] = new DanmakuCommand(),
            ["help"] = new HelpCommand(),
            ["version"] = new VersionCommand()
        };
    }

    /// <summary>
    /// 执行 CLI 命令
    /// </summary>
    public async Task<int> RunAsync(string[] args, CancellationToken cancellationToken = default)
    {
        // 从环境变量读取默认后端
        var envBackend = Environment.GetEnvironmentVariable("CHAOS_CLI_BACKEND");

        var options = CliParser.Parse(args);

        if (!string.IsNullOrEmpty(envBackend))
        {
            options.Backend = envBackend;
        }

        // 显示帮助
        if (options.ShowHelp || string.IsNullOrEmpty(options.Command))
        {
            Console.WriteLine(CliParser.GetHelpText());
            return 0;
        }

        // TUI 模式
        if (options.TuiMode)
        {
            var tui = new CliTui();
            return await tui.RunAsync(cancellationToken);
        }

        // 执行命令
        if (!_commands.TryGetValue(options.Command, out var command))
        {
            Console.Error.WriteLine($"错误: 未知命令 '{options.Command}'");
            Console.WriteLine("使用 --help 查看可用命令");
            return 1;
        }

        try
        {
            return await command.ExecuteAsync(options, cancellationToken);
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"错误: {ex.Message}");
            return 1;
        }
    }
}

/// <summary>
/// 帮助命令
/// </summary>
public class HelpCommand : ICliCommand
{
    public string Name => "help";
    public string Description => "显示帮助信息";

    public Task<int> ExecuteAsync(CliOptions options, CancellationToken cancellationToken = default)
    {
        Console.WriteLine(CliParser.GetHelpText());
        return Task.FromResult(0);
    }
}

/// <summary>
/// 版本命令
/// </summary>
public class VersionCommand : ICliCommand
{
    public string Name => "version";
    public string Description => "显示版本信息";

    public Task<int> ExecuteAsync(CliOptions options, CancellationToken cancellationToken = default)
    {
        var version = typeof(CliRunner).Assembly.GetName().Version?.ToString() ?? "unknown";
        var sb = new StringBuilder();
        sb.AppendLine();
        sb.AppendLine("ChaosSeed.WinUI3 CLI");
        sb.AppendLine($"版本: {version}");
        sb.AppendLine();
        sb.AppendLine("支持的后端模式:");
        sb.AppendLine("  - FFI:    直接调用 chaos_ffi.dll (推荐)");
        sb.AppendLine("  - Daemon: 通过 chaos-daemon.exe (暂未完全支持)");
        sb.AppendLine();
        sb.AppendLine("环境变量:");
        sb.AppendLine("  CHAOS_CLI_BACKEND=ffi    设置默认后端模式");
        sb.AppendLine();

        Console.WriteLine(sb.ToString());
        return Task.FromResult(0);
    }
}
