namespace ChaosSeed.WinUI3.Cli;

/// <summary>
/// CLI 命令接口
/// </summary>
public interface ICliCommand
{
    /// <summary>
    /// 命令名称
    /// </summary>
    string Name { get; }

    /// <summary>
    /// 命令描述
    /// </summary>
    string Description { get; }

    /// <summary>
    /// 执行命令
    /// </summary>
    /// <param name="options">命令行选项</param>
    /// <param name="cancellationToken">取消令牌</param>
    /// <returns>退出码 (0 表示成功)</returns>
    Task<int> ExecuteAsync(CliOptions options, CancellationToken cancellationToken = default);
}

/// <summary>
/// CLI 命令上下文
/// </summary>
public class CliContext
{
    /// <summary>
    /// 后端模式
    /// </summary>
    public string Backend { get; set; } = "auto";

    /// <summary>
    /// 是否 JSON 输出
    /// </summary>
    public bool JsonOutput { get; set; }

    /// <summary>
    /// 是否为 TUI 模式
    /// </summary>
    public bool TuiMode { get; set; }
}
