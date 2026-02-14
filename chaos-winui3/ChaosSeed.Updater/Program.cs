using System.Diagnostics;
using System.Globalization;
using System.IO.Compression;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;

static int Main(string[] args)
{
    var logPath = GetLogPath();
    Directory.CreateDirectory(Path.GetDirectoryName(logPath)!);
    using var log = new StreamWriter(new FileStream(logPath, FileMode.Create, FileAccess.Write, FileShare.ReadWrite))
    {
        AutoFlush = true
    };

    void Info(string msg)
    {
        var line = $"[{DateTime.Now:HH:mm:ss.fff}] {msg}";
        try { log.WriteLine(line); } catch { }
        try { Console.WriteLine(line); } catch { }
    }

    Info("starting updater");
    Info("raw args: " + string.Join(" ", args.Select(QuoteArg)));

    var opt = Options.Parse(args);
    if (opt is null)
    {
        const string usage = "Usage: ChaosSeed.Updater --app-dir <dir> --zip <path> --expected-sha256 <hex> --restart-exe <exe> --version <ver> --parent-pid <pid>";
        Info("invalid args; exiting");
        Info(usage);
        TryShowError($"Update failed: invalid arguments.\n\n{usage}\n\nLog:\n{logPath}");
        return 2;
    }

    try
    {
        // If we run from the app directory, we may not be able to overwrite ChaosSeed.Updater.exe while it's running.
        // Relaunch a copy from %TEMP% so we can replace the updater itself too.
        if (!opt.Relocated && TryRelaunchFromTemp(args, Info))
        {
            return 0;
        }

        using var mutex = new Mutex(initiallyOwned: true, name: "ChaosSeed.Updater", out var isNew);
        if (!isNew)
        {
            Info("another updater instance is running; exiting");
            return 3;
        }

        Info($"updater log: {logPath}");
        Info($"appDir={opt.AppDir}");
        Info($"zip={opt.ZipPath}");
        Info($"version={opt.Version}");

        WaitForParentExit(opt.ParentPid, Info);

        VerifySha256(opt.ZipPath, opt.ExpectedSha256, Info);

        var stagingRoot = Path.Combine(Path.GetTempPath(), "ChaosSeed.UpdateStaging", opt.Version);
        if (Directory.Exists(stagingRoot))
        {
            TryDeleteDir(stagingRoot);
        }
        Directory.CreateDirectory(stagingRoot);
        Info($"extracting to staging: {stagingRoot}");
        ZipFile.ExtractToDirectory(opt.ZipPath, stagingRoot, overwriteFiles: true);

        var backupRoot = Path.Combine(opt.AppDir, ".bak", opt.Version);
        Directory.CreateDirectory(backupRoot);

        var plan = ComputeReplacePlan(stagingRoot, opt.AppDir);
        Info($"files to write: {plan.Count}");

        var backups = new List<(string AppPath, string BackupPath)>();
        try
        {
            foreach (var item in plan)
            {
                var appPath = item.AppPath;
                var srcPath = item.SrcPath;

                Directory.CreateDirectory(Path.GetDirectoryName(appPath)!);

                if (File.Exists(appPath))
                {
                    var rel = Path.GetRelativePath(opt.AppDir, appPath);
                    var backupPath = Path.Combine(backupRoot, rel);
                    Directory.CreateDirectory(Path.GetDirectoryName(backupPath)!);
                    CopyFileWithRetry(appPath, backupPath, Info);
                    backups.Add((appPath, backupPath));
                }

                CopyFileWithRetry(srcPath, appPath, Info);
            }
        }
        catch (Exception ex)
        {
            Info($"replace failed: {ex.Message}");
            Info("attempting rollback...");
            Rollback(backups, Info);
            throw;
        }
        finally
        {
            TryDeleteDir(stagingRoot);
        }

        // Best-effort: keep pending updates small.
        TryCleanupPendingUpdates(opt.Version, Info);

        Info("starting updated app...");
        var exePath = Path.Combine(opt.AppDir, opt.RestartExe);
        Process.Start(new ProcessStartInfo
        {
            FileName = exePath,
            WorkingDirectory = opt.AppDir,
            UseShellExecute = true,
        });

        Info("done");
        return 0;
    }
    catch (Exception ex)
    {
        try { log.WriteLine(ex.ToString()); } catch { }
        TryShowError($"Update failed: {ex.Message}\n\nLog:\n{logPath}");
        return 1;
    }
}

static bool TryRelaunchFromTemp(string[] originalArgs, Action<string> info)
{
    try
    {
        var exePath = Environment.ProcessPath;
        if (string.IsNullOrWhiteSpace(exePath) || !File.Exists(exePath))
        {
            return false;
        }

        var srcDir = Path.GetDirectoryName(exePath)!;
        var exeName = Path.GetFileName(exePath);
        if (!string.Equals(exeName, "ChaosSeed.Updater.exe", StringComparison.OrdinalIgnoreCase))
        {
            return false;
        }

        var tempDir = Path.Combine(Path.GetTempPath(), "ChaosSeed.UpdaterRunner", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);

        foreach (var f in Directory.EnumerateFiles(srcDir, "ChaosSeed.Updater.*", SearchOption.TopDirectoryOnly))
        {
            var dst = Path.Combine(tempDir, Path.GetFileName(f));
            File.Copy(f, dst, overwrite: true);
        }

        var newExe = Path.Combine(tempDir, "ChaosSeed.Updater.exe");
        if (!File.Exists(newExe))
        {
            return false;
        }

        var newArgs = originalArgs.Concat(new[] { "--relocated", "1" }).ToArray();
        var argLine = string.Join(" ", newArgs.Select(QuoteArg));
        info($"relaunching updater from temp: {newExe}");
        Process.Start(new ProcessStartInfo
        {
            FileName = newExe,
            Arguments = argLine,
            UseShellExecute = false,
            CreateNoWindow = true,
            WorkingDirectory = tempDir,
        });
        return true;
    }
    catch
    {
        return false;
    }
}

static string QuoteArg(string s)
{
    if (string.IsNullOrEmpty(s))
    {
        return "\"\"";
    }
    if (s.IndexOfAny(new[] { ' ', '\t', '"', '\r', '\n' }) < 0)
    {
        return s;
    }
    return "\"" + s.Replace("\"", "\"\"") + "\"";
}

static void WaitForParentExit(int parentPid, Action<string> info)
{
    if (parentPid <= 0)
    {
        return;
    }

    try
    {
        var p = Process.GetProcessById(parentPid);
        info($"waiting for parent pid={parentPid} to exit...");
        if (!p.WaitForExit(30_000))
        {
            throw new Exception("timeout waiting for app to exit");
        }
        info("parent exited");
    }
    catch (ArgumentException)
    {
        // already exited
    }
}

static void VerifySha256(string path, string expected, Action<string> info)
{
    expected = (expected ?? "").Trim().ToLowerInvariant();
    if (expected.Length != 64)
    {
        throw new Exception("expected sha256 is invalid");
    }

    info("verifying sha256...");
    using var fs = File.OpenRead(path);
    var got = Convert.ToHexString(SHA256.HashData(fs)).ToLowerInvariant();
    if (!string.Equals(got, expected, StringComparison.OrdinalIgnoreCase))
    {
        throw new Exception($"sha256 mismatch: expected={expected} got={got}");
    }
    info("sha256 OK");
}

static List<(string SrcPath, string AppPath)> ComputeReplacePlan(string stagingRoot, string appRoot)
{
    var list = new List<(string SrcPath, string AppPath)>();
    foreach (var src in Directory.EnumerateFiles(stagingRoot, "*", SearchOption.AllDirectories))
    {
        var rel = Path.GetRelativePath(stagingRoot, src);
        var dst = Path.Combine(appRoot, rel);
        list.Add((src, dst));
    }
    return list;
}

static void CopyFileWithRetry(string src, string dst, Action<string> info)
{
    const int maxTries = 12;
    var delayMs = 80;
    for (var i = 1; i <= maxTries; i++)
    {
        try
        {
            File.Copy(src, dst, overwrite: true);
            return;
        }
        catch (Exception ex) when (i < maxTries)
        {
            info($"copy failed (try {i}/{maxTries}): {Path.GetFileName(dst)} ({ex.Message})");
            Thread.Sleep(delayMs);
            delayMs = Math.Min(delayMs * 2, 1500);
        }
    }
}

static void Rollback(List<(string AppPath, string BackupPath)> backups, Action<string> info)
{
    // Reverse order: restore most recently overwritten first.
    for (var i = backups.Count - 1; i >= 0; i--)
    {
        var (appPath, backupPath) = backups[i];
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(appPath)!);
            File.Copy(backupPath, appPath, overwrite: true);
        }
        catch (Exception ex)
        {
            info($"rollback failed for {appPath}: {ex.Message}");
        }
    }
}

static void TryDeleteDir(string dir)
{
    try
    {
        Directory.Delete(dir, recursive: true);
    }
    catch
    {
        // ignore
    }
}

static string GetLogPath()
{
    var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
    if (string.IsNullOrWhiteSpace(root))
    {
        root = Path.GetTempPath();
    }
    var dir = Path.Combine(root, "ChaosSeed.WinUI3", "logs");
    return Path.Combine(dir, $"updater-{DateTime.Now:yyyyMMdd-HHmmss}.log");
}

static void TryCleanupPendingUpdates(string keepVersion, Action<string> info)
{
    try
    {
        var root = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "ChaosSeed.WinUI3",
            "updates",
            "pending"
        );
        if (!Directory.Exists(root))
        {
            return;
        }

        foreach (var d in Directory.EnumerateDirectories(root))
        {
            var name = Path.GetFileName(d);
            if (string.Equals(name, keepVersion, StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }
            TryDeleteDir(d);
        }
    }
    catch (Exception ex)
    {
        info($"cleanup pending updates failed: {ex.Message}");
    }
}

static void TryShowError(string msg)
{
    try
    {
        _ = MessageBoxW(IntPtr.Zero, msg, "ChaosSeed Updater", 0x00000010 /* MB_ICONERROR */);
    }
    catch
    {
        // ignore
    }
}

[DllImport("user32.dll", CharSet = CharSet.Unicode)]
static extern int MessageBoxW(IntPtr hWnd, string text, string caption, uint type);

sealed class Options
{
    public string AppDir { get; init; } = "";
    public string ZipPath { get; init; } = "";
    public string ExpectedSha256 { get; init; } = "";
    public string RestartExe { get; init; } = "";
    public string Version { get; init; } = "";
    public int ParentPid { get; init; }
    public bool Relocated { get; init; }

    public static Options? Parse(string[] args)
    {
        string? appDir = null;
        string? zip = null;
        string? sha = null;
        string? restartExe = null;
        string? version = null;
        int parentPid = 0;
        var relocated = false;

        for (var i = 0; i < args.Length; i++)
        {
            var a = args[i];
            string? Next() => i + 1 < args.Length ? args[++i] : null;
            switch (a)
            {
                case "--app-dir":
                    appDir = Next();
                    break;
                case "--zip":
                    zip = Next();
                    break;
                case "--expected-sha256":
                    sha = Next();
                    break;
                case "--restart-exe":
                    restartExe = Next();
                    break;
                case "--version":
                    version = Next();
                    break;
                case "--parent-pid":
                    _ = int.TryParse(Next(), NumberStyles.Integer, CultureInfo.InvariantCulture, out parentPid);
                    break;
                case "--relocated":
                    _ = Next();
                    relocated = true;
                    break;
            }
        }

        if (string.IsNullOrWhiteSpace(appDir) || string.IsNullOrWhiteSpace(zip) || string.IsNullOrWhiteSpace(sha))
        {
            return null;
        }
        if (string.IsNullOrWhiteSpace(restartExe) || string.IsNullOrWhiteSpace(version))
        {
            return null;
        }

        appDir = appDir.Trim().TrimEnd(Path.DirectorySeparatorChar);
        if (!Directory.Exists(appDir))
        {
            return null;
        }
        if (!File.Exists(zip))
        {
            return null;
        }

        return new Options
        {
            AppDir = appDir,
            ZipPath = zip,
            ExpectedSha256 = sha,
            RestartExe = restartExe,
            Version = version,
            ParentPid = parentPid,
            Relocated = relocated,
        };
    }
}
