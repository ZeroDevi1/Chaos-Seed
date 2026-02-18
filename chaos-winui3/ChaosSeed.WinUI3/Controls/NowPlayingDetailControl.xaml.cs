using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Globalization;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using ChaosSeed.WinUI3.Services.Lyrics;
using ChaosSeed.WinUI3.Services.LyricsBackends;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Windows.UI.Text;

namespace ChaosSeed.WinUI3.Controls;

public sealed partial class NowPlayingDetailControl : UserControl
{
    private readonly DispatcherQueue _dq = DispatcherQueue.GetForCurrentThread();
    private DispatcherTimer? _timer;
    private bool _updatingPos;
    private bool _isOpen;

    private ILyricsBackend? _lyricsBackend;
    private CancellationTokenSource? _lyricsCts;
    private string? _lastLyricsKey;
    private int _lastActiveLineIndex = -2;

    public ObservableCollection<NowPlayingLyricsLineVm> LyricsLines { get; } = new();

    public event EventHandler? CloseRequested;

    public NowPlayingDetailControl()
    {
        InitializeComponent();

        Loaded += (_, _) =>
        {
            MusicPlayerService.Instance.Changed += OnPlayerChanged;
            EnsureTimer();
            UpdateUi();
        };
        Unloaded += (_, _) =>
        {
            MusicPlayerService.Instance.Changed -= OnPlayerChanged;
            CancelLyricsSearch();
            try { _lyricsBackend?.Dispose(); } catch { }
            _lyricsBackend = null;
        };
    }

    public void Open()
    {
        _isOpen = true;
        UpdateUi();
        _ = EnsureLyricsAsync(CancellationToken.None);
    }

    private void EnsureTimer()
    {
        if (_timer is not null)
        {
            return;
        }

        _timer = new DispatcherTimer
        {
            Interval = TimeSpan.FromMilliseconds(250),
        };
        _timer.Tick += (_, _) =>
        {
            UpdateTimeline();
            UpdateActiveLyricLine();
        };
        _timer.Start();
    }

    private void OnPlayerChanged(object? sender, EventArgs e)
    {
        _ = sender;
        _ = e;
        _dq.TryEnqueue(() =>
        {
            UpdateUi();
            if (_isOpen)
            {
                _ = EnsureLyricsAsync(CancellationToken.None);
            }
        });
    }

    private void UpdateUi()
    {
        var svc = MusicPlayerService.Instance;
        PlayPauseIcon.Symbol = svc.IsPlaying ? Symbol.Pause : Symbol.Play;

        var t = svc.Track;
        TitleText.Text = t?.Title ?? "-";

        var artist = t?.Artists is null ? "" : string.Join(" / ", t.Artists.Where(s => !string.IsNullOrWhiteSpace(s)));
        ArtistText.Text = artist;
        AlbumText.Text = (t?.Album ?? "").Trim();

        HeaderSubText.Text = string.IsNullOrWhiteSpace(AlbumText.Text) ? artist : $"{artist} · {AlbumText.Text}";

        try
        {
            var u = t?.CoverUrl;
            CoverImg.Source = string.IsNullOrWhiteSpace(u) ? null : MusicUiUtil.TryCreateBitmap(u);
        }
        catch
        {
            CoverImg.Source = null;
        }

        UpdateLoopText();
        UpdateTimeline(force: true);
    }

    private void UpdateLoopText()
    {
        var svc = MusicPlayerService.Instance;
        LoopText.Text = svc.LoopMode switch
        {
            MusicLoopMode.All => "∞",
            MusicLoopMode.Off => "—",
            _ => "1",
        };
    }

    private void UpdateTimeline(bool force = false)
    {
        var svc = MusicPlayerService.Instance;
        if (!svc.IsOpen)
        {
            if (force)
            {
                PosText.Text = "00:00";
                DurText.Text = "--:--";
            }
            return;
        }

        var (pos, dur) = svc.GetTimeline();
        var durSeconds = dur.TotalSeconds;
        if (durSeconds <= 0.5)
        {
            durSeconds = 1.0;
        }

        _updatingPos = true;
        try
        {
            PosSlider.Maximum = durSeconds;
            var p = pos.TotalSeconds;
            if (p < 0) p = 0;
            if (p > durSeconds) p = durSeconds;
            PosSlider.Value = p;
        }
        finally
        {
            _updatingPos = false;
        }

        PosText.Text = FormatTime(pos);
        DurText.Text = dur.TotalSeconds <= 0.5 ? "--:--" : FormatTime(dur);
    }

    private static string FormatTime(TimeSpan t)
    {
        if (t.TotalHours >= 1)
        {
            return $"{(int)t.TotalHours:00}:{t.Minutes:00}:{t.Seconds:00}";
        }
        return $"{(int)t.TotalMinutes:00}:{t.Seconds:00}";
    }

    private void UpdateActiveLyricLine()
    {
        if (LyricsLines.Count == 0)
        {
            return;
        }

        var (pos, _) = MusicPlayerService.Instance.GetTimeline();
        var posMs = (ulong)Math.Max(0, pos.TotalMilliseconds);

        var best = -1;
        for (var i = 0; i < LyricsLines.Count; i++)
        {
            var tm = LyricsLines[i].TimeMs;
            if (tm is null)
            {
                continue;
            }
            if (tm.Value <= posMs)
            {
                best = i;
            }
        }

        for (var i = 0; i < LyricsLines.Count; i++)
        {
            LyricsLines[i].IsActive = i == best;
        }

        if (best >= 0 && best < LyricsLines.Count && best != _lastActiveLineIndex)
        {
            _lastActiveLineIndex = best;
            try { LyricsList.ScrollIntoView(LyricsLines[best]); } catch { }
        }
    }

    private async Task EnsureLyricsAsync(CancellationToken ct)
    {
        var track = MusicPlayerService.Instance.Track;
        if (track is null)
        {
            LyricsStatusText.Text = "";
            LyricsLines.Clear();
            _lastLyricsKey = null;
            return;
        }

        var key = MusicQueueItem.BuildKey(track);
        if (!string.IsNullOrWhiteSpace(_lastLyricsKey) && string.Equals(_lastLyricsKey, key, StringComparison.Ordinal))
        {
            return;
        }

        _lastLyricsKey = key;
        CancelLyricsSearch();
        _lyricsCts = new CancellationTokenSource();
        var linked = CancellationTokenSource.CreateLinkedTokenSource(ct, _lyricsCts.Token);
        var lct = linked.Token;

        LyricsStatusText.Text = "搜索中...";
        LyricsLines.Clear();
        _lastActiveLineIndex = -2;

        EnsureLyricsBackend();
        var backend = _lyricsBackend;
        if (backend is null)
        {
            LyricsStatusText.Text = "歌词后端不可用";
            return;
        }

        LyricsSearchResult? best = null;
        try
        {
            var s = SettingsService.Instance.Current;
            var providers = NormalizeProviders(s.LyricsProviders);
            var threshold = Math.Clamp(s.LyricsThreshold, 0, 100);
            var limit = Math.Clamp(s.LyricsLimit, 1, 50);
            var timeoutMs = Math.Clamp(s.LyricsTimeoutMs, 1, 60000);

            var title = (track.Title ?? "").Trim();
            var artist = (track.Artists is null ? "" : string.Join(" / ", track.Artists.Where(x => !string.IsNullOrWhiteSpace(x)))).Trim();
            var album = (track.Album ?? "").Trim();

            foreach (var provider in providers)
            {
                lct.ThrowIfCancellationRequested();

                LyricsSearchResult[] items;
                try
                {
                    items = await backend.SearchLyricsAsync(
                        new LyricsSearchParams
                        {
                            Title = title,
                            Artist = string.IsNullOrWhiteSpace(artist) ? null : artist,
                            Album = string.IsNullOrWhiteSpace(album) ? null : album,
                            DurationMs = track.DurationMs,
                            Limit = (uint)limit,
                            StrictMatch = false,
                            Services = new[] { provider },
                            TimeoutMs = (ulong)timeoutMs,
                        },
                        lct
                    );
                }
                catch
                {
                    continue;
                }

                if (items.Length == 0)
                {
                    continue;
                }

                var cand = items.OrderByDescending(x => x.MatchPercentage).FirstOrDefault();
                if (cand is null)
                {
                    continue;
                }

                if (best is null || cand.MatchPercentage > best.MatchPercentage)
                {
                    best = cand;
                }

                if (cand.MatchPercentage >= threshold)
                {
                    best = cand;
                    break;
                }
            }
        }
        catch (OperationCanceledException)
        {
            return;
        }
        catch
        {
            // ignore
        }

        if (best is null || string.IsNullOrWhiteSpace(best.LyricsOriginal))
        {
            LyricsStatusText.Text = "未找到歌词";
            LyricsLines.Clear();
            return;
        }

        LyricsStatusText.Text = $"{best.Service} · {best.MatchPercentage.ToString(CultureInfo.InvariantCulture)}";

        foreach (var l in LyricsParser.ParseMergedLines(best.LyricsOriginal ?? "", best.LyricsTranslation))
        {
            LyricsLines.Add(NowPlayingLyricsLineVm.From(l));
        }
    }

    private void EnsureLyricsBackend()
    {
        if (_lyricsBackend is not null)
        {
            return;
        }

        try
        {
            _lyricsBackend = LyricsBackendFactory.Create();
        }
        catch
        {
            _lyricsBackend = null;
        }
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

    private void CancelLyricsSearch()
    {
        try { _lyricsCts?.Cancel(); } catch { }
        try { _lyricsCts?.Dispose(); } catch { }
        _lyricsCts = null;
    }

    private void OnCloseClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _isOpen = false;
        CancelLyricsSearch();
        CloseRequested?.Invoke(this, EventArgs.Empty);
    }

    private void OnPrevClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _ = MusicPlayerService.Instance.PrevAsync(CancellationToken.None);
    }

    private void OnNextClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _ = MusicPlayerService.Instance.NextAsync(CancellationToken.None);
    }

    private void OnPlayPauseClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        MusicPlayerService.Instance.TogglePlayPause();
    }

    private void OnLoopClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        var svc = MusicPlayerService.Instance;
        svc.LoopMode = svc.LoopMode switch
        {
            MusicLoopMode.Single => MusicLoopMode.All,
            MusicLoopMode.All => MusicLoopMode.Off,
            _ => MusicLoopMode.Single,
        };
        UpdateLoopText();
    }

    private void OnPositionChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        _ = sender;
        if (_updatingPos)
        {
            return;
        }

        try
        {
            MusicPlayerService.Instance.SeekToSeconds(e.NewValue);
        }
        catch
        {
            // ignore
        }
    }
}

public sealed class NowPlayingLyricsLineVm : INotifyPropertyChanged
{
    private NowPlayingLyricsLineVm(ulong? timeMs, string original, string? translation)
    {
        TimeMs = timeMs;
        Original = original ?? "";
        Translation = translation;
    }

    public static NowPlayingLyricsLineVm From(LyricsLineVm vm)
        => new(vm.TimeMs, vm.Original, vm.Translation);

    public ulong? TimeMs { get; }
    public string Original { get; }
    public string? Translation { get; }

    public string TranslationText => string.IsNullOrWhiteSpace(Translation) ? "" : Translation!;
    public Visibility TranslationVisibility =>
        string.IsNullOrWhiteSpace(Translation) ? Visibility.Collapsed : Visibility.Visible;

    private bool _isActive;
    public bool IsActive
    {
        get => _isActive;
        set
        {
            if (_isActive == value)
            {
                return;
            }
            _isActive = value;
            OnPropertyChanged();
            OnPropertyChanged(nameof(Opacity));
            OnPropertyChanged(nameof(TranslationOpacity));
            OnPropertyChanged(nameof(FontWeight));
        }
    }

    public double Opacity => IsActive ? 1.0 : 0.55;
    public double TranslationOpacity => IsActive ? 0.85 : 0.45;
    public FontWeight FontWeight => IsActive
        ? new FontWeight { Weight = 600 }
        : new FontWeight { Weight = 400 };

    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
