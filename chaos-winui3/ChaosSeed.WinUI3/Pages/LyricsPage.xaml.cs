using System.Globalization;
using System.Linq;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services.LyricsBackends;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class LyricsPage : Page
{
    private readonly DispatcherQueue _dq;

    private ILyricsBackend? _backend;
    private CancellationTokenSource? _watchCts;
    private Task? _watchTask;
    private CancellationTokenSource? _searchCts;
    private Task? _searchTask;

    private string? _lastSongKey;
    private LyricsSearchResult? _currentLyrics;

    public LyricsPage()
    {
        InitializeComponent();
        _dq = DispatcherQueue.GetForCurrentThread();

        Loaded += (_, _) => InitBackend();
        Unloaded += (_, _) => Shutdown();
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

    private void Shutdown()
    {
        StopWatch();

        try { _backend?.Dispose(); } catch { }
        _backend = null;
    }

    private async void OnRefreshNowPlayingClicked(object sender, RoutedEventArgs e)
    {
        try
        {
            await RefreshNowPlayingAsync(CancellationToken.None);
        }
        catch (Exception ex)
        {
            ShowBackendError(ex.Message);
        }
    }

    private async void OnAutoDetectToggled(object sender, RoutedEventArgs e)
    {
        try
        {
            if (AutoToggle.IsOn)
            {
                StartWatch();
            }
            else
            {
                StopWatch();
            }
        }
        catch (Exception ex)
        {
            ShowBackendError(ex.Message);
        }
    }

    private void StartWatch()
    {
        InitBackend();

        StopWatch();

        _watchCts = new CancellationTokenSource();
        var ct = _watchCts.Token;
        _watchTask = Task.Run(() => WatchLoopAsync(ct), ct);
    }

    private void StopWatch()
    {
        try { _watchCts?.Cancel(); } catch { }
        try { _watchCts?.Dispose(); } catch { }
        _watchCts = null;

        try { _searchCts?.Cancel(); } catch { }
        try { _searchCts?.Dispose(); } catch { }
        _searchCts = null;

        _watchTask = null;
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
                if (snap?.NowPlaying is not null)
                {
                    var key = BuildSongKey(snap.NowPlaying);
                    if (!string.IsNullOrWhiteSpace(key) && key != _lastSongKey)
                    {
                        _lastSongKey = key;
                        TriggerSequentialSearch(snap.NowPlaying, ct);
                    }
                }
            }
            catch
            {
                // ignore
            }

            var isPlaying = snap?.NowPlaying?.PlaybackStatus?.Equals("Playing", StringComparison.OrdinalIgnoreCase) == true;
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
        try { _searchCts?.Cancel(); } catch { }
        try { _searchCts?.Dispose(); } catch { }
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

        var providers = ParseProvidersCsv(await ReadUiAsync(() => ProvidersBox.Text));
        if (providers.Length == 0)
        {
            providers = new[] { "qq", "netease", "lrclib" };
        }

        var threshold = (int)Math.Round(await ReadUiAsync(() => ThresholdBox.Value));
        threshold = Math.Clamp(threshold, 0, 100);

        var limit = (int)Math.Round(await ReadUiAsync(() => LimitBox.Value));
        limit = Math.Clamp(limit, 1, 50);

        var timeoutMs = (int)Math.Round(await ReadUiAsync(() => TimeoutBox.Value));
        timeoutMs = Math.Clamp(timeoutMs, 1, 60000);

        var title = (np.Title ?? "").Trim();
        if (string.IsNullOrWhiteSpace(title))
        {
            await RunOnUiAsync(ClearLyricsUi);
            return;
        }

        var artist = (np.Artist ?? "").Trim();
        var album = (np.AlbumTitle ?? "").Trim();

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

            var best = items
                .OrderByDescending(x => x.MatchPercentage)
                .FirstOrDefault();
            if (best is null)
            {
                continue;
            }

            if (best.MatchPercentage >= threshold)
            {
                _currentLyrics = best;
                await RunOnUiAsync(() => ApplyLyricsUi(best));
                return;
            }
        }

        _currentLyrics = null;
        await RunOnUiAsync(ClearLyricsUi);
    }

    private async Task<NowPlayingSnapshot> RefreshNowPlayingAsync(CancellationToken ct)
    {
        InitBackend();

        var backend = _backend ?? throw new InvalidOperationException("lyrics backend not initialized");

        var snap = await backend.SnapshotNowPlayingAsync(
            includeThumbnail: false,
            maxThumbBytes: 1,
            maxSessions: 32,
            ct
        );

        await RunOnUiAsync(() => ApplyNowPlayingUi(snap));
        return snap;
    }

    private void ApplyNowPlayingUi(NowPlayingSnapshot snap)
    {
        SupportedText.Text = snap.Supported ? "true" : "false";

        var np = snap.NowPlaying;
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
    }

    private void ApplyLyricsUi(LyricsSearchResult item)
    {
        LyricsServiceText.Text = string.IsNullOrWhiteSpace(item.Service) ? "-" : item.Service;
        LyricsMatchText.Text = item.MatchPercentage.ToString(CultureInfo.InvariantCulture);
        LyricsOriginalText.Text = item.LyricsOriginal ?? "";
        LyricsTranslationText.Text = item.LyricsTranslation ?? "无翻译";
    }

    private void ClearLyricsUi()
    {
        LyricsServiceText.Text = "-";
        LyricsMatchText.Text = "-";
        LyricsOriginalText.Text = "";
        LyricsTranslationText.Text = "无翻译";
    }

    private void ShowBackendError(string msg)
    {
        BackendBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error;
        BackendBar.Title = _backend?.Name ?? "Lyrics";
        BackendBar.Message = msg;
        BackendBar.IsOpen = true;
    }

    private string BuildSongKey(NowPlayingSession np)
    {
        var appId = (np.AppId ?? "").Trim();
        var title = (np.Title ?? "").Trim();
        var artist = (np.Artist ?? "").Trim();
        var album = (np.AlbumTitle ?? "").Trim();
        var dur = np.DurationMs?.ToString(CultureInfo.InvariantCulture) ?? "0";
        return $"{appId}|{title}|{artist}|{album}|{dur}";
    }

    private static string[] ParseProvidersCsv(string? csv)
    {
        var raw = (csv ?? "").Split(',', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries);
        return raw
            .Select(s => s.Trim())
            .Where(s => !string.IsNullOrWhiteSpace(s))
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();
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

    private Task<T> ReadUiAsync<T>(Func<T> action)
    {
        if (_dq.HasThreadAccess)
        {
            return Task.FromResult(action());
        }

        var tcs = new TaskCompletionSource<T>(TaskCreationOptions.RunContinuationsAsynchronously);
        var ok = _dq.TryEnqueue(() =>
        {
            try
            {
                tcs.TrySetResult(action());
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
