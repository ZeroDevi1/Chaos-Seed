using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Windows.Storage;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace ChaosSeed.WinUI3.Controls;

public sealed partial class TtsMiniPlayerControl : UserControl
{
    private readonly DispatcherQueue _dq;
    private DispatcherTimer? _timer;
    private bool _updatingPos;
    private bool _seeking;
    private bool _seekHandlersAttached;

    public TtsMiniPlayerControl()
    {
        InitializeComponent();
        _dq = DispatcherQueue.GetForCurrentThread();

        Loaded += (_, _) =>
        {
            TtsPlayerService.Instance.Changed += OnChanged;
            AttachSeekHandlers();
            UpdateUi();
            EnsureTimer();
        };
        Unloaded += (_, _) =>
        {
            TtsPlayerService.Instance.Changed -= OnChanged;
        };
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

        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(250) };
        _timer.Tick += (_, _) =>
        {
            try { UpdatePositionUi(); } catch { }
        };
        _timer.Start();
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

    private void UpdateUi()
    {
        var svc = TtsPlayerService.Instance;
        Visibility = svc.IsOpen ? Visibility.Visible : Visibility.Collapsed;
        PlayPauseIcon.Symbol = svc.IsPlaying ? Symbol.Pause : Symbol.Play;

        var clip = svc.Current;
        if (clip is null)
        {
            TitleText.Text = "-";
            SubtitleText.Text = "";
        }
        else
        {
            TitleText.Text = string.IsNullOrWhiteSpace(clip.Text) ? "TTS" : ClipTitle(clip.Text);
            SubtitleText.Text = $"{clip.SampleRate} Hz · {clip.DurationMs} ms · {clip.SessionId}";
        }

        UpdatePositionUi();
    }

    private void UpdatePositionUi()
    {
        var svc = TtsPlayerService.Instance;
        if (!svc.IsOpen || _seeking)
        {
            return;
        }

        var pos = svc.Position;
        var dur = svc.Duration;
        if (dur <= TimeSpan.Zero)
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

    private static string ClipTitle(string text)
    {
        var s = (text ?? "").Trim();
        if (s.Length <= 30)
        {
            return s;
        }
        return s.Substring(0, 30) + "...";
    }

    private static string Fmt(TimeSpan t)
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
        TtsPlayerService.Instance.TogglePlayPause();
    }

    private void OnStopClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        TtsPlayerService.Instance.StopKeepOpen();
    }

    private async void OnSaveClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        var clip = TtsPlayerService.Instance.Current;
        if (clip is null || clip.WavBytes.Length == 0)
        {
            return;
        }

        try
        {
            var picker = new FileSavePicker
            {
                SuggestedStartLocation = PickerLocationId.Downloads,
                SuggestedFileName = string.IsNullOrWhiteSpace(clip.SessionId) ? "tts" : clip.SessionId,
            };
            picker.FileTypeChoices.Add("WAV 音频", new List<string> { ".wav" });

            var win = App.MainWindowInstance;
            if (win is null)
            {
                throw new InvalidOperationException("MainWindow not ready");
            }
            InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(win));

            var file = await picker.PickSaveFileAsync();
            if (file is null)
            {
                return;
            }
            await FileIO.WriteBytesAsync(file, clip.WavBytes);
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsMiniPlayerControl.Save", ex);
        }
    }

    private void OnCloseClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        TtsPlayerService.Instance.Close();
    }

    private void OnPositionChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (_updatingPos || _seeking)
        {
            return;
        }
        try
        {
            TtsPlayerService.Instance.SeekToSeconds(PosSlider.Value);
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsMiniPlayerControl.SeekToSeconds", ex);
        }
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
            TtsPlayerService.Instance.SeekToSeconds(PosSlider.Value);
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsMiniPlayerControl.SeekToSeconds", ex);
        }
    }

    private void OnPosPointerCaptureLost(object sender, PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _seeking = false;
    }

    private void OnPosPointerCanceled(object sender, PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _seeking = false;
    }
}
