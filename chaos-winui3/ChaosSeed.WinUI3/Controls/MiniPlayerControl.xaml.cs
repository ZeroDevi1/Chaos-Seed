using ChaosSeed.WinUI3.Pages;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Input;
using System.Linq;
using System;

namespace ChaosSeed.WinUI3.Controls;

public sealed partial class MiniPlayerControl : UserControl
{
    private readonly DispatcherQueue _dq = DispatcherQueue.GetForCurrentThread();
    private DispatcherTimer? _timer;
    private bool _updatingVolume;
    private bool _updatingPos;
    private bool _seeking;
    private bool _seekCommitPending;
    private bool _seekHandlersAttached;

    public MiniPlayerControl()
    {
        InitializeComponent();
        Loaded += (_, _) =>
        {
            MusicPreviewPlayerService.Instance.Changed += OnChanged;
            AttachSeekHandlers();
            UpdateUi();
            EnsureTimer();
        };
        Unloaded += (_, _) => MusicPreviewPlayerService.Instance.Changed -= OnChanged;
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

    private void OnChanged(object? sender, EventArgs e)
    {
        _ = sender;
        _ = e;
        _dq.TryEnqueue(UpdateUi);
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
        _timer.Tick += (_, _) => UpdateTimeline();
        _timer.Start();
    }

    private void UpdateUi()
    {
        var svc = MusicPreviewPlayerService.Instance;
        Visibility = svc.IsOpen ? Visibility.Visible : Visibility.Collapsed;
        PlayPauseIcon.Symbol = svc.IsPlaying ? Symbol.Pause : Symbol.Play;
        TitleText.Text = svc.Track?.Title ?? "-";

        var artist = svc.Track?.Artists is null ? "" : string.Join(" / ", svc.Track.Artists.Where(s => !string.IsNullOrWhiteSpace(s)));
        var album = (svc.Track?.Album ?? "").Trim();
        SubtitleText.Text = string.IsNullOrWhiteSpace(album) ? artist : $"{artist} · {album}";

        try
        {
            var u = svc.Track?.CoverUrl;
            CoverImg.Source = string.IsNullOrWhiteSpace(u) ? null : MusicUiUtil.TryCreateBitmap(u);
        }
        catch
        {
            CoverImg.Source = null;
        }

        _updatingVolume = true;
        try
        {
            VolumeSlider.Value = svc.Volume * 100.0;
            VolumePercentText.Text = $"{(int)Math.Round(VolumeSlider.Value)}%";
            VolumeIcon.Symbol = VolumeSlider.Value <= 0.1 ? Symbol.Mute : Symbol.Volume;
        }
        finally
        {
            _updatingVolume = false;
        }

        UpdateTimeline(force: true);
    }

    private void UpdateTimeline(bool force = false)
    {
        var svc = MusicPreviewPlayerService.Instance;
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

    private void OnPlayPauseClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        MusicPreviewPlayerService.Instance.TogglePlayPause();
    }

    private void OnStopClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        MusicPreviewPlayerService.Instance.StopKeepOpen();
    }

    private void OnCloseClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        MusicPreviewPlayerService.Instance.Stop();
    }

    private void OnVolumeChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        _ = sender;
        if (_updatingVolume)
        {
            return;
        }
        MusicPreviewPlayerService.Instance.Volume = e.NewValue / 100.0;
        VolumePercentText.Text = $"{(int)Math.Round(e.NewValue)}%";
        VolumeIcon.Symbol = e.NewValue <= 0.1 ? Symbol.Mute : Symbol.Volume;
    }

    private void OnPosPointerPressed(object sender, PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _seeking = true;
        _seekCommitPending = true;

        TrySeekFromPointer(e, seekNow: true);
    }

    private void OnPosPointerReleased(object sender, PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        TrySeekFromPointer(e, seekNow: false);
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

    private void TrySeekFromPointer(PointerRoutedEventArgs e, bool seekNow)
    {
        try
        {
            var pt = e.GetCurrentPoint(PosSlider);
            if (!pt.Properties.IsLeftButtonPressed && seekNow)
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

            if (seekNow)
            {
                MusicPreviewPlayerService.Instance.SeekToSeconds(PosSlider.Value);
                _seekCommitPending = false;
                _seeking = false;
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
        MusicPreviewPlayerService.Instance.SeekToSeconds(PosSlider.Value);
    }

    private void OnPositionChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        _ = sender;
        if (_updatingPos)
        {
            return;
        }
        if (_seeking)
        {
            PosText.Text = FormatTime(TimeSpan.FromSeconds(e.NewValue));
        }
    }
}
