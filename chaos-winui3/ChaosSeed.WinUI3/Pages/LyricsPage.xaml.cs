using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Globalization;
using System.Linq;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Text.RegularExpressions;
using System.Threading;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using ChaosSeed.WinUI3.Services.LyricsBackends;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Storage.Streams;
using Muxc = Microsoft.UI.Xaml.Controls;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class LyricsPage : Page
{
    private readonly Microsoft.UI.Dispatching.DispatcherQueue _dq =
        Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();

    public ObservableCollection<LyricsCandidateVm> Candidates { get; } = new();
    public ObservableCollection<LyricsLineVm> Lines { get; } = new();
    public ObservableCollection<NowPlayingSessionVm> SessionOptions { get; } = new();

    private ILyricsBackend? _backend;

    private bool _uiInit;
    private bool _sessionInit;
    private bool _suppressCandidateChanged;

    private CancellationTokenSource? _watchCts;
    private Task? _watchTask;
    private CancellationTokenSource? _searchCts;
    private Task? _searchTask;

    private string? _lastSongKey;
    private NowPlayingSession? _lastNowPlaying;
    private NowPlayingSnapshot? _lastSnapshot;
    private LyricsSearchResult? _currentLyrics;
    private long _coverSeq;

    public LyricsPage()
    {
        InitializeComponent();

        Loaded += (_, _) =>
        {
            InitBackend();
            InitUiFromSettings();
            SettingsService.Instance.SettingsChanged += OnSettingsChanged;
            EnsureWatchFromSettings();
        };

        Unloaded += (_, _) => Shutdown();
    }

    private void Shutdown()
    {
        SettingsService.Instance.SettingsChanged -= OnSettingsChanged;
        StopWatch();

        try
        {
            _backend?.Dispose();
        }
        catch
        {
            // ignore
        }
        _backend = null;
    }

    private void InitBackend()
    {
        if (_backend is not null)
        {
            return;
        }

        _backend = LyricsBackendFactory.Create();
        if (!string.IsNullOrWhiteSpace(_backend.InitNotice))
        {
            BackendBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational;
            BackendBar.Title = _backend.Name;
            BackendBar.Message = _backend.InitNotice;
            BackendBar.IsOpen = true;
        }
        else
        {
            BackendBar.IsOpen = false;
        }
    }

    private void OnSettingsChanged(object? sender, EventArgs e)
    {
        _ = sender;
        _ = e;
        InitUiFromSettings();
        EnsureWatchFromSettings();
    }

    private void InitUiFromSettings()
    {
        _uiInit = true;
        try
        {
            var s = SettingsService.Instance.Current;

            ThresholdBox.Value = Math.Clamp(s.LyricsThreshold, 0, 100);
            LimitBox.Value = Math.Clamp(s.LyricsLimit, 1, 50);
            TimeoutBox.Value = Math.Clamp(s.LyricsTimeoutMs, 1, 60000);

            ApplyProviderChecksToFlyout(s.LyricsProviders);
            UpdateProvidersSummary();
        }
        finally
        {
            _uiInit = false;
        }
    }

    private void EnsureWatchFromSettings()
    {
        try
        {
            if (SettingsService.Instance.Current.LyricsAutoDetect)
            {
                StartWatch();
            }
            else
            {
                StopWatch();
            }
        }
        catch
        {
            // ignore
        }
    }

    private async void OnRefreshNowPlayingClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;

        try
        {
            await RefreshNowPlayingAsync(CancellationToken.None);
        }
        catch (Exception ex)
        {
            ShowBackendError(ex.Message);
        }
    }

    private void StartWatch()
    {
        InitBackend();

        if (_watchCts is not null)
        {
            return;
        }

        _watchCts = new CancellationTokenSource();
        var ct = _watchCts.Token;
        _watchTask = Task.Run(() => WatchLoopAsync(ct), ct);
    }

    private void StopWatch()
    {
        try
        {
            _watchCts?.Cancel();
        }
        catch
        {
            // ignore
        }

        try
        {
            _watchCts?.Dispose();
        }
        catch
        {
            // ignore
        }

        _watchCts = null;
        _watchTask = null;

        try
        {
            _searchCts?.Cancel();
        }
        catch
        {
            // ignore
        }

        try
        {
            _searchCts?.Dispose();
        }
        catch
        {
            // ignore
        }

        _searchCts = null;
        _searchTask = null;
    }

    private async Task WatchLoopAsync(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            NowPlayingSnapshot? snap = null;
            try
            {
                snap = await RefreshNowPlayingAsync(ct);
            }
            catch (OperationCanceledException)
            {
                break;
            }
            catch (Exception ex)
            {
                await RunOnUiAsync(() => ShowBackendError(ex.Message));
            }

            try
            {
                var np = _lastNowPlaying ?? snap?.NowPlaying;
                if (np is not null)
                {
                    var key = BuildSongKey(np);
                    if (!string.IsNullOrWhiteSpace(key) && key != _lastSongKey)
                    {
                        _lastSongKey = key;
                        TriggerSequentialSearch(np, ct);
                    }
                }
            }
            catch
            {
                // ignore
            }

            var isPlaying = (_lastNowPlaying ?? snap?.NowPlaying)?.PlaybackStatus?.Equals("Playing", StringComparison.OrdinalIgnoreCase) == true;
            var sleepMs = isPlaying ? 2000 : 8000;
            try
            {
                await Task.Delay(sleepMs, ct);
            }
            catch (OperationCanceledException)
            {
                break;
            }
        }
    }

    private void TriggerSequentialSearch(NowPlayingSession np, CancellationToken parentCt)
    {
        try
        {
            _searchCts?.Cancel();
        }
        catch
        {
            // ignore
        }

        try
        {
            _searchCts?.Dispose();
        }
        catch
        {
            // ignore
        }

        _searchCts = CancellationTokenSource.CreateLinkedTokenSource(parentCt);
        var ct = _searchCts.Token;
        _searchTask = Task.Run(() => SequentialSearchAsync(np, ct), ct);
    }

    private async Task SequentialSearchAsync(NowPlayingSession np, CancellationToken ct)
    {
        var backend = _backend;
        if (backend is null)
        {
            return;
        }

        var title = (np.Title ?? "").Trim();
        if (string.IsNullOrWhiteSpace(title))
        {
            await RunOnUiAsync(() =>
            {
                UpdateCandidates(Array.Empty<LyricsSearchResult>(), autoSelect: false);
                ClearLyricsUi();
            });
            return;
        }

        var s = SettingsService.Instance.Current;
        var providers = NormalizeProviders(s.LyricsProviders);
        var threshold = Math.Clamp(s.LyricsThreshold, 0, 100);
        var limit = Math.Clamp(s.LyricsLimit, 1, 50);
        var timeoutMs = Math.Clamp(s.LyricsTimeoutMs, 1, 60000);

        var artist = (np.Artist ?? "").Trim();
        var album = (np.AlbumTitle ?? "").Trim();

        var all = new List<LyricsSearchResult>();
        LyricsSearchResult? bestOverAll = null;

        foreach (var provider in providers)
        {
            ct.ThrowIfCancellationRequested();

            var p = new LyricsSearchParams
            {
                Title = title,
                Artist = string.IsNullOrWhiteSpace(artist) ? null : artist,
                Album = string.IsNullOrWhiteSpace(album) ? null : album,
                DurationMs = np.DurationMs,
                Limit = (uint)limit,
                StrictMatch = false,
                Services = new[] { provider },
                TimeoutMs = (ulong)timeoutMs,
            };

            LyricsSearchResult[] items;
            try
            {
                items = await backend.SearchLyricsAsync(p, ct);
            }
            catch
            {
                continue;
            }

            if (items.Length == 0)
            {
                continue;
            }

            all.AddRange(items);

            var best = items.OrderByDescending(x => x.MatchPercentage).FirstOrDefault();
            if (best is null)
            {
                continue;
            }

            if (bestOverAll is null || best.MatchPercentage > bestOverAll.MatchPercentage)
            {
                bestOverAll = best;
            }

            if (best.MatchPercentage >= threshold)
            {
                bestOverAll = best;
                break;
            }
        }

        var sorted = all
            .OrderByDescending(x => x.MatchPercentage)
            .ThenBy(x => (x.Service ?? "").Trim(), StringComparer.OrdinalIgnoreCase)
            .Take(50)
            .ToArray();

        if (bestOverAll is not null && bestOverAll.MatchPercentage >= threshold)
        {
            _currentLyrics = bestOverAll;
            await RunOnUiAsync(() =>
            {
                UpdateCandidates(sorted, autoSelect: true, select: bestOverAll);
                ApplyLyricsUi(bestOverAll);
            });
        }
        else
        {
            _currentLyrics = null;
            await RunOnUiAsync(() =>
            {
                UpdateCandidates(sorted, autoSelect: false);
                ClearLyricsUi();
            });
        }
    }

    private async Task<NowPlayingSnapshot> RefreshNowPlayingAsync(CancellationToken ct)
    {
        InitBackend();

        var backend = _backend ?? throw new InvalidOperationException("lyrics backend not initialized");

        // Include thumbnail for the UI cover image; keep bytes capped to avoid UI stalls.
        var snap = await backend.SnapshotNowPlayingAsync(
            includeThumbnail: true,
            maxThumbBytes: 262_144,
            maxSessions: 32,
            ct
        );

        await RunOnUiAsync(() => ApplyNowPlayingUi(snap));
        return snap;
    }

    private void ApplyNowPlayingUi(NowPlayingSnapshot snap)
    {
        _lastSnapshot = snap;
        SupportedText.Text = snap.Supported ? "true" : "false";

        var sel = UpdateSessionOptions(snap);
        ApplyNowPlayingSession(sel ?? snap.NowPlaying);
    }

    private void ApplyNowPlayingSession(NowPlayingSession? np)
    {
        _lastNowPlaying = np;

        StatusText.Text = np?.PlaybackStatus ?? "-";
        TitleText.Text = np?.Title ?? "-";
        ArtistText.Text = np?.Artist ?? "-";
        AlbumText.Text = np?.AlbumTitle ?? "-";
        AppIdText.Text = np?.AppId ?? "-";

        if (np is null)
        {
            TimelineText.Text = "-";
        }
        else
        {
            TimelineText.Text = $"{FormatMs(np.PositionMs)} / {FormatMs(np.DurationMs)}";
        }

        _ = UpdateCoverAsync(np?.Thumbnail);
    }

    private NowPlayingSession? UpdateSessionOptions(NowPlayingSnapshot snap)
    {
        SessionOptions.Clear();

        var sessions = snap.Sessions ?? Array.Empty<NowPlayingSession>();
        if (sessions.Length <= 1)
        {
            SessionCombo.Visibility = Visibility.Collapsed;
            UseSessionBtn.Visibility = Visibility.Collapsed;
            return sessions.FirstOrDefault() ?? snap.NowPlaying;
        }

        foreach (var s in sessions)
        {
            SessionOptions.Add(NowPlayingSessionVm.From(s));
        }

        // Prefer saved app id, then "isCurrent", then pickedAppId, then first session.
        var preferred = (SettingsService.Instance.Current.LyricsPreferredAppId ?? "").Trim();
        NowPlayingSessionVm? sel = null;
        if (!string.IsNullOrWhiteSpace(preferred))
        {
            sel = SessionOptions.FirstOrDefault(x =>
                string.Equals(x.Session.AppId, preferred, StringComparison.OrdinalIgnoreCase)
            );
        }
        sel ??= SessionOptions.FirstOrDefault(x => x.Session.IsCurrent);
        if (sel is null && !string.IsNullOrWhiteSpace(snap.PickedAppId))
        {
            sel = SessionOptions.FirstOrDefault(x =>
                string.Equals(x.Session.AppId, snap.PickedAppId, StringComparison.OrdinalIgnoreCase)
            );
        }
        sel ??= SessionOptions.FirstOrDefault();

        _sessionInit = true;
        SessionCombo.SelectedItem = sel;
        _sessionInit = false;
        SessionCombo.Visibility = Visibility.Visible;
        UseSessionBtn.Visibility = Visibility.Collapsed;

        return sel?.Session;
    }

    private void OnSessionSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        _ = sender;
        _ = e;

        if (_sessionInit || _uiInit)
        {
            return;
        }

        if (SessionCombo.SelectedItem is not NowPlayingSessionVm vm)
        {
            return;
        }

        ApplySelectedSession(vm.Session, persist: true);
    }

    private void OnUseSelectedSessionClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;

        if (SessionCombo.SelectedItem is not NowPlayingSessionVm vm)
        {
            return;
        }

        ApplySelectedSession(vm.Session, persist: true);
    }

    private void ApplySelectedSession(NowPlayingSession? session, bool persist)
    {
        if (session is null)
        {
            return;
        }

        ApplyNowPlayingSession(session);

        if (persist)
        {
            try
            {
                var appId = (session.AppId ?? "").Trim();
                SettingsService.Instance.Update(s =>
                    s.LyricsPreferredAppId = string.IsNullOrWhiteSpace(appId) ? null : appId
                );
            }
            catch
            {
                // ignore
            }
        }

        try
        {
            var key = BuildSongKey(session);
            _lastSongKey = key;
            TriggerSequentialSearch(session, CancellationToken.None);
        }
        catch
        {
            // ignore
        }
    }
    private async Task UpdateCoverAsync(NowPlayingThumbnail? thumb)
    {
        var seq = Interlocked.Increment(ref _coverSeq);

        if (thumb is null || string.IsNullOrWhiteSpace(thumb.Base64))
        {
            await RunOnUiAsync(() =>
            {
                if (seq != _coverSeq)
                {
                    return;
                }
                CoverImage.Source = null;
            });
            return;
        }

        byte[]? bytes = null;
        try
        {
            bytes = Convert.FromBase64String(thumb.Base64);
        }
        catch
        {
            bytes = null;
        }

        if (bytes is null || bytes.Length == 0)
        {
            await RunOnUiAsync(() =>
            {
                if (seq != _coverSeq)
                {
                    return;
                }
                CoverImage.Source = null;
            });
            return;
        }

        await RunOnUiAsync(async () =>
        {
            if (seq != _coverSeq)
            {
                return;
            }

            try
            {
                using var ms = new InMemoryRandomAccessStream();
                await ms.WriteAsync(bytes.AsBuffer());
                ms.Seek(0);

                var bmp = new BitmapImage();
                await bmp.SetSourceAsync(ms);
                CoverImage.Source = bmp;
            }
            catch
            {
                CoverImage.Source = null;
            }
        });
    }

    private void UpdateCandidates(LyricsSearchResult[] items, bool autoSelect, LyricsSearchResult? select = null)
    {
        _suppressCandidateChanged = true;
        try
        {
            Candidates.Clear();
            foreach (var x in items)
            {
                Candidates.Add(new LyricsCandidateVm(x));
            }

            if (autoSelect && select is not null)
            {
                var vm = Candidates.FirstOrDefault(c => ReferenceEquals(c.Item, select));
                CandidateCombo.SelectedItem = vm;
            }
            else
            {
                CandidateCombo.SelectedItem = null;
            }
        }
        finally
        {
            _suppressCandidateChanged = false;
        }
    }

    private void ApplyLyricsUi(LyricsSearchResult item)
    {
        LyricsServiceText.Text = string.IsNullOrWhiteSpace(item.Service) ? "-" : item.Service;
        LyricsMatchText.Text = item.MatchPercentage.ToString(CultureInfo.InvariantCulture);

        Lines.Clear();
        foreach (var l in BuildMergedLines(item))
        {
            Lines.Add(l);
        }

        LyricsEmptyText.Visibility = Lines.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    private void ClearLyricsUi()
    {
        LyricsServiceText.Text = "-";
        LyricsMatchText.Text = "-";
        Lines.Clear();
        LyricsEmptyText.Visibility = Visibility.Visible;
    }

    private IEnumerable<LyricsLineVm> BuildMergedLines(LyricsSearchResult item)
    {
        var orig = item.LyricsOriginal ?? "";
        var tran = item.LyricsTranslation ?? "";

        var origTimed = ParseLrcTimedLines(orig);
        var tranTimed = ParseLrcTimedLines(tran);

        var hasAnyTimed = origTimed.Any(x => x.TimeMs is not null) || tranTimed.Any(x => x.TimeMs is not null);

        if (hasAnyTimed)
        {
            // Use timestamps when available; fallback to index-alignment if one side lacks timestamps.
            var origTimeCount = origTimed.Count(x => x.TimeMs is not null);
            var tranTimeCount = tranTimed.Count(x => x.TimeMs is not null);

            if (origTimeCount == 0)
            {
                // No timestamps on original: pair by index.
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
                // No timestamps on translation: pair by index to original time-sorted list.
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

        // No timestamps on either: pair by line index.
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
                // Keep the line for index-alignment fallback.
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
        if (x.StartsWith("[") && x.Contains(':') && x.EndsWith("]"))
        {
            // Likely a metadata line: [ar:xxx] / [ti:xxx] / ...
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
                // [mm:ss.xx] or [mm:ss.xxx]
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

    private static string BuildSongKey(NowPlayingSession np)
    {
        var appId = (np.AppId ?? "").Trim();
        var title = (np.Title ?? "").Trim();
        var artist = (np.Artist ?? "").Trim();
        var album = (np.AlbumTitle ?? "").Trim();
        var dur = np.DurationMs?.ToString(CultureInfo.InvariantCulture) ?? "0";
        return $"{appId}|{title}|{artist}|{album}|{dur}";
    }

    private static string[] NormalizeProviders(string[]? services)
    {
        var raw = services ?? Array.Empty<string>();
        var set = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var s in raw)
        {
            var x = (s ?? "").Trim();
            if (x.Length == 0)
            {
                continue;
            }
            set.Add(x);
        }

        if (set.Count == 0)
        {
            return new[] { "qq", "netease", "lrclib" };
        }

        return set.ToArray();
    }

    private void ApplyProviderChecksToFlyout(string[]? selected)
    {
        var set = new HashSet<string>(NormalizeProviders(selected), StringComparer.OrdinalIgnoreCase);

        if (ProvidersBtn.Flyout is not MenuFlyout mf)
        {
            return;
        }

        foreach (var item in mf.Items)
        {
            if (item is not ToggleMenuFlyoutItem t)
            {
                continue;
            }
            var tag = (t.Tag as string ?? "").Trim();
            if (tag.Length == 0)
            {
                continue;
            }
            t.IsChecked = set.Contains(tag);
        }
    }

    private string[] GetSelectedProvidersFromFlyout()
    {
        var list = new List<string>();
        if (ProvidersBtn.Flyout is not MenuFlyout mf)
        {
            return NormalizeProviders(null);
        }

        foreach (var item in mf.Items)
        {
            if (item is not ToggleMenuFlyoutItem t)
            {
                continue;
            }

            if (!t.IsChecked)
            {
                continue;
            }

            var tag = (t.Tag as string ?? "").Trim();
            if (tag.Length == 0)
            {
                continue;
            }

            list.Add(tag);
        }

        return NormalizeProviders(list.ToArray());
    }

    private void UpdateProvidersSummary()
    {
        var providers = SettingsService.Instance.Current.LyricsProviders;
        var list = NormalizeProviders(providers);
        ProvidersBtn.Content = string.Join(", ", list);
    }

    private void OnProviderToggleClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;

        if (_uiInit)
        {
            return;
        }

        try
        {
            var selected = GetSelectedProvidersFromFlyout();
            SettingsService.Instance.Update(s => s.LyricsProviders = selected);
            UpdateProvidersSummary();
        }
        catch
        {
            // ignore
        }
    }

    private void OnThresholdChanged(Muxc.NumberBox sender, Muxc.NumberBoxValueChangedEventArgs args)
    {
        _ = args;
        if (_uiInit)
        {
            return;
        }

        var v = (int)Math.Round(sender.Value);
        v = Math.Clamp(v, 0, 100);
        SettingsService.Instance.Update(s => s.LyricsThreshold = v);
    }

    private void OnLimitChanged(Muxc.NumberBox sender, Muxc.NumberBoxValueChangedEventArgs args)
    {
        _ = args;
        if (_uiInit)
        {
            return;
        }

        var v = (int)Math.Round(sender.Value);
        v = Math.Clamp(v, 1, 50);
        SettingsService.Instance.Update(s => s.LyricsLimit = v);
    }

    private void OnTimeoutChanged(Muxc.NumberBox sender, Muxc.NumberBoxValueChangedEventArgs args)
    {
        _ = args;
        if (_uiInit)
        {
            return;
        }

        var v = (int)Math.Round(sender.Value);
        v = Math.Clamp(v, 1, 60000);
        SettingsService.Instance.Update(s => s.LyricsTimeoutMs = v);
    }

    private void OnFillFromNowPlayingClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;

        var np = _lastNowPlaying;
        if (np is null)
        {
            return;
        }

        SearchTitleBox.Text = (np.Title ?? "").Trim();
        SearchArtistBox.Text = (np.Artist ?? "").Trim();
        SearchAlbumBox.Text = (np.AlbumTitle ?? "").Trim();
    }

    private async void OnManualSearchClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;

        try
        {
            await ManualSearchAsync(CancellationToken.None);
        }
        catch (Exception ex)
        {
            ShowBackendError(ex.Message);
        }
    }

    private async Task ManualSearchAsync(CancellationToken ct)
    {
        InitBackend();

        var backend = _backend ?? throw new InvalidOperationException("lyrics backend not initialized");

        var title = (SearchTitleBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(title))
        {
            ShowBackendError("title is empty");
            return;
        }

        var s = SettingsService.Instance.Current;
        var providers = NormalizeProviders(s.LyricsProviders);
        var limit = Math.Clamp(s.LyricsLimit, 1, 50);
        var timeoutMs = Math.Clamp(s.LyricsTimeoutMs, 1, 60000);

        var artist = (SearchArtistBox.Text ?? "").Trim();
        var album = (SearchAlbumBox.Text ?? "").Trim();
        var dur = _lastNowPlaying?.DurationMs;

        var p = new LyricsSearchParams
        {
            Title = title,
            Artist = string.IsNullOrWhiteSpace(artist) ? null : artist,
            Album = string.IsNullOrWhiteSpace(album) ? null : album,
            DurationMs = dur,
            Limit = (uint)limit,
            StrictMatch = false,
            Services = providers,
            TimeoutMs = (ulong)timeoutMs,
        };

        var items = await backend.SearchLyricsAsync(p, ct);
        var sorted = items
            .OrderByDescending(x => x.MatchPercentage)
            .ThenBy(x => (x.Service ?? "").Trim(), StringComparer.OrdinalIgnoreCase)
            .Take(50)
            .ToArray();

        UpdateCandidates(sorted, autoSelect: true, select: sorted.FirstOrDefault());

        var best = sorted.FirstOrDefault();
        if (best is not null)
        {
            _currentLyrics = best;
            ApplyLyricsUi(best);
        }
        else
        {
            _currentLyrics = null;
            ClearLyricsUi();
        }
    }

    private void OnCandidateChanged(object sender, SelectionChangedEventArgs e)
    {
        _ = sender;
        _ = e;

        if (_suppressCandidateChanged)
        {
            return;
        }

        if (CandidateCombo.SelectedItem is not LyricsCandidateVm vm)
        {
            return;
        }

        _currentLyrics = vm.Item;
        ApplyLyricsUi(vm.Item);
    }

    private void ShowBackendError(string msg)
    {
        BackendBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error;
        BackendBar.Title = _backend?.Name ?? "Lyrics";
        BackendBar.Message = msg;
        BackendBar.IsOpen = true;
    }

    private static string FormatMs(ulong? ms)
    {
        if (ms is null)
        {
            return "-";
        }

        var ts = TimeSpan.FromMilliseconds((double)ms.Value);
        if (ts.TotalHours >= 1)
        {
            return $"{(int)ts.TotalHours:D2}:{ts.Minutes:D2}:{ts.Seconds:D2}";
        }

        return $"{ts.Minutes:D2}:{ts.Seconds:D2}";
    }

    private Task RunOnUiAsync(Action action)
    {
        if (_dq.HasThreadAccess)
        {
            action();
            return Task.CompletedTask;
        }

        var tcs = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);
        var ok = _dq.TryEnqueue(() =>
        {
            try
            {
                action();
                tcs.TrySetResult(null);
            }
            catch (Exception ex)
            {
                tcs.TrySetException(ex);
            }
        });

        if (!ok)
        {
            tcs.TrySetException(new InvalidOperationException("failed to enqueue UI action"));
        }

        return tcs.Task;
    }

    private Task RunOnUiAsync(Func<Task> action)
    {
        if (_dq.HasThreadAccess)
        {
            return action();
        }

        var tcs = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);
        var ok = _dq.TryEnqueue(async () =>
        {
            try
            {
                await action();
                tcs.TrySetResult(null);
            }
            catch (Exception ex)
            {
                tcs.TrySetException(ex);
            }
        });

        if (!ok)
        {
            tcs.TrySetException(new InvalidOperationException("failed to enqueue UI action"));
        }

        return tcs.Task;
    }
}

public sealed class NowPlayingSessionVm
{
    public NowPlayingSessionVm(NowPlayingSession session, string display)
    {
        Session = session ?? throw new ArgumentNullException(nameof(session));
        Display = display ?? "";
    }

    public NowPlayingSession Session { get; }
    public string Display { get; }

    public static NowPlayingSessionVm From(NowPlayingSession s)
    {
        var app = string.IsNullOrWhiteSpace(s.AppId) ? "-" : s.AppId.Trim();
        var title = string.IsNullOrWhiteSpace(s.Title) ? "-" : s.Title!.Trim();
        var artist = string.IsNullOrWhiteSpace(s.Artist) ? "" : $" - {s.Artist!.Trim()}";
        var flag = s.IsCurrent ? " (current)" : "";
        return new NowPlayingSessionVm(s, $"{app}{flag}: {title}{artist}");
    }
}
