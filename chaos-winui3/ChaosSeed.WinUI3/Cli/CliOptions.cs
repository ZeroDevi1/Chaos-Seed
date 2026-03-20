namespace ChaosSeed.WinUI3.Cli;

/// <summary>
/// CLI 命令行选项
/// </summary>
public class CliOptions
{
    /// <summary>
    /// 是否显示帮助
    /// </summary>
    public bool ShowHelp { get; set; }

    /// <summary>
    /// 是否输出 JSON 格式
    /// </summary>
    public bool JsonOutput { get; set; }

    /// <summary>
    /// 后端模式: "ffi" 或 "daemon"
    /// </summary>
    public string Backend { get; set; } = "auto";

    /// <summary>
    /// 命令名称
    /// </summary>
    public string? Command { get; set; }

    /// <summary>
    /// 子命令名称
    /// </summary>
    public string? SubCommand { get; set; }

    /// <summary>
    /// 位置参数
    /// </summary>
    public List<string> Arguments { get; set; } = new();

    /// <summary>
    /// 命名参数
    /// </summary>
    public Dictionary<string, string> NamedArgs { get; set; } = new();

    /// <summary>
    /// 是否为 TUI 模式
    /// </summary>
    public bool TuiMode { get; set; }

    /// <summary>
    /// 是否为交互式模式
    /// </summary>
    public bool Interactive { get; set; }
}
