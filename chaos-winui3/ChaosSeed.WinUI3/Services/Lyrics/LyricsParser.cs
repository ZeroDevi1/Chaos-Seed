using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text.RegularExpressions;
using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services.Lyrics;

public static partial class LyricsParser
{
    public static IEnumerable<LyricsLineVm> ParseMergedLines(string lyricsOriginal, string? lyricsTranslation)
    {
        var orig = lyricsOriginal ?? "";
        var tran = lyricsTranslation ?? "";

        var origTimed = ParseLrcTimedLines(orig);
        var tranTimed = ParseLrcTimedLines(tran);

        var hasAnyTimed = origTimed.Any(x => x.TimeMs is not null) || tranTimed.Any(x => x.TimeMs is not null);

        if (hasAnyTimed)
        {
            var origTimeCount = origTimed.Count(x => x.TimeMs is not null);
            var tranTimeCount = tranTimed.Count(x => x.TimeMs is not null);

            if (origTimeCount == 0)
            {
                foreach (var x in BuildIndexAlignedLines(
                             SplitNonEmptyLines(orig),
                             SplitNonEmptyLines(tran),
                             timeLabels: null
                         ))
                {
                    yield return x;
                }
                yield break;
            }

            if (tranTimeCount == 0)
            {
                var o = origTimed.Where(x => x.TimeMs is not null).OrderBy(x => x.TimeMs).ToList();
                var t = SplitNonEmptyLines(tran);
                for (var i = 0; i < o.Count; i++)
                {
                    var tr = i < t.Count ? t[i] : null;
                    yield return new LyricsLineVm(o[i].TimeMs, o[i].Text, tr);
                }
                yield break;
            }

            var oDict = origTimed
                .Where(x => x.TimeMs is not null && !string.IsNullOrWhiteSpace(x.Text))
                .GroupBy(x => x.TimeMs!.Value)
                .ToDictionary(g => g.Key, g => string.Join(" ", g.Select(x => x.Text).Where(s => !string.IsNullOrWhiteSpace(s))));

            var tDict = tranTimed
                .Where(x => x.TimeMs is not null && !string.IsNullOrWhiteSpace(x.Text))
                .GroupBy(x => x.TimeMs!.Value)
                .ToDictionary(g => g.Key, g => string.Join(" ", g.Select(x => x.Text).Where(s => !string.IsNullOrWhiteSpace(s))));

            var times = oDict.Keys
                .Union(tDict.Keys)
                .OrderBy(x => x)
                .ToList();

            foreach (var t in times)
            {
                oDict.TryGetValue(t, out var oText);
                tDict.TryGetValue(t, out var trText);
                if (string.IsNullOrWhiteSpace(oText) && string.IsNullOrWhiteSpace(trText))
                {
                    continue;
                }
                yield return new LyricsLineVm(t, oText ?? "", trText);
            }
            yield break;
        }

        foreach (var x in BuildIndexAlignedLines(SplitNonEmptyLines(orig), SplitNonEmptyLines(tran), timeLabels: null))
        {
            yield return x;
        }
    }

    private static IEnumerable<LyricsLineVm> BuildIndexAlignedLines(
        List<string> originalLines,
        List<string> translationLines,
        List<ulong?>? timeLabels
    )
    {
        var n = Math.Max(originalLines.Count, translationLines.Count);
        for (var i = 0; i < n; i++)
        {
            var o = i < originalLines.Count ? originalLines[i] : "";
            var t = i < translationLines.Count ? translationLines[i] : null;
            var tm = timeLabels is not null && i < timeLabels.Count ? timeLabels[i] : null;
            if (string.IsNullOrWhiteSpace(o) && string.IsNullOrWhiteSpace(t))
            {
                continue;
            }
            yield return new LyricsLineVm(tm, o, t);
        }
    }

    private sealed record TimedLine(ulong? TimeMs, string Text);

    private static List<TimedLine> ParseLrcTimedLines(string raw)
    {
        var lines = new List<TimedLine>();
        foreach (var line in (raw ?? "").Split('\n', StringSplitOptions.RemoveEmptyEntries))
        {
            var s = line.TrimEnd('\r').Trim();
            if (s.Length == 0)
            {
                continue;
            }

            var matches = TimeTagRegex().Matches(s);
            if (matches.Count == 0)
            {
                lines.Add(new TimedLine(null, StripMetaTags(s)));
                continue;
            }

            var text = TimeTagRegex().Replace(s, "").Trim();
            text = StripMetaTags(text);

            foreach (Match m in matches)
            {
                var timeMs = ParseTimeTagToMs(m);
                lines.Add(new TimedLine(timeMs, text));
            }
        }

        return lines;
    }

    private static string StripMetaTags(string s)
    {
        var x = (s ?? "").Trim();
        if (x.StartsWith("[", StringComparison.Ordinal) && x.Contains(':') && x.EndsWith("]", StringComparison.Ordinal))
        {
            return "";
        }
        return x;
    }

    private static List<string> SplitNonEmptyLines(string raw)
    {
        return (raw ?? "")
            .Split('\n', StringSplitOptions.RemoveEmptyEntries)
            .Select(x => x.TrimEnd('\r').Trim())
            .Where(x => !string.IsNullOrWhiteSpace(x) && !x.StartsWith("[", StringComparison.Ordinal))
            .ToList();
    }

    private static ulong ParseTimeTagToMs(Match m)
    {
        try
        {
            var min = int.Parse(m.Groups["mm"].Value, CultureInfo.InvariantCulture);
            var sec = int.Parse(m.Groups["ss"].Value, CultureInfo.InvariantCulture);
            var fracRaw = m.Groups["ff"].Success ? m.Groups["ff"].Value : "";

            var ms = 0;
            if (!string.IsNullOrWhiteSpace(fracRaw))
            {
                if (fracRaw.Length == 1)
                {
                    ms = int.Parse(fracRaw, CultureInfo.InvariantCulture) * 100;
                }
                else if (fracRaw.Length == 2)
                {
                    ms = int.Parse(fracRaw, CultureInfo.InvariantCulture) * 10;
                }
                else
                {
                    ms = int.Parse(fracRaw.Substring(0, 3), CultureInfo.InvariantCulture);
                }
            }

            var total = (min * 60 * 1000L) + (sec * 1000L) + ms;
            if (total < 0)
            {
                total = 0;
            }
            return (ulong)total;
        }
        catch
        {
            return 0;
        }
    }

    [GeneratedRegex(@"\[(?<mm>\d{1,2}):(?<ss>\d{2})(?:\.(?<ff>\d{1,3}))?\]", RegexOptions.Compiled)]
    private static partial Regex TimeTagRegex();
}

