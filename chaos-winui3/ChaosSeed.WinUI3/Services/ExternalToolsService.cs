using System.IO.Compression;
using System.Net;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text.Json;
using System.Linq;

namespace ChaosSeed.WinUI3.Services;

public sealed class ExternalToolProgress
{
    public string Phase { get; init; } = "";
    public long? BytesDownloaded { get; init; }
    public long? BytesTotal { get; init; }
    public double? Percent { get; init; }
    public string? Message { get; init; }
}

public sealed record FfmpegInstallResult(
    string Version,
    string FfmpegPath,
    bool VerifiedSha256
);

public sealed class ExternalToolsService
{
    public static ExternalToolsService Instance { get; } = new();

    private static readonly Uri LatestFfmpegReleaseApi = new("https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest");

    private readonly HttpClient _http;
    private readonly SemaphoreSlim _gate = new(1, 1);

    private ExternalToolsService()
    {
        _http = new HttpClient
        {
            Timeout = TimeSpan.FromMinutes(15),
        };
        _http.DefaultRequestHeaders.UserAgent.ParseAdd($"ChaosSeed.WinUI3/{UpdateService.GetCurrentVersion()}");
        _http.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");
    }

    public async Task<FfmpegInstallResult> InstallOrUpdateFfmpegAsync(IProgress<ExternalToolProgress>? progress = null, CancellationToken ct = default)
    {
        await _gate.WaitAsync(ct).ConfigureAwait(false);
        try
        {
            progress?.Report(new ExternalToolProgress { Phase = "check", Message = "检查 ffmpeg 最新版本…" });

            using var req = new HttpRequestMessage(HttpMethod.Get, LatestFfmpegReleaseApi);
            using var resp = await _http.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, ct).ConfigureAwait(false);
            if (resp.StatusCode == HttpStatusCode.Forbidden)
            {
                throw new Exception("GitHub API returned 403 (rate limit or forbidden). Try again later.");
            }
            if (!resp.IsSuccessStatusCode)
            {
                throw new Exception($"GitHub API failed: {(int)resp.StatusCode} {resp.ReasonPhrase}");
            }

            await using var stream = await resp.Content.ReadAsStreamAsync(ct).ConfigureAwait(false);
            using var doc = await JsonDocument.ParseAsync(stream, cancellationToken: ct).ConfigureAwait(false);
            var root = doc.RootElement;

            var tag = root.TryGetProperty("tag_name", out var tagEl) ? (tagEl.GetString() ?? "") : "";
            var ver = (tag ?? "").Trim();
            if (string.IsNullOrWhiteSpace(ver))
            {
                throw new Exception("Invalid ffmpeg release tag_name.");
            }

            if (!TryFindFfmpegWin64GplAssets(root, out var zipUrl, out var shaUrl, out var zipName, out var err))
            {
                throw new Exception(err);
            }

            var toolRoot = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "ChaosSeed.WinUI3",
                "tools",
                "ffmpeg",
                ver
            );
            var binDir = Path.Combine(toolRoot, "bin");
            Directory.CreateDirectory(binDir);

            var ffmpegExe = Path.Combine(binDir, "ffmpeg.exe");
            if (File.Exists(ffmpegExe))
            {
                SettingsService.Instance.Update(s => s.FfmpegPath = ffmpegExe);
                return new FfmpegInstallResult(ver, ffmpegExe, VerifiedSha256: true);
            }

            var downloadDir = Path.Combine(toolRoot, "downloads");
            Directory.CreateDirectory(downloadDir);
            var zipPath = Path.Combine(downloadDir, zipName);

            progress?.Report(new ExternalToolProgress { Phase = "sha256", Message = "下载 sha256…" });
            var expectedSha = await DownloadSha256Async(shaUrl, ct).ConfigureAwait(false);

            progress?.Report(new ExternalToolProgress { Phase = "download", Message = "下载 ffmpeg 压缩包…" });
            await DownloadFileWithHashAsync(zipUrl, zipPath, expectedSha, progress, ct).ConfigureAwait(false);

            progress?.Report(new ExternalToolProgress { Phase = "extract", Message = "解压 ffmpeg.exe…" });
            ExtractFfmpegExe(zipPath, ffmpegExe);

            if (!File.Exists(ffmpegExe))
            {
                throw new FileNotFoundException("ffmpeg.exe not found after extraction.", ffmpegExe);
            }

            SettingsService.Instance.Update(s => s.FfmpegPath = ffmpegExe);
            progress?.Report(new ExternalToolProgress { Phase = "done", Message = "ffmpeg 已安装。" });
            return new FfmpegInstallResult(ver, ffmpegExe, VerifiedSha256: true);
        }
        finally
        {
            _gate.Release();
        }
    }

    private static bool TryFindFfmpegWin64GplAssets(
        JsonElement releaseRoot,
        out string zipUrl,
        out string shaUrl,
        out string zipName,
        out string err
    )
    {
        zipUrl = "";
        shaUrl = "";
        zipName = "";
        err = "";

        if (!releaseRoot.TryGetProperty("assets", out var assetsEl) || assetsEl.ValueKind != JsonValueKind.Array)
        {
            err = "Invalid GitHub release JSON: missing assets.";
            return false;
        }

        // Prefer BtbN standard naming.
        var preferred = "ffmpeg-master-latest-win64-gpl.zip";
        var preferredSha = preferred + ".sha256";

        string? bestZipUrl = null;
        string? bestZipName = null;
        string? bestShaUrl = null;

        foreach (var a in assetsEl.EnumerateArray())
        {
            var name = a.TryGetProperty("name", out var nameEl) ? (nameEl.GetString() ?? "") : "";
            var url = a.TryGetProperty("browser_download_url", out var urlEl) ? (urlEl.GetString() ?? "") : "";
            if (string.IsNullOrWhiteSpace(name) || string.IsNullOrWhiteSpace(url))
            {
                continue;
            }

            if (string.Equals(name, preferred, StringComparison.OrdinalIgnoreCase))
            {
                bestZipUrl = url;
                bestZipName = name;
            }
            else if (bestZipUrl is null
                     && name.EndsWith(".zip", StringComparison.OrdinalIgnoreCase)
                     && name.Contains("win64", StringComparison.OrdinalIgnoreCase)
                     && name.Contains("gpl", StringComparison.OrdinalIgnoreCase))
            {
                bestZipUrl = url;
                bestZipName = name;
            }

            if (string.Equals(name, preferredSha, StringComparison.OrdinalIgnoreCase))
            {
                bestShaUrl = url;
            }
        }

        if (bestZipUrl is null || bestZipName is null)
        {
            err = "Latest ffmpeg release missing a suitable win64 gpl zip asset.";
            return false;
        }

        // Find sha256 matching the chosen zip.
        var wantedShaName = bestZipName + ".sha256";
        foreach (var a in assetsEl.EnumerateArray())
        {
            var name = a.TryGetProperty("name", out var nameEl) ? (nameEl.GetString() ?? "") : "";
            var url = a.TryGetProperty("browser_download_url", out var urlEl) ? (urlEl.GetString() ?? "") : "";
            if (string.Equals(name, wantedShaName, StringComparison.OrdinalIgnoreCase))
            {
                bestShaUrl = url;
                break;
            }
        }

        if (bestShaUrl is null)
        {
            err = $"Latest ffmpeg release missing sha256 asset: {wantedShaName}";
            return false;
        }

        zipUrl = bestZipUrl;
        shaUrl = bestShaUrl;
        zipName = bestZipName;
        return true;
    }

    private async Task<string> DownloadSha256Async(string url, CancellationToken ct)
    {
        using var req = new HttpRequestMessage(HttpMethod.Get, url);
        using var resp = await _http.SendAsync(req, ct).ConfigureAwait(false);
        resp.EnsureSuccessStatusCode();

        var txt = (await resp.Content.ReadAsStringAsync(ct).ConfigureAwait(false)).Trim();
        // Expected format: "<sha256>  <filename>"
        var sha = txt.Split(new[] { ' ', '\t', '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries).FirstOrDefault() ?? "";
        sha = sha.Trim();
        if (sha.Length != 64)
        {
            throw new Exception($"Invalid sha256 file contents: '{txt}'");
        }
        return sha;
    }

    private async Task DownloadFileWithHashAsync(
        string url,
        string outPath,
        string expectedSha256,
        IProgress<ExternalToolProgress>? progress,
        CancellationToken ct
    )
    {
        using var req = new HttpRequestMessage(HttpMethod.Get, url);
        using var resp = await _http.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, ct).ConfigureAwait(false);
        resp.EnsureSuccessStatusCode();

        var total = resp.Content.Headers.ContentLength;

        Directory.CreateDirectory(Path.GetDirectoryName(outPath)!);
        await using var httpStream = await resp.Content.ReadAsStreamAsync(ct).ConfigureAwait(false);
        await using var fs = new FileStream(outPath, FileMode.Create, FileAccess.Write, FileShare.None, 1024 * 64, useAsync: true);
        using var hasher = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);

        var buf = new byte[1024 * 64];
        long done = 0;
        while (true)
        {
            var n = await httpStream.ReadAsync(buf, ct).ConfigureAwait(false);
            if (n <= 0)
            {
                break;
            }

            await fs.WriteAsync(buf.AsMemory(0, n), ct).ConfigureAwait(false);
            hasher.AppendData(buf, 0, n);

            done += n;
            double? pct = null;
            if (total is not null && total.Value > 0)
            {
                pct = Math.Clamp(done / (double)total.Value * 100.0, 0.0, 100.0);
            }

            progress?.Report(new ExternalToolProgress
            {
                Phase = "download",
                BytesDownloaded = done,
                BytesTotal = total,
                Percent = pct,
            });
        }

        await fs.FlushAsync(ct).ConfigureAwait(false);
        var got = Convert.ToHexString(hasher.GetHashAndReset()).ToLowerInvariant();
        if (!string.Equals(got, expectedSha256, StringComparison.OrdinalIgnoreCase))
        {
            throw new Exception($"sha256 mismatch: expected={expectedSha256} got={got}");
        }
    }

    private static void ExtractFfmpegExe(string zipPath, string outFfmpegExePath)
    {
        using var zip = ZipFile.OpenRead(zipPath);
        ZipArchiveEntry? entry = null;

        foreach (var e in zip.Entries)
        {
            var name = (e.FullName ?? "").Replace('\\', '/');
            if (name.EndsWith("/bin/ffmpeg.exe", StringComparison.OrdinalIgnoreCase))
            {
                entry = e;
                break;
            }
        }

        if (entry is null)
        {
            throw new Exception("zip does not contain bin/ffmpeg.exe");
        }

        Directory.CreateDirectory(Path.GetDirectoryName(outFfmpegExePath)!);
        entry.ExtractToFile(outFfmpegExePath, overwrite: true);
    }
}
