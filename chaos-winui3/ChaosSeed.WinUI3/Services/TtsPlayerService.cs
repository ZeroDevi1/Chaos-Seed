using ChaosSeed.WinUI3.Models.Tts;
using Windows.Media;
using Windows.Media.Core;
using Windows.Media.Playback;
using Windows.Storage.Streams;

namespace ChaosSeed.WinUI3.Services;

/// <summary>
/// TTS 音频播放器（只播放内存中的 WAV；切换页面不丢失）。
/// </summary>
public sealed class TtsPlayerService
{
    public static TtsPlayerService Instance => _instance.Value;
    private static readonly Lazy<TtsPlayerService> _instance = new(() => new TtsPlayerService());

    private readonly MediaPlayer _player;
    private InMemoryRandomAccessStream? _stream;

    public event EventHandler? Changed;

    public bool IsOpen { get; private set; }
    public bool IsPlaying => _player.PlaybackSession.PlaybackState == MediaPlaybackState.Playing;

    public TtsClip? Current { get; private set; }

    private TtsPlayerService()
    {
        _player = new MediaPlayer
        {
            AudioCategory = MediaPlayerAudioCategory.Speech,
            IsLoopingEnabled = false,
        };

        try
        {
            _player.PlaybackSession.PlaybackStateChanged += (_, _) => RaiseChanged();
            _player.MediaEnded += (_, _) => RaiseChanged();
            _player.MediaFailed += (_, _) => RaiseChanged();

            var smtc = _player.SystemMediaTransportControls;
            smtc.IsEnabled = true;
            smtc.IsPlayEnabled = true;
            smtc.IsPauseEnabled = true;
            smtc.IsStopEnabled = true;
            smtc.ButtonPressed += (_, e) =>
            {
                try
                {
                    switch (e.Button)
                    {
                        case SystemMediaTransportControlsButton.Play:
                            Resume();
                            break;
                        case SystemMediaTransportControlsButton.Pause:
                            Pause();
                            break;
                        case SystemMediaTransportControlsButton.Stop:
                            StopKeepOpen();
                            break;
                    }
                }
                catch
                {
                    // ignore
                }
            };
        }
        catch
        {
            // ignore
        }
    }

    public TimeSpan Position => _player.PlaybackSession.Position;

    public TimeSpan Duration
    {
        get
        {
            try
            {
                var d = _player.PlaybackSession.NaturalDuration;
                if (d == TimeSpan.Zero && Current is not null && Current.DurationMs > 0)
                {
                    return TimeSpan.FromMilliseconds((double)Current.DurationMs);
                }
                return d;
            }
            catch
            {
                return TimeSpan.Zero;
            }
        }
    }

    public async Task OpenAsync(TtsClip clip, bool autoPlay, CancellationToken ct = default)
    {
        if (clip is null) throw new ArgumentNullException(nameof(clip));
        if (clip.WavBytes is null || clip.WavBytes.Length == 0)
        {
            throw new ArgumentException("empty wav bytes", nameof(clip));
        }

        ct.ThrowIfCancellationRequested();

        try
        {
            // 重新创建 stream，避免复用导致的奇怪状态/引用问题。
            var s = new InMemoryRandomAccessStream();
            using (var w = new DataWriter(s))
            {
                w.WriteBytes(clip.WavBytes);
                _ = await w.StoreAsync();
                _ = await w.FlushAsync();
                w.DetachStream();
            }
            s.Seek(0);

            _stream = s;
            _player.Source = MediaSource.CreateFromStream(_stream, clip.Mime);
            Current = clip;
            IsOpen = true;
            RaiseChanged();

            if (autoPlay)
            {
                _player.Play();
                RaiseChanged();
            }
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsPlayerService.OpenAsync", ex);
            throw;
        }
    }

    public void TogglePlayPause()
    {
        if (!IsOpen)
        {
            return;
        }

        if (IsPlaying)
        {
            Pause();
        }
        else
        {
            Resume();
        }
    }

    public void Resume()
    {
        if (!IsOpen)
        {
            return;
        }
        try
        {
            _player.Play();
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsPlayerService.Resume", ex);
        }
        RaiseChanged();
    }

    public void Pause()
    {
        if (!IsOpen)
        {
            return;
        }
        try
        {
            _player.Pause();
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsPlayerService.Pause", ex);
        }
        RaiseChanged();
    }

    public void StopKeepOpen()
    {
        if (!IsOpen)
        {
            return;
        }
        try
        {
            _player.Pause();
            _player.PlaybackSession.Position = TimeSpan.Zero;
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsPlayerService.StopKeepOpen", ex);
        }
        RaiseChanged();
    }

    public void Close()
    {
        try
        {
            _player.Pause();
        }
        catch
        {
            // ignore
        }

        IsOpen = false;
        Current = null;
        _stream = null;
        try { _player.Source = null; } catch { }
        RaiseChanged();
    }

    public void SeekToSeconds(double seconds)
    {
        if (!IsOpen)
        {
            return;
        }
        if (!double.IsFinite(seconds))
        {
            return;
        }
        try
        {
            var d = Duration;
            var t = TimeSpan.FromSeconds(Math.Clamp(seconds, 0, Math.Max(0, d.TotalSeconds)));
            _player.PlaybackSession.Position = t;
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsPlayerService.SeekToSeconds", ex);
        }
        RaiseChanged();
    }

    private void RaiseChanged()
    {
        try { Changed?.Invoke(this, EventArgs.Empty); } catch { }
    }
}
