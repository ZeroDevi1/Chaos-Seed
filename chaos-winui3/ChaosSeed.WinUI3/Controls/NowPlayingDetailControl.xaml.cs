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
using Microsoft.UI.Xaml.Input;
using Windows.Foundation;
using Windows.UI.Text;

namespace ChaosSeed.WinUI3.Controls;

public sealed partial class NowPlayingDetailControl : UserControl
{
    private readonly DispatcherQueue _dq = DispatcherQueue.GetForCurrentThread();
    private DispatcherTimer? _timer;
    private bool _updatingPos;
    private bool _isOpen;
    private bool _seeking;
    private bool _seekCommitPending;
    private bool _seekHandlersAttached;
    private XamlRoot? _trackedXamlRoot;

    private ILyricsBackend? _lyricsBackend;
    private CancellationTokenSource? _lyricsCts;
    private string? _lastLyricsKey;
    private int _lastActiveLineIndex = -2;

    public ObservableCollection<NowPlayingLyricsLineVm> LyricsLines { get; } = new();

    public event EventHandler? CloseRequested;

    public double CardWidth
    {
        get => (double)GetValue(CardWidthProperty);
        set => SetValue(CardWidthProperty, value);
    }
    public static readonly DependencyProperty CardWidthProperty =
        DependencyProperty.Register(nameof(CardWidth), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(980.0));

    public double CardHeight
    {
        get => (double)GetValue(CardHeightProperty);
        set => SetValue(CardHeightProperty, value);
    }
    public static readonly DependencyProperty CardHeightProperty =
        DependencyProperty.Register(nameof(CardHeight), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(640.0));

    public Thickness CardPadding
    {
        get => (Thickness)GetValue(CardPaddingProperty);
        set => SetValue(CardPaddingProperty, value);
    }
    public static readonly DependencyProperty CardPaddingProperty =
        DependencyProperty.Register(nameof(CardPadding), typeof(Thickness), typeof(NowPlayingDetailControl), new PropertyMetadata(new Thickness(18)));

    public double LayoutRowSpacing
    {
        get => (double)GetValue(LayoutRowSpacingProperty);
        set => SetValue(LayoutRowSpacingProperty, value);
    }
    public static readonly DependencyProperty LayoutRowSpacingProperty =
        DependencyProperty.Register(nameof(LayoutRowSpacing), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(14.0));

    public double LayoutColumnSpacing
    {
        get => (double)GetValue(LayoutColumnSpacingProperty);
        set => SetValue(LayoutColumnSpacingProperty, value);
    }
    public static readonly DependencyProperty LayoutColumnSpacingProperty =
        DependencyProperty.Register(nameof(LayoutColumnSpacing), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(18.0));

    public double ContentColumnSpacing
    {
        get => (double)GetValue(ContentColumnSpacingProperty);
        set => SetValue(ContentColumnSpacingProperty, value);
    }
    public static readonly DependencyProperty ContentColumnSpacingProperty =
        DependencyProperty.Register(nameof(ContentColumnSpacing), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(24.0));

    public double ContentRowSpacing
    {
        get => (double)GetValue(ContentRowSpacingProperty);
        set => SetValue(ContentRowSpacingProperty, value);
    }
    public static readonly DependencyProperty ContentRowSpacingProperty =
        DependencyProperty.Register(nameof(ContentRowSpacing), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(10.0));

    public double CoverSize
    {
        get => (double)GetValue(CoverSizeProperty);
        set => SetValue(CoverSizeProperty, value);
    }
    public static readonly DependencyProperty CoverSizeProperty =
        DependencyProperty.Register(nameof(CoverSize), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(300.0));

    public CornerRadius CoverCornerRadius
    {
        get => (CornerRadius)GetValue(CoverCornerRadiusProperty);
        set => SetValue(CoverCornerRadiusProperty, value);
    }
    public static readonly DependencyProperty CoverCornerRadiusProperty =
        DependencyProperty.Register(nameof(CoverCornerRadius), typeof(CornerRadius), typeof(NowPlayingDetailControl), new PropertyMetadata(new CornerRadius(16)));

    public double TitleFontSize
    {
        get => (double)GetValue(TitleFontSizeProperty);
        set => SetValue(TitleFontSizeProperty, value);
    }
    public static readonly DependencyProperty TitleFontSizeProperty =
        DependencyProperty.Register(nameof(TitleFontSize), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(30.0));

    public double MetaFontSize
    {
        get => (double)GetValue(MetaFontSizeProperty);
        set => SetValue(MetaFontSizeProperty, value);
    }
    public static readonly DependencyProperty MetaFontSizeProperty =
        DependencyProperty.Register(nameof(MetaFontSize), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(14.0));

    public double HeaderFontSize
    {
        get => (double)GetValue(HeaderFontSizeProperty);
        set => SetValue(HeaderFontSizeProperty, value);
    }
    public static readonly DependencyProperty HeaderFontSizeProperty =
        DependencyProperty.Register(nameof(HeaderFontSize), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(16.0));

    public double HeaderSubFontSize
    {
        get => (double)GetValue(HeaderSubFontSizeProperty);
        set => SetValue(HeaderSubFontSizeProperty, value);
    }
    public static readonly DependencyProperty HeaderSubFontSizeProperty =
        DependencyProperty.Register(nameof(HeaderSubFontSize), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(12.0));

    public double TitleSpacing
    {
        get => (double)GetValue(TitleSpacingProperty);
        set => SetValue(TitleSpacingProperty, value);
    }
    public static readonly DependencyProperty TitleSpacingProperty =
        DependencyProperty.Register(nameof(TitleSpacing), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(4.0));

    public double TitleMaxWidth
    {
        get => (double)GetValue(TitleMaxWidthProperty);
        set => SetValue(TitleMaxWidthProperty, value);
    }
    public static readonly DependencyProperty TitleMaxWidthProperty =
        DependencyProperty.Register(nameof(TitleMaxWidth), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(680.0));

    public Thickness LyricsPadding
    {
        get => (Thickness)GetValue(LyricsPaddingProperty);
        set => SetValue(LyricsPaddingProperty, value);
    }
    public static readonly DependencyProperty LyricsPaddingProperty =
        DependencyProperty.Register(nameof(LyricsPadding), typeof(Thickness), typeof(NowPlayingDetailControl), new PropertyMetadata(new Thickness(12)));

    public double LyricsLineSpacing
    {
        get => (double)GetValue(LyricsLineSpacingProperty);
        set => SetValue(LyricsLineSpacingProperty, value);
    }
    public static readonly DependencyProperty LyricsLineSpacingProperty =
        DependencyProperty.Register(nameof(LyricsLineSpacing), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(4.0));

    public double LyricFontSize
    {
        get => (double)GetValue(LyricFontSizeProperty);
        set => SetValue(LyricFontSizeProperty, value);
    }
    public static readonly DependencyProperty LyricFontSizeProperty =
        DependencyProperty.Register(nameof(LyricFontSize), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(18.0));

    public double LyricTranslationFontSize
    {
        get => (double)GetValue(LyricTranslationFontSizeProperty);
        set => SetValue(LyricTranslationFontSizeProperty, value);
    }
    public static readonly DependencyProperty LyricTranslationFontSizeProperty =
        DependencyProperty.Register(nameof(LyricTranslationFontSize), typeof(double), typeof(NowPlayingDetailControl), new PropertyMetadata(14.0));

    public NowPlayingDetailControl()
    {
        InitializeComponent();

        Loaded += (_, _) =>
        {
            HookXamlRoot();
            UpdateResponsiveLayout();
            MusicPlayerService.Instance.Changed += OnPlayerChanged;
            AttachSeekHandlers();
            EnsureTimer();
            UpdateUi();
        };
        Unloaded += (_, _) =>
        {
            UnhookXamlRoot();
            MusicPlayerService.Instance.Changed -= OnPlayerChanged;
            CancelLyricsSearch();
            try { _lyricsBackend?.Dispose(); } catch { }
            _lyricsBackend = null;
        };
    }

    private void HookXamlRoot()
    {
        var xr = XamlRoot;
        if (ReferenceEquals(_trackedXamlRoot, xr))
        {
            return;
        }

        UnhookXamlRoot();
        _trackedXamlRoot = xr;
        if (_trackedXamlRoot is not null)
        {
            _trackedXamlRoot.Changed += OnXamlRootChanged;
        }
    }

    private void UnhookXamlRoot()
    {
        if (_trackedXamlRoot is null)
        {
            return;
        }
        try { _trackedXamlRoot.Changed -= OnXamlRootChanged; } catch { }
        _trackedXamlRoot = null;
    }

    private void OnXamlRootChanged(XamlRoot sender, XamlRootChangedEventArgs args)
    {
        _ = sender;
        _ = args;
        _dq.TryEnqueue(UpdateResponsiveLayout);
    }

    private void UpdateResponsiveLayout()
    {
        var s = _trackedXamlRoot?.Size ?? new Size(1200, 760);
        var w = Math.Max(1.0, s.Width);
        var h = Math.Max(1.0, s.Height);

        var cardW = Math.Min(1320.0, Math.Max(1.0, w - 48.0));
        var cardH = Math.Min(900.0, Math.Max(1.0, h - 48.0));

        CardWidth = cardW;
        CardHeight = cardH;

        var scale = Math.Min(cardW / 1200.0, cardH / 760.0);
        scale = Math.Clamp(scale, 0.55, 1.28);

        var pad = Math.Clamp(18.0 * scale, 12.0, 26.0);
        CardPadding = new Thickness(pad);

        LayoutRowSpacing = Math.Clamp(14.0 * scale, 10.0, 22.0);
        LayoutColumnSpacing = Math.Clamp(18.0 * scale, 12.0, 26.0);
        ContentColumnSpacing = Math.Clamp(26.0 * scale, 12.0, 34.0);
        ContentRowSpacing = Math.Clamp(12.0 * scale, 8.0, 18.0);

        var coverFromScale = 320.0 * scale;
        var coverMax = Math.Min(cardW - (pad * 2), cardH - (pad * 2) - 160.0);
        if (double.IsNaN(coverMax) || double.IsInfinity(coverMax))
        {
            coverMax = 440.0;
        }
        coverMax = Math.Max(140.0, coverMax);
        CoverSize = Math.Clamp(coverFromScale, 140.0, Math.Min(440.0, coverMax));
        var cr = Math.Clamp(18.0 * scale, 14.0, 26.0);
        CoverCornerRadius = new CornerRadius(cr);

        HeaderFontSize = Math.Clamp(16.0 * scale, 12.0, 20.0);
        HeaderSubFontSize = Math.Clamp(12.0 * scale, 11.0, 16.0);

        TitleFontSize = Math.Clamp(34.0 * scale, 20.0, 46.0);
        MetaFontSize = Math.Clamp(14.0 * scale, 12.0, 18.0);
        TitleSpacing = Math.Clamp(6.0 * scale, 2.0, 10.0);

        var titleMax = cardW - CoverSize - (pad * 2) - ContentColumnSpacing - 24.0;
        if (titleMax < 1)
        {
            titleMax = cardW - (pad * 2);
        }
        TitleMaxWidth = Math.Clamp(titleMax, 180.0, 980.0);

        LyricFontSize = Math.Clamp(22.0 * scale, 14.0, 34.0);
        LyricTranslationFontSize = Math.Clamp(16.0 * scale, 12.0, 26.0);
        LyricsLineSpacing = Math.Clamp(6.0 * scale, 2.0, 12.0);

        var lyricPadX = Math.Clamp(14.0 * scale, 8.0, 24.0);
        var lyricPadY = Math.Clamp(18.0 * scale, 10.0, 30.0);
        LyricsPadding = new Thickness(lyricPadX, lyricPadY, lyricPadX, lyricPadY);
    }

    public void Open()
    {
        _isOpen = true;
        HookXamlRoot();
        UpdateResponsiveLayout();
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

        ArtistText.Visibility = string.IsNullOrWhiteSpace(ArtistText.Text) ? Visibility.Collapsed : Visibility.Visible;
        AlbumText.Visibility = string.IsNullOrWhiteSpace(AlbumText.Text) ? Visibility.Collapsed : Visibility.Visible;

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

    private void SetLyricsStatus(string? text)
    {
        LyricsStatusText.Text = (text ?? "").Trim();
        LyricsStatusText.Visibility = string.IsNullOrWhiteSpace(LyricsStatusText.Text) ? Visibility.Collapsed : Visibility.Visible;
    }

    private void AttachSeekHandlers()
    {
        if (_seekHandlersAttached)
        {
            return;
        }

        try
        {
            PosSlider.AddHandler(PointerPressedEvent, new PointerEventHandler(OnPosPointerPressed), true);
            PosSlider.AddHandler(PointerReleasedEvent, new PointerEventHandler(OnPosPointerReleased), true);
            PosSlider.AddHandler(PointerCaptureLostEvent, new PointerEventHandler(OnPosPointerCaptureLost), true);
            PosSlider.AddHandler(PointerCanceledEvent, new PointerEventHandler(OnPosPointerCanceled), true);
            _seekHandlersAttached = true;
        }
        catch
        {
            // ignore
        }
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
            if (!_seeking)
            {
                var p = pos.TotalSeconds;
                if (p < 0) p = 0;
                if (p > durSeconds) p = durSeconds;
                PosSlider.Value = p;
            }
        }
        finally
        {
            _updatingPos = false;
        }

        PosText.Text = _seeking ? FormatTime(TimeSpan.FromSeconds(PosSlider.Value)) : FormatTime(pos);
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
            SetLyricsStatus("");
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

        SetLyricsStatus("搜索中...");
        LyricsLines.Clear();
        _lastActiveLineIndex = -2;

        EnsureLyricsBackend();
        var backend = _lyricsBackend;
        if (backend is null)
        {
            SetLyricsStatus("歌词后端不可用");
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
            SetLyricsStatus("未找到歌词");
            LyricsLines.Clear();
            return;
        }

        SetLyricsStatus($"{best.Service} · {best.MatchPercentage.ToString(CultureInfo.InvariantCulture)}");

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

    private void OnPosPointerPressed(object sender, PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _seeking = true;
        _seekCommitPending = true;
        TrySeekFromPointer(e);
    }

    private void OnPosPointerReleased(object sender, PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        CommitSeek();
    }

    private void OnPosPointerCaptureLost(object sender, PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        CommitSeek();
    }

    private void OnPosPointerCanceled(object sender, PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        CommitSeek();
    }

    private void TrySeekFromPointer(PointerRoutedEventArgs e)
    {
        try
        {
            var pt = e.GetCurrentPoint(PosSlider);
            if (!pt.Properties.IsLeftButtonPressed)
            {
                return;
            }

            var w = PosSlider.ActualWidth;
            var range = PosSlider.Maximum - PosSlider.Minimum;
            if (w <= 1 || range <= 0.0001)
            {
                return;
            }

            var x = pt.Position.X;
            if (x < 0) x = 0;
            if (x > w) x = w;

            var v = PosSlider.Minimum + (x / w) * range;
            _updatingPos = true;
            try
            {
                PosSlider.Value = v;
            }
            finally
            {
                _updatingPos = false;
            }
        }
        catch
        {
            // ignore
        }
    }

    private void CommitSeek()
    {
        if (!_seekCommitPending)
        {
            _seeking = false;
            return;
        }

        _seekCommitPending = false;
        _seeking = false;

        try
        {
            MusicPlayerService.Instance.SeekToSeconds(PosSlider.Value);
        }
        catch
        {
            // ignore
        }
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
            if (_seeking)
            {
                PosText.Text = FormatTime(TimeSpan.FromSeconds(e.NewValue));
            }
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
