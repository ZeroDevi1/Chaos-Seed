using System;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;

namespace ChaosSeed.WinUI3.Services;

public static class AppLog
{
    private static readonly object _gate = new();

    private static string LogDir =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "ChaosSeed", "logs");

    private static string LogPath => Path.Combine(LogDir, "winui3.log");

    public static void Info(string message) => Write("INFO", message);

    public static void Error(string message) => Write("ERROR", message);

    public static void Exception(string context, Exception ex) =>
        Write("EX", $"{context}\n{ex}");

    private static void Write(string level, string message)
    {
        try
        {
            var ts = DateTimeOffset.Now.ToString("yyyy-MM-dd HH:mm:ss.fff zzz");
            var line = $"[{ts}] [{level}] {MaskSecrets(message ?? "")}\n";
            lock (_gate)
            {
                Directory.CreateDirectory(LogDir);
                File.AppendAllText(LogPath, line, Encoding.UTF8);
            }
        }
        catch
        {
            // ignore
        }
    }

    private static string MaskSecrets(string s)
    {
        if (string.IsNullOrEmpty(s))
        {
            return s;
        }

        static string KeepPrefix(Match m)
        {
            var v = m.Groups["v"].Value;
            if (string.IsNullOrEmpty(v))
            {
                return $"{m.Groups["k"].Value}=<redacted>";
            }
            var keep = v.Length <= 6 ? v : v.Substring(0, 6);
            return $"{m.Groups["k"].Value}={keep}***";
        }

        // Bilibili cookies/tokens
        s = Regex.Replace(s, @"(?<k>SESSDATA)=(?<v>[^;\s]+)", KeepPrefix, RegexOptions.IgnoreCase);
        s = Regex.Replace(s, @"(?<k>bili_jct)=(?<v>[^;\s]+)", KeepPrefix, RegexOptions.IgnoreCase);
        s = Regex.Replace(s, @"(?<k>refresh_token)=(?<v>[^;\s]+)", KeepPrefix, RegexOptions.IgnoreCase);
        s = Regex.Replace(s, @"(?<k>access_token)=(?<v>[^;\s]+)", KeepPrefix, RegexOptions.IgnoreCase);

        // Generic token-like kv
        s = Regex.Replace(s, @"(?<k>token)=(?<v>[^;\s]+)", KeepPrefix, RegexOptions.IgnoreCase);
        return s;
    }
}

