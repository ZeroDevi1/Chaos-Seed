using System.Text;
using System.Text.Json;

namespace ChaosSeed.WinUI3.Cli.Commands;

/// <summary>
/// Stream 命令 - 直播源相关操作
/// </summary>
public class StreamCommand : ICliCommand
{
    public string Name => "stream";
    public string Description => "直播源解析和清晰度管理";

    public async Task<int> ExecuteAsync(CliOptions options, CancellationToken cancellationToken = default)
    {
        var backend = CliBackendFactory.Create(options.Backend);
        await backend.InitializeAsync(cancellationToken);

        try
        {
            return options.SubCommand?.ToLowerInvariant() switch
            {
                "resolve" => await ExecuteResolveAsync(backend, options, cancellationToken),
                "variants" => await ExecuteVariantsAsync(backend, options, cancellationToken),
                null or "" => await ExecuteResolveAsync(backend, options, cancellationToken),
                _ => throw new InvalidOperationException($"未知子命令: {options.SubCommand}")
            };
        }
        finally
        {
            await backend.DisposeAsync();
        }
    }

    private async Task<int> ExecuteResolveAsync(ICliBackend backend, CliOptions options, CancellationToken ct)
    {
        if (options.Arguments.Count == 0)
        {
            Console.Error.WriteLine("错误: 需要提供直播间 URL");
            Console.WriteLine("用法: stream resolve <URL> [--variant <ID>]");
            return 1;
        }

        var input = options.Arguments[0];
        var variantId = options.NamedArgs.GetValueOrDefault("variant") ?? options.NamedArgs.GetValueOrDefault("v");

        try
        {
            var manifest = await backend.DecodeManifestAsync(input, true, ct);
            if (manifest == null)
            {
                Console.Error.WriteLine("错误: 无法解析直播源");
                return 1;
            }

            if (!string.IsNullOrEmpty(variantId))
            {
                // 解析指定清晰度
                var variant = await backend.ResolveVariantAsync(manifest.Site, manifest.RoomId, variantId, ct);
                if (variant == null)
                {
                    Console.Error.WriteLine($"错误: 无法获取清晰度: {variantId}");
                    return 1;
                }

                if (options.JsonOutput)
                {
                    Console.WriteLine(JsonSerializer.Serialize(variant, new JsonSerializerOptions { WriteIndented = true }));
                }
                else
                {
                    PrintVariant(variant, manifest.Info);
                }
            }
            else
            {
                // 显示所有清晰度选项
                if (options.JsonOutput)
                {
                    Console.WriteLine(JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }));
                }
                else
                {
                    PrintManifest(manifest);
                }
            }

            return 0;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"错误: {ex.Message}");
            return 1;
        }
    }

    private async Task<int> ExecuteVariantsAsync(ICliBackend backend, CliOptions options, CancellationToken ct)
    {
        if (options.Arguments.Count == 0)
        {
            Console.Error.WriteLine("错误: 需要提供直播间 URL");
            Console.WriteLine("用法: stream variants <URL>");
            return 1;
        }

        var input = options.Arguments[0];

        try
        {
            var manifest = await backend.DecodeManifestAsync(input, true, ct);
            if (manifest == null)
            {
                Console.Error.WriteLine("错误: 无法解析直播源");
                return 1;
            }

            if (options.JsonOutput)
            {
                Console.WriteLine(JsonSerializer.Serialize(manifest.Variants, new JsonSerializerOptions { WriteIndented = true }));
            }
            else
            {
                PrintVariants(manifest.Variants, manifest.Info);
            }

            return 0;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"错误: {ex.Message}");
            return 1;
        }
    }

    private void PrintManifest(LiveManifest manifest)
    {
        var sb = new StringBuilder();
        sb.AppendLine();
        sb.AppendLine("╔════════════════════════════════════════════════════════╗");
        sb.AppendLine("║                   直播源解析结果                        ║");
        sb.AppendLine("╠════════════════════════════════════════════════════════╣");
        sb.AppendLine($"║ 平台:    {manifest.Site,-42} ║");
        sb.AppendLine($"║ 房间号:  {manifest.RoomId,-42} ║");
        sb.AppendLine($"║ 标题:    {Truncate(manifest.Info.Title, 40),-42} ║");
        sb.AppendLine($"║ 主播:    {manifest.Info.Name ?? "N/A",-42} ║");
        sb.AppendLine($"║ 状态:    {(manifest.Info.IsLiving ? "直播中" : "未直播"),-42} ║");
        sb.AppendLine("╠════════════════════════════════════════════════════════╣");
        sb.AppendLine("║ 可用清晰度:                                            ║");

        for (var i = 0; i < manifest.Variants.Count; i++)
        {
            var v = manifest.Variants[i];
            var marker = i == 0 ? "*" : " ";
            sb.AppendLine($"║   {marker} [{i + 1}] {v.Label,-10} (ID: {v.Id,-25}) ║");
        }

        sb.AppendLine("╠════════════════════════════════════════════════════════╣");
        sb.AppendLine("║ 使用提示:                                              ║");
        sb.AppendLine("║   获取指定清晰度:                                      ║");
        sb.AppendLine($"║   --variant \"{manifest.Variants.FirstOrDefault()?.Id}\"                 ║");
        sb.AppendLine("╚════════════════════════════════════════════════════════╝");
        sb.AppendLine();

        Console.WriteLine(sb.ToString());
    }

    private void PrintVariants(List<StreamVariant> variants, LiveInfo info)
    {
        var sb = new StringBuilder();
        sb.AppendLine();
        sb.AppendLine($"标题: {info.Title}");
        sb.AppendLine($"主播: {info.Name ?? "Unknown"}");
        sb.AppendLine();
        sb.AppendLine("可用清晰度列表:");
        sb.AppendLine();

        for (var i = 0; i < variants.Count; i++)
        {
            var v = variants[i];
            sb.AppendLine($"  [{i + 1}] {v.Label,-10}  ID: {v.Id}");

            if (!string.IsNullOrEmpty(v.Url))
            {
                sb.AppendLine($"      URL: {Truncate(v.Url, 60)}");
            }

            if (v.BackupUrls.Count > 0)
            {
                sb.AppendLine($"      备用: {v.BackupUrls.Count} 个");
            }

            sb.AppendLine();
        }

        Console.WriteLine(sb.ToString());
    }

    private void PrintVariant(StreamVariant variant, LiveInfo info)
    {
        var sb = new StringBuilder();
        sb.AppendLine();
        sb.AppendLine("╔════════════════════════════════════════════════════════╗");
        sb.AppendLine("║                   清晰度详情                            ║");
        sb.AppendLine("╠════════════════════════════════════════════════════════╣");
        sb.AppendLine($"║ 标题:    {Truncate(info.Title, 40),-42} ║");
        sb.AppendLine($"║ 主播:    {info.Name ?? "N/A",-42} ║");
        sb.AppendLine("╠════════════════════════════════════════════════════════╣");
        sb.AppendLine($"║ 清晰度:  {variant.Label,-42} ║");
        sb.AppendLine($"║ ID:      {variant.Id,-42} ║");
        sb.AppendLine($"║ 质量值:  {variant.Quality,-42} ║");

        if (variant.Rate.HasValue)
        {
            sb.AppendLine($"║ 码率:    {variant.Rate.Value,-42} ║");
        }

        if (!string.IsNullOrEmpty(variant.Url))
        {
            sb.AppendLine("╠════════════════════════════════════════════════════════╣");
            sb.AppendLine("║ 播放地址:                                              ║");
            sb.AppendLine($"║ {Truncate(variant.Url, 52),-52} ║");

            foreach (var backup in variant.BackupUrls.Take(2))
            {
                sb.AppendLine($"║ [备用] {Truncate(backup, 46),-46} ║");
            }
        }

        sb.AppendLine("╚════════════════════════════════════════════════════════╝");
        sb.AppendLine();

        Console.WriteLine(sb.ToString());
    }

    private static string Truncate(string? value, int maxLength)
    {
        if (string.IsNullOrEmpty(value)) return "";
        return value.Length <= maxLength ? value : value[..(maxLength - 3)] + "...";
    }
}
