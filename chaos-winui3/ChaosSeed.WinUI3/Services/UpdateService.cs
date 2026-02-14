using System;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace ChaosSeed.WinUI3.Services;

public sealed class UpdateService
{
    public static UpdateService Instance { get; } = new();

    private static readonly Uri LatestReleaseApi = new("https://api.github.com/repos/ZeroDevi1/Chaos-Seed/releases/latest");
    private const string WinuiZipAsset = "chaos-winui3-windows-x86_64.zip";
    private const string WinuiZipSha256Asset = "chaos-winui3-windows-x86_64.zip.sha256";

    private readonly HttpClient _http;
    private readonly SemaphoreSlim _gate = new(1, 1);

    private UpdateService()
    {
        _http = new HttpClient
        {
            Timeout = TimeSpan.FromSeconds(20),
        };
        var ua = $"ChaosSeed.WinUI3/{GetCurrentVersion()}";
        _http.DefaultRequestHeaders.UserAgent.ParseAdd(ua);
        _http.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");
    }

    public UpdateCheckResult? LastResult { get; private set; }
    public UpdateAvailable? AvailableUpdate => LastResult as UpdateAvailable;

    public bool ShouldAutoCheck()
    {
        var s = SettingsService.Instance.Current;
        if (!s.AutoUpdateEnabled)
        {
            return false;
        }

        var last = s.AutoUpdateLastCheckUnixMs;
        if (last is null || last <= 0)
        {
            return true;
        }

        var interval = Math.Clamp(s.AutoUpdateIntervalHours, 1, 24 * 14);
        var next = DateTimeOffset.FromUnixTimeMilliseconds(last.Value).AddHours(interval);
        return DateTimeOffset.UtcNow >= next;
    }

    public async Task TryAutoCheckAsync(CancellationToken ct = default)
    {
        if (AppIdentityService.IsPackaged)
        {
            return;
        }

        if (!ShouldAutoCheck())
        {
            return;
        }

        try
        {
            await CheckAsync(force: false, ct);
        }
        catch
        {
            // best-effort
        }
    }

    public async Task<UpdateCheckResult> CheckAsync(bool force, CancellationToken ct = default)
    {
        if (AppIdentityService.IsPackaged)
        {
            LastResult = new UpdateError("Packaged (MSIX) installs are updated by the system (AppInstaller).");
            return LastResult;
        }

        await _gate.WaitAsync(ct);
        try
        {
            var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            SettingsService.Instance.UpdateSilently(s => s.AutoUpdateLastCheckUnixMs = now);

            using var req = new HttpRequestMessage(HttpMethod.Get, LatestReleaseApi);
            using var resp = await _http.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, ct);
            if (resp.StatusCode == HttpStatusCode.Forbidden)
            {
                // GitHub rate limit is the common case for anonymous clients.
                LastResult = new UpdateError("GitHub API returned 403 (rate limit or forbidden). Try again later.");
                return LastResult;
            }
            if (!resp.IsSuccessStatusCode)
            {
                LastResult = new UpdateError($"GitHub API failed: {(int)resp.StatusCode} {resp.ReasonPhrase}");
                return LastResult;
            }

            await using var stream = await resp.Content.ReadAsStreamAsync(ct);
            using var doc = await JsonDocument.ParseAsync(stream, cancellationToken: ct);
            var root = doc.RootElement;

            var tag = root.TryGetProperty("tag_name", out var tagEl) ? (tagEl.GetString() ?? "") : "";
            var htmlUrl = root.TryGetProperty("html_url", out var urlEl) ? (urlEl.GetString() ?? "") : "";
            var ver = NormalizeVersion(tag);
            if (string.IsNullOrWhiteSpace(ver))
            {
                LastResult = new UpdateError($"Invalid tag_name in latest release: '{tag}'");
                return LastResult;
            }

            var cur = NormalizeVersion(GetCurrentVersion());
            if (!force && !IsSemverLike(cur))
            {
                // If local build doesn't have a semver-ish version, we don't auto-update it by default.
                LastResult = new UpdateError($"Current version '{cur}' is not a semver release build; skipping update check.");
                return LastResult;
            }

            if (!TryFindAssetUrls(root, out var zipUrl, out var shaUrl, out var err))
            {
                LastResult = new UpdateError(err);
                return LastResult;
            }

            if (CompareVersions(cur, ver) >= 0)
            {
                LastResult = new UpdateUpToDate(cur, htmlUrl);
                return LastResult;
            }

            LastResult = new UpdateAvailable(
                CurrentVersion: cur,
                LatestVersion: ver,
                ReleasePageUrl: htmlUrl,
                ZipUrl: zipUrl,
                Sha256Url: shaUrl
            );
            return LastResult;
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception ex)
        {
            LastResult = new UpdateError($"update check failed: {ex.Message}");
            return LastResult;
        }
        finally
        {
            _gate.Release();
        }
    }

    public async Task<UpdatePending> DownloadAsync(UpdateAvailable u, IProgress<UpdateProgress>? progress = null, CancellationToken ct = default)
    {
        if (AppIdentityService.IsPackaged)
        {
            throw new InvalidOperationException("Packaged installs should not use zip self-updater.");
        }

        var pendingDir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "ChaosSeed.WinUI3",
            "updates",
            "pending",
            u.LatestVersion
        );
        Directory.CreateDirectory(pendingDir);

        var sha = await DownloadSha256Async(u.Sha256Url, ct);
        var zipPath = Path.Combine(pendingDir, WinuiZipAsset);

        await DownloadFileWithHashAsync(u.ZipUrl, zipPath, sha, progress, ct);

        var pending = new UpdatePending
        {
            Version = u.LatestVersion,
            ZipPath = zipPath,
            Sha256 = sha,
            DownloadedAtUnixMs = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
        };

        var pendingJsonPath = Path.Combine(pendingDir, "pending.json");
        await File.WriteAllTextAsync(pendingJsonPath, JsonSerializer.Serialize(pending, new JsonSerializerOptions { WriteIndented = true }), ct);

        return pending;
    }

    public void ApplyAndRestart(UpdatePending pending)
    {
        if (AppIdentityService.IsPackaged)
        {
            return;
        }

        var appDir = AppContext.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar);
        var updaterExe = Path.Combine(appDir, "ChaosSeed.Updater.exe");
        if (!File.Exists(updaterExe))
        {
            throw new FileNotFoundException("Missing ChaosSeed.Updater.exe next to WinUI executable.", updaterExe);
        }
        if (!File.Exists(pending.ZipPath))
        {
            throw new FileNotFoundException("Missing downloaded update zip.", pending.ZipPath);
        }

        // Ensure daemon is not holding locks.
        try { DaemonClient.Instance.Dispose(); } catch { }

        var pid = Environment.ProcessId;
        var args = new StringBuilder();
        args.Append("--app-dir ").Append(Quote(appDir)).Append(' ');
        args.Append("--zip ").Append(Quote(pending.ZipPath)).Append(' ');
        args.Append("--expected-sha256 ").Append(Quote(pending.Sha256)).Append(' ');
        args.Append("--restart-exe ").Append(Quote("ChaosSeed.WinUI3.exe")).Append(' ');
        args.Append("--version ").Append(Quote(pending.Version)).Append(' ');
        args.Append("--parent-pid ").Append(pid.ToString(CultureInfo.InvariantCulture));

        Process.Start(
            new ProcessStartInfo
            {
                FileName = updaterExe,
                Arguments = args.ToString(),
                UseShellExecute = false,
                CreateNoWindow = true,
                WorkingDirectory = appDir,
            }
        );

        Environment.Exit(0);
    }

    public static string GetCurrentVersion()
    {
        try
        {
            var p = Path.Combine(AppContext.BaseDirectory, "chaos-version.txt");
            if (File.Exists(p))
            {
                var txt = (File.ReadAllText(p) ?? "").Trim();
                if (!string.IsNullOrWhiteSpace(txt))
                {
                    return NormalizeVersion(txt);
                }
            }
        }
        catch
        {
            // ignore
        }

        // Fallback: assembly version (often 1.0.0.0). Still better than null.
        try
        {
            return (typeof(UpdateService).Assembly.GetName().Version?.ToString() ?? "0.0.0").Trim();
        }
        catch
        {
            return "0.0.0";
        }
    }

    private static bool TryFindAssetUrls(JsonElement root, out string zipUrl, out string shaUrl, out string err)
    {
        zipUrl = "";
        shaUrl = "";
        err = "";

        if (!root.TryGetProperty("assets", out var assets) || assets.ValueKind != JsonValueKind.Array)
        {
            err = "GitHub release JSON missing assets[].";
            return false;
        }

        foreach (var a in assets.EnumerateArray())
        {
            if (!a.TryGetProperty("name", out var nameEl))
            {
                continue;
            }
            var name = nameEl.GetString() ?? "";
            if (!a.TryGetProperty("browser_download_url", out var dlEl))
            {
                continue;
            }
            var dl = dlEl.GetString() ?? "";
            if (string.Equals(name, WinuiZipAsset, StringComparison.Ordinal))
            {
                zipUrl = dl;
            }
            else if (string.Equals(name, WinuiZipSha256Asset, StringComparison.Ordinal))
            {
                shaUrl = dl;
            }
        }

        if (string.IsNullOrWhiteSpace(zipUrl))
        {
            err = $"Latest release missing asset: {WinuiZipAsset}";
            return false;
        }
        if (string.IsNullOrWhiteSpace(shaUrl))
        {
            err = $"Latest release missing asset: {WinuiZipSha256Asset}";
            return false;
        }

        return true;
    }

    private async Task<string> DownloadSha256Async(string url, CancellationToken ct)
    {
        using var req = new HttpRequestMessage(HttpMethod.Get, url);
        using var resp = await _http.SendAsync(req, HttpCompletionOption.ResponseContentRead, ct);
        resp.EnsureSuccessStatusCode();
        var txt = (await resp.Content.ReadAsStringAsync(ct)).Trim();

        // The asset file is expected to be just the hex hash.
        txt = txt.Split(new[] { ' ', '\t', '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries)[0];
        txt = txt.Trim().ToLowerInvariant();
        if (txt.Length != 64 || !IsHex(txt))
        {
            throw new Exception($"Invalid sha256 file contents: '{txt}'");
        }
        return txt;
    }

    private async Task DownloadFileWithHashAsync(
        string url,
        string outPath,
        string expectedSha256,
        IProgress<UpdateProgress>? progress,
        CancellationToken ct
    )
    {
        var tmp = outPath + ".tmp";
        if (File.Exists(tmp))
        {
            try { File.Delete(tmp); } catch { }
        }

        using var req = new HttpRequestMessage(HttpMethod.Get, url);
        using var resp = await _http.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, ct);
        resp.EnsureSuccessStatusCode();

        var total = resp.Content.Headers.ContentLength;
        await using var src = await resp.Content.ReadAsStreamAsync(ct);
        await using var dst = new FileStream(tmp, FileMode.Create, FileAccess.Write, FileShare.None);

        var hasher = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        var buf = new byte[1024 * 128];
        long readTotal = 0;

        var sw = Stopwatch.StartNew();
        while (true)
        {
            var n = await src.ReadAsync(buf.AsMemory(0, buf.Length), ct);
            if (n <= 0)
            {
                break;
            }

            await dst.WriteAsync(buf.AsMemory(0, n), ct);
            hasher.AppendData(buf, 0, n);
            readTotal += n;

            if (progress is not null)
            {
                var speed = sw.Elapsed.TotalSeconds <= 0 ? 0 : (readTotal / sw.Elapsed.TotalSeconds);
                progress.Report(new UpdateProgress { BytesDownloaded = readTotal, TotalBytes = total, BytesPerSecond = speed });
            }
        }

        await dst.FlushAsync(ct);

        var got = ToHex(hasher.GetHashAndReset());
        if (!string.Equals(got, expectedSha256, StringComparison.OrdinalIgnoreCase))
        {
            try { File.Delete(tmp); } catch { }
            throw new Exception($"sha256 mismatch: expected={expectedSha256} got={got}");
        }

        File.Copy(tmp, outPath, overwrite: true);
        try { File.Delete(tmp); } catch { }
    }

    public static string NormalizeVersion(string raw)
    {
        var s = (raw ?? "").Trim();
        if (s.StartsWith("v", StringComparison.OrdinalIgnoreCase))
        {
            s = s[1..];
        }
        return s.Trim();
    }

    public static int CompareVersionStrings(string a, string b)
        => CompareVersions(NormalizeVersion(a), NormalizeVersion(b));

    private static bool IsHex(string s)
    {
        foreach (var c in s)
        {
            var ok = (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
            if (!ok)
            {
                return false;
            }
        }
        return true;
    }

    private static string ToHex(byte[] bytes)
    {
        var sb = new StringBuilder(bytes.Length * 2);
        foreach (var b in bytes)
        {
            _ = sb.Append(b.ToString("x2", CultureInfo.InvariantCulture));
        }
        return sb.ToString();
    }

    private static int CompareVersions(string a, string b)
    {
        // Minimal semver-ish compare: split by '.' and compare numeric parts.
        var pa = a.Split('.', StringSplitOptions.RemoveEmptyEntries);
        var pb = b.Split('.', StringSplitOptions.RemoveEmptyEntries);
        var n = Math.Max(pa.Length, pb.Length);
        for (var i = 0; i < n; i++)
        {
            var ai = i < pa.Length ? ParseIntPrefix(pa[i]) : 0;
            var bi = i < pb.Length ? ParseIntPrefix(pb[i]) : 0;
            if (ai != bi)
            {
                return ai.CompareTo(bi);
            }
        }
        return 0;
    }

    private static int ParseIntPrefix(string s)
    {
        var x = 0;
        foreach (var c in (s ?? ""))
        {
            if (c < '0' || c > '9')
            {
                break;
            }
            x = checked(x * 10 + (c - '0'));
        }
        return x;
    }

    private static bool IsSemverLike(string s)
        => !string.IsNullOrWhiteSpace(s) && s.Split('.', StringSplitOptions.RemoveEmptyEntries).Length >= 2;

    private static string Quote(string s)
    {
        if (s.Contains('"'))
        {
            s = s.Replace("\"", "\"\"");
        }
        return $"\"{s}\"";
    }
}

public abstract record UpdateCheckResult;

public sealed record UpdateUpToDate(string CurrentVersion, string ReleasePageUrl) : UpdateCheckResult;

public sealed record UpdateAvailable(
    string CurrentVersion,
    string LatestVersion,
    string ReleasePageUrl,
    string ZipUrl,
    string Sha256Url
) : UpdateCheckResult;

public sealed record UpdateError(string Message) : UpdateCheckResult;

public sealed class UpdateProgress
{
    public long BytesDownloaded { get; init; }
    public long? TotalBytes { get; init; }
    public double BytesPerSecond { get; init; }
}

public sealed class UpdatePending
{
    public string Version { get; set; } = "";
    public string ZipPath { get; set; } = "";
    public string Sha256 { get; set; } = "";
    public long DownloadedAtUnixMs { get; set; }
}
