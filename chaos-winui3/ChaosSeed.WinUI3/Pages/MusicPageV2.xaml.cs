using System;
using System.Linq;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class MusicPageV2 : Page
{
    private readonly DispatcherQueue _dq = DispatcherQueue.GetForCurrentThread();
    private DispatcherTimer? _timer;
    private bool _updatingPos;
    private bool _seeking;
    private bool _updatingSelection;

    public MusicPageV2()
    {
        InitializeComponent();

        Loaded += (_, _) =>
        {
            MusicPlayerService.Instance.Changed += OnPlayerChanged;
            MusicPlayerService.Instance.PlaylistChanged += OnPlaylistChanged;
            EnsureTimer();
            UpdateUi();
        };
        Unloaded += (_, _) =>
        {
            MusicPlayerService.Instance.Changed -= OnPlayerChanged;
            MusicPlayerService.Instance.PlaylistChanged -= OnPlaylistChanged;
            StopTimer();
        };
    }

    private void EnsureTimer()
    {
        if (_timer is not null)
        {
            return;
        }
        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(250) };
        _timer.Tick += (_, _) => UpdateTimeline();
        _timer.Start();
    }

    private void StopTimer()
    {
        if (_timer is null)
        {
            return;
        }
        try { _timer.Stop(); } catch { }
        _timer = null;
    }

    private void OnPlayerChanged(object? sender, EventArgs e)
    {
        _ = sender;
        _ = e;
        _dq.TryEnqueue(UpdateUi);
    }

    private void OnPlaylistChanged(object? sender, EventArgs e)
    {
        _ = sender;
        _ = e;
        _dq.TryEnqueue(UpdatePlaylist);
    }

    private void UpdateUi()
    {
        try
        {
            var svc = MusicPlayerService.Instance;
            var t = svc.Track;
            TitleText.Text = t?.Title ?? "-";

            var artist = t?.Artists is null ? "" : string.Join(" / ", t.Artists.Where(s => !string.IsNullOrWhiteSpace(s)));
            var album = (t?.Album ?? "").Trim();
            MetaText.Text = string.IsNullOrWhiteSpace(album) ? artist : $"{artist} · {album}";

            try
            {
                var u = t?.CoverUrl;
                CoverImg.Source = string.IsNullOrWhiteSpace(u) ? null : MusicUiUtil.TryCreateBitmap(u);
            }
            catch
            {
                CoverImg.Source = null;
            }

            PlayPauseBtn.Content = svc.IsPlaying ? "Pause" : "Play";
            UpdatePlaylist();
            UpdateTimeline(force: true);
        }
        catch (Exception ex)
        {
            AppLog.Exception("MusicPageV2.UpdateUi", ex);
        }
    }

    private void UpdatePlaylist()
    {
        try
        {
            var svc = MusicPlayerService.Instance;
            PlaylistList.ItemsSource = svc.Playlist;

            _updatingSelection = true;
            try
            {
                PlaylistList.SelectedIndex = svc.CurrentIndex;
            }
            finally
            {
                _updatingSelection = false;
            }
        }
        catch (Exception ex)
        {
            AppLog.Exception("MusicPageV2.UpdatePlaylist", ex);
        }
    }

    private static string Fmt(TimeSpan t)
    {
        var total = (int)Math.Max(0, Math.Floor(t.TotalSeconds));
        var mm = total / 60;
        var ss = total % 60;
        return $"{mm:00}:{ss:00}";
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

        if (_seeking)
        {
            return;
        }

        var (pos, dur) = svc.GetTimeline();
        if (dur.TotalSeconds <= 0.1)
        {
            dur = TimeSpan.FromSeconds(1);
        }

        _updatingPos = true;
        try
        {
            PosSlider.Maximum = dur.TotalSeconds;
            PosSlider.Value = Math.Clamp(pos.TotalSeconds, 0, dur.TotalSeconds);
            PosText.Text = Fmt(pos);
            DurText.Text = Fmt(dur);
        }
        finally
        {
            _updatingPos = false;
        }
    }

    private void OnPrevClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _ = MusicPlayerService.Instance.PrevAsync(System.Threading.CancellationToken.None);
    }

    private void OnNextClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _ = MusicPlayerService.Instance.NextAsync(System.Threading.CancellationToken.None);
    }

    private void OnPlayPauseClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        var svc = MusicPlayerService.Instance;
        if (!svc.IsOpen)
        {
            return;
        }
        if (svc.IsPlaying)
        {
            svc.Pause();
        }
        else
        {
            svc.Resume();
        }
    }

    private void OnStopClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        MusicPlayerService.Instance.StopKeepOpen();
    }

    private void OnPlaylistSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        _ = e;
        if (_updatingSelection)
        {
            return;
        }

        if (sender is not ListView lv)
        {
            return;
        }

        var idx = lv.SelectedIndex;
        if (idx < 0)
        {
            return;
        }
        _ = MusicPlayerService.Instance.PlayAtAsync(idx, System.Threading.CancellationToken.None);
    }

    private void OnPosPointerPressed(object sender, PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _seeking = true;
    }

    private void OnPosPointerReleased(object sender, PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _seeking = false;
        try
        {
            MusicPlayerService.Instance.SeekToSeconds(PosSlider.Value);
        }
        catch (Exception ex)
        {
            AppLog.Exception("MusicPageV2.SeekToSeconds", ex);
        }
    }

    private void OnPosSliderValueChanged(object sender, Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs e)
    {
        _ = sender;
        if (_updatingPos)
        {
            return;
        }
        if (_seeking)
        {
            PosText.Text = Fmt(TimeSpan.FromSeconds(e.NewValue));
        }
    }
}

